impl<'a> Vue3DomMirCodegen<'a> {
    fn push_prop_helpers(&self, props: &Vue3DomProps, helpers: &mut Vec<RuntimeHelper>) {
        if props.segments.is_empty() {
            for binding in &props.dynamic_bindings {
                push_vue3_dom_binding_helpers(binding, helpers);
            }
            for event in &props.events {
                push_vue3_dom_event_helpers(event, helpers);
            }
            for _ in &props.object_listeners {
                push_unique_helper(helpers, RuntimeHelper::Vue3ToHandlers);
            }
        } else {
            for segment in &props.segments {
                match segment {
                    Vue3DomPropSegment::DynamicBinding(binding) => {
                        push_vue3_dom_binding_helpers(binding, helpers);
                    }
                    Vue3DomPropSegment::Event(event) => {
                        push_vue3_dom_event_helpers(event, helpers);
                    }
                    Vue3DomPropSegment::ObjectListeners(_) => {
                        push_unique_helper(helpers, RuntimeHelper::Vue3ToHandlers);
                    }
                    Vue3DomPropSegment::Content(content) => {
                        if matches!(
                            content,
                            Vue3DomContent::Text {
                                expression: Some(_)
                            }
                        ) && !vue3_dom_content_text_is_static(content, self.js, self.options)
                        {
                            push_unique_helper(helpers, RuntimeHelper::Vue3ToDisplayString);
                        }
                    }
                    Vue3DomPropSegment::Model(_) => {}
                    Vue3DomPropSegment::StaticAttr(_) | Vue3DomPropSegment::ObjectBinding(_) => {}
                }
            }
        }
        if props_requires_merge_call(props) {
            push_unique_helper(helpers, RuntimeHelper::Vue3MergeProps);
        }
        if props.normalize.guard_reactive_props {
            push_unique_helper(helpers, RuntimeHelper::Vue3GuardReactiveProps);
        }
        if props.normalize.normalize_props {
            push_unique_helper(helpers, RuntimeHelper::Vue3NormalizeProps);
        }
    }

    fn directive_declarations(&self) -> Vec<String> {
        let mut directives = Vec::new();
        for node in &self.mir.nodes {
            let Vue3DomMirKind::VNodeCall(call) = &node.kind else {
                continue;
            };
            for directive in &call.directives {
                if !directives.iter().any(|name| name == &directive.name) {
                    directives.push(directive.name.clone());
                }
            }
        }
        directives
            .iter()
            .map(|directive| {
                format!(
                    "const {} = _resolveDirective({})",
                    directive_asset_id(directive),
                    quote_string(directive)
                )
            })
            .collect()
    }

    fn component_declarations(&self) -> Vec<String> {
        let mut components = Vec::new();
        for node in &self.mir.nodes {
            let Vue3DomMirKind::VNodeCall(call) = &node.kind else {
                continue;
            };
            if let Vue3DomTag::ComponentAsset(component) = &call.tag {
                if !components.iter().any(|name| name == component) {
                    components.push(component.clone());
                }
            }
        }
        components
            .iter()
            .map(|component| {
                format!(
                    "const {} = _resolveComponent({})",
                    component_asset_id(component),
                    quote_string(component)
                )
            })
            .collect()
    }

    fn render_root_children(&self, parent: NodeId, scope: &RenderScope) -> String {
        let rendered = self.render_children(parent, Vue3DomMirRenderMode::Root, scope);
        match rendered.as_slice() {
            [] => "null".into(),
            [single] => single.clone(),
            _ => render_array(&rendered),
        }
    }

    fn render_children(
        &self,
        parent: NodeId,
        mode: Vue3DomMirRenderMode,
        scope: &RenderScope,
    ) -> Vec<String> {
        self.mir
            .node(parent)
            .map(|node| {
                node.children
                    .iter()
                    .filter_map(|child_id| self.render_node(*child_id, mode, scope))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn render_node(
        &self,
        node_id: NodeId,
        mode: Vue3DomMirRenderMode,
        scope: &RenderScope,
    ) -> Option<String> {
        let node = self.mir.node(node_id)?;
        match &node.kind {
            Vue3DomMirKind::Root(_) => Some(self.render_root_children(node_id, scope)),
            Vue3DomMirKind::VNodeCall(call) => {
                Some(self.render_vnode_call(node_id, call, mode, scope))
            }
            Vue3DomMirKind::TextCall { value } => Some(format!(
                "_createTextVNode({})",
                self.render_mir_expr(value, scope)
            )),
            Vue3DomMirKind::Interpolation { expression } => Some(format!(
                "_createTextVNode(_toDisplayString({}), 1 /* TEXT */)",
                self.render_js_expr(*expression, scope)
            )),
            Vue3DomMirKind::If { condition } => Some(self.render_if(node_id, *condition, scope)),
            Vue3DomMirKind::For(for_mir) => Some(self.render_for(node_id, for_mir, scope)),
            Vue3DomMirKind::RenderSlot(slot) => Some(self.render_slot_outlet(slot, scope)),
            Vue3DomMirKind::WithDirectives => {
                let children = self.render_children(node_id, Vue3DomMirRenderMode::Child, scope);
                children.first().cloned()
            }
            Vue3DomMirKind::Cache { index } => {
                let rendered = self.render_root_children(node_id, scope);
                Some(format!("_cache[{index}] || (_cache[{index}] = {rendered})"))
            }
            Vue3DomMirKind::Memo { expression, index } => {
                let rendered = self.render_root_children(node_id, scope);
                Some(format!(
                    "_withMemo({}, () => {}, _cache, {index})",
                    self.render_js_expr(*expression, scope),
                    rendered
                ))
            }
            Vue3DomMirKind::Hoisted { index } => Some(format!("_hoisted_{index}")),
            Vue3DomMirKind::Fragment => {
                let children = self.render_children(node_id, Vue3DomMirRenderMode::Child, scope);
                Some(render_array(&children))
            }
        }
    }

    fn render_vnode_call(
        &self,
        node_id: NodeId,
        call: &Vue3VNodeCall,
        mode: Vue3DomMirRenderMode,
        scope: &RenderScope,
    ) -> String {
        let children = self.render_vnode_children(call, scope);
        let patch_flag =
            render_patch_flag_text((call.patch_flag.bits != 0).then_some(call.patch_flag.bits));
        let dynamic_props = if call.dynamic_props.is_empty() {
            String::new()
        } else {
            format!(
                ", [{}]",
                call.dynamic_props
                    .iter()
                    .map(|prop| quote_string(prop))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let helper = if mode == Vue3DomMirRenderMode::Root || call.is_block {
            if call.is_component {
                "_createBlock"
            } else {
                "_createElementBlock"
            }
        } else if call.is_component {
            "_createVNode"
        } else {
            "_createElementVNode"
        };
        let args = render_call_args(
            self.render_dom_tag(&call.tag, scope),
            self.render_props(call, scope).as_deref(),
            children.as_deref(),
            patch_flag.as_str(),
            dynamic_props.as_str(),
        );
        let rendered = if mode == Vue3DomMirRenderMode::Root || call.is_block {
            format!(
                "(_openBlock({}), {}({}))",
                if call.disable_tracking { "true" } else { "" },
                helper,
                args
            )
        } else if self
            .mir
            .node(node_id)
            .is_some_and(|node| node.parent == Some(self.mir.root))
        {
            format!("{}({})", helper, args)
        } else {
            format!("{}({})", helper, args)
        };
        self.render_with_directives(rendered, call, scope)
    }

    fn render_dom_tag(&self, tag: &Vue3DomTag, scope: &RenderScope) -> String {
        match tag {
            Vue3DomTag::Native(name) => quote_string(name),
            Vue3DomTag::ComponentAsset(name) => component_asset_id(name),
            Vue3DomTag::DynamicComponent(expression) => {
                format!(
                    "_resolveDynamicComponent({})",
                    self.render_js_expr(*expression, scope)
                )
            }
            Vue3DomTag::RuntimeHelper(helper) => helper_reference(*helper),
        }
    }

    fn render_props(&self, call: &Vue3VNodeCall, scope: &RenderScope) -> Option<String> {
        let rendered = self.render_ordered_props(&call.props, &call.tag, scope)?;
        Some(self.render_normalized_props(&call.props, rendered))
    }

    fn render_ordered_props(
        &self,
        props: &Vue3DomProps,
        tag: &Vue3DomTag,
        scope: &RenderScope,
    ) -> Option<String> {
        if props.segments.is_empty() {
            let mut entries = Vec::new();
            for attr in &props.static_attrs {
                entries.push(self.render_static_attr(attr));
            }
            for binding in &props.dynamic_bindings {
                if dom_tag_consumes_binding(tag, binding) {
                    continue;
                }
                entries.push(self.render_dynamic_binding(binding, scope));
            }
            for event in &props.events {
                entries.push(self.render_event(event, scope));
            }
            return self.render_plain_props(&entries);
        }

        let mut merge_args = Vec::new();
        let mut object_entries = Vec::new();
        for segment in &props.segments {
            match segment {
                Vue3DomPropSegment::StaticAttr(attr) => {
                    object_entries.push(self.render_static_attr(attr));
                }
                Vue3DomPropSegment::DynamicBinding(binding) => {
                    if dom_tag_consumes_binding(tag, binding) {
                        continue;
                    }
                    object_entries.push(self.render_dynamic_binding(binding, scope));
                }
                Vue3DomPropSegment::Content(content) => {
                    object_entries.push(self.render_content_prop(content, scope));
                }
                Vue3DomPropSegment::Model(model) => {
                    object_entries.push(self.render_model_update_prop(model, scope));
                }
                Vue3DomPropSegment::Event(event) => {
                    object_entries.push(self.render_event(event, scope));
                }
                Vue3DomPropSegment::ObjectBinding(binding) => {
                    self.push_merge_object_arg(&mut merge_args, &mut object_entries);
                    merge_args.push(self.render_js_expr(binding.value, scope));
                }
                Vue3DomPropSegment::ObjectListeners(listeners) => {
                    self.push_merge_object_arg(&mut merge_args, &mut object_entries);
                    merge_args.push(self.render_object_listeners(listeners, scope));
                }
            }
        }
        self.push_merge_object_arg(&mut merge_args, &mut object_entries);
        if merge_args.is_empty() {
            None
        } else if merge_args.len() == 1 {
            merge_args.into_iter().next()
        } else {
            Some(format!("_mergeProps({})", merge_args.join(", ")))
        }
    }

    fn render_static_attr(&self, attr: &Vue3DomStaticAttr) -> String {
        format!("{}: {}", json_key(&attr.name), quote_string(&attr.value))
    }

    fn render_dynamic_binding(&self, binding: &Vue3DomBinding, scope: &RenderScope) -> String {
        let value = self.render_js_expr(binding.value, scope);
        if binding.dynamic_arg {
            format!(
                "[{}]: {}",
                render_vue3_dom_binding_dynamic_key(
                    binding,
                    binding
                        .dynamic_name
                        .map(|id| self.render_js_expr(id, scope))
                        .unwrap_or_else(|| binding.name.clone()),
                    true,
                ),
                value
            )
        } else if binding.name == "class" {
            format!("class: _normalizeClass({value})")
        } else {
            format!(
                "{}: {}",
                json_key(&render_vue3_dom_binding_static_key(binding, true)),
                value
            )
        }
    }

    fn render_content_prop(&self, content: &Vue3DomContent, scope: &RenderScope) -> String {
        let name = match content {
            Vue3DomContent::Html { .. } => "innerHTML",
            Vue3DomContent::Text { .. } => "textContent",
        };
        format!(
            "{}: {}",
            json_key(name),
            self.render_content_value(content, scope)
        )
    }

    fn render_content_value(&self, content: &Vue3DomContent, scope: &RenderScope) -> String {
        match content {
            Vue3DomContent::Html { expression } => expression
                .map(|expression| self.render_js_expr(expression, scope))
                .unwrap_or_else(|| quote_string("")),
            Vue3DomContent::Text { expression } => {
                let Some(expression) = expression else {
                    return quote_string("");
                };
                let value = self.render_js_expr(*expression, scope);
                if vue3_dom_content_text_is_static(content, self.js, self.options) {
                    value
                } else {
                    format!("_toDisplayString({value})")
                }
            }
        }
    }

    fn render_model_update_prop(&self, model: &Vue3DomModel, scope: &RenderScope) -> String {
        format!(
            "{}: {}",
            json_key("onUpdate:modelValue"),
            self.render_model_assignment(model, scope)
        )
    }

    fn render_model_assignment(&self, model: &Vue3DomModel, scope: &RenderScope) -> String {
        let raw = self.raw_js_expr(model.expression).unwrap_or_default();
        render_inline_model_assignment(
            raw,
            "$event",
            self.options.binding_metadata.get(raw).map(String::as_str),
            self.options,
            || self.render_js_expr(model.expression, scope),
        )
    }

    fn render_event(&self, event: &Vue3DomEvent, scope: &RenderScope) -> String {
        let handler = self.render_cached_event_handler(event, scope);
        let key = self.render_event_key(event, scope);
        format!("{key}: {handler}")
    }

    fn render_event_key(&self, event: &Vue3DomEvent, scope: &RenderScope) -> String {
        if event.dynamic_arg {
            let name = event
                .dynamic_name
                .map(|id| self.render_js_expr(id, scope))
                .unwrap_or_else(|| event.name.clone());
            let handler_key = format!("_toHandlerKey({})", name.trim());
            let transformed = self.render_event_click_key(event, handler_key);
            return format!("[{}]", self.render_event_option_key(event, transformed));
        }
        json_key(&self.render_event_option_key(event, event.name.clone()))
    }

    fn render_event_option_key(&self, event: &Vue3DomEvent, key: String) -> String {
        let postfix = vue3_dom_event_option_postfix(&event.option_modifiers);
        if postfix.is_empty() {
            key
        } else if event.dynamic_arg {
            format!("({key}) + {}", quote_string(&postfix))
        } else {
            format!("{key}{postfix}")
        }
    }

    fn render_cached_event_handler(&self, event: &Vue3DomEvent, scope: &RenderScope) -> String {
        let handler = self.render_guarded_event_handler(event, scope);
        let Some(cache) = &event.cache else {
            return handler;
        };
        format!(
            "_cache[{}] || (_cache[{}] = {})",
            cache.index, cache.index, handler
        )
    }

    fn render_guarded_event_handler(&self, event: &Vue3DomEvent, scope: &RenderScope) -> String {
        let mut handler = self.render_js_stmt(event.handler, scope);
        if !event.runtime_modifiers.is_empty() {
            handler = format!(
                "_withModifiers({handler}, {})",
                render_string_array(&event.runtime_modifiers)
            );
        }
        if !event.key_modifiers.is_empty() {
            handler = format!(
                "_withKeys({handler}, {})",
                render_string_array(&event.key_modifiers)
            );
        }
        handler
    }

    fn render_event_click_key(&self, event: &Vue3DomEvent, key: String) -> String {
        match event.click_event {
            Some(Vue3DomClickEvent::ContextMenu) => {
                format!("({key}) === \"onClick\" ? \"onContextmenu\" : ({key})")
            }
            Some(Vue3DomClickEvent::MouseUp) => {
                format!("({key}) === \"onClick\" ? \"onMouseup\" : ({key})")
            }
            None => key,
        }
    }

    fn render_object_listeners(
        &self,
        listeners: &Vue3DomObjectListeners,
        scope: &RenderScope,
    ) -> String {
        let handlers = self.render_js_expr(listeners.value, scope);
        if listeners.preserve_case {
            format!("_toHandlers({handlers}, true)")
        } else {
            format!("_toHandlers({handlers})")
        }
    }

    fn push_merge_object_arg(
        &self,
        merge_args: &mut Vec<String>,
        object_entries: &mut Vec<String>,
    ) {
        if let Some(object) = self.render_plain_props(object_entries) {
            merge_args.push(object);
            object_entries.clear();
        }
    }

    fn render_plain_props(&self, entries: &[String]) -> Option<String> {
        if entries.is_empty() {
            None
        } else if entries.len() == 1 {
            Some(format!("{{ {} }}", entries.join(", ")))
        } else {
            Some(render_object(entries))
        }
    }

    fn render_normalized_props(&self, props: &Vue3DomProps, rendered: String) -> String {
        if !props.normalize.normalize_props {
            return rendered;
        }
        let argument = if props.normalize.guard_reactive_props {
            format!("_guardReactiveProps({rendered})")
        } else {
            rendered
        };
        format!("_normalizeProps({argument})")
    }

    fn render_with_directives(
        &self,
        vnode: String,
        call: &Vue3VNodeCall,
        scope: &RenderScope,
    ) -> String {
        if call.directives.is_empty() && call.models.is_empty() && call.v_show.is_none() {
            return vnode;
        }
        let mut directives = call
            .models
            .iter()
            .map(|model| self.render_model_directive_arg(model, scope))
            .collect::<Vec<_>>();
        if let Some(v_show) = call.v_show {
            directives.push(self.render_v_show_directive_arg(v_show, scope));
        }
        directives.extend(
            call.directives
                .iter()
                .map(|directive| self.render_directive_arg(directive, scope)),
        );
        format!("_withDirectives({vnode}, {})", render_array(&directives))
    }

    fn render_model_directive_arg(&self, model: &Vue3DomModel, scope: &RenderScope) -> String {
        let mut args = vec![
            helper_reference(vue3_dom_model_runtime_helper(model.kind)),
            self.render_js_expr(model.expression, scope),
        ];
        if !model.modifiers.is_empty() {
            args.push("void 0".into());
            let modifiers = model
                .modifiers
                .iter()
                .map(|modifier| format!("{}: true", json_key(modifier)))
                .collect::<Vec<_>>();
            args.push(render_object(&modifiers));
        }
        format!("[{}]", args.join(", "))
    }

    fn render_v_show_directive_arg(&self, expression: JsExprId, scope: &RenderScope) -> String {
        format!("[_vShow, {}]", self.render_js_expr(expression, scope))
    }

}
