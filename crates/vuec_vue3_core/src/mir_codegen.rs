use crate::*;

pub(crate) fn render_helpers_from_code(order: &[RuntimeHelper], code: &str) -> Vec<RuntimeHelper> {
    let mut helpers = order
        .iter()
        .copied()
        .filter(|helper| code.contains(&helper_reference(*helper)))
        .collect::<Vec<_>>();
    apply_vue3_memo_helper_order(&mut helpers);
    helpers
}

pub(crate) struct Vue3DomMirCodegen<'a> {
    pub(crate) mir: &'a Vue3DomMir,
    pub(crate) js: &'a JsAstStore,
    pub(crate) options: &'a Vue3CompilerOptions,
}

impl<'a> Vue3DomMirCodegen<'a> {
    pub(crate) fn new(
        mir: &'a Vue3DomMir,
        js: &'a JsAstStore,
        options: &'a Vue3CompilerOptions,
    ) -> Self {
        Self { mir, js, options }
    }

    pub(crate) fn generate(self) -> CodegenResult {
        let mut writer = CodeWriter::new();
        let scope = RenderScope::default();
        let hoists = self.hoist_declarations(&scope);
        let declarations = self
            .component_declarations()
            .into_iter()
            .chain(self.directive_declarations())
            .collect::<Vec<_>>();
        let body = self.render_root_children(self.mir.root, &scope);
        let helpers = self.helpers(&format!(
            "{}\n{}\n{}",
            hoists.join("\n"),
            declarations.join("\n"),
            body
        ));
        self.render_preamble(&mut writer, &helpers, &hoists);
        self.render_render_start(&mut writer);
        writer.indent();
        let use_with = self.use_with_block();
        if use_with {
            writer.push_line("with (_ctx) {");
            writer.indent();
            if !helpers.is_empty() {
                writer.push_line(&format!("const {{ {} }} = _Vue", helper_aliases(&helpers)));
                writer.newline();
            }
        }
        for declaration in &declarations {
            writer.push_line(declaration);
        }
        if !declarations.is_empty() {
            writer.newline();
        }
        writer.push_line(&format!("return {body}"));
        if use_with {
            writer.dedent();
            writer.push_line("}");
        }
        writer.dedent();
        writer.push_line("}");
        CodegenResult {
            code: writer.finish().trim_end().to_string(),
            map: None,
            ast_summary: format!("vue3-dom-mir-nodes={}", self.mir.len()),
            diagnostics: Vec::new(),
            preamble: String::new(),
        }
    }

    fn render_preamble(
        &self,
        writer: &mut CodeWriter,
        helpers: &[RuntimeHelper],
        hoists: &[String],
    ) {
        if self.options.mode == "module" {
            if !helpers.is_empty() {
                writer.push_line(&format!(
                    "import {{ {} }} from \"vue\"",
                    import_helper_aliases(helpers)
                ));
                writer.newline();
            }
            for import in self.asset_imports() {
                writer.push_line(&format!("import {} from '{}'", import.name, import.path));
            }
            if !self.asset_imports().is_empty() {
                writer.newline();
                writer.newline();
            }
        } else if self.options.prefix_identifiers {
            if !helpers.is_empty() {
                writer.push_line(&format!("const {{ {} }} = Vue", helper_aliases(helpers)));
                writer.newline();
            }
        } else if !helpers.is_empty() {
            writer.push_line("const _Vue = Vue");
            if !hoists.is_empty() {
                let static_helpers = hoist_static_helpers(helpers);
                if !static_helpers.is_empty() {
                    writer.push_line(&format!(
                        "const {{ {} }} = _Vue",
                        helper_aliases(&static_helpers)
                    ));
                }
            }
            writer.newline();
        }
        for hoist in hoists {
            writer.push_line(hoist);
        }
        if !hoists.is_empty() {
            writer.newline();
        }
    }

    fn asset_imports(&self) -> &[vuec_ast::Vue3ImportItem] {
        self.mir
            .node(self.mir.root)
            .and_then(|node| match &node.kind {
                Vue3DomMirKind::Root(root) => Some(root.imports.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }

    fn render_render_start(&self, writer: &mut CodeWriter) {
        if self.options.mode == "module" {
            writer.push_line(&format!(
                "export function render({}) {{",
                render_args(self.options)
            ));
        } else if self.options.prefix_identifiers {
            writer.push_line(&format!(
                "return function render({}) {{",
                render_args(self.options)
            ));
        } else {
            writer.push_line(&format!(
                "return function render({}) {{",
                render_args(self.options)
            ));
        }
    }

    fn use_with_block(&self) -> bool {
        !self.options.prefix_identifiers && self.options.mode != "module"
    }

    fn hoist_declarations(&self, scope: &RenderScope) -> Vec<String> {
        self.mir
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                Vue3DomMirKind::Hoisted { index } => {
                    Some((index, self.render_hoisted_value(node.id, scope)))
                }
                _ => None,
            })
            .map(|(index, value)| format!("const _hoisted_{index} = {value}"))
            .collect()
    }

    fn render_hoisted_value(&self, node_id: NodeId, scope: &RenderScope) -> String {
        let children = self.render_children(node_id, Vue3DomMirRenderMode::Child, scope);
        match children.as_slice() {
            [] => "null".into(),
            [single] => single.clone(),
            _ => render_array(&children),
        }
    }

    fn helpers(&self, helper_probe: &str) -> Vec<RuntimeHelper> {
        let mut helpers = Vec::new();
        for node in &self.mir.nodes {
            match &node.kind {
                Vue3DomMirKind::Root(_)
                | Vue3DomMirKind::WithDirectives
                | Vue3DomMirKind::Fragment => {}
                Vue3DomMirKind::VNodeCall(call) => {
                    match &call.tag {
                        Vue3DomTag::Native(_) => {}
                        Vue3DomTag::ComponentAsset(_) => {
                            push_unique_helper(&mut helpers, RuntimeHelper::Vue3ResolveComponent);
                        }
                        Vue3DomTag::DynamicComponent(_) => {
                            push_unique_helper(
                                &mut helpers,
                                RuntimeHelper::Vue3ResolveDynamicComponent,
                            );
                        }
                        Vue3DomTag::RuntimeHelper(helper) => {
                            push_unique_helper(&mut helpers, *helper);
                        }
                    }
                    if let MirChildren::Slots(slots) = &call.children {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3WithCtx);
                        if !slots.dynamic_slots.is_empty() {
                            push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateSlots);
                        }
                        if slots
                            .dynamic_slots
                            .iter()
                            .any(|slot| matches!(slot, vuec_ast::Vue3DomDynamicSlot::For(_)))
                        {
                            push_unique_helper(&mut helpers, RuntimeHelper::Vue3RenderList);
                        }
                    }
                    self.push_prop_helpers(&call.props, &mut helpers);
                    if !call.directives.is_empty() || call.v_show.is_some() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3WithDirectives);
                    }
                    if !call.models.is_empty() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3WithDirectives);
                    }
                    if call.v_show.is_some() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3VShow);
                    }
                    if !call.directives.is_empty() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3ResolveDirective);
                    }
                    for model in &call.models {
                        push_unique_helper(&mut helpers, vue3_dom_model_runtime_helper(model.kind));
                    }
                    let is_root_call = node.parent == Some(self.mir.root) || call.is_block;
                    if is_root_call {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3OpenBlock);
                        push_unique_helper(
                            &mut helpers,
                            if call.is_component {
                                RuntimeHelper::Vue3CreateBlock
                            } else {
                                RuntimeHelper::Vue3CreateElementBlock
                            },
                        );
                    } else {
                        push_unique_helper(
                            &mut helpers,
                            if call.is_component {
                                RuntimeHelper::Vue3CreateVNode
                            } else {
                                RuntimeHelper::Vue3CreateElementVNode
                            },
                        );
                    }
                }
                Vue3DomMirKind::TextCall { .. } => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateTextVNode);
                }
                Vue3DomMirKind::Interpolation { .. } => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateTextVNode);
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3ToDisplayString);
                }
                Vue3DomMirKind::If { condition } => {
                    if condition.is_some() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateCommentVNode);
                    }
                }
                Vue3DomMirKind::For(for_mir) => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3OpenBlock);
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateElementBlock);
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3Fragment);
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3RenderList);
                    if for_mir.memo.is_some() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3IsMemoSame);
                    }
                }
                Vue3DomMirKind::RenderSlot(slot) => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3RenderSlot);
                    self.push_prop_helpers(&slot.props, &mut helpers);
                }
                Vue3DomMirKind::Memo { .. } => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3WithMemo);
                }
                Vue3DomMirKind::Cache { .. } | Vue3DomMirKind::Hoisted { .. } => {}
            }
        }
        for helper in render_helpers_from_code(vue3_helper_order(false), helper_probe) {
            push_unique_helper(&mut helpers, helper);
        }
        sort_helpers_by_order(&mut helpers, vue3_helper_order(false));
        helpers
    }

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

    fn render_slot_outlet(&self, slot: &vuec_ast::Vue3RenderSlot, scope: &RenderScope) -> String {
        let slots = if uses_prefixed_identifiers(self.options) {
            "_ctx.$slots"
        } else {
            "$slots"
        };
        let name = self.render_slot_name(&slot.name, scope);
        let props = self.render_normalized_props(
            &slot.props,
            self.render_ordered_props(&slot.props, &Vue3DomTag::Native("slot".into()), scope)
                .unwrap_or_else(|| "{}".into()),
        );
        let fallback = if slot.fallback.is_empty() {
            None
        } else {
            let rendered = slot
                .fallback
                .iter()
                .filter_map(|child_id| {
                    self.render_node(*child_id, Vue3DomMirRenderMode::Child, scope)
                })
                .collect::<Vec<_>>();
            Some(format!("() => {}", render_array(&rendered)))
        };
        let mut args = vec![slots.to_string(), name];
        if props != "{}" || fallback.is_some() {
            args.push(props);
        }
        if let Some(fallback) = fallback {
            args.push(fallback);
        }
        format!("_renderSlot({})", args.join(", "))
    }

    fn render_directive_arg(&self, directive: &Vue3DomDirective, scope: &RenderScope) -> String {
        let runtime = directive_asset_id(&directive.name);
        let mut args = vec![runtime];
        if let Some(expression) = directive.expression {
            args.push(self.render_js_expr(expression, scope));
        } else if directive.argument.is_some()
            || directive.dynamic_argument.is_some()
            || !directive.modifiers.is_empty()
        {
            args.push("void 0".into());
        }
        if let Some(argument) = &directive.argument {
            args.push(quote_string(argument));
        } else if let Some(argument) = directive.dynamic_argument {
            args.push(self.render_js_expr(argument, scope));
        } else if !directive.modifiers.is_empty() {
            args.push("void 0".into());
        }
        if !directive.modifiers.is_empty() {
            let modifiers = directive
                .modifiers
                .iter()
                .map(|modifier| format!("{}: true", json_key(modifier)))
                .collect::<Vec<_>>();
            args.push(render_object(&modifiers));
        }
        format!("[{}]", args.join(", "))
    }

    fn render_vnode_children(&self, call: &Vue3VNodeCall, scope: &RenderScope) -> Option<String> {
        match &call.children {
            MirChildren::None => None,
            MirChildren::Text(value) => Some(quote_string(value)),
            MirChildren::Slots(slots) => Some(self.render_slots(slots, scope)),
            MirChildren::Nodes(children) => {
                let rendered = children
                    .iter()
                    .filter_map(|child_id| {
                        self.render_node(*child_id, Vue3DomMirRenderMode::Child, scope)
                    })
                    .collect::<Vec<_>>();
                if rendered.is_empty() {
                    None
                } else if rendered.len() == 1 {
                    rendered.into_iter().next()
                } else {
                    Some(render_array(&rendered))
                }
            }
        }
    }

    fn render_slots(&self, slots: &vuec_ast::Vue3DomSlots, scope: &RenderScope) -> String {
        let mut properties = slots
            .slots
            .iter()
            .map(|slot| self.render_slot(slot, scope))
            .collect::<Vec<_>>();
        properties.push(format!("_: {}", vue3_slot_flag_value(slots.flag)));
        let base = render_object(&properties);
        if slots.dynamic_slots.is_empty() {
            base
        } else {
            format!(
                "_createSlots({base}, {})",
                self.render_dynamic_slots(slots, scope)
            )
        }
    }

    fn render_slot(&self, slot: &vuec_ast::Vue3DomSlot, scope: &RenderScope) -> String {
        let params = slot
            .params
            .map(|params| format!("({})", self.render_js_pattern(params)))
            .unwrap_or_else(|| "()".into());
        let child_scope = slot
            .params
            .map(|params| self.scope_with_pattern(scope, params))
            .unwrap_or_else(|| scope.clone());
        let rendered = slot
            .children
            .iter()
            .filter_map(|child_id| {
                self.render_node(*child_id, Vue3DomMirRenderMode::Child, &child_scope)
            })
            .collect::<Vec<_>>();
        let body = render_array(&rendered);
        format!("{}: _withCtx({params} => {body})", json_key(&slot.name))
    }

    fn render_dynamic_slots(&self, slots: &vuec_ast::Vue3DomSlots, scope: &RenderScope) -> String {
        let rendered = slots
            .dynamic_slots
            .iter()
            .map(|slot| self.render_dynamic_slot(slot, scope))
            .collect::<Vec<_>>();
        render_array(&rendered)
    }

    fn render_dynamic_slot(
        &self,
        slot: &vuec_ast::Vue3DomDynamicSlot,
        scope: &RenderScope,
    ) -> String {
        match slot {
            vuec_ast::Vue3DomDynamicSlot::Slot(slot) => {
                self.render_dynamic_slot_object(slot, scope)
            }
            vuec_ast::Vue3DomDynamicSlot::Conditional(slot) => {
                let condition = slot
                    .condition
                    .map(|condition| {
                        render_condition(&self.render_js_expr(condition, scope), self.options)
                    })
                    .unwrap_or_else(|| "true".into());
                let slot_object = self.render_dynamic_slot_object(&slot.slot, scope);
                let alternate = slot
                    .alternate
                    .as_deref()
                    .map(|alternate| self.render_dynamic_slot(alternate, scope))
                    .unwrap_or_else(|| "undefined".into());
                format!(
                    "{condition}\n  ? {}\n  : {}",
                    indent_after_first_line(&slot_object, 4),
                    indent_after_first_line(&alternate, 4)
                )
            }
            vuec_ast::Vue3DomDynamicSlot::For(slot) => self.render_for_slot(slot, scope),
        }
    }

    fn render_for_slot(&self, slot: &vuec_ast::Vue3DomForSlot, scope: &RenderScope) -> String {
        let source = self.render_js_expr(slot.source, scope);
        let params = self.render_for_slot_params(slot);
        let child_scope = self.scope_with_for_slot(scope, slot);
        let body = self.render_dynamic_slot_object(&slot.slot, &child_scope);
        format!(
            "_renderList({source}, ({params}) => {{\n  return {}\n}})",
            indent_after_first_line(&body, 2)
        )
    }

    fn render_for_slot_params(&self, slot: &vuec_ast::Vue3DomForSlot) -> String {
        let mut params = vec![self.render_js_pattern(slot.value_alias)];
        if let Some(key) = slot.key_alias {
            params.push(self.render_js_pattern(key));
        }
        if let Some(index) = slot.index_alias {
            params.push(self.render_js_pattern(index));
        }
        params.join(", ")
    }

    fn render_dynamic_slot_object(
        &self,
        slot: &vuec_ast::Vue3DomDynamicSlotObject,
        scope: &RenderScope,
    ) -> String {
        let params = slot
            .params
            .map(|params| format!("({})", self.render_js_pattern(params)))
            .unwrap_or_else(|| "()".into());
        let child_scope = slot
            .params
            .map(|params| self.scope_with_pattern(scope, params))
            .unwrap_or_else(|| scope.clone());
        let children = slot
            .children
            .iter()
            .filter_map(|child_id| {
                self.render_node(*child_id, Vue3DomMirRenderMode::Child, &child_scope)
            })
            .collect::<Vec<_>>();
        let body = render_array(&children);
        let mut properties = vec![
            format!("name: {}", self.render_slot_name(&slot.name, scope)),
            format!("fn: _withCtx({params} => {body})"),
        ];
        if let Some(key) = &slot.key {
            properties.push(format!("key: {}", quote_string(key)));
        }
        render_object(&properties)
    }

    fn render_slot_name(&self, name: &Vue3DomSlotName, scope: &RenderScope) -> String {
        match name {
            Vue3DomSlotName::Static(name) => quote_string(name),
            Vue3DomSlotName::Dynamic(name) => self.render_js_expr(*name, scope),
        }
    }

    fn render_if(
        &self,
        node_id: NodeId,
        condition: Option<JsExprId>,
        scope: &RenderScope,
    ) -> String {
        let branch = self.render_root_children(node_id, scope);
        let Some(condition) = condition else {
            return branch;
        };
        let condition = self.render_js_expr(condition, scope);
        format!(
            "{}\n  ? {}\n  : _createCommentVNode(\"v-if\", true)",
            render_condition(&condition, self.options),
            indent_after_first_line(&branch, 4)
        )
    }

    fn render_for(&self, node_id: NodeId, for_mir: &Vue3ForMir, scope: &RenderScope) -> String {
        let source = self.render_js_expr(for_mir.source, scope);
        let child_scope = self.scope_with_for_mir(scope, for_mir);
        let children = self.render_children(node_id, Vue3DomMirRenderMode::Root, &child_scope);
        let body = match children.as_slice() {
            [] => "null".into(),
            [single] => single.clone(),
            _ => render_array(&children),
        };
        let params = self.render_for_params(for_mir);
        if let Some(memo) = &for_mir.memo {
            return self.render_memo_for(
                &source,
                &params,
                for_mir.key.as_ref(),
                memo,
                &body,
                &child_scope,
            );
        }
        let fragment_flag = if for_mir.key.is_some() {
            "128 /* KEYED_FRAGMENT */"
        } else {
            "256 /* UNKEYED_FRAGMENT */"
        };
        format!(
            "(_openBlock(true), _createElementBlock(_Fragment, null, _renderList({source}, ({params}) => {{\n  return {}\n}}), {fragment_flag}))",
            indent_after_first_line(&body, 2)
        )
    }

    fn render_for_params(&self, for_mir: &Vue3ForMir) -> String {
        let mut params = vec![self.render_js_pattern(for_mir.value_alias)];
        if let Some(key) = for_mir.key_alias {
            params.push(self.render_js_pattern(key));
        }
        if let Some(index) = for_mir.index_alias {
            params.push(self.render_js_pattern(index));
        }
        params.join(", ")
    }

    fn render_memo_for(
        &self,
        source: &str,
        params: &str,
        key: Option<&MirExpr>,
        memo: &Vue3ForMemo,
        body: &str,
        scope: &RenderScope,
    ) -> String {
        let params = format!("{params}, __, ___, _cached");
        let memo_expression = self.render_js_expr(memo.expression, scope);
        let guard = key.map_or_else(
            || "_cached && _cached.el && _isMemoSame(_cached, _memo)".into(),
            |key| {
                format!(
                    "_cached && _cached.el && _cached.key === {} && _isMemoSame(_cached, _memo)",
                    self.render_mir_expr(key, scope)
                )
            },
        );
        format!(
            "(_openBlock(true), _createElementBlock(_Fragment, null, _renderList({source}, ({params}) => {{\n  const _memo = ({memo_expression})\n  if ({guard}) return _cached\n  const _item = {}\n  _item.memo = _memo\n  return _item\n}}, _cache, {}), 128 /* KEYED_FRAGMENT */))",
            indent_after_first_line(body, 2),
            memo.index
        )
    }

    fn render_mir_expr(&self, expr: &MirExpr, scope: &RenderScope) -> String {
        match expr {
            MirExpr::String(value) => quote_string(value),
            MirExpr::Bool(value) => value.to_string(),
            MirExpr::Null => "null".into(),
            MirExpr::JsExpr(expr) => self.render_js_expr(*expr, scope),
            MirExpr::Helper(helper) => helper_reference(*helper),
        }
    }

    fn render_js_expr(&self, id: JsExprId, scope: &RenderScope) -> String {
        self.js
            .expressions()
            .get(id.0 as usize)
            .map(|entry| rewrite_expression_with_scope(&entry.source, self.options, scope))
            .unwrap_or_else(|| "undefined".into())
    }

    fn raw_js_expr(&self, id: JsExprId) -> Option<&str> {
        self.js
            .expressions()
            .get(id.0 as usize)
            .map(|entry| entry.source.trim())
    }

    fn render_js_stmt(&self, id: vuec_ast::JsStmtId, scope: &RenderScope) -> String {
        self.js
            .statements()
            .get(id.0 as usize)
            .map(|entry| rewrite_handler_expression_with_scope(&entry.source, self.options, scope))
            .unwrap_or_else(|| "undefined".into())
    }

    fn render_js_pattern(&self, id: JsPatternId) -> String {
        self.js
            .patterns()
            .get(id.0 as usize)
            .map(|entry| entry.source.to_string())
            .unwrap_or_else(|| "_item".into())
    }

    fn scope_with_for_mir(&self, scope: &RenderScope, for_mir: &Vue3ForMir) -> RenderScope {
        let mut locals = self.pattern_locals(for_mir.value_alias);
        if let Some(key) = for_mir.key_alias {
            locals.extend(self.pattern_locals(key));
        }
        if let Some(index) = for_mir.index_alias {
            locals.extend(self.pattern_locals(index));
        }
        scope.with_locals(locals)
    }

    fn scope_with_for_slot(
        &self,
        scope: &RenderScope,
        slot: &vuec_ast::Vue3DomForSlot,
    ) -> RenderScope {
        let mut locals = self.pattern_locals(slot.value_alias);
        if let Some(key) = slot.key_alias {
            locals.extend(self.pattern_locals(key));
        }
        if let Some(index) = slot.index_alias {
            locals.extend(self.pattern_locals(index));
        }
        scope.with_locals(locals)
    }

    fn scope_with_pattern(&self, scope: &RenderScope, pattern: JsPatternId) -> RenderScope {
        scope.with_locals(self.pattern_locals(pattern))
    }

    fn pattern_locals(&self, pattern: JsPatternId) -> Vec<String> {
        extract_v_for_alias_locals(&self.render_js_pattern(pattern))
    }
}

pub(crate) struct Vue3SsrMirCodegen<'a> {
    pub(crate) mir: &'a Vue3SsrMir,
    pub(crate) js: &'a JsAstStore,
    pub(crate) options: &'a Vue3CompilerOptions,
}

#[derive(Clone, Debug)]
pub(crate) struct SsrRootAttrs {
    pub(crate) attrs: Option<String>,
    pub(crate) css_vars: Option<String>,
    pub(crate) target_start: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SsrRootSpan {
    pub(crate) start: usize,
    pub(crate) attrs_index: Option<usize>,
}

impl<'a> Vue3SsrMirCodegen<'a> {
    pub(crate) fn new(
        mir: &'a Vue3SsrMir,
        js: &'a JsAstStore,
        options: &'a Vue3CompilerOptions,
    ) -> Self {
        Self { mir, js, options }
    }

    pub(crate) fn generate(self) -> CodegenResult {
        let mut writer = CodeWriter::new();
        let scope = RenderScope::default();
        let vue_helpers = self.vue_helpers();
        let ssr_helpers = self.ssr_helpers();
        let mut declarations = self.component_declarations();
        declarations.extend(self.directive_declarations());
        let mut preamble = String::new();
        self.render_preamble(&mut writer, &mut preamble, &vue_helpers, &ssr_helpers);
        self.render_function_start(&mut writer);
        writer.indent();
        if self.needs_dynamic_model_temp() {
            writer.push_line("let _temp0");
            writer.newline();
        }
        let use_with = self.use_with_block();
        if use_with {
            writer.push_line("with (_ctx) {");
            writer.indent();
        }
        for declaration in &declarations {
            writer.push_line(declaration);
        }
        if !declarations.is_empty() {
            writer.newline();
        }
        if let Some(css_vars) = self.render_ssr_css_vars(&scope) {
            writer.push_str("const _cssVars = { style: ");
            writer.push_raw(&css_vars);
            writer.push_str("}");
            writer.newline();
        }
        self.render_root_children(&scope, &mut writer);
        if use_with {
            writer.dedent();
            writer.push_line("}");
        }
        writer.dedent();
        writer.push_line("}");
        CodegenResult {
            code: self.finish_code(writer),
            map: None,
            ast_summary: format!("vue3-ssr-mir-nodes={}", self.mir.len()),
            diagnostics: Vec::new(),
            preamble,
        }
    }

    fn finish_code(&self, writer: CodeWriter) -> String {
        let code = writer.finish().trim_end().to_string();
        if self.options.inline || self.options.mode == "module" || code.starts_with("const ") {
            code
        } else {
            format!("\n{code}")
        }
    }

    fn render_preamble(
        &self,
        writer: &mut CodeWriter,
        preamble: &mut String,
        vue_helpers: &[RuntimeHelper],
        ssr_helpers: &[RuntimeHelper],
    ) {
        if self.options.inline {
            let mut inline = CodeWriter::new();
            if self.options.mode == "module" {
                if !vue_helpers.is_empty() {
                    inline.push_line(&format!(
                        "import {{ {} }} from \"vue\"",
                        import_helper_aliases(vue_helpers)
                    ));
                }
                if !ssr_helpers.is_empty() {
                    inline.push_line(&format!(
                        "import {{ {} }} from \"vue/server-renderer\"",
                        import_helper_aliases(ssr_helpers)
                    ));
                }
            } else {
                if !vue_helpers.is_empty() {
                    inline.push_line(&format!(
                        "const {{ {} }} = require(\"vue\")",
                        helper_aliases(vue_helpers)
                    ));
                }
                if !ssr_helpers.is_empty() {
                    inline.push_line(&format!(
                        "const {{ {} }} = require(\"vue/server-renderer\")",
                        helper_aliases(ssr_helpers)
                    ));
                }
                inline.push_str("return ");
            }
            let rendered = inline.finish();
            if !rendered.is_empty() {
                if self.options.mode == "module" {
                    *preamble = format!("{}\n\n", rendered.trim_end());
                } else {
                    *preamble = rendered;
                }
            }
            return;
        }
        if self.options.mode == "module" {
            if !vue_helpers.is_empty() {
                writer.push_line(&format!(
                    "import {{ {} }} from \"vue\"",
                    import_helper_aliases(vue_helpers)
                ));
            }
            if !ssr_helpers.is_empty() {
                writer.push_line(&format!(
                    "import {{ {} }} from \"vue/server-renderer\"",
                    import_helper_aliases(ssr_helpers)
                ));
            }
            if (!vue_helpers.is_empty() || !ssr_helpers.is_empty())
                && self.asset_imports().is_empty()
            {
                writer.newline();
            }
            for import in self.asset_imports() {
                writer.push_line(&format!("import {} from '{}'", import.name, import.path));
            }
            if !self.asset_imports().is_empty() {
                writer.newline();
                writer.newline();
            }
            writer.push_str("export ");
            return;
        }
        if !vue_helpers.is_empty() {
            writer.push_line(&format!(
                "const {{ {} }} = require(\"vue\")",
                helper_aliases(vue_helpers)
            ));
        }
        if !ssr_helpers.is_empty() {
            writer.push_line(&format!(
                "const {{ {} }} = require(\"vue/server-renderer\")",
                helper_aliases(ssr_helpers)
            ));
        }
        if !vue_helpers.is_empty() || !ssr_helpers.is_empty() {
            writer.newline();
        }
        writer.push_str("return ");
    }

    fn asset_imports(&self) -> &[vuec_ast::Vue3ImportItem] {
        self.mir
            .node(self.mir.root)
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::Root(root) => Some(root.imports.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }

    fn root_children(&self) -> &[NodeId] {
        self.mir
            .node(self.mir.root)
            .map(|node| node.children.as_slice())
            .unwrap_or(&[])
    }

    fn effective_root_children<'b>(&self, children: &'b [NodeId]) -> &'b [NodeId] {
        if children.len() < 3 {
            return children;
        }
        let has_template_open = self.ssr_push_string(children[0]).is_some_and(|value| {
            parse_ssr_open_tag_start(value)
                .is_some_and(|(tag, attrs)| tag == "template" && attrs.is_empty())
        }) && self
            .ssr_push_string(children[1])
            .is_some_and(|value| value == ">");
        let has_template_close = children
            .last()
            .and_then(|id| self.ssr_push_string(*id))
            .is_some_and(|value| value == "</template>");
        if has_template_open && has_template_close {
            &children[2..children.len() - 1]
        } else {
            children
        }
    }

    fn root_spans(&self, children: &[NodeId]) -> Vec<SsrRootSpan> {
        let mut spans = Vec::new();
        let mut index = 0usize;
        while index < children.len() {
            if let Some(next_index) = self.collect_ssr_text_span(children, index) {
                spans.push(SsrRootSpan {
                    start: index,
                    attrs_index: None,
                });
                index = next_index;
            } else if let Some((_, next_index)) =
                self.collect_ssr_static_element_span(children, index)
            {
                let attrs_index = children.get(index + 1).and_then(|id| {
                    self.mir
                        .node(*id)
                        .is_some_and(|node| matches!(node.kind, Vue3SsrMirKind::RenderAttrs(_)))
                        .then_some(index + 1)
                });
                spans.push(SsrRootSpan {
                    start: index,
                    attrs_index,
                });
                index = next_index;
            } else {
                spans.push(SsrRootSpan {
                    start: index,
                    attrs_index: None,
                });
                index += 1;
            }
        }
        spans
    }

    fn collect_ssr_text_span(&self, children: &[NodeId], index: usize) -> Option<usize> {
        let mut cursor = index;
        while cursor < children.len() && self.is_ssr_text_span_part(children[cursor]) {
            cursor += 1;
        }
        (cursor > index).then_some(cursor)
    }

    fn is_ssr_text_span_part(&self, child: NodeId) -> bool {
        match self.mir.node(child).map(|node| &node.kind) {
            Some(Vue3SsrMirKind::PushInterpolated(_)) => true,
            Some(Vue3SsrMirKind::PushString(value)) => {
                !value.starts_with("<!--")
                    && value != ">"
                    && value != "/>"
                    && !value.starts_with("</")
                    && parse_ssr_open_tag_start(value).is_none()
            }
            _ => false,
        }
    }

    fn collect_ssr_static_element_span(
        &self,
        children: &[NodeId],
        index: usize,
    ) -> Option<(String, usize)> {
        let (tag, _) = self
            .mir
            .node(*children.get(index)?)
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => parse_ssr_open_tag_start(value),
                _ => None,
            })?;
        let mut cursor = index + 1;
        if children
            .get(cursor)
            .and_then(|id| self.mir.node(*id))
            .is_some_and(|node| matches!(node.kind, Vue3SsrMirKind::RenderAttrs(_)))
        {
            cursor += 1;
        }
        let close_open = self.ssr_push_string(*children.get(cursor)?)?;
        cursor += 1;
        if close_open == "/>" {
            return Some((tag, cursor));
        }
        if close_open != ">" {
            return None;
        }
        while cursor < children.len() {
            if self
                .ssr_push_string(children[cursor])
                .is_some_and(|value| value == format!("</{tag}>"))
            {
                return Some((tag, cursor + 1));
            }
            if let Some(value) = self.ssr_push_string(children[cursor]) {
                if parse_ssr_open_tag_start(value).is_some() {
                    let (_, next_index) = self.collect_ssr_static_element_span(children, cursor)?;
                    cursor = next_index;
                    continue;
                }
            }
            cursor += 1;
        }
        None
    }

    fn root_static_merge_entries(
        &self,
        entries: &[(String, Option<String>)],
    ) -> Vec<(String, Option<String>)> {
        entries
            .iter()
            .filter(|(name, _)| !self.is_compiler_root_static_attr(name))
            .cloned()
            .collect()
    }

    fn root_static_tail_entries(
        &self,
        entries: &[(String, Option<String>)],
    ) -> Vec<(String, Option<String>)> {
        if self.options.scope_id.is_none() {
            return Vec::new();
        }
        let mut tail = Vec::new();
        if !entries
            .iter()
            .any(|(name, _)| self.is_compiler_root_static_attr(name))
        {
            if let Some(scope_id) = &self.options.scope_id {
                tail.push((scope_id.clone(), None));
            }
        }
        tail.extend(
            entries
                .iter()
                .filter(|(name, _)| self.is_compiler_root_static_attr(name))
                .cloned(),
        );
        tail
    }

    fn is_compiler_root_static_attr(&self, name: &str) -> bool {
        self.options
            .scope_id
            .as_deref()
            .is_some_and(|scope_id| scope_id == name)
    }

    fn render_static_attr_tail(&self, entries: &[(String, Option<String>)]) -> String {
        let mut rendered = String::new();
        for (name, value) in entries {
            rendered.push(' ');
            rendered.push_str(name);
            if let Some(value) = value {
                rendered.push_str("=\"");
                rendered.push_str(value);
                rendered.push('"');
            }
        }
        rendered
    }

    fn component_declarations(&self) -> Vec<String> {
        let mut components = Vec::<String>::new();
        for node in &self.mir.nodes {
            let Vue3SsrMirKind::RenderComponent(component) = &node.kind else {
                continue;
            };
            if component.dynamic {
                continue;
            }
            let MirExpr::String(tag) = &component.tag else {
                continue;
            };
            if components.iter().any(|item| item == tag) {
                continue;
            }
            components.push(tag.clone());
        }
        components
            .iter()
            .map(|tag| {
                format!(
                    "const {} = _resolveComponent({})",
                    component_asset_id(tag),
                    quote_string(tag)
                )
            })
            .collect()
    }

    fn directive_declarations(&self) -> Vec<String> {
        let mut directives = Vec::<String>::new();
        for node in &self.mir.nodes {
            let node_directives = match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => attrs.directives.as_slice(),
                Vue3SsrMirKind::RenderComponent(component) => component.directives.as_slice(),
                _ => &[],
            };
            for directive in node_directives {
                if directives.iter().any(|item| item == &directive.name) {
                    continue;
                }
                directives.push(directive.name.clone());
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

    fn render_function_start(&self, writer: &mut CodeWriter) {
        let args = self.render_function_args();
        if self.options.inline {
            writer.push_line(&format!("({args}) => {{"));
        } else {
            writer.push_line(&format!("function ssrRender({args}) {{"));
        }
    }

    fn render_function_args(&self) -> String {
        if self.options.inline || self.options.binding_metadata.is_empty() {
            "_ctx, _push, _parent, _attrs".into()
        } else {
            "_ctx, _push, _parent, _attrs, $props, $setup, $data, $options".into()
        }
    }

    fn use_with_block(&self) -> bool {
        !self.options.prefix_identifiers && self.options.mode != "module"
    }

    fn needs_dynamic_model_temp(&self) -> bool {
        self.mir.nodes.iter().any(|node| {
            matches!(
                &node.kind,
                Vue3SsrMirKind::RenderAttrs(attrs)
                    if attrs.directive_content || attrs.textarea_value_fallback.is_some() || matches!(
                        attrs.v_model.as_ref().map(|model| &model.kind),
                        Some(Vue3SsrModelKind::InputDynamicProps)
                    )
            )
        })
    }

    fn has_ssr_css_vars(&self) -> bool {
        self.options
            .ssr_css_vars
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
    }

    fn render_ssr_css_vars(&self, scope: &RenderScope) -> Option<String> {
        let vars = self.options.ssr_css_vars.as_ref()?.trim();
        if vars.is_empty() {
            return None;
        }
        Some(rewrite_ssr_css_vars_expression(vars, self.options, scope))
    }

    fn root_attrs_for_children(&self, children: &[NodeId]) -> Option<SsrRootAttrs> {
        let children = self.effective_root_children(children);
        let css_vars = self.has_ssr_css_vars().then(|| "_cssVars".to_string());
        let root_spans = self.root_spans(children);
        if root_spans.len() == 1 {
            let root_span = *root_spans.first()?;
            let root = *children.get(root_span.start)?;
            if self.node_accepts_root_attrs(root) {
                return Some(SsrRootAttrs {
                    attrs: Some("_attrs".to_string()),
                    css_vars,
                    target_start: Some(root_span.start),
                });
            }
            return None;
        }
        let visible_spans = root_spans
            .iter()
            .copied()
            .filter(|span| !self.is_ssr_root_comment_span(children, *span))
            .collect::<Vec<_>>();
        if let [root_span] = visible_spans.as_slice() {
            let root = *children.get(root_span.start)?;
            if self.node_accepts_root_attrs(root) {
                return Some(SsrRootAttrs {
                    attrs: Some("_attrs".to_string()),
                    css_vars,
                    target_start: Some(root_span.start),
                });
            }
        }
        css_vars.map(|css_vars| SsrRootAttrs {
            attrs: None,
            css_vars: Some(css_vars),
            target_start: None,
        })
    }

    fn is_ssr_root_comment_span(&self, children: &[NodeId], span: SsrRootSpan) -> bool {
        children
            .get(span.start)
            .and_then(|id| self.ssr_push_string(*id))
            .is_some_and(|value| {
                value.starts_with("<!--") && value != "<!--[-->" && value != "<!--]-->"
            })
    }

    fn is_ssr_comment_or_fragment_marker_span(
        &self,
        children: &[NodeId],
        span: SsrRootSpan,
    ) -> bool {
        children
            .get(span.start)
            .and_then(|id| self.ssr_push_string(*id))
            .is_some_and(|value| value.starts_with("<!--"))
    }

    fn root_attrs_for_branch_children(
        &self,
        children: &[NodeId],
        root_attrs: Option<&SsrRootAttrs>,
    ) -> Option<SsrRootAttrs> {
        let root_attrs = root_attrs?;
        if root_attrs.attrs.is_none() {
            return Some(root_attrs.clone());
        }
        let root_spans = self.root_spans(children);
        let visible_spans = root_spans
            .iter()
            .copied()
            .filter(|span| !self.is_ssr_comment_or_fragment_marker_span(children, *span))
            .collect::<Vec<_>>();
        if let [root_span] = visible_spans.as_slice() {
            if children
                .get(root_span.start)
                .is_some_and(|root| self.node_accepts_root_attrs(*root))
            {
                let mut attrs = root_attrs.clone();
                attrs.target_start = Some(root_span.start);
                return Some(attrs);
            }
        }
        self.root_attrs_css_vars_only(root_attrs)
    }

    fn root_attrs_css_vars_only(&self, root_attrs: &SsrRootAttrs) -> Option<SsrRootAttrs> {
        root_attrs.css_vars.as_ref().map(|css_vars| SsrRootAttrs {
            attrs: None,
            css_vars: Some(css_vars.clone()),
            target_start: None,
        })
    }

    fn ssr_css_vars_only_root_attrs(&self) -> Option<SsrRootAttrs> {
        self.has_ssr_css_vars().then(|| SsrRootAttrs {
            attrs: None,
            css_vars: Some("_cssVars".to_string()),
            target_start: None,
        })
    }

    fn root_attr_node_for_children(
        &self,
        children: &[NodeId],
        root_attrs: &SsrRootAttrs,
    ) -> Option<NodeId> {
        let target_start = root_attrs.target_start?;
        let children = self.effective_root_children(children);
        let root_spans = self.root_spans(children);
        let root_span = root_spans.iter().find(|span| span.start == target_start)?;
        let attrs_index = root_span.attrs_index?;
        children.get(attrs_index).copied()
    }

    fn root_attrs_for_child_slice(
        &self,
        root_attrs: Option<&SsrRootAttrs>,
        offset: usize,
    ) -> Option<SsrRootAttrs> {
        root_attrs.and_then(|attrs| {
            let mut attrs = attrs.clone();
            if let Some(target_start) = attrs.target_start {
                attrs.target_start = target_start.checked_sub(offset);
                if attrs.target_start.is_none() {
                    return None;
                }
            }
            Some(attrs)
        })
    }

    fn root_attrs_for_render_index(
        &self,
        children: &[NodeId],
        index: usize,
        root_attrs: &SsrRootAttrs,
    ) -> Option<SsrRootAttrs> {
        if let Some(target_start) = root_attrs.target_start {
            if index == target_start
                || self.root_attrs_apply_to_render_attrs_index(children, index, target_start)
            {
                let mut attrs = root_attrs.clone();
                attrs.target_start = Some(0);
                return Some(attrs);
            }
            return None;
        }
        if root_attrs.attrs.is_none() && root_attrs.css_vars.is_some() {
            return Some(root_attrs.clone());
        }
        None
    }

    fn root_attrs_apply_to_render_attrs_index(
        &self,
        children: &[NodeId],
        index: usize,
        target_start: usize,
    ) -> bool {
        target_start.checked_add(1) == Some(index)
            && children
                .get(target_start)
                .and_then(|id| self.ssr_push_string(*id))
                .is_some_and(|value| parse_ssr_open_tag_start(value).is_some())
            && children.get(index).is_some_and(|id| {
                self.mir
                    .node(*id)
                    .is_some_and(|node| matches!(node.kind, Vue3SsrMirKind::RenderAttrs(_)))
            })
    }

    fn node_accepts_root_attrs(&self, node_id: NodeId) -> bool {
        match self.mir.node(node_id).map(|node| &node.kind) {
            Some(Vue3SsrMirKind::RenderComponent(_)) => true,
            Some(Vue3SsrMirKind::PushString(value)) => parse_ssr_open_tag_start(value).is_some(),
            Some(Vue3SsrMirKind::If { .. }) => self
                .mir
                .node(node_id)
                .is_some_and(|node| self.if_branches_accept_root_attrs(&node.children)),
            _ => false,
        }
    }

    fn if_branches_accept_root_attrs(&self, branch_ids: &[NodeId]) -> bool {
        let mut current = branch_ids;
        let mut accepts = false;
        loop {
            let (primary, alternate) = self.split_if_children(current);
            if !primary.is_empty() && self.children_accept_root_attrs(&primary) {
                accepts = true;
            }
            let Some(alternate) = alternate else {
                return accepts;
            };
            let Some(alternate_node) = self.mir.node(alternate) else {
                return accepts;
            };
            if matches!(alternate_node.kind, Vue3SsrMirKind::If { .. }) {
                current = &alternate_node.children;
                continue;
            }
            return accepts || self.node_accepts_root_attrs(alternate);
        }
    }

    fn children_accept_root_attrs(&self, children: &[NodeId]) -> bool {
        let spans = self.root_spans(children);
        let [span] = spans.as_slice() else {
            return false;
        };
        children
            .get(span.start)
            .is_some_and(|root| self.node_accepts_root_attrs(*root))
    }

    fn vue_helpers(&self) -> Vec<RuntimeHelper> {
        let mut helpers = Vec::new();
        let root_children = self.root_children();
        let root_attrs = self.root_attrs_for_children(root_children);
        let root_attr_node = root_attrs
            .as_ref()
            .and_then(|root_attrs| self.root_attr_node_for_children(root_children, root_attrs));
        if let Some(root_attrs) = &root_attrs {
            if self.root_attrs_need_merge_props(self.root_children(), &root_attrs) {
                push_unique_helper(&mut helpers, RuntimeHelper::Vue3MergeProps);
            }
        }
        for node in &self.mir.nodes {
            match &node.kind {
                Vue3SsrMirKind::RenderComponent(component) => {
                    if component.dynamic {
                        push_unique_helper(
                            &mut helpers,
                            RuntimeHelper::Vue3ResolveDynamicComponent,
                        );
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateVNode);
                    } else if matches!(&component.tag, MirExpr::String(_)) {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3ResolveComponent);
                    }
                    if !component.directives.is_empty() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3ResolveDirective);
                        if component.directives.len() > 1
                            || root_attr_node == Some(node.id)
                            || self.node_accepts_root_attrs(node.id)
                            || self
                                .render_component_props(&component.props, &RenderScope::default())
                                != "null"
                        {
                            push_unique_helper(&mut helpers, RuntimeHelper::Vue3MergeProps);
                        }
                    }
                    if let Some(slots) = &component.slots {
                        let rendered = self.render_component_slots(slots, &RenderScope::default());
                        for helper in render_helpers_from_code(vue3_helper_order(false), &rendered)
                        {
                            push_unique_helper(&mut helpers, helper);
                        }
                    } else if self
                        .mir
                        .node(node.id)
                        .is_some_and(|node| !node.children.is_empty())
                    {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3WithCtx);
                        if self.vnode_fallback_uses_create_vnode(node.id) {
                            push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateVNode);
                        }
                        self.push_vnode_fallback_helpers(node.id, &mut helpers);
                        self.push_rendered_vnode_fallback_helpers(node.id, &mut helpers);
                    }
                    self.push_prop_helpers(&component.props, &mut helpers);
                }
                Vue3SsrMirKind::RenderSlot(slot) => {
                    self.push_prop_helpers(&slot.props, &mut helpers);
                }
                Vue3SsrMirKind::RenderAttrs(attrs) => {
                    if root_attr_node == Some(node.id)
                        && root_attrs
                            .as_ref()
                            .is_some_and(|root_attrs| root_attrs.attrs.is_none())
                    {
                        continue;
                    }
                    if !attrs.directives.is_empty() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3ResolveDirective);
                    }
                    if self.ssr_attrs_need_merge_props(attrs, root_attr_node == Some(node.id)) {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3MergeProps);
                    }
                    if attrs.force_render_attrs {
                        self.push_prop_helpers(&attrs.props, &mut helpers);
                        continue;
                    }
                    if root_attr_node == Some(node.id) {
                        if matches!(
                            attrs.v_model.as_ref().map(|model| &model.kind),
                            Some(Vue3SsrModelKind::InputDynamicProps)
                        ) {
                            self.push_dynamic_model_props_vue_helpers(&attrs.props, &mut helpers);
                        } else if attrs.v_show.is_some() {
                            self.push_v_show_merged_props_vue_helpers(&attrs.props, &mut helpers);
                        } else if self.ssr_attrs_use_rebuilt_element_attrs(attrs) {
                            for binding in &attrs.props.dynamic_bindings {
                                if binding.dynamic_arg && binding.camel {
                                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3Camelize);
                                }
                            }
                        } else {
                            self.push_prop_helpers(&attrs.props, &mut helpers);
                        }
                    }
                    if ((attrs.v_show.is_some() && ssr_attrs_has_object_binding(&attrs.props))
                        || matches!(
                            attrs.v_model.as_ref().map(|model| &model.kind),
                            Some(Vue3SsrModelKind::InputDynamicProps)
                        ))
                        && root_attr_node != Some(node.id)
                    {
                        if matches!(
                            attrs.v_model.as_ref().map(|model| &model.kind),
                            Some(Vue3SsrModelKind::InputDynamicProps)
                        ) {
                            self.push_dynamic_model_props_vue_helpers(&attrs.props, &mut helpers);
                        } else {
                            self.push_v_show_merged_props_vue_helpers(&attrs.props, &mut helpers);
                        }
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3MergeProps);
                    }
                }
                Vue3SsrMirKind::Suspense(_) => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3WithCtx);
                }
                _ => {}
            }
        }
        sort_helpers_by_order(&mut helpers, vue3_helper_order(false));
        if helpers.contains(&RuntimeHelper::Vue3ResolveComponent)
            && helpers.contains(&RuntimeHelper::Vue3WithCtx)
        {
            if helpers.contains(&RuntimeHelper::Vue3ToDisplayString) {
                move_helper_after(
                    &mut helpers,
                    RuntimeHelper::Vue3ToDisplayString,
                    RuntimeHelper::Vue3WithCtx,
                );
            }
            if helpers.contains(&RuntimeHelper::Vue3CreateTextVNode) {
                let after = if helpers.contains(&RuntimeHelper::Vue3ToDisplayString) {
                    RuntimeHelper::Vue3ToDisplayString
                } else {
                    RuntimeHelper::Vue3WithCtx
                };
                move_helper_after(&mut helpers, RuntimeHelper::Vue3CreateTextVNode, after);
            }
        }
        if helpers.contains(&RuntimeHelper::Vue3RenderSlot)
            && helpers.contains(&RuntimeHelper::Vue3WithCtx)
        {
            move_helper_after(
                &mut helpers,
                RuntimeHelper::Vue3RenderSlot,
                RuntimeHelper::Vue3WithCtx,
            );
        }
        if helpers.contains(&RuntimeHelper::Vue3ResolveComponent)
            && helpers.contains(&RuntimeHelper::Vue3WithCtx)
        {
            move_helper_before(
                &mut helpers,
                RuntimeHelper::Vue3ResolveComponent,
                RuntimeHelper::Vue3WithCtx,
            );
            if helpers.contains(&RuntimeHelper::Vue3RenderList) {
                move_helper_after(
                    &mut helpers,
                    RuntimeHelper::Vue3RenderList,
                    RuntimeHelper::Vue3WithCtx,
                );
            }
            if helpers.contains(&RuntimeHelper::Vue3Fragment) {
                let after = if helpers.contains(&RuntimeHelper::Vue3RenderList) {
                    RuntimeHelper::Vue3RenderList
                } else {
                    RuntimeHelper::Vue3WithCtx
                };
                move_helper_after(&mut helpers, RuntimeHelper::Vue3Fragment, after);
            }
            if helpers.contains(&RuntimeHelper::Vue3OpenBlock) {
                let after = if helpers.contains(&RuntimeHelper::Vue3Fragment) {
                    RuntimeHelper::Vue3Fragment
                } else {
                    RuntimeHelper::Vue3WithCtx
                };
                move_helper_after(&mut helpers, RuntimeHelper::Vue3OpenBlock, after);
            }
            if helpers.contains(&RuntimeHelper::Vue3CreateBlock) {
                move_helper_after(
                    &mut helpers,
                    RuntimeHelper::Vue3CreateBlock,
                    RuntimeHelper::Vue3OpenBlock,
                );
            }
            if helpers.contains(&RuntimeHelper::Vue3CreateCommentVNode) {
                let after = if helpers.contains(&RuntimeHelper::Vue3CreateBlock) {
                    RuntimeHelper::Vue3CreateBlock
                } else {
                    RuntimeHelper::Vue3WithCtx
                };
                move_helper_after(&mut helpers, RuntimeHelper::Vue3CreateCommentVNode, after);
            }
            if helpers.contains(&RuntimeHelper::Vue3Transition) {
                move_helper_after(
                    &mut helpers,
                    RuntimeHelper::Vue3Transition,
                    RuntimeHelper::Vue3CreateCommentVNode,
                );
            }
            if helpers.contains(&RuntimeHelper::Vue3CreateVNode) {
                let after = if helpers.contains(&RuntimeHelper::Vue3Transition) {
                    RuntimeHelper::Vue3Transition
                } else {
                    RuntimeHelper::Vue3WithCtx
                };
                move_helper_after(&mut helpers, RuntimeHelper::Vue3CreateVNode, after);
            }
        }
        if helpers.contains(&RuntimeHelper::Vue3ResolveDynamicComponent)
            && helpers.contains(&RuntimeHelper::Vue3MergeProps)
            && helpers.contains(&RuntimeHelper::Vue3CreateVNode)
        {
            move_helper_before(
                &mut helpers,
                RuntimeHelper::Vue3MergeProps,
                RuntimeHelper::Vue3CreateVNode,
            );
        }
        if helpers.contains(&RuntimeHelper::Vue3CreateVNode)
            && helpers.contains(&RuntimeHelper::Vue3CreateTextVNode)
            && !helpers.contains(&RuntimeHelper::Vue3ToDisplayString)
        {
            move_helper_before(
                &mut helpers,
                RuntimeHelper::Vue3CreateVNode,
                RuntimeHelper::Vue3CreateTextVNode,
            );
        }
        if helpers.contains(&RuntimeHelper::Vue3CreateSlots)
            && helpers.contains(&RuntimeHelper::Vue3RenderList)
        {
            move_helper_before(
                &mut helpers,
                RuntimeHelper::Vue3RenderList,
                RuntimeHelper::Vue3CreateSlots,
            );
        }
        helpers
    }

    fn root_attrs_need_merge_props(&self, children: &[NodeId], root_attrs: &SsrRootAttrs) -> bool {
        let root_extra =
            root_attrs.attrs.is_some() as usize + root_attrs.css_vars.is_some() as usize;
        if root_extra > 1 {
            return true;
        }
        if root_extra == 0 {
            return false;
        }
        self.root_spans(children).iter().any(|root_span| {
            let Some(root) = children.get(root_span.start) else {
                return false;
            };
            match self.mir.node(*root).map(|node| &node.kind) {
                Some(Vue3SsrMirKind::RenderComponent(component)) => self
                    .render_ordered_props(&component.props, &RenderScope::default())
                    .is_some(),
                Some(Vue3SsrMirKind::PushString(_)) => {
                    self.root_element_has_rendered_attrs(children, *root_span)
                        || self
                            .root_element_static_merge_props(children, *root_span)
                            .is_some()
                }
                _ => false,
            }
        })
    }

    fn root_element_has_rendered_attrs(&self, children: &[NodeId], root_span: SsrRootSpan) -> bool {
        root_span
            .attrs_index
            .and_then(|index| children.get(index))
            .is_some_and(|id| {
                self.mir
                    .node(*id)
                    .is_some_and(|node| matches!(node.kind, Vue3SsrMirKind::RenderAttrs(_)))
            })
    }

    fn root_element_static_merge_props(
        &self,
        children: &[NodeId],
        root_span: SsrRootSpan,
    ) -> Option<String> {
        let (_, entries) = children
            .get(root_span.start)
            .and_then(|id| self.mir.node(*id))
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => parse_ssr_open_tag_start(value),
                _ => None,
            })?;
        self.root_static_props(&entries)
    }

    fn root_static_props(&self, entries: &[(String, Option<String>)]) -> Option<String> {
        let props = self
            .root_static_merge_entries(entries)
            .into_iter()
            .map(|(name, value)| {
                (
                    name,
                    value.map(|value| decode_vue3_ssr_escaped_attr(&value)),
                )
            })
            .collect::<Vec<_>>();
        self.render_vnode_fallback_static_props(&props)
    }

    fn push_vnode_fallback_helpers(&self, parent: NodeId, helpers: &mut Vec<RuntimeHelper>) {
        let Some(node) = self.mir.node(parent) else {
            return;
        };
        for child_id in &node.children {
            let Some(child) = self.mir.node(*child_id) else {
                continue;
            };
            match &child.kind {
                Vue3SsrMirKind::PushInterpolated(_) => {
                    push_unique_helper(helpers, RuntimeHelper::Vue3CreateTextVNode);
                    push_unique_helper(helpers, RuntimeHelper::Vue3ToDisplayString);
                }
                Vue3SsrMirKind::RenderAttrs(attrs) => {
                    if attrs.props.normalize.guard_reactive_props {
                        push_unique_helper(helpers, RuntimeHelper::Vue3GuardReactiveProps);
                    }
                    if attrs.props.normalize.normalize_props {
                        push_unique_helper(helpers, RuntimeHelper::Vue3NormalizeProps);
                    }
                    if props_requires_merge_call(&attrs.props)
                        || self.ssr_attrs_has_static_fallback_props(*child_id)
                    {
                        push_unique_helper(helpers, RuntimeHelper::Vue3MergeProps);
                    }
                    self.push_prop_helpers(&attrs.props, helpers);
                }
                Vue3SsrMirKind::RenderComponent(component) => {
                    push_unique_helper(helpers, RuntimeHelper::Vue3CreateVNode);
                    if component.dynamic {
                        push_unique_helper(helpers, RuntimeHelper::Vue3ResolveDynamicComponent);
                    } else if matches!(&component.tag, MirExpr::String(_)) {
                        push_unique_helper(helpers, RuntimeHelper::Vue3ResolveComponent);
                    }
                    self.push_prop_helpers(&component.props, helpers);
                    self.push_vnode_fallback_helpers(*child_id, helpers);
                }
                Vue3SsrMirKind::RenderSlot(slot) => {
                    if self.render_slot_as_vnode_fallback(slot) {
                        push_unique_helper(helpers, RuntimeHelper::Vue3RenderSlot);
                    }
                }
                Vue3SsrMirKind::If { .. } | Vue3SsrMirKind::For(_) => {
                    self.push_vnode_fallback_helpers(*child_id, helpers);
                }
                _ => {}
            }
        }
    }

    fn push_rendered_vnode_fallback_helpers(
        &self,
        parent: NodeId,
        helpers: &mut Vec<RuntimeHelper>,
    ) {
        let scope = RenderScope::default().with_locals(vec!["_scopeId".into()]);
        let rendered = self
            .render_component_slot_vnode_fallback_children(parent, &scope)
            .join("\n");
        for helper in render_helpers_from_code(vue3_helper_order(false), &rendered) {
            push_unique_helper(helpers, helper);
        }
    }

    fn vnode_fallback_uses_create_vnode(&self, parent: NodeId) -> bool {
        let Some(node) = self.mir.node(parent) else {
            return false;
        };
        node.children.iter().any(|child_id| {
            self.mir
                .node(*child_id)
                .is_some_and(|child| match &child.kind {
                    Vue3SsrMirKind::PushString(value) => parse_ssr_open_tag_start(value).is_some(),
                    Vue3SsrMirKind::RenderComponent(_) => true,
                    Vue3SsrMirKind::If { .. } | Vue3SsrMirKind::For(_) => {
                        self.vnode_fallback_uses_create_vnode(*child_id)
                    }
                    _ => false,
                })
        })
    }

    fn ssr_attrs_has_static_fallback_props(&self, attrs_id: NodeId) -> bool {
        let Some(attrs_node) = self.mir.node(attrs_id) else {
            return false;
        };
        let Some(parent_id) = attrs_node.parent else {
            return false;
        };
        let Some(parent) = self.mir.node(parent_id) else {
            return false;
        };
        let Some(index) = parent.children.iter().position(|id| *id == attrs_id) else {
            return false;
        };
        parent.children[..index].iter().rev().find_map(|id| {
            self.mir.node(*id).and_then(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => {
                    parse_ssr_open_tag_start(value).map(|(_, attrs)| !attrs.is_empty())
                }
                _ => None,
            })
        }) == Some(true)
    }

    fn ssr_helpers(&self) -> Vec<RuntimeHelper> {
        let mut helpers = Vec::new();
        let root_children = self.root_children();
        let root_attrs = self.root_attrs_for_children(root_children);
        let root_spans = self.root_spans(root_children);
        let root_attr_node = root_attrs
            .as_ref()
            .and_then(|root_attrs| self.root_attr_node_for_children(root_children, root_attrs));
        for node in &self.mir.nodes {
            match &node.kind {
                Vue3SsrMirKind::Root(_) | Vue3SsrMirKind::PushString(_) => {}
                Vue3SsrMirKind::PushInterpolated(_) => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3SsrInterpolate);
                }
                Vue3SsrMirKind::RenderContent(Vue3SsrContent::Text { .. }) => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3SsrInterpolate);
                }
                Vue3SsrMirKind::RenderContent(Vue3SsrContent::Html { .. }) => {}
                Vue3SsrMirKind::RenderAttrs(attrs) => {
                    if root_attr_node == Some(node.id)
                        && root_attrs
                            .as_ref()
                            .is_some_and(|root_attrs| root_attrs.attrs.is_none())
                    {
                        continue;
                    }
                    if attrs.force_render_attrs {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3SsrRenderAttrs);
                        continue;
                    }
                    self.push_ssr_attr_helpers(
                        attrs,
                        root_attr_node == Some(node.id),
                        &mut helpers,
                    );
                    if attrs.v_show.is_some()
                        && !ssr_attrs_has_object_binding(&attrs.props)
                        && root_attr_node != Some(node.id)
                    {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3SsrRenderStyle);
                    }
                }
                Vue3SsrMirKind::RenderComponent(component) => {
                    push_unique_helper(
                        &mut helpers,
                        if component.dynamic {
                            RuntimeHelper::Vue3SsrRenderVNode
                        } else {
                            RuntimeHelper::Vue3SsrRenderComponent
                        },
                    );
                    if !component.directives.is_empty() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3SsrGetDirectiveProps);
                    }
                    if let Some(slots) = &component.slots {
                        let rendered = self.render_component_slots(slots, &RenderScope::default());
                        for helper in render_helpers_from_code(vue3_ssr_helper_order(), &rendered) {
                            push_unique_helper(&mut helpers, helper);
                        }
                    }
                }
                Vue3SsrMirKind::Transition => {
                    if self
                        .mir
                        .node(node.id)
                        .is_some_and(|node| !node.children.is_empty())
                    {
                        for child in &node.children {
                            if matches!(
                                self.mir.node(*child).map(|child| &child.kind),
                                Some(Vue3SsrMirKind::For(_))
                            ) {
                                push_unique_helper(&mut helpers, RuntimeHelper::Vue3SsrRenderList);
                            }
                        }
                    }
                }
                Vue3SsrMirKind::RenderSlot(slot) => {
                    push_unique_helper(
                        &mut helpers,
                        if slot.inner {
                            RuntimeHelper::Vue3SsrRenderSlotInner
                        } else {
                            RuntimeHelper::Vue3SsrRenderSlot
                        },
                    );
                }
                Vue3SsrMirKind::If { .. } => {}
                Vue3SsrMirKind::For(_) => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3SsrRenderList);
                }
                Vue3SsrMirKind::Teleport(_) => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3SsrRenderTeleport);
                }
                Vue3SsrMirKind::Suspense(suspense) => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3SsrRenderSuspense);
                    if self.has_ssr_css_vars()
                        && self.suspense_slots_need_css_var_ssr_render_attrs(suspense)
                    {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3SsrRenderAttrs);
                    }
                }
            }
        }
        if let Some(root_attrs) = root_attrs {
            if (root_attrs.attrs.is_some() || root_attrs.css_vars.is_some())
                && self.root_attrs_need_ssr_render_attrs(root_children, &root_spans)
            {
                push_unique_helper(&mut helpers, RuntimeHelper::Vue3SsrRenderAttrs);
            }
        }
        sort_helpers_by_order(&mut helpers, vue3_ssr_helper_order());
        self.apply_ssr_helper_order_preferences(&mut helpers);
        helpers
    }

    fn apply_ssr_helper_order_preferences(&self, helpers: &mut Vec<RuntimeHelper>) {
        if helpers.contains(&RuntimeHelper::Vue3SsrGetDirectiveProps) {
            move_helper_before_if_present(
                helpers,
                RuntimeHelper::Vue3SsrGetDirectiveProps,
                RuntimeHelper::Vue3SsrRenderComponent,
            );
        }
        if self.mir.nodes.iter().any(|node| {
            matches!(
                &node.kind,
                Vue3SsrMirKind::RenderComponent(component) if component.slots.is_some()
            )
        }) && helpers.contains(&RuntimeHelper::Vue3SsrRenderComponent)
            && helpers.contains(&RuntimeHelper::Vue3SsrInterpolate)
        {
            move_helper_before(
                helpers,
                RuntimeHelper::Vue3SsrRenderComponent,
                RuntimeHelper::Vue3SsrInterpolate,
            );
        }
        if helpers.contains(&RuntimeHelper::Vue3SsrRenderSlot) {
            move_helper_before_if_present(
                helpers,
                RuntimeHelper::Vue3SsrRenderSlot,
                RuntimeHelper::Vue3SsrInterpolate,
            );
            move_helper_before_if_present(
                helpers,
                RuntimeHelper::Vue3SsrRenderSlot,
                RuntimeHelper::Vue3SsrRenderComponent,
            );
            move_helper_before_if_present(
                helpers,
                RuntimeHelper::Vue3SsrRenderSlot,
                RuntimeHelper::Vue3SsrRenderAttrs,
            );
        }
        if helpers.contains(&RuntimeHelper::Vue3SsrRenderSlotInner) {
            move_helper_before_if_present(
                helpers,
                RuntimeHelper::Vue3SsrRenderSlotInner,
                RuntimeHelper::Vue3SsrRenderAttrs,
            );
        }
        if !helpers.contains(&RuntimeHelper::Vue3SsrRenderAttrs) {
            if helpers.contains(&RuntimeHelper::Vue3SsrRenderSlotInner) {
                move_helper_before(
                    helpers,
                    RuntimeHelper::Vue3SsrRenderSlotInner,
                    RuntimeHelper::Vue3SsrRenderSlot,
                );
            }
            return;
        }
        let has_dynamic_attrs = self.mir.nodes.iter().any(|node| {
            matches!(
                &node.kind,
                Vue3SsrMirKind::RenderAttrs(attrs)
                    if attrs.props.dynamic_bindings.iter().any(|binding| {
                        !binding.dynamic_arg
                            && !matches!(binding.name.as_str(), "class" | "style")
                    })
            )
        });
        let has_radio_or_true_value = self.mir.nodes.iter().any(|node| {
            matches!(
                &node.kind,
                Vue3SsrMirKind::RenderAttrs(attrs)
                    if matches!(
                        attrs.v_model.as_ref().map(|model| &model.kind),
                        Some(Vue3SsrModelKind::InputRadio { .. })
                            | Some(Vue3SsrModelKind::InputCheckboxTrueValue { .. })
                    )
            )
        });
        let has_checkbox = self.mir.nodes.iter().any(|node| {
            matches!(
                &node.kind,
                Vue3SsrMirKind::RenderAttrs(attrs)
                    if matches!(
                        attrs.v_model.as_ref().map(|model| &model.kind),
                        Some(Vue3SsrModelKind::InputCheckbox { .. })
                    )
            )
        });
        if !has_dynamic_attrs && has_radio_or_true_value {
            move_helper_before(
                helpers,
                RuntimeHelper::Vue3SsrLooseEqual,
                RuntimeHelper::Vue3SsrIncludeBooleanAttr,
            );
        } else if !has_dynamic_attrs && has_checkbox {
            move_helper_before(
                helpers,
                RuntimeHelper::Vue3SsrLooseContain,
                RuntimeHelper::Vue3SsrIncludeBooleanAttr,
            );
        }
        if helpers.contains(&RuntimeHelper::Vue3SsrInterpolate) {
            move_helper_before(
                helpers,
                RuntimeHelper::Vue3SsrInterpolate,
                RuntimeHelper::Vue3SsrRenderList,
            );
        }
        move_helper_before_if_present(
            helpers,
            RuntimeHelper::Vue3SsrRenderAttrs,
            RuntimeHelper::Vue3SsrInterpolate,
        );
        if helpers.contains(&RuntimeHelper::Vue3SsrGetDynamicModelProps) {
            move_helper_before(
                helpers,
                RuntimeHelper::Vue3SsrRenderAttrs,
                RuntimeHelper::Vue3SsrGetDynamicModelProps,
            );
        }
        if helpers.contains(&RuntimeHelper::Vue3SsrRenderList) {
            move_helper_before(
                helpers,
                RuntimeHelper::Vue3SsrRenderAttrs,
                RuntimeHelper::Vue3SsrRenderList,
            );
        }
        move_helper_before_if_present(
            helpers,
            RuntimeHelper::Vue3SsrRenderAttrs,
            RuntimeHelper::Vue3SsrInterpolate,
        );
        if helpers.contains(&RuntimeHelper::Vue3SsrRenderSlot) {
            move_helper_before_if_present(
                helpers,
                RuntimeHelper::Vue3SsrRenderSlot,
                RuntimeHelper::Vue3SsrRenderAttrs,
            );
        }
    }

    fn root_attrs_need_ssr_render_attrs(
        &self,
        children: &[NodeId],
        root_spans: &[SsrRootSpan],
    ) -> bool {
        root_spans.iter().any(|span| {
            children
                .get(span.start)
                .is_some_and(|root| self.node_needs_root_attr_ssr_render_attrs(*root))
        })
    }

    fn node_needs_root_attr_ssr_render_attrs(&self, node_id: NodeId) -> bool {
        match self.mir.node(node_id).map(|node| &node.kind) {
            Some(Vue3SsrMirKind::PushString(value)) => parse_ssr_open_tag_start(value).is_some(),
            Some(Vue3SsrMirKind::If { .. }) => self.mir.node(node_id).is_some_and(|node| {
                self.if_branches_need_root_attr_ssr_render_attrs(&node.children)
            }),
            Some(Vue3SsrMirKind::RenderComponent(_)) => false,
            _ => false,
        }
    }

    fn if_branches_need_root_attr_ssr_render_attrs(&self, branch_ids: &[NodeId]) -> bool {
        let mut current = branch_ids;
        loop {
            let (primary, alternate) = self.split_if_children(current);
            if self.children_need_root_attr_ssr_render_attrs(&primary) {
                return true;
            }
            let Some(alternate) = alternate else {
                return false;
            };
            let Some(alternate_node) = self.mir.node(alternate) else {
                return false;
            };
            if matches!(alternate_node.kind, Vue3SsrMirKind::If { .. }) {
                current = &alternate_node.children;
                continue;
            }
            return self.node_needs_root_attr_ssr_render_attrs(alternate);
        }
    }

    fn children_need_root_attr_ssr_render_attrs(&self, children: &[NodeId]) -> bool {
        let spans = self.root_spans(children);
        let [span] = spans.as_slice() else {
            return false;
        };
        children
            .get(span.start)
            .is_some_and(|root| self.node_needs_root_attr_ssr_render_attrs(*root))
    }

    fn suspense_slots_need_css_var_ssr_render_attrs(&self, suspense: &Vue3SsrSuspense) -> bool {
        suspense
            .slots
            .slots
            .iter()
            .any(|slot| self.children_need_css_var_ssr_render_attrs(&slot.children))
    }

    fn children_need_css_var_ssr_render_attrs(&self, children: &[NodeId]) -> bool {
        self.root_spans(children).iter().any(|span| {
            children
                .get(span.start)
                .is_some_and(|root| self.node_needs_root_attr_ssr_render_attrs(*root))
        })
    }

    fn push_ssr_attr_helpers(
        &self,
        attrs: &Vue3SsrAttrs,
        root_attrs_take_over: bool,
        helpers: &mut Vec<RuntimeHelper>,
    ) {
        self.push_ssr_v_model_helpers(attrs, helpers);
        if !attrs.directives.is_empty() {
            push_unique_helper(helpers, RuntimeHelper::Vue3SsrGetDirectiveProps);
            push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderAttrs);
            if attrs.directive_content {
                push_unique_helper(helpers, RuntimeHelper::Vue3SsrInterpolate);
            }
        }
        if attrs.textarea_value_fallback.is_some() {
            push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderAttrs);
            push_unique_helper(helpers, RuntimeHelper::Vue3SsrInterpolate);
        }
        if root_attrs_take_over {
            return;
        }
        let props = &attrs.props;
        if self.ssr_attrs_need_render_attrs(attrs) {
            push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderAttrs);
        }
        if attrs.v_show.is_none()
            && attrs.v_model.is_none()
            && self.ssr_attrs_need_render_attrs(attrs)
        {
            for binding in &props.dynamic_bindings {
                if binding.dynamic_arg && binding.camel {
                    push_unique_helper(helpers, RuntimeHelper::Vue3Camelize);
                }
            }
            return;
        }
        if matches!(
            attrs.v_model.as_ref().map(|model| &model.kind),
            Some(Vue3SsrModelKind::InputDynamicProps)
        ) {
            return;
        }
        if (attrs.v_show.is_some()
            || matches!(
                attrs.v_model.as_ref().map(|model| &model.kind),
                Some(Vue3SsrModelKind::InputDynamicProps)
            ))
            && ssr_attrs_has_object_binding(props)
        {
            push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderAttrs);
            return;
        }
        if props.segments.is_empty() {
            for binding in &props.dynamic_bindings {
                if attrs.v_show.is_none() || binding.name != "style" {
                    self.push_ssr_binding_helper(binding, helpers);
                }
            }
            for binding in &props.object_bindings {
                let _ = binding;
                push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderAttrs);
            }
        } else {
            for segment in &props.segments {
                match segment {
                    Vue3DomPropSegment::DynamicBinding(binding) => {
                        if attrs.v_show.is_none() || binding.name != "style" {
                            self.push_ssr_binding_helper(binding, helpers);
                        }
                    }
                    Vue3DomPropSegment::ObjectBinding(_) => {
                        push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderAttrs);
                    }
                    Vue3DomPropSegment::StaticAttr(_)
                    | Vue3DomPropSegment::Content(_)
                    | Vue3DomPropSegment::Model(_)
                    | Vue3DomPropSegment::Event(_)
                    | Vue3DomPropSegment::ObjectListeners(_) => {}
                }
            }
        }
    }

    fn ssr_attrs_need_render_attrs(&self, attrs: &Vue3SsrAttrs) -> bool {
        !attrs.directives.is_empty()
            || !attrs.props.object_bindings.is_empty()
            || attrs
                .props
                .dynamic_bindings
                .iter()
                .any(|binding| binding.dynamic_arg)
    }

    fn ssr_attrs_need_merge_props(&self, attrs: &Vue3SsrAttrs, has_root_attrs: bool) -> bool {
        if attrs.v_show.is_some() && !ssr_attrs_has_object_binding(&attrs.props) {
            return false;
        }
        let prop_chunks = ssr_attrs_prop_chunk_count(&attrs.props);
        has_root_attrs && (prop_chunks > 0 || !attrs.directives.is_empty())
            || prop_chunks > 1
            || attrs.props.object_bindings.len() > 1
            || (!attrs.directives.is_empty()
                && (prop_chunks > 0
                    || attrs.directives.len() > 1
                    || !attrs.props.object_bindings.is_empty()))
    }

    fn push_ssr_v_model_helpers(&self, attrs: &Vue3SsrAttrs, helpers: &mut Vec<RuntimeHelper>) {
        let Some(model) = &attrs.v_model else {
            return;
        };
        match &model.kind {
            Vue3SsrModelKind::InputValue => {
                push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderAttr);
            }
            Vue3SsrModelKind::InputRadio { .. }
            | Vue3SsrModelKind::InputCheckboxTrueValue { .. }
            | Vue3SsrModelKind::SelectOption { .. } => {
                push_unique_helper(helpers, RuntimeHelper::Vue3SsrIncludeBooleanAttr);
                push_unique_helper(helpers, RuntimeHelper::Vue3SsrLooseEqual);
                if matches!(model.kind, Vue3SsrModelKind::SelectOption { .. }) {
                    push_unique_helper(helpers, RuntimeHelper::Vue3SsrLooseContain);
                }
            }
            Vue3SsrModelKind::InputCheckbox { .. } => {
                push_unique_helper(helpers, RuntimeHelper::Vue3SsrIncludeBooleanAttr);
                push_unique_helper(helpers, RuntimeHelper::Vue3SsrLooseContain);
            }
            Vue3SsrModelKind::InputDynamicType { .. } => {
                push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderDynamicModel);
            }
            Vue3SsrModelKind::InputDynamicProps => {
                push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderAttrs);
                push_unique_helper(helpers, RuntimeHelper::Vue3SsrGetDynamicModelProps);
            }
            Vue3SsrModelKind::Textarea => {}
        }
    }

    fn push_ssr_binding_helper(&self, binding: &Vue3DomBinding, helpers: &mut Vec<RuntimeHelper>) {
        if binding.dynamic_arg {
            push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderAttrs);
            if binding.camel {
                push_unique_helper(helpers, RuntimeHelper::Vue3Camelize);
            }
        } else if binding.name == "class" {
            push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderClass);
        } else if binding.name == "style" {
            push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderStyle);
        } else if vue3_ssr_is_boolean_attr(&binding.name) {
            push_unique_helper(helpers, RuntimeHelper::Vue3SsrIncludeBooleanAttr);
        } else {
            push_unique_helper(helpers, RuntimeHelper::Vue3SsrRenderAttr);
        }
    }

    fn push_prop_helpers(&self, props: &Vue3DomProps, helpers: &mut Vec<RuntimeHelper>) {
        for binding in &props.dynamic_bindings {
            push_vue3_dom_binding_helpers(binding, helpers);
        }
        for event in &props.events {
            if event.dynamic_arg {
                push_unique_helper(helpers, RuntimeHelper::Vue3ToHandlerKey);
            }
        }
        for listeners in &props.object_listeners {
            if listeners.preserve_case {
                push_unique_helper(helpers, RuntimeHelper::Vue3ToHandlers);
            }
        }
        for segment in &props.segments {
            match segment {
                Vue3DomPropSegment::DynamicBinding(binding) => {
                    push_vue3_dom_binding_helpers(binding, helpers);
                }
                Vue3DomPropSegment::Event(event) if event.dynamic_arg => {
                    push_unique_helper(helpers, RuntimeHelper::Vue3ToHandlerKey);
                }
                Vue3DomPropSegment::ObjectListeners(_) => {
                    push_unique_helper(helpers, RuntimeHelper::Vue3ToHandlers);
                }
                Vue3DomPropSegment::StaticAttr(_)
                | Vue3DomPropSegment::Content(_)
                | Vue3DomPropSegment::Model(_)
                | Vue3DomPropSegment::Event(_)
                | Vue3DomPropSegment::ObjectBinding(_) => {}
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

    fn push_dynamic_model_props_vue_helpers(
        &self,
        props: &Vue3DomProps,
        helpers: &mut Vec<RuntimeHelper>,
    ) {
        for binding in &props.dynamic_bindings {
            if binding.dynamic_arg && binding.camel {
                push_unique_helper(helpers, RuntimeHelper::Vue3Camelize);
            }
        }
        for segment in &props.segments {
            if let Vue3DomPropSegment::DynamicBinding(binding) = segment {
                if binding.dynamic_arg && binding.camel {
                    push_unique_helper(helpers, RuntimeHelper::Vue3Camelize);
                }
            }
        }
        if props_requires_merge_call(props) {
            push_unique_helper(helpers, RuntimeHelper::Vue3MergeProps);
        }
        if !self.props_base_only_feed_dynamic_model_temp(props) {
            if props.normalize.guard_reactive_props {
                push_unique_helper(helpers, RuntimeHelper::Vue3GuardReactiveProps);
            }
            if props.normalize.normalize_props {
                push_unique_helper(helpers, RuntimeHelper::Vue3NormalizeProps);
            }
        }
    }

    fn push_v_show_merged_props_vue_helpers(
        &self,
        props: &Vue3DomProps,
        helpers: &mut Vec<RuntimeHelper>,
    ) {
        for binding in &props.dynamic_bindings {
            if binding.dynamic_arg && binding.camel {
                push_unique_helper(helpers, RuntimeHelper::Vue3Camelize);
            }
        }
        for segment in &props.segments {
            if let Vue3DomPropSegment::DynamicBinding(binding) = segment {
                if binding.dynamic_arg && binding.camel {
                    push_unique_helper(helpers, RuntimeHelper::Vue3Camelize);
                }
            }
        }
    }

    fn render_children(&self, parent: NodeId, scope: &RenderScope, writer: &mut CodeWriter) {
        let Some(node) = self.mir.node(parent) else {
            return;
        };
        self.render_child_slice(&node.children, scope, None, None, writer);
    }

    fn render_root_children(&self, scope: &RenderScope, writer: &mut CodeWriter) {
        let children = self.root_children();
        let effective_children = self.effective_root_children(children);
        let root_spans = self.root_spans(effective_children);
        let root_attrs = self.root_attrs_for_children(children);
        if effective_children.len() != children.len() {
            if let Some((html, next_index)) = self.render_ssr_template_literal_slice_with_prefix(
                effective_children,
                0,
                scope,
                None,
                "<template>",
                root_attrs.as_ref(),
            ) {
                if next_index == effective_children.len() {
                    writer.push_line(&format!(
                        "_push({})",
                        append_static_to_ssr_template_literal(html, "</template>")
                    ));
                    return;
                }
                writer.push_line(&format!("_push({html})"));
                let sliced_root_attrs =
                    self.root_attrs_for_child_slice(root_attrs.as_ref(), next_index);
                let close_appended = self.render_child_slice_with_final_suffix(
                    &effective_children[next_index..],
                    scope,
                    None,
                    sliced_root_attrs.as_ref(),
                    "</template>",
                    writer,
                );
                if !close_appended {
                    writer.push_line("_push(`</template>`)");
                }
                return;
            }
            writer.push_line("_push(`<template>`)");
            self.render_child_slice(effective_children, scope, None, root_attrs.as_ref(), writer);
            writer.push_line("_push(`</template>`)");
            return;
        }
        if self.root_children_have_explicit_fragment(children) {
            self.render_child_slice(children, scope, None, root_attrs.as_ref(), writer);
        } else if self.effective_root_children(children).len() == children.len()
            && root_spans.len() > 1
        {
            let mut index = 0usize;
            if let Some((html, next_index)) = self.render_ssr_template_literal_slice_with_prefix(
                children,
                0,
                scope,
                None,
                "<!--[-->",
                root_attrs.as_ref(),
            ) {
                if next_index == children.len() {
                    writer.push_line(&format!(
                        "_push({})",
                        append_static_to_ssr_template_literal(html, "<!--]-->")
                    ));
                    return;
                }
                writer.push_line(&format!("_push({html})"));
                index = next_index;
            } else {
                writer.push_line("_push(`<!--[-->`)");
            }
            let sliced_root_attrs = self.root_attrs_for_child_slice(root_attrs.as_ref(), index);
            let close_appended = self.render_child_slice_with_final_suffix(
                &children[index..],
                scope,
                None,
                sliced_root_attrs.as_ref(),
                "<!--]-->",
                writer,
            );
            if !close_appended {
                writer.push_line("_push(`<!--]-->`)");
            }
        } else {
            self.render_child_slice(children, scope, None, root_attrs.as_ref(), writer);
        }
    }

    fn root_children_have_explicit_fragment(&self, children: &[NodeId]) -> bool {
        children
            .first()
            .and_then(|id| self.ssr_push_string(*id))
            .is_some_and(|value| value == "<!--[-->")
            && children
                .last()
                .and_then(|id| self.ssr_push_string(*id))
                .is_some_and(|value| value == "<!--]-->")
    }

    fn render_child_slice(
        &self,
        children: &[NodeId],
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        let _ =
            self.render_child_slice_inner(children, scope, scope_id_expr, root_attrs, None, writer);
    }

    fn render_child_slice_with_final_suffix(
        &self,
        children: &[NodeId],
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
        final_suffix: &str,
        writer: &mut CodeWriter,
    ) -> bool {
        self.render_child_slice_inner(
            children,
            scope,
            scope_id_expr,
            root_attrs,
            Some(final_suffix),
            writer,
        )
    }

    fn render_child_slice_inner(
        &self,
        children: &[NodeId],
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
        final_suffix: Option<&str>,
        writer: &mut CodeWriter,
    ) -> bool {
        let mut index = 0usize;
        let mut appended_final_suffix = false;
        while index < children.len() {
            let current_root_attrs = root_attrs
                .and_then(|attrs| self.root_attrs_for_render_index(children, index, attrs));
            if let Some((html, next_index)) = self.render_ssr_template_literal_slice(
                children,
                index,
                scope,
                scope_id_expr,
                current_root_attrs.as_ref(),
            ) {
                let html = if next_index == children.len() {
                    if let Some(final_suffix) = final_suffix {
                        appended_final_suffix = true;
                        append_static_to_ssr_template_literal(html, final_suffix)
                    } else {
                        html
                    }
                } else {
                    html
                };
                writer.push_line(&format!("_push({html})"));
                index = next_index;
                continue;
            }
            if let Some(next_index) = self.render_ssr_static_shell_with_dynamic_descendant(
                children,
                index,
                scope,
                scope_id_expr,
                current_root_attrs.as_ref(),
                writer,
            ) {
                index = next_index;
                continue;
            }
            self.render_node(children[index], scope, current_root_attrs.as_ref(), writer);
            index += 1;
        }
        appended_final_suffix
    }

    fn render_ssr_static_shell_with_dynamic_descendant(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<usize> {
        if let Some(next_index) = self.render_ssr_static_shell_with_single_dynamic_child(
            children,
            index,
            scope,
            scope_id_expr,
            root_attrs,
            writer,
        ) {
            return Some(next_index);
        }
        self.render_ssr_static_shell_around_nested_dynamic(
            children, index, scope, root_attrs, writer,
        )
    }

    fn render_ssr_static_shell_with_single_dynamic_child(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<usize> {
        let open_start = self.ssr_push_string(*children.get(index)?)?;
        let (open_tag, static_entries) = parse_ssr_open_tag_start(open_start)?;
        let mut open_end = index + 1;
        let attrs = children
            .get(open_end)
            .and_then(|id| self.mir.node(*id))
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            });
        if attrs.is_some() {
            open_end += 1;
        }
        if self.ssr_push_string(*children.get(open_end)?)? != ">" {
            return None;
        }
        let (_, end_index) = self.collect_ssr_static_element_span(children, index)?;
        let close_index = end_index.checked_sub(1)?;
        let attrs = children
            .get(index + 1)
            .and_then(|id| self.mir.node(*id))
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            });
        let forced_attrs = attrs.is_some_and(|attrs| attrs.force_render_attrs);
        let rebuilt_attrs = attrs.is_some_and(|attrs| {
            attrs.v_show.is_none()
                && attrs.v_model.is_none()
                && self.ssr_attrs_use_rebuilt_element_attrs(attrs)
        });
        let [dynamic_id] = children.get(open_end + 1..close_index)? else {
            return None;
        };
        if !matches!(
            self.mir.node(*dynamic_id).map(|node| &node.kind),
            Some(Vue3SsrMirKind::For(_))
                | Some(Vue3SsrMirKind::If { .. })
                | Some(Vue3SsrMirKind::RenderSlot(_))
        ) {
            return None;
        }
        let open_static = if root_attrs.is_some() {
            format!("<{open_tag}")
        } else {
            open_start.to_string()
        };
        let mut open_parts = Vec::new();
        if let Some(expression) = self.dynamic_tag_name_expr(&open_tag, scope) {
            open_parts.push(SsrTemplatePart::Static("<".into()));
            open_parts.push(SsrTemplatePart::Expr(expression));
        } else if forced_attrs || rebuilt_attrs {
            open_parts.push(SsrTemplatePart::Static(format!("<{open_tag}")));
        } else {
            open_parts.push(SsrTemplatePart::Static(open_static));
        }
        let dynamic_open_tag = self.dynamic_tag_name_expr(&open_tag, scope).is_some();
        if let Some(attrs) = attrs {
            let root_attrs = root_attrs
                .filter(|root_attrs| root_attrs.attrs.is_some() || root_attrs.css_vars.is_some());
            if let Some(root_attrs) = root_attrs {
                if matches!(
                    attrs.v_model.as_ref().map(|model| &model.kind),
                    Some(Vue3SsrModelKind::InputDynamicProps)
                ) {
                    return None;
                }
                if attrs.v_show.is_some() {
                    open_parts.push(SsrTemplatePart::Expr(
                        self.render_root_element_v_show_attrs_expr_with_static(
                            attrs,
                            root_attrs,
                            scope,
                            &static_entries,
                        ),
                    ));
                } else {
                    open_parts.push(SsrTemplatePart::Expr(
                        self.render_root_element_attrs_expr_with_static(
                            attrs,
                            root_attrs,
                            scope,
                            &open_tag,
                            &static_entries,
                        ),
                    ));
                }
            } else {
                self.collect_ssr_template_attrs_for_open_tag(
                    attrs,
                    scope,
                    &open_tag,
                    dynamic_open_tag,
                    &static_entries,
                    &mut open_parts,
                )?;
            }
        } else if let Some(root_attrs) = root_attrs {
            let rendered = self.render_root_attrs_expr_with_static(root_attrs, &static_entries);
            if !rendered.is_empty() {
                open_parts.push(SsrTemplatePart::Expr(rendered));
            }
        }
        if root_attrs.is_some() || dynamic_open_tag || forced_attrs || rebuilt_attrs {
            let tail =
                self.render_static_attr_tail(&self.root_static_tail_entries(&static_entries));
            if !tail.is_empty() {
                open_parts.push(SsrTemplatePart::Static(tail));
            }
        }
        if let Some(scope_id_expr) = scope_id_expr {
            open_parts.push(SsrTemplatePart::Expr(scope_id_expr.to_string()));
        }
        let close = self.ssr_push_string(*children.get(close_index)?)?;
        open_parts.push(SsrTemplatePart::Static(">".into()));
        if matches!(
            self.mir.node(*dynamic_id).map(|node| &node.kind),
            Some(Vue3SsrMirKind::For(for_mir)) if for_mir.fragment
        ) {
            open_parts.push(SsrTemplatePart::Static("<!--[-->".into()));
        }
        let open = render_ssr_template_literal(&open_parts);
        writer.push_line(&format!("_push({open})"));
        match self.mir.node(*dynamic_id).map(|node| &node.kind)? {
            Vue3SsrMirKind::For(for_mir) => {
                self.render_for_list(*dynamic_id, for_mir, scope, writer)
            }
            Vue3SsrMirKind::If { condition, comment } => {
                self.render_if(*dynamic_id, *condition, *comment, scope, None, writer)
            }
            Vue3SsrMirKind::RenderSlot(slot) => self.render_slot(slot, scope, writer),
            _ => return None,
        }
        let close_prefix = if matches!(
            self.mir.node(*dynamic_id).map(|node| &node.kind),
            Some(Vue3SsrMirKind::For(for_mir)) if for_mir.fragment
        ) {
            "<!--]-->"
        } else {
            ""
        };
        let close = if let Some(expression) = self.dynamic_tag_name_expr(&open_tag, scope) {
            let close_parts = vec![
                SsrTemplatePart::Static(close_prefix.to_string()),
                SsrTemplatePart::Static("</".into()),
                SsrTemplatePart::Expr(expression),
                SsrTemplatePart::Static(">".into()),
            ];
            render_ssr_template_literal(&close_parts)
        } else {
            render_ssr_template_literal(&[SsrTemplatePart::Static(format!(
                "{close_prefix}{close}"
            ))])
        };
        writer.push_line(&format!("_push({close})"));
        Some(end_index)
    }

    fn render_ssr_static_shell_around_nested_dynamic(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<usize> {
        let (_, end_index) = self.collect_ssr_static_element_span(children, index)?;
        let close_index = end_index.checked_sub(1)?;
        let dynamic_count = children
            .get(index + 1..close_index)
            .unwrap_or(&[])
            .iter()
            .filter(|node_id| {
                matches!(
                    self.mir.node(**node_id).map(|node| &node.kind),
                    Some(Vue3SsrMirKind::For(_))
                        | Some(Vue3SsrMirKind::If { .. })
                        | Some(Vue3SsrMirKind::RenderSlot(_))
                )
            })
            .count();
        if dynamic_count > 1 {
            return self.render_ssr_shell_with_dynamic_children(
                children, index, end_index, scope, root_attrs, writer,
            );
        }
        let (dynamic_index, dynamic_id) =
            self.find_dynamic_descendant_in_static_shell(children.get(index..close_index)?, index)?;
        let mut prefix_parts = Vec::new();
        let prefix_end = self.collect_ssr_linear_template_parts(
            children,
            index,
            dynamic_index,
            scope,
            root_attrs,
            &mut prefix_parts,
        )?;
        if prefix_end != dynamic_index || prefix_parts.is_empty() {
            return None;
        }
        if matches!(
            self.mir.node(dynamic_id).map(|node| &node.kind),
            Some(Vue3SsrMirKind::For(for_mir)) if for_mir.fragment
        ) {
            prefix_parts.push(SsrTemplatePart::Static("<!--[-->".into()));
        }
        writer.push_line(&format!(
            "_push({})",
            render_ssr_template_literal(&prefix_parts)
        ));
        match self.mir.node(dynamic_id).map(|node| &node.kind)? {
            Vue3SsrMirKind::For(for_mir) => {
                self.render_for_list_with_optional_fragment(dynamic_id, for_mir, scope, writer)
            }
            Vue3SsrMirKind::If { condition, comment } => {
                self.render_if(dynamic_id, *condition, *comment, scope, None, writer)
            }
            Vue3SsrMirKind::RenderSlot(slot) => self.render_slot(slot, scope, writer),
            _ => return None,
        }
        let mut suffix_parts = Vec::new();
        if matches!(
            self.mir.node(dynamic_id).map(|node| &node.kind),
            Some(Vue3SsrMirKind::For(for_mir)) if for_mir.fragment
        ) {
            suffix_parts.push(SsrTemplatePart::Static("<!--]-->".into()));
        }
        let suffix_start = dynamic_index + 1;
        if suffix_start < end_index {
            let suffix_end = self.collect_ssr_linear_template_parts(
                children,
                suffix_start,
                end_index,
                scope,
                None,
                &mut suffix_parts,
            )?;
            if suffix_end != end_index {
                return None;
            }
        }
        writer.push_line(&format!(
            "_push({})",
            render_ssr_template_literal(&suffix_parts)
        ));
        Some(end_index)
    }

    fn render_ssr_shell_with_dynamic_children(
        &self,
        children: &[NodeId],
        index: usize,
        end_index: usize,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<usize> {
        let open_end = self.render_ssr_shell_open(children, index, scope, root_attrs, writer)?;
        let close_index = end_index.checked_sub(1)?;
        let mut cursor = open_end;
        while cursor < close_index {
            self.render_node(children[cursor], scope, None, writer);
            cursor += 1;
        }
        let close = self.ssr_push_string(*children.get(close_index)?)?;
        writer.push_line(&format!(
            "_push({})",
            render_ssr_template_literal(&[SsrTemplatePart::Static(close.to_string())])
        ));
        Some(end_index)
    }

    fn render_ssr_shell_open(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<usize> {
        let open_start = self.ssr_push_string(*children.get(index)?)?;
        let (open_tag, static_entries) = parse_ssr_open_tag_start(open_start)?;
        let attrs = children
            .get(index + 1)
            .and_then(|id| self.mir.node(*id))
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            });
        let forced_attrs = attrs.is_some_and(|attrs| attrs.force_render_attrs);
        let rebuilt_attrs = attrs.is_some_and(|attrs| {
            attrs.v_show.is_none()
                && attrs.v_model.is_none()
                && self.ssr_attrs_use_rebuilt_element_attrs(attrs)
        });
        let mut parts = Vec::new();
        if let Some(expression) = self.dynamic_tag_name_expr(&open_tag, scope) {
            parts.push(SsrTemplatePart::Static("<".into()));
            parts.push(SsrTemplatePart::Expr(expression));
        } else if root_attrs.is_some() || forced_attrs || rebuilt_attrs {
            parts.push(SsrTemplatePart::Static(format!("<{open_tag}")));
        } else {
            parts.push(SsrTemplatePart::Static(open_start.to_string()));
        }
        let mut cursor = index + 1;
        let dynamic_open_tag = self.dynamic_tag_name_expr(&open_tag, scope).is_some();
        if let Some(attrs) = attrs {
            if let Some(root_attrs) = root_attrs {
                if attrs.v_show.is_some() {
                    parts.push(SsrTemplatePart::Expr(
                        self.render_root_element_v_show_attrs_expr_with_static(
                            attrs,
                            root_attrs,
                            scope,
                            &static_entries,
                        ),
                    ));
                } else {
                    parts.push(SsrTemplatePart::Expr(
                        self.render_root_element_attrs_expr_with_static(
                            attrs,
                            root_attrs,
                            scope,
                            &open_tag,
                            &static_entries,
                        ),
                    ));
                }
            } else {
                self.collect_ssr_template_attrs_for_open_tag(
                    attrs,
                    scope,
                    &open_tag,
                    dynamic_open_tag,
                    &static_entries,
                    &mut parts,
                )?;
            }
            cursor += 1;
        } else if let Some(root_attrs) = root_attrs {
            let rendered = self.render_root_attrs_expr_with_static(root_attrs, &static_entries);
            if !rendered.is_empty() {
                parts.push(SsrTemplatePart::Expr(rendered));
            }
        }
        if root_attrs.is_some() || dynamic_open_tag || forced_attrs || rebuilt_attrs {
            let tail =
                self.render_static_attr_tail(&self.root_static_tail_entries(&static_entries));
            if !tail.is_empty() {
                parts.push(SsrTemplatePart::Static(tail));
            }
        }
        if self.ssr_push_string(*children.get(cursor)?)? != ">" {
            return None;
        }
        parts.push(SsrTemplatePart::Static(">".into()));
        writer.push_line(&format!("_push({})", render_ssr_template_literal(&parts)));
        Some(cursor + 1)
    }

    fn find_dynamic_descendant_in_static_shell(
        &self,
        slice: &[NodeId],
        base_index: usize,
    ) -> Option<(usize, NodeId)> {
        let mut found = None;
        for (offset, node_id) in slice.iter().copied().enumerate() {
            if matches!(
                self.mir.node(node_id).map(|node| &node.kind),
                Some(Vue3SsrMirKind::For(_))
                    | Some(Vue3SsrMirKind::If { .. })
                    | Some(Vue3SsrMirKind::RenderSlot(_))
            ) {
                if found.is_some() {
                    return None;
                }
                found = Some((base_index + offset, node_id));
            }
        }
        found
    }

    fn collect_ssr_linear_template_parts(
        &self,
        children: &[NodeId],
        index: usize,
        end_index: usize,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        parts: &mut Vec<SsrTemplatePart>,
    ) -> Option<usize> {
        let mut cursor = index;
        while cursor < end_index {
            match self.mir.node(*children.get(cursor)?)?.kind.clone() {
                Vue3SsrMirKind::PushString(value) => {
                    if let Some((tag, static_entries)) = parse_ssr_open_tag_start(&value) {
                        if let Some(root_attrs) = root_attrs.filter(|_| cursor == index) {
                            parts.push(SsrTemplatePart::Static(format!("<{tag}")));
                            if let Some(attrs) = children
                                .get(cursor + 1)
                                .and_then(|id| self.mir.node(*id))
                                .and_then(|node| match &node.kind {
                                    Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                                    _ => None,
                                })
                            {
                                if attrs.v_show.is_some() {
                                    parts.push(SsrTemplatePart::Expr(
                                        self.render_root_element_v_show_attrs_expr_with_static(
                                            attrs,
                                            root_attrs,
                                            scope,
                                            &static_entries,
                                        ),
                                    ));
                                } else {
                                    parts.push(SsrTemplatePart::Expr(
                                        self.render_root_element_attrs_expr_with_static(
                                            attrs,
                                            root_attrs,
                                            scope,
                                            &tag,
                                            &static_entries,
                                        ),
                                    ));
                                }
                                cursor += 1;
                            } else {
                                let rendered = self.render_root_attrs_expr_with_static(
                                    root_attrs,
                                    &static_entries,
                                );
                                if !rendered.is_empty() {
                                    parts.push(SsrTemplatePart::Expr(rendered));
                                }
                            }
                            let root_tail_entries = self.root_static_tail_entries(&static_entries);
                            let tail = self.render_static_attr_tail(&root_tail_entries);
                            if !tail.is_empty() {
                                parts.push(SsrTemplatePart::Static(tail));
                            }
                        } else {
                            parts.push(SsrTemplatePart::Static(value));
                        }
                    } else if value == "/>" {
                        parts.push(SsrTemplatePart::Static(">".into()));
                    } else {
                        parts.push(SsrTemplatePart::Static(value));
                    }
                }
                Vue3SsrMirKind::RenderAttrs(attrs) => {
                    self.collect_ssr_template_attrs(&attrs, scope, parts)?;
                }
                Vue3SsrMirKind::PushInterpolated(expr) => {
                    parts.push(SsrTemplatePart::Expr(format!(
                        "_ssrInterpolate({})",
                        self.render_mir_expr(&expr, scope)
                    )));
                }
                Vue3SsrMirKind::RenderContent(Vue3SsrContent::Html { expression }) => {
                    parts.push(SsrTemplatePart::Expr(format!(
                        "({}) ?? ''",
                        self.render_js_expr(expression, scope)
                    )));
                }
                Vue3SsrMirKind::RenderContent(Vue3SsrContent::Text { expression }) => {
                    parts.push(SsrTemplatePart::Expr(format!(
                        "_ssrInterpolate({})",
                        self.render_js_expr(expression, scope)
                    )));
                }
                _ => return None,
            }
            cursor += 1;
        }
        Some(cursor)
    }

    fn render_ssr_template_literal_slice(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
    ) -> Option<(String, usize)> {
        self.render_ssr_template_literal_slice_with_prefix(
            children,
            index,
            scope,
            scope_id_expr,
            "",
            root_attrs,
        )
    }

    fn render_ssr_template_literal_slice_with_prefix(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        prefix: &str,
        root_attrs: Option<&SsrRootAttrs>,
    ) -> Option<(String, usize)> {
        let mut parts = Vec::new();
        if !prefix.is_empty() {
            parts.push(SsrTemplatePart::Static(prefix.to_string()));
        }
        let mut dynamic = !prefix.is_empty() || scope_id_expr.is_some();
        let current_root_attrs =
            root_attrs.and_then(|attrs| self.root_attrs_for_render_index(children, index, attrs));
        let next_index = self.collect_ssr_template_node(
            children,
            index,
            scope,
            scope_id_expr,
            &mut parts,
            &mut dynamic,
            current_root_attrs.as_ref(),
        )?;
        let mut next_index = next_index;
        while next_index < children.len() {
            let continuation_root_attrs = root_attrs
                .and_then(|attrs| self.root_attrs_for_render_index(children, next_index, attrs));
            let Some(next) = self.collect_ssr_template_node(
                children,
                next_index,
                scope,
                scope_id_expr,
                &mut parts,
                &mut dynamic,
                continuation_root_attrs.as_ref(),
            ) else {
                break;
            };
            next_index = next;
        }
        let has_content = dynamic
            || parts.iter().any(|part| match part {
                SsrTemplatePart::Static(value) => !value.is_empty(),
                SsrTemplatePart::Expr(_) => true,
            });
        if !has_content {
            return None;
        }
        Some((render_ssr_template_literal(&parts), next_index))
    }

    fn collect_ssr_template_node(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        parts: &mut Vec<SsrTemplatePart>,
        dynamic: &mut bool,
        root_attrs: Option<&SsrRootAttrs>,
    ) -> Option<usize> {
        let node = self.mir.node(*children.get(index)?)?;
        match &node.kind {
            Vue3SsrMirKind::PushString(value) if parse_ssr_open_tag_start(value).is_some() => self
                .collect_ssr_template_element(
                    children,
                    index,
                    scope,
                    scope_id_expr,
                    parts,
                    dynamic,
                    root_attrs,
                ),
            Vue3SsrMirKind::PushString(value)
                if value != ">" && value != "/>" && !value.starts_with("</") =>
            {
                parts.push(SsrTemplatePart::Static(value.clone()));
                Some(index + 1)
            }
            Vue3SsrMirKind::PushInterpolated(expr) => {
                parts.push(SsrTemplatePart::Expr(format!(
                    "_ssrInterpolate({})",
                    self.render_mir_expr(expr, scope)
                )));
                *dynamic = true;
                Some(index + 1)
            }
            Vue3SsrMirKind::RenderContent(Vue3SsrContent::Html { expression }) => {
                parts.push(SsrTemplatePart::Expr(format!(
                    "({}) ?? ''",
                    self.render_js_expr(*expression, scope)
                )));
                *dynamic = true;
                Some(index + 1)
            }
            Vue3SsrMirKind::RenderContent(Vue3SsrContent::Text { expression }) => {
                parts.push(SsrTemplatePart::Expr(format!(
                    "_ssrInterpolate({})",
                    self.render_js_expr(*expression, scope)
                )));
                *dynamic = true;
                Some(index + 1)
            }
            _ => None,
        }
    }

    fn collect_ssr_template_element(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        parts: &mut Vec<SsrTemplatePart>,
        dynamic: &mut bool,
        root_attrs: Option<&SsrRootAttrs>,
    ) -> Option<usize> {
        let (tag, static_entries) =
            self.mir
                .node(*children.get(index)?)
                .and_then(|node| match &node.kind {
                    Vue3SsrMirKind::PushString(value) => parse_ssr_open_tag_start(value),
                    _ => None,
                })?;
        let attrs = children
            .get(index + 1)
            .and_then(|id| self.mir.node(*id))
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            });
        let forced_attrs = attrs.is_some_and(|attrs| attrs.force_render_attrs);
        let rebuilt_attrs = attrs.is_some_and(|attrs| {
            attrs.v_show.is_none()
                && attrs.v_model.is_none()
                && self.ssr_attrs_use_rebuilt_element_attrs(attrs)
        });
        if let Some(expression) = self.dynamic_tag_name_expr(&tag, scope) {
            parts.push(SsrTemplatePart::Static("<".into()));
            parts.push(SsrTemplatePart::Expr(expression));
            *dynamic = true;
        } else if root_attrs.is_some() || forced_attrs || rebuilt_attrs {
            parts.push(SsrTemplatePart::Static(format!("<{tag}")));
        } else {
            self.push_ssr_open_tag_start_part(
                self.ssr_push_string(children[index])?,
                scope,
                parts,
                dynamic,
            );
        }
        let mut cursor = index + 1;
        let dynamic_open_tag = self.dynamic_tag_name_expr(&tag, scope).is_some();
        if let Some(attrs) = attrs {
            if let Some(root_attrs) = root_attrs {
                if matches!(
                    attrs.v_model.as_ref().map(|model| &model.kind),
                    Some(Vue3SsrModelKind::InputDynamicProps)
                ) {
                    return None;
                }
                if attrs.v_show.is_some() {
                    parts.push(SsrTemplatePart::Expr(
                        self.render_root_element_v_show_attrs_expr_with_static(
                            attrs,
                            root_attrs,
                            scope,
                            &static_entries,
                        ),
                    ));
                } else {
                    parts.push(SsrTemplatePart::Expr(
                        self.render_root_element_attrs_expr_with_static(
                            attrs,
                            root_attrs,
                            scope,
                            &tag,
                            &static_entries,
                        ),
                    ));
                }
            } else {
                self.collect_ssr_template_attrs_for_open_tag(
                    attrs,
                    scope,
                    &tag,
                    dynamic_open_tag,
                    &static_entries,
                    parts,
                )?;
            }
            *dynamic = true;
            cursor += 1;
        } else if let Some(root_attrs) = root_attrs {
            let rendered = self.render_root_attrs_expr_with_static(root_attrs, &static_entries);
            if !rendered.is_empty() {
                parts.push(SsrTemplatePart::Expr(rendered));
                *dynamic = true;
            }
        }
        if root_attrs.is_some() || dynamic_open_tag || forced_attrs || rebuilt_attrs {
            let root_tail_entries = self.root_static_tail_entries(&static_entries);
            let tail = self.render_static_attr_tail(&root_tail_entries);
            if !tail.is_empty() {
                parts.push(SsrTemplatePart::Static(tail));
            }
        }
        if let Some(scope_id_expr) = scope_id_expr {
            parts.push(SsrTemplatePart::Expr(scope_id_expr.to_string()));
            *dynamic = true;
        }
        let close_open = self.ssr_push_string(*children.get(cursor)?)?;
        if close_open == "/>" {
            parts.push(SsrTemplatePart::Static(">".into()));
            if let Some(attrs) = attrs.filter(|attrs| attrs.directive_content) {
                let _ = attrs;
                parts.push(SsrTemplatePart::Expr(
                    self.render_ssr_directive_content_expr(),
                ));
                *dynamic = true;
            } else if let Some(fallback) =
                attrs.and_then(|attrs| attrs.textarea_value_fallback.as_deref())
            {
                parts.push(SsrTemplatePart::Expr(
                    self.render_ssr_textarea_value_content_expr(fallback),
                ));
                *dynamic = true;
            }
            return Some(cursor + 1);
        }
        if close_open != ">" {
            return None;
        }
        parts.push(SsrTemplatePart::Static(">".into()));
        if let Some(attrs) = attrs.filter(|attrs| attrs.directive_content) {
            let _ = attrs;
            parts.push(SsrTemplatePart::Expr(
                self.render_ssr_directive_content_expr(),
            ));
            *dynamic = true;
        } else if let Some(fallback) =
            attrs.and_then(|attrs| attrs.textarea_value_fallback.as_deref())
        {
            parts.push(SsrTemplatePart::Expr(
                self.render_ssr_textarea_value_content_expr(fallback),
            ));
            *dynamic = true;
        }
        cursor += 1;
        while cursor < children.len() {
            if self
                .ssr_push_string(children[cursor])
                .is_some_and(|value| value == format!("</{tag}>"))
            {
                if let Some(expression) = self.dynamic_tag_name_expr(&tag, scope) {
                    parts.push(SsrTemplatePart::Static("</".into()));
                    parts.push(SsrTemplatePart::Expr(expression));
                    parts.push(SsrTemplatePart::Static(">".into()));
                    *dynamic = true;
                } else {
                    self.push_ssr_close_tag_part(
                        self.ssr_push_string(children[cursor])?,
                        scope,
                        parts,
                        dynamic,
                    );
                }
                return Some(cursor + 1);
            }
            if self
                .mir
                .node(children[cursor])
                .is_some_and(|node| matches!(node.kind, Vue3SsrMirKind::PushString(_)))
            {
                if parse_ssr_open_tag_start(self.ssr_push_string(children[cursor])?).is_some() {
                    cursor = self.collect_ssr_template_node(
                        children,
                        cursor,
                        scope,
                        scope_id_expr,
                        parts,
                        dynamic,
                        None,
                    )?;
                    continue;
                }
            }
            match self.mir.node(children[cursor]).map(|node| &node.kind) {
                Some(Vue3SsrMirKind::PushString(value)) => {
                    parts.push(SsrTemplatePart::Static(value.clone()));
                }
                Some(Vue3SsrMirKind::PushInterpolated(expr)) => {
                    parts.push(SsrTemplatePart::Expr(format!(
                        "_ssrInterpolate({})",
                        self.render_mir_expr(expr, scope)
                    )));
                    *dynamic = true;
                }
                Some(Vue3SsrMirKind::RenderContent(Vue3SsrContent::Html { expression })) => {
                    parts.push(SsrTemplatePart::Expr(format!(
                        "({}) ?? ''",
                        self.render_js_expr(*expression, scope)
                    )));
                    *dynamic = true;
                }
                Some(Vue3SsrMirKind::RenderContent(Vue3SsrContent::Text { expression })) => {
                    parts.push(SsrTemplatePart::Expr(format!(
                        "_ssrInterpolate({})",
                        self.render_js_expr(*expression, scope)
                    )));
                    *dynamic = true;
                }
                _ => return None,
            }
            cursor += 1;
        }
        None
    }

    fn push_ssr_open_tag_start_part(
        &self,
        raw: &str,
        scope: &RenderScope,
        parts: &mut Vec<SsrTemplatePart>,
        dynamic: &mut bool,
    ) {
        if let Some(expression) = self.dynamic_tag_placeholder_expr(raw, "<#expr", scope) {
            parts.push(SsrTemplatePart::Static("<".into()));
            parts.push(SsrTemplatePart::Expr(expression));
            *dynamic = true;
        } else {
            parts.push(SsrTemplatePart::Static(raw.to_string()));
        }
    }

    fn push_ssr_close_tag_part(
        &self,
        raw: &str,
        scope: &RenderScope,
        parts: &mut Vec<SsrTemplatePart>,
        dynamic: &mut bool,
    ) {
        if let Some(expression) = self.dynamic_tag_placeholder_expr(raw, "</#expr", scope) {
            parts.push(SsrTemplatePart::Static("</".into()));
            parts.push(SsrTemplatePart::Expr(expression));
            parts.push(SsrTemplatePart::Static(">".into()));
            *dynamic = true;
        } else {
            parts.push(SsrTemplatePart::Static(raw.to_string()));
        }
    }

    fn dynamic_tag_placeholder_expr(
        &self,
        raw: &str,
        prefix: &str,
        scope: &RenderScope,
    ) -> Option<String> {
        let id = raw.strip_prefix(prefix)?;
        let id = id.strip_prefix("expr").unwrap_or(id);
        let id_end = id
            .char_indices()
            .find_map(|(index, ch)| (is_vue3_html_whitespace(ch) || ch == '>').then_some(index))
            .unwrap_or(id.len());
        let id = id.get(..id_end)?.parse::<u32>().ok()?;
        Some(self.render_js_expr(JsExprId(id), scope))
    }

    fn dynamic_tag_name_expr(&self, tag: &str, scope: &RenderScope) -> Option<String> {
        let id = tag.strip_prefix("#expr")?.parse::<u32>().ok()?;
        Some(self.render_js_expr(JsExprId(id), scope))
    }

    fn collect_ssr_template_attrs(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        parts: &mut Vec<SsrTemplatePart>,
    ) -> Option<()> {
        if attrs.force_render_attrs {
            parts.push(SsrTemplatePart::Expr(
                self.render_forced_ssr_attrs_expr(attrs, scope, None)?,
            ));
            return Some(());
        }
        if attrs.v_model.is_some() {
            return self.collect_ssr_template_attrs_with_v_model(attrs, scope, parts);
        }
        if attrs.v_show.is_some() {
            if ssr_attrs_has_object_binding(&attrs.props) {
                let style =
                    self.render_v_show_style(&attrs.props, attrs.v_show.expect("v-show"), scope);
                let attrs_expr = self.render_v_show_merged_attrs(&attrs.props, &style, scope, None);
                parts.push(SsrTemplatePart::Expr(format!(
                    "_ssrRenderAttrs({attrs_expr})"
                )));
                return Some(());
            }
            self.collect_ssr_template_attrs_without_v_show_style(attrs, scope, parts);
            let style =
                self.render_v_show_style(&attrs.props, attrs.v_show.expect("v-show"), scope);
            parts.push(SsrTemplatePart::Static(" style=\"".into()));
            parts.push(SsrTemplatePart::Expr(format!("_ssrRenderStyle({style})")));
            parts.push(SsrTemplatePart::Static("\"".into()));
            return Some(());
        }
        let props = &attrs.props;
        if props.segments.is_empty() {
            for binding in &props.dynamic_bindings {
                parts.push(SsrTemplatePart::Expr(
                    self.render_ssr_binding(binding, scope),
                ));
            }
            for binding in &props.object_bindings {
                parts.push(SsrTemplatePart::Expr(format!(
                    "_ssrRenderAttrs({})",
                    self.render_js_expr(binding.value, scope)
                )));
            }
            return Some(());
        }
        for segment in &props.segments {
            match segment {
                Vue3DomPropSegment::StaticAttr(attr) => {
                    self.collect_ssr_template_static_attr_for_v_model(attr, parts);
                }
                Vue3DomPropSegment::DynamicBinding(binding) => {
                    parts.push(SsrTemplatePart::Expr(
                        self.render_ssr_binding(binding, scope),
                    ));
                }
                Vue3DomPropSegment::ObjectBinding(binding) => {
                    parts.push(SsrTemplatePart::Expr(format!(
                        "_ssrRenderAttrs({})",
                        self.render_js_expr(binding.value, scope)
                    )));
                }
                Vue3DomPropSegment::Content(_)
                | Vue3DomPropSegment::Model(_)
                | Vue3DomPropSegment::Event(_)
                | Vue3DomPropSegment::ObjectListeners(_) => {}
            }
        }
        Some(())
    }

    fn collect_ssr_template_attrs_for_open_tag(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        tag: &str,
        dynamic_tag: bool,
        static_entries: &[(String, Option<String>)],
        parts: &mut Vec<SsrTemplatePart>,
    ) -> Option<()> {
        if attrs.force_render_attrs {
            let static_props = dynamic_tag
                .then(|| self.root_static_props(static_entries))
                .flatten();
            parts.push(SsrTemplatePart::Expr(self.render_forced_ssr_attrs_expr(
                attrs,
                scope,
                static_props,
            )?));
            return Some(());
        }
        if attrs.v_show.is_none()
            && attrs.v_model.is_none()
            && self.ssr_attrs_use_rebuilt_element_attrs(attrs)
        {
            if self.ssr_attrs_need_render_attrs(attrs) {
                parts.push(SsrTemplatePart::Expr(self.render_ssr_render_attrs_call(
                    attrs,
                    scope,
                    None,
                    Some(tag),
                )?));
            } else {
                self.collect_ssr_template_ordered_attrs(attrs, scope, parts);
            }
            return Some(());
        }
        self.collect_ssr_template_attrs(attrs, scope, parts)
    }

    fn ssr_attrs_use_rebuilt_element_attrs(&self, attrs: &Vue3SsrAttrs) -> bool {
        !attrs.props.static_attrs.is_empty()
            || !attrs.directives.is_empty()
            || attrs.props.dynamic_bindings.iter().any(|binding| {
                binding.dynamic_arg
                    || matches!(binding.name.as_str(), "class" | "style")
                    || vue3_ssr_is_boolean_attr(&binding.name)
            })
            || !attrs.props.object_bindings.is_empty()
    }

    fn collect_ssr_template_ordered_attrs(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        parts: &mut Vec<SsrTemplatePart>,
    ) {
        let class_expr = self.render_ssr_class_expr(&attrs.props, scope, true);
        let style_expr = self.render_ssr_style_expr(&attrs.props, scope, true);
        let mut emitted_class = false;
        let mut emitted_style = false;
        if attrs.props.segments.is_empty() {
            for attr in &attrs.props.static_attrs {
                self.collect_ssr_template_static_or_merged_attr(
                    attr,
                    class_expr.as_deref(),
                    style_expr.as_deref(),
                    &mut emitted_class,
                    &mut emitted_style,
                    parts,
                );
            }
            for binding in &attrs.props.dynamic_bindings {
                self.collect_ssr_template_dynamic_or_merged_attr(
                    binding,
                    scope,
                    class_expr.as_deref(),
                    style_expr.as_deref(),
                    &mut emitted_class,
                    &mut emitted_style,
                    parts,
                );
            }
            return;
        }
        for segment in &attrs.props.segments {
            match segment {
                Vue3DomPropSegment::StaticAttr(attr) => {
                    self.collect_ssr_template_static_or_merged_attr(
                        attr,
                        class_expr.as_deref(),
                        style_expr.as_deref(),
                        &mut emitted_class,
                        &mut emitted_style,
                        parts,
                    );
                }
                Vue3DomPropSegment::DynamicBinding(binding) => {
                    self.collect_ssr_template_dynamic_or_merged_attr(
                        binding,
                        scope,
                        class_expr.as_deref(),
                        style_expr.as_deref(),
                        &mut emitted_class,
                        &mut emitted_style,
                        parts,
                    );
                }
                Vue3DomPropSegment::Content(_)
                | Vue3DomPropSegment::Model(_)
                | Vue3DomPropSegment::Event(_)
                | Vue3DomPropSegment::ObjectBinding(_)
                | Vue3DomPropSegment::ObjectListeners(_) => {}
            }
        }
    }

    fn collect_ssr_template_static_or_merged_attr(
        &self,
        attr: &Vue3DomStaticAttr,
        class_expr: Option<&str>,
        style_expr: Option<&str>,
        emitted_class: &mut bool,
        emitted_style: &mut bool,
        parts: &mut Vec<SsrTemplatePart>,
    ) {
        match attr.name.as_str() {
            "class" => {
                if !*emitted_class {
                    if let Some(class_expr) = class_expr {
                        parts.push(SsrTemplatePart::Static(" class=\"".into()));
                        parts.push(SsrTemplatePart::Expr(format!(
                            "_ssrRenderClass({class_expr})"
                        )));
                        parts.push(SsrTemplatePart::Static("\"".into()));
                    } else {
                        parts.push(SsrTemplatePart::Static(format!(
                            " class=\"{}\"",
                            vue3_ssr_escape_attr(&attr.value)
                        )));
                    }
                    *emitted_class = true;
                }
            }
            "style" => {
                if !*emitted_style {
                    if let Some(style_expr) = style_expr {
                        parts.push(SsrTemplatePart::Static(" style=\"".into()));
                        parts.push(SsrTemplatePart::Expr(format!(
                            "_ssrRenderStyle({style_expr})"
                        )));
                        parts.push(SsrTemplatePart::Static("\"".into()));
                    } else {
                        parts.push(SsrTemplatePart::Static(format!(
                            " style=\"{}\"",
                            vue3_ssr_escape_attr(&attr.value)
                        )));
                    }
                    *emitted_style = true;
                }
            }
            _ => parts.push(SsrTemplatePart::Static(format!(
                " {}=\"{}\"",
                attr.name,
                vue3_ssr_escape_attr(&attr.value)
            ))),
        }
    }

    fn collect_ssr_template_dynamic_or_merged_attr(
        &self,
        binding: &Vue3DomBinding,
        scope: &RenderScope,
        class_expr: Option<&str>,
        style_expr: Option<&str>,
        emitted_class: &mut bool,
        emitted_style: &mut bool,
        parts: &mut Vec<SsrTemplatePart>,
    ) {
        match binding.name.as_str() {
            "class" if !binding.dynamic_arg => {
                if !*emitted_class {
                    if let Some(class_expr) = class_expr {
                        parts.push(SsrTemplatePart::Static(" class=\"".into()));
                        parts.push(SsrTemplatePart::Expr(format!(
                            "_ssrRenderClass({class_expr})"
                        )));
                        parts.push(SsrTemplatePart::Static("\"".into()));
                    }
                    *emitted_class = true;
                }
            }
            "style" if !binding.dynamic_arg => {
                if !*emitted_style {
                    if let Some(style_expr) = style_expr {
                        parts.push(SsrTemplatePart::Static(" style=\"".into()));
                        parts.push(SsrTemplatePart::Expr(format!(
                            "_ssrRenderStyle({style_expr})"
                        )));
                        parts.push(SsrTemplatePart::Static("\"".into()));
                    }
                    *emitted_style = true;
                }
            }
            _ if vue3_ssr_is_boolean_attr(&binding.name) && !binding.dynamic_arg => {
                parts.push(SsrTemplatePart::Expr(format!(
                    "(_ssrIncludeBooleanAttr({})) ? \" {}\" : \"\"",
                    self.render_js_expr(binding.value, scope),
                    binding.name
                )));
            }
            _ => parts.push(SsrTemplatePart::Expr(
                self.render_ssr_binding(binding, scope),
            )),
        }
    }

    fn render_ssr_render_attrs_call(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        extra_props: Option<String>,
        tag: Option<&str>,
    ) -> Option<String> {
        let mut merge_args = self.render_ssr_attrs_merge_args(&attrs.props, scope);
        if let Some(extra_props) = extra_props {
            merge_args.push(extra_props);
        }
        merge_args.extend(
            attrs
                .directives
                .iter()
                .map(|directive| self.render_ssr_directive_props(directive, scope)),
        );
        if merge_args.is_empty() {
            return None;
        }
        let props = if merge_args.len() == 1 {
            merge_args.pop().unwrap_or_else(|| "{}".into())
        } else {
            format!("_mergeProps({})", merge_args.join(", "))
        };
        let props = if attrs.directive_content || attrs.textarea_value_fallback.is_some() {
            format!("_temp0 = {props}")
        } else {
            props
        };
        let tag_arg = self.ssr_render_attrs_tag_arg(tag);
        Some(match tag_arg {
            Some(tag) => format!("_ssrRenderAttrs({props}, {tag})"),
            None => format!("_ssrRenderAttrs({props})"),
        })
    }

    fn render_ssr_attrs_merge_args(
        &self,
        props: &Vue3DomProps,
        scope: &RenderScope,
    ) -> Vec<String> {
        let mut merge_args = Vec::new();
        let mut object_entries = Vec::new();
        let mut emitted_class = false;
        let mut emitted_style = false;
        let class_expr = self.render_ssr_class_expr(props, scope, false);
        let style_expr = self.render_ssr_style_expr(props, scope, false);
        if props.segments.is_empty() {
            for attr in &props.static_attrs {
                self.push_ssr_attrs_object_static_entry(
                    attr,
                    class_expr.as_deref(),
                    style_expr.as_deref(),
                    &mut emitted_class,
                    &mut emitted_style,
                    &mut object_entries,
                );
            }
            for binding in &props.dynamic_bindings {
                self.push_ssr_attrs_object_binding_entry(
                    binding,
                    scope,
                    class_expr.as_deref(),
                    style_expr.as_deref(),
                    &mut emitted_class,
                    &mut emitted_style,
                    &mut object_entries,
                );
            }
            self.push_ssr_attrs_object_arg(&mut merge_args, &mut object_entries);
            merge_args.extend(
                props
                    .object_bindings
                    .iter()
                    .map(|binding| self.render_js_expr(binding.value, scope)),
            );
            return merge_args;
        }
        for segment in &props.segments {
            match segment {
                Vue3DomPropSegment::StaticAttr(attr) => {
                    self.push_ssr_attrs_object_static_entry(
                        attr,
                        class_expr.as_deref(),
                        style_expr.as_deref(),
                        &mut emitted_class,
                        &mut emitted_style,
                        &mut object_entries,
                    );
                }
                Vue3DomPropSegment::DynamicBinding(binding) => {
                    self.push_ssr_attrs_object_binding_entry(
                        binding,
                        scope,
                        class_expr.as_deref(),
                        style_expr.as_deref(),
                        &mut emitted_class,
                        &mut emitted_style,
                        &mut object_entries,
                    );
                }
                Vue3DomPropSegment::ObjectBinding(binding) => {
                    self.push_ssr_attrs_object_arg(&mut merge_args, &mut object_entries);
                    merge_args.push(self.render_js_expr(binding.value, scope));
                }
                Vue3DomPropSegment::Content(_)
                | Vue3DomPropSegment::Model(_)
                | Vue3DomPropSegment::Event(_)
                | Vue3DomPropSegment::ObjectListeners(_) => {}
            }
        }
        self.push_ssr_attrs_object_arg(&mut merge_args, &mut object_entries);
        merge_args
    }

    fn push_ssr_attrs_object_static_entry(
        &self,
        attr: &Vue3DomStaticAttr,
        class_expr: Option<&str>,
        style_expr: Option<&str>,
        emitted_class: &mut bool,
        emitted_style: &mut bool,
        object_entries: &mut Vec<String>,
    ) {
        match attr.name.as_str() {
            "class" => {
                if !*emitted_class {
                    if let Some(class_expr) = class_expr {
                        object_entries.push(format!("class: {class_expr}"));
                    }
                    *emitted_class = true;
                }
            }
            "style" => {
                if !*emitted_style {
                    if let Some(style_expr) = style_expr {
                        object_entries.push(format!("style: {style_expr}"));
                    }
                    *emitted_style = true;
                }
            }
            _ => object_entries.push(Self::render_ssr_static_prop_entry(
                &attr.name,
                Some(&attr.value),
            )),
        }
    }

    fn push_ssr_attrs_object_binding_entry(
        &self,
        binding: &Vue3DomBinding,
        scope: &RenderScope,
        class_expr: Option<&str>,
        style_expr: Option<&str>,
        emitted_class: &mut bool,
        emitted_style: &mut bool,
        object_entries: &mut Vec<String>,
    ) {
        match binding.name.as_str() {
            "class" if !binding.dynamic_arg => {
                if !*emitted_class {
                    if let Some(class_expr) = class_expr {
                        object_entries.push(format!("class: {class_expr}"));
                    }
                    *emitted_class = true;
                }
            }
            "style" if !binding.dynamic_arg => {
                if !*emitted_style {
                    if let Some(style_expr) = style_expr {
                        object_entries.push(format!("style: {style_expr}"));
                    }
                    *emitted_style = true;
                }
            }
            _ => object_entries.push(self.render_ssr_object_binding(binding, scope)),
        }
    }

    fn push_ssr_attrs_object_arg(
        &self,
        merge_args: &mut Vec<String>,
        object_entries: &mut Vec<String>,
    ) {
        if !object_entries.is_empty() {
            let object = if object_entries.len() == 1
                && object_entries.first().is_some_and(|entry| {
                    entry.starts_with("class: [") || entry.starts_with("style: [")
                }) {
                render_object(object_entries)
            } else {
                self.render_plain_props(object_entries)
                    .unwrap_or_else(|| "{}".into())
            };
            merge_args.push(object);
            object_entries.clear();
        }
    }

    fn render_ssr_class_expr(
        &self,
        props: &Vue3DomProps,
        scope: &RenderScope,
        dynamic_first: bool,
    ) -> Option<String> {
        let mut static_classes = Vec::new();
        let mut dynamic_classes = Vec::new();
        self.collect_ssr_class_parts(props, scope, &mut static_classes, &mut dynamic_classes);
        if dynamic_first && dynamic_classes.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        if dynamic_first {
            parts.extend(dynamic_classes);
            parts.extend(static_classes);
        } else {
            parts.extend(static_classes);
            parts.extend(dynamic_classes);
        }
        match parts.as_slice() {
            [] => None,
            [single] => Some(single.clone()),
            _ => Some(render_inline_array(&parts)),
        }
    }

    fn collect_ssr_class_parts(
        &self,
        props: &Vue3DomProps,
        scope: &RenderScope,
        static_classes: &mut Vec<String>,
        dynamic_classes: &mut Vec<String>,
    ) {
        if props.segments.is_empty() {
            static_classes.extend(
                props
                    .static_attrs
                    .iter()
                    .filter(|attr| attr.name == "class")
                    .map(|attr| quote_string(&attr.value)),
            );
            dynamic_classes.extend(
                props
                    .dynamic_bindings
                    .iter()
                    .filter(|binding| !binding.dynamic_arg && binding.name == "class")
                    .map(|binding| self.render_js_expr(binding.value, scope)),
            );
            return;
        }
        for segment in &props.segments {
            match segment {
                Vue3DomPropSegment::StaticAttr(attr) if attr.name == "class" => {
                    static_classes.push(quote_string(&attr.value));
                }
                Vue3DomPropSegment::DynamicBinding(binding)
                    if !binding.dynamic_arg && binding.name == "class" =>
                {
                    dynamic_classes.push(self.render_js_expr(binding.value, scope));
                }
                _ => {}
            }
        }
    }

    fn render_ssr_style_expr(
        &self,
        props: &Vue3DomProps,
        scope: &RenderScope,
        require_dynamic: bool,
    ) -> Option<String> {
        let mut parts = Vec::new();
        let mut has_dynamic = false;
        if props.segments.is_empty() {
            parts.extend(
                props
                    .static_attrs
                    .iter()
                    .filter(|attr| attr.name == "style")
                    .map(|attr| vue3_static_style_object_expr(&attr.value)),
            );
            parts.extend(
                props
                    .dynamic_bindings
                    .iter()
                    .filter(|binding| !binding.dynamic_arg && binding.name == "style")
                    .map(|binding| {
                        has_dynamic = true;
                        self.render_js_expr(binding.value, scope)
                    }),
            );
        } else {
            for segment in &props.segments {
                match segment {
                    Vue3DomPropSegment::StaticAttr(attr) if attr.name == "style" => {
                        parts.push(vue3_static_style_object_expr(&attr.value));
                    }
                    Vue3DomPropSegment::DynamicBinding(binding)
                        if !binding.dynamic_arg && binding.name == "style" =>
                    {
                        has_dynamic = true;
                        parts.push(self.render_js_expr(binding.value, scope));
                    }
                    _ => {}
                }
            }
        }
        if require_dynamic && !has_dynamic {
            return None;
        }
        match parts.as_slice() {
            [] => None,
            [single] => Some(single.clone()),
            _ => Some(render_inline_array(&parts)),
        }
    }

    fn render_ssr_directive_props(
        &self,
        directive: &Vue3DomDirective,
        scope: &RenderScope,
    ) -> String {
        let mut args = vec!["_ctx".to_string(), directive_asset_id(&directive.name)];
        if let Some(expression) = directive.expression {
            args.push(self.render_js_expr(expression, scope));
        } else if directive.argument.is_some()
            || directive.dynamic_argument.is_some()
            || !directive.modifiers.is_empty()
        {
            args.push("void 0".into());
        }
        if let Some(argument) = &directive.argument {
            args.push(quote_string(argument));
        } else if let Some(argument) = directive.dynamic_argument {
            args.push(self.render_js_expr(argument, scope));
        } else if !directive.modifiers.is_empty() {
            args.push("void 0".into());
        }
        if !directive.modifiers.is_empty() {
            let modifiers = directive
                .modifiers
                .iter()
                .map(|modifier| format!("{}: true", json_key(modifier)))
                .collect::<Vec<_>>();
            args.push(
                self.render_plain_props(&modifiers)
                    .unwrap_or_else(|| "{}".into()),
            );
        }
        format!("_ssrGetDirectiveProps({})", args.join(", "))
    }

    fn render_ssr_directive_content_expr(&self) -> String {
        "(\"textContent\" in _temp0) ? _ssrInterpolate(_temp0.textContent) : _temp0.innerHTML ?? ''"
            .into()
    }

    fn render_ssr_textarea_value_content_expr(&self, fallback: &str) -> String {
        format!(
            "_ssrInterpolate((\"value\" in _temp0) ? _temp0.value : {})",
            quote_string(fallback)
        )
    }

    fn ssr_render_attrs_tag_arg(&self, tag: Option<&str>) -> Option<String> {
        let tag = tag?.trim();
        if tag.is_empty() || tag.starts_with("#expr") {
            return None;
        }
        if tag == "textarea" || self.options.custom_elements.iter().any(|item| item == tag) {
            Some(quote_string(tag))
        } else {
            None
        }
    }

    fn collect_ssr_template_attrs_with_v_model(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        parts: &mut Vec<SsrTemplatePart>,
    ) -> Option<()> {
        let Some(model) = &attrs.v_model else {
            return Some(());
        };
        if matches!(model.kind, Vue3SsrModelKind::Textarea) {
            return Some(());
        }
        if attrs.v_show.is_some() {
            return None;
        }
        if matches!(model.kind, Vue3SsrModelKind::InputDynamicProps) {
            parts.push(SsrTemplatePart::Expr(format!(
                "_ssrRenderAttrs({})",
                self.render_dynamic_model_props_expr(attrs, scope, None)
            )));
            return Some(());
        }
        let mut model_rendered = false;
        if attrs.props.segments.is_empty() {
            for binding in &attrs.props.dynamic_bindings {
                parts.push(SsrTemplatePart::Expr(
                    self.render_ssr_binding(binding, scope),
                ));
                if self.v_model_should_render_after_binding(model, binding) {
                    self.collect_ssr_template_v_model_attr(model, scope, parts)?;
                    model_rendered = true;
                }
            }
            for binding in &attrs.props.object_bindings {
                parts.push(SsrTemplatePart::Expr(format!(
                    "_ssrRenderAttrs({})",
                    self.render_js_expr(binding.value, scope)
                )));
            }
            if !model_rendered {
                self.collect_ssr_template_v_model_attr(model, scope, parts)?;
            }
            if matches!(
                model.kind,
                Vue3SsrModelKind::InputDynamicType {
                    type_expr: _,
                    value: _
                }
            ) {
                for attr in &attrs.props.static_attrs {
                    self.collect_ssr_template_static_attr_for_v_model(attr, parts);
                }
            }
            return Some(());
        }
        for segment in &attrs.props.segments {
            match segment {
                Vue3DomPropSegment::DynamicBinding(binding) => {
                    parts.push(SsrTemplatePart::Expr(
                        self.render_ssr_binding(binding, scope),
                    ));
                    if self.v_model_should_render_after_binding(model, binding) {
                        self.collect_ssr_template_v_model_attr(model, scope, parts)?;
                        model_rendered = true;
                    }
                }
                Vue3DomPropSegment::ObjectBinding(binding) => {
                    parts.push(SsrTemplatePart::Expr(format!(
                        "_ssrRenderAttrs({})",
                        self.render_js_expr(binding.value, scope)
                    )));
                }
                Vue3DomPropSegment::StaticAttr(attr) => {
                    if matches!(
                        model.kind,
                        Vue3SsrModelKind::InputDynamicType {
                            type_expr: _,
                            value: _
                        }
                    ) {
                        self.collect_ssr_template_static_attr_for_v_model(attr, parts);
                    }
                }
                Vue3DomPropSegment::Content(_)
                | Vue3DomPropSegment::Model(_)
                | Vue3DomPropSegment::Event(_)
                | Vue3DomPropSegment::ObjectListeners(_) => {}
            }
        }
        if !model_rendered {
            self.collect_ssr_template_v_model_attr(model, scope, parts)?;
        }
        Some(())
    }

    fn collect_ssr_template_v_model_attr(
        &self,
        model: &Vue3SsrModel,
        scope: &RenderScope,
        parts: &mut Vec<SsrTemplatePart>,
    ) -> Option<()> {
        parts.push(SsrTemplatePart::Expr(
            self.render_v_model_attr_expr(model, scope)?,
        ));
        Some(())
    }

    fn collect_ssr_template_static_attr_for_v_model(
        &self,
        attr: &Vue3DomStaticAttr,
        parts: &mut Vec<SsrTemplatePart>,
    ) {
        parts.push(SsrTemplatePart::Static(format!(
            " {}=\"{}\"",
            attr.name,
            vue3_ssr_escape_attr(&attr.value)
        )));
    }

    fn v_model_should_render_after_binding(
        &self,
        model: &Vue3SsrModel,
        binding: &Vue3DomBinding,
    ) -> bool {
        matches!(
            model.kind,
            Vue3SsrModelKind::InputDynamicType {
                type_expr: _,
                value: _
            }
        ) && !binding.dynamic_arg
            && binding.name == "type"
    }

    fn collect_ssr_template_attrs_without_v_show_style(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        parts: &mut Vec<SsrTemplatePart>,
    ) {
        let props = &attrs.props;
        if props.segments.is_empty() {
            for binding in &props.dynamic_bindings {
                if binding.name != "style" {
                    parts.push(SsrTemplatePart::Expr(
                        self.render_ssr_binding(binding, scope),
                    ));
                }
            }
            return;
        }
        for segment in &props.segments {
            if let Vue3DomPropSegment::DynamicBinding(binding) = segment {
                if binding.name != "style" {
                    parts.push(SsrTemplatePart::Expr(
                        self.render_ssr_binding(binding, scope),
                    ));
                }
            }
        }
    }

    fn ssr_push_string(&self, node_id: NodeId) -> Option<&str> {
        self.mir.node(node_id).and_then(|node| match &node.kind {
            Vue3SsrMirKind::PushString(value) => Some(value.as_str()),
            _ => None,
        })
    }

    fn render_node(
        &self,
        node_id: NodeId,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        let Some(node) = self.mir.node(node_id) else {
            return;
        };
        match &node.kind {
            Vue3SsrMirKind::Root(_) => self.render_children(node_id, scope, writer),
            Vue3SsrMirKind::PushString(value) => {
                if let Some(tag) = self.render_dynamic_tag_push_string(value, scope) {
                    writer.push_line(&format!("_push({tag});"));
                    return;
                }
                let value = if value == "/>" { ">" } else { value };
                writer.push_line(&format!("_push({});", quote_string(value)));
            }
            Vue3SsrMirKind::PushInterpolated(expr) => {
                writer.push_line(&format!(
                    "_push(_ssrInterpolate({}));",
                    self.render_mir_expr(expr, scope)
                ));
            }
            Vue3SsrMirKind::RenderContent(content) => {
                self.render_content(content, scope, writer);
            }
            Vue3SsrMirKind::RenderAttrs(attrs) => {
                self.render_attrs_with_root_attrs(attrs, scope, root_attrs, writer);
            }
            Vue3SsrMirKind::RenderComponent(component) => {
                self.render_component(node_id, component, scope, root_attrs, writer);
            }
            Vue3SsrMirKind::Transition => {
                self.render_transition(node_id, scope, root_attrs, writer);
            }
            Vue3SsrMirKind::RenderSlot(slot) => {
                self.render_slot(slot, scope, writer);
            }
            Vue3SsrMirKind::If { condition, comment } => {
                self.render_if(node_id, *condition, *comment, scope, root_attrs, writer);
            }
            Vue3SsrMirKind::For(for_mir) => {
                self.render_for(node_id, for_mir, scope, writer);
            }
            Vue3SsrMirKind::Teleport(teleport) => {
                self.render_teleport(node_id, teleport, scope, writer);
            }
            Vue3SsrMirKind::Suspense(suspense) => {
                self.render_suspense(suspense, scope, writer);
            }
        }
    }

    fn render_transition(
        &self,
        node_id: NodeId,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        if scope.locals.iter().any(|local| local == "_scopeId") {
            writer.push_line("_push(``)");
        }
        let children = self
            .mir
            .node(node_id)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        self.render_child_slice(&children, scope, Some("_scopeId"), root_attrs, writer);
    }

    fn render_dynamic_tag_push_string(&self, value: &str, scope: &RenderScope) -> Option<String> {
        if let Some((tag, _)) = parse_ssr_open_tag_start(value) {
            if let Some(expression) = self.dynamic_tag_name_expr(&tag, scope) {
                return Some(render_ssr_template_literal(&[
                    SsrTemplatePart::Static("<".into()),
                    SsrTemplatePart::Expr(expression),
                ]));
            }
        }
        if value.starts_with("<#expr") {
            let mut parts = Vec::new();
            let mut dynamic = false;
            self.push_ssr_open_tag_start_part(value, scope, &mut parts, &mut dynamic);
            return dynamic.then(|| render_ssr_template_literal(&parts));
        }
        if value.starts_with("</#expr") {
            let mut parts = Vec::new();
            let mut dynamic = false;
            self.push_ssr_close_tag_part(value, scope, &mut parts, &mut dynamic);
            return dynamic.then(|| render_ssr_template_literal(&parts));
        }
        None
    }

    fn render_component(
        &self,
        node_id: NodeId,
        component: &Vue3SsrComponent,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        let props = self.render_component_props_with_root_attrs(
            &component.props,
            &component.directives,
            scope,
            root_attrs,
        );
        if component.dynamic {
            let tag = self.render_dynamic_component_tag(&component.tag, scope);
            let slots = if let Some(slots) = &component.slots {
                self.render_component_slots(slots, scope)
            } else if self
                .mir
                .node(node_id)
                .is_some_and(|node| !node.children.is_empty())
            {
                self.render_component_slots_object(node_id, scope)
            } else {
                "null".to_string()
            };
            writer.push_line(&format!(
                "_ssrRenderVNode(_push, _createVNode({}, {}, {}), _parent)",
                tag, props, slots
            ));
            return;
        }
        let tag = self.render_component_tag(&component.tag, scope);
        let scope_id_arg = self.render_component_scope_id_arg(scope);
        if let Some(slots) = &component.slots {
            let slots = self.render_component_slots(slots, scope);
            writer.push_line(&format!(
                "_push(_ssrRenderComponent({}, {}, {}, _parent{}))",
                tag, props, slots, scope_id_arg
            ));
        } else if self
            .mir
            .node(node_id)
            .is_some_and(|node| node.children.is_empty())
        {
            writer.push_line(&format!(
                "_push(_ssrRenderComponent({}, {}, null, _parent{}))",
                tag, props, scope_id_arg
            ));
        } else {
            writer.push_line(&format!("_push(_ssrRenderComponent({}, {}, {{", tag, props));
            writer.indent();
            self.render_component_default_slot(node_id, scope, writer);
            writer.dedent();
            writer.push_line(&format!("}}, _parent{}))", scope_id_arg));
        }
    }

    fn render_component_scope_id_arg(&self, scope: &RenderScope) -> &'static str {
        if scope.locals.iter().any(|local| local == "_scopeId") {
            ", _scopeId"
        } else {
            ""
        }
    }

    fn render_component_tag(&self, tag: &MirExpr, scope: &RenderScope) -> String {
        match tag {
            MirExpr::String(name) => component_asset_id(name),
            _ => self.render_mir_expr(tag, scope),
        }
    }

    fn render_dynamic_component_tag(&self, tag: &MirExpr, scope: &RenderScope) -> String {
        format!(
            "_resolveDynamicComponent({})",
            self.render_mir_expr(tag, scope)
        )
    }

    fn render_component_vnode_tag(
        &self,
        component: &Vue3SsrComponent,
        scope: &RenderScope,
    ) -> String {
        if component.dynamic {
            self.render_dynamic_component_tag(&component.tag, scope)
        } else {
            self.render_component_tag(&component.tag, scope)
        }
    }

    fn render_component_slots_object(&self, node_id: NodeId, scope: &RenderScope) -> String {
        let mut writer = CodeWriter::new();
        writer.push_line("{");
        writer.indent();
        self.render_component_default_slot(node_id, scope, &mut writer);
        writer.dedent();
        writer.push_line("}");
        writer.finish().trim_end().to_string()
    }

    fn render_component_slots(
        &self,
        slots: &vuec_ast::Vue3DomSlots,
        scope: &RenderScope,
    ) -> String {
        if slots.dynamic_slots.is_empty() {
            self.render_stable_component_slots(slots, scope)
        } else {
            self.render_dynamic_component_slots(slots, scope)
        }
    }

    fn render_stable_component_slots(
        &self,
        slots: &vuec_ast::Vue3DomSlots,
        scope: &RenderScope,
    ) -> String {
        let mut writer = CodeWriter::new();
        writer.push_line("{");
        writer.indent();
        for slot in &slots.slots {
            self.render_component_static_slot(slot, scope, &mut writer);
        }
        writer.push_line(&format!("_: {}", vue3_slot_flag_with_comment(slots.flag)));
        writer.dedent();
        writer.push_line("}");
        writer.finish().trim_end().to_string()
    }

    fn render_dynamic_component_slots(
        &self,
        slots: &vuec_ast::Vue3DomSlots,
        scope: &RenderScope,
    ) -> String {
        let mut writer = CodeWriter::new();
        writer.push_line(&format!(
            "_createSlots({{ _: {} }}, [",
            vue3_slot_flag_with_comment(slots.flag)
        ));
        writer.indent();
        for (index, slot) in slots.dynamic_slots.iter().enumerate() {
            let rendered = self.render_component_dynamic_slot(slot, scope);
            let suffix = if index + 1 == slots.dynamic_slots.len() {
                ""
            } else {
                ","
            };
            writer.push_str(&rendered);
            writer.push_line(suffix);
        }
        writer.dedent();
        writer.push_line("])");
        writer.finish().trim_end().to_string()
    }

    fn render_component_static_slot(
        &self,
        slot: &vuec_ast::Vue3DomSlot,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        writer.push_line(&format!(
            "{}: {},",
            json_key(&slot.name),
            self.render_component_slot_function(slot.params, &slot.children, scope)
        ));
    }

    fn render_component_dynamic_slot(
        &self,
        slot: &vuec_ast::Vue3DomDynamicSlot,
        scope: &RenderScope,
    ) -> String {
        match slot {
            vuec_ast::Vue3DomDynamicSlot::Slot(slot) => {
                self.render_component_dynamic_slot_object(slot, scope)
            }
            vuec_ast::Vue3DomDynamicSlot::Conditional(slot) => {
                self.render_component_conditional_slot(slot, scope)
            }
            vuec_ast::Vue3DomDynamicSlot::For(slot) => self.render_component_for_slot(slot, scope),
        }
    }

    fn render_component_conditional_slot(
        &self,
        slot: &vuec_ast::Vue3DomConditionalSlot,
        scope: &RenderScope,
    ) -> String {
        let condition = slot
            .condition
            .map(|condition| self.render_js_expr(condition, scope))
            .unwrap_or_else(|| "true".into());
        let condition = render_vue3_ssr_slot_condition(condition);
        let consequent = indent_after_first_line(
            &self.render_component_dynamic_slot_object(&slot.slot, scope),
            4,
        );
        let alternate = slot
            .alternate
            .as_ref()
            .map(|alternate| self.render_component_dynamic_slot(alternate, scope))
            .unwrap_or_else(|| "undefined".into());
        format!("{condition}\n  ? {consequent}\n  : {alternate}")
    }

    fn render_component_for_slot(
        &self,
        slot: &vuec_ast::Vue3DomForSlot,
        scope: &RenderScope,
    ) -> String {
        let source = self.render_js_expr(slot.source, scope);
        let params = self.render_dom_for_slot_params(slot);
        let child_scope = self.scope_with_dom_for_slot(scope, slot);
        let body = indent_after_first_line(
            &self.render_component_dynamic_slot_object(&slot.slot, &child_scope),
            2,
        );
        format!("_renderList({source}, ({params}) => {{\n  return {body}\n}})")
    }

    fn render_component_dynamic_slot_object(
        &self,
        slot: &vuec_ast::Vue3DomDynamicSlotObject,
        scope: &RenderScope,
    ) -> String {
        let mut writer = CodeWriter::new();
        writer.push_line("{");
        writer.indent();
        writer.push_line(&format!(
            "name: {},",
            self.render_slot_name(&slot.name, scope)
        ));
        let slot_fn = self.render_component_slot_function(slot.params, &slot.children, scope);
        if let Some(key) = &slot.key {
            writer.push_line(&format!("fn: {},", slot_fn));
            writer.push_line(&format!("key: {}", quote_string(key)));
        } else {
            writer.push_line(&format!("fn: {}", slot_fn));
        }
        writer.dedent();
        writer.push_line("}");
        writer.finish().trim_end().to_string()
    }

    fn render_component_slot_function(
        &self,
        params: Option<JsPatternId>,
        children: &[NodeId],
        scope: &RenderScope,
    ) -> String {
        let params_text = params
            .map(|params| self.render_js_pattern(params))
            .filter(|params| !params.trim().is_empty())
            .unwrap_or_else(|| "_".into());
        let mut locals = vec!["_scopeId".into()];
        if let Some(params) = params {
            locals.extend(extract_v_for_alias_locals(&self.render_js_pattern(params)));
        }
        let slot_scope = scope.with_locals(locals);
        let mut writer = CodeWriter::new();
        writer.push_line(&format!(
            "_withCtx(({}, _push, _parent, _scopeId) => {{",
            params_text
        ));
        writer.indent();
        writer.push_line("if (_push) {");
        writer.indent();
        self.render_child_slice(children, &slot_scope, Some("_scopeId"), None, &mut writer);
        writer.dedent();
        writer.push_line("} else {");
        writer.indent();
        let fallback =
            self.render_component_slot_vnode_fallback_children_from(children, &slot_scope);
        writer.push_line(&format!("return {}", render_array(&fallback)));
        writer.dedent();
        writer.push_line("}");
        writer.dedent();
        writer.push_line("})");
        writer.finish().trim_end().to_string()
    }

    fn render_component_default_slot(
        &self,
        node_id: NodeId,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        writer.push_line("default: _withCtx((_, _push, _parent, _scopeId) => {");
        writer.indent();
        let slot_scope = scope.with_locals(vec!["_scopeId".into()]);
        writer.push_line("if (_push) {");
        writer.indent();
        if let Some(node) = self.mir.node(node_id) {
            self.render_child_slice(&node.children, &slot_scope, Some("_scopeId"), None, writer);
        }
        writer.dedent();
        writer.push_line("} else {");
        writer.indent();
        let fallback = self.render_component_slot_vnode_fallback_children(node_id, &slot_scope);
        writer.push_line(&format!("return {}", render_array(&fallback)));
        writer.dedent();
        writer.push_line("}");
        writer.dedent();
        writer.push_line("}),");
        let flag = if self.component_slot_is_forwarded(node_id) {
            Vue3SlotFlag::Forwarded
        } else {
            Vue3SlotFlag::Stable
        };
        writer.push_line(&format!("_: {}", vue3_slot_flag_with_comment(flag)));
    }

    fn render_component_slot_vnode_fallback_children(
        &self,
        parent: NodeId,
        scope: &RenderScope,
    ) -> Vec<String> {
        let children = self
            .mir
            .node(parent)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        self.render_component_slot_vnode_fallback_children_from(&children, scope)
    }

    fn render_component_slot_vnode_fallback_children_from(
        &self,
        children: &[NodeId],
        scope: &RenderScope,
    ) -> Vec<String> {
        self.render_vnode_fallback_children_from(children, scope)
            .into_iter()
            .map(|child| {
                if vue3_expression_is_string_literal(&child) {
                    format!("_createTextVNode({child})")
                } else {
                    child
                }
            })
            .collect()
    }

    fn component_slot_is_forwarded(&self, node_id: NodeId) -> bool {
        let Some(node) = self.mir.node(node_id) else {
            return false;
        };
        node.children.iter().any(|child_id| {
            self.mir
                .node(*child_id)
                .is_some_and(|child| match &child.kind {
                    Vue3SsrMirKind::RenderSlot(slot) => self.render_slot_as_vnode_fallback(slot),
                    Vue3SsrMirKind::If { .. } | Vue3SsrMirKind::For(_) => {
                        self.component_slot_is_forwarded(*child_id)
                    }
                    _ => false,
                })
        })
    }

    fn render_vnode_fallback_children(&self, parent: NodeId, scope: &RenderScope) -> Vec<String> {
        let children = self
            .mir
            .node(parent)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        self.render_vnode_fallback_children_from(&children, scope)
    }

    fn render_vnode_fallback_children_from(
        &self,
        children: &[NodeId],
        scope: &RenderScope,
    ) -> Vec<String> {
        let mut rendered = Vec::new();
        let mut index = 0usize;
        while index < children.len() {
            if let Some((element, next_index)) =
                self.render_vnode_fallback_element(&children, index, scope)
            {
                rendered.push(element);
                index = next_index;
                continue;
            }
            if let Some(node) = self.render_vnode_fallback_node(children[index], scope) {
                rendered.push(node);
            }
            index += 1;
        }
        rendered
    }

    fn render_vnode_fallback_element(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
    ) -> Option<(String, usize)> {
        let (tag, static_entries) =
            self.mir
                .node(children[index])
                .and_then(|node| match &node.kind {
                    Vue3SsrMirKind::PushString(value) => parse_ssr_open_tag_start(value),
                    _ => None,
                })?;
        let mut props = self.render_vnode_fallback_static_props(&static_entries);
        let mut cursor = index + 1;
        if let Some(attrs) = children
            .get(cursor)
            .and_then(|id| self.mir.node(*id))
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            })
        {
            props = self.merge_vnode_fallback_props(
                props,
                self.render_vnode_fallback_dynamic_props(&attrs.props, scope),
            );
            cursor += 1;
        }
        if children
            .get(cursor)
            .and_then(|id| self.mir.node(*id))
            .is_some_and(
                |node| matches!(&node.kind, Vue3SsrMirKind::PushString(value) if value == "/>"),
            )
        {
            return Some((
                self.render_vnode_fallback_call(&tag, props, Vec::new()),
                cursor + 1,
            ));
        }
        if !children
            .get(cursor)
            .and_then(|id| self.mir.node(*id))
            .is_some_and(
                |node| matches!(&node.kind, Vue3SsrMirKind::PushString(value) if value == ">"),
            )
        {
            return None;
        }
        cursor += 1;
        let mut child_nodes = Vec::new();
        while cursor < children.len() {
            if children
                .get(cursor)
                .and_then(|id| self.mir.node(*id))
                .is_some_and(|node| matches!(&node.kind, Vue3SsrMirKind::PushString(value) if value == &format!("</{tag}>")))
            {
                return Some((
                    self.render_vnode_fallback_call(&tag, props, child_nodes),
                    cursor + 1,
                ));
            }
            if let Some((element, next_index)) =
                self.render_vnode_fallback_element(children, cursor, scope)
            {
                child_nodes.push(element);
                cursor = next_index;
                continue;
            }
            if let Some(node) = self.render_vnode_fallback_node(children[cursor], scope) {
                child_nodes.push(node);
            }
            cursor += 1;
        }
        Some((
            self.render_vnode_fallback_call(&tag, props, child_nodes),
            cursor,
        ))
    }

    fn render_vnode_fallback_block_element(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        key: Option<usize>,
    ) -> Option<(String, usize)> {
        let (tag, static_entries) =
            self.mir
                .node(*children.get(index)?)
                .and_then(|node| match &node.kind {
                    Vue3SsrMirKind::PushString(value) => parse_ssr_open_tag_start(value),
                    _ => None,
                })?;
        let mut props = self.render_vnode_fallback_static_props(&static_entries);
        props = self.merge_vnode_fallback_key_prop(props, key);
        let mut cursor = index + 1;
        if let Some(attrs) = children
            .get(cursor)
            .and_then(|id| self.mir.node(*id))
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            })
        {
            props = self.merge_vnode_fallback_props(
                props,
                self.render_vnode_fallback_dynamic_props(&attrs.props, scope),
            );
            cursor += 1;
        }
        if children
            .get(cursor)
            .and_then(|id| self.mir.node(*id))
            .is_some_and(
                |node| matches!(&node.kind, Vue3SsrMirKind::PushString(value) if value == "/>"),
            )
        {
            return Some((
                self.render_vnode_fallback_block_call(&tag, props, Vec::new()),
                cursor + 1,
            ));
        }
        if !children
            .get(cursor)
            .and_then(|id| self.mir.node(*id))
            .is_some_and(
                |node| matches!(&node.kind, Vue3SsrMirKind::PushString(value) if value == ">"),
            )
        {
            return None;
        }
        cursor += 1;
        let mut child_nodes = Vec::new();
        while cursor < children.len() {
            if children
                .get(cursor)
                .and_then(|id| self.mir.node(*id))
                .is_some_and(|node| matches!(&node.kind, Vue3SsrMirKind::PushString(value) if value == &format!("</{tag}>")))
            {
                return Some((
                    self.render_vnode_fallback_block_call(&tag, props, child_nodes),
                    cursor + 1,
                ));
            }
            if let Some((element, next_index)) =
                self.render_vnode_fallback_block_element(children, cursor, scope, None)
            {
                child_nodes.push(element);
                cursor = next_index;
                continue;
            }
            if let Some(node) = self.render_vnode_fallback_node(children[cursor], scope) {
                child_nodes.push(node);
            }
            cursor += 1;
        }
        Some((
            self.render_vnode_fallback_block_call(&tag, props, child_nodes),
            cursor,
        ))
    }

    fn render_vnode_fallback_node(&self, node_id: NodeId, scope: &RenderScope) -> Option<String> {
        let node = self.mir.node(node_id)?;
        match &node.kind {
            Vue3SsrMirKind::PushString(value) if !value.starts_with('<') => {
                Some(quote_string(value))
            }
            Vue3SsrMirKind::PushInterpolated(expr) => Some(format!(
                "_createTextVNode(_toDisplayString({}), 1 /* TEXT */)",
                self.render_mir_expr(expr, scope)
            )),
            Vue3SsrMirKind::RenderSlot(slot) if self.render_slot_as_vnode_fallback(slot) => {
                Some(self.render_vnode_fallback_slot(slot, scope))
            }
            Vue3SsrMirKind::RenderComponent(component) => {
                Some(self.render_component_vnode_fallback(node_id, component, scope))
            }
            Vue3SsrMirKind::Transition => {
                Some(self.render_transition_vnode_fallback(node_id, scope))
            }
            Vue3SsrMirKind::If { condition, comment } => {
                let condition = condition
                    .map(|condition| self.render_js_expr(condition, scope))
                    .unwrap_or_else(|| "true".into());
                let condition = render_vue3_ssr_slot_condition(condition);
                let children = self.render_vnode_fallback_if_consequent(node_id, scope);
                let alternate: String = if *comment {
                    "_createCommentVNode(\"v-if\", true)".into()
                } else {
                    "null".into()
                };
                Some(format!("{condition}\n  ? {children}\n  : {alternate}"))
            }
            Vue3SsrMirKind::For(for_mir) => {
                Some(self.render_vnode_fallback_for(node_id, for_mir, scope))
            }
            _ => None,
        }
    }

    fn render_transition_vnode_fallback(&self, node_id: NodeId, scope: &RenderScope) -> String {
        let children = self.render_vnode_fallback_children(node_id, scope);
        format!(
            "_createVNode(_Transition, null, {{\n  default: _withCtx(() => {}),\n  _: 1 /* STABLE */\n}})",
            render_array(&children).replace('\n', "\n  ")
        )
    }

    fn render_vnode_fallback_if_consequent(&self, node_id: NodeId, scope: &RenderScope) -> String {
        let children = self
            .mir
            .node(node_id)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        if let Some((rendered, _)) =
            self.render_vnode_fallback_block_element(&children, 0, scope, Some(0))
        {
            return rendered;
        }
        let rendered = self.render_vnode_fallback_children_from(&children, scope);
        match rendered.as_slice() {
            [single] => single.clone(),
            _ => render_array(&rendered),
        }
    }

    fn render_vnode_fallback_for(
        &self,
        node_id: NodeId,
        for_mir: &Vue3SsrFor,
        scope: &RenderScope,
    ) -> String {
        let source = self.render_js_expr(for_mir.source, scope);
        let params = self.render_for_params(for_mir);
        let child_scope = self.scope_with_for_mir(scope, for_mir);
        let children = self
            .mir
            .node(node_id)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        let body = if let Some((rendered, _)) =
            self.render_vnode_fallback_block_element(&children, 0, &child_scope, None)
        {
            rendered
        } else {
            let rendered = self.render_vnode_fallback_children_from(&children, &child_scope);
            match rendered.as_slice() {
                [single] => single.clone(),
                _ => render_array(&rendered),
            }
        };
        format!(
            "(_openBlock(true), _createBlock(_Fragment, null, _renderList({source}, ({params}) => {{\n  return {body}\n}}), 256 /* UNKEYED_FRAGMENT */))"
        )
    }

    fn render_component_vnode_fallback(
        &self,
        node_id: NodeId,
        component: &Vue3SsrComponent,
        scope: &RenderScope,
    ) -> String {
        let tag = self.render_component_vnode_tag(component, scope);
        let props = self.render_component_props(&component.props, scope);
        let has_children = self
            .mir
            .node(node_id)
            .is_some_and(|node| !node.children.is_empty());
        if !has_children {
            return self.render_vnode_fallback_call(
                &tag,
                (props != "null").then_some(props),
                Vec::new(),
            );
        }
        let slots = self.render_component_vnode_fallback_slots_object(node_id, scope);
        format!("_createVNode({tag}, {props}, {slots})")
    }

    fn render_component_vnode_fallback_slots_object(
        &self,
        node_id: NodeId,
        scope: &RenderScope,
    ) -> String {
        let children = self.render_component_slot_vnode_fallback_children(node_id, scope);
        let rendered_children = render_array(&children).replace('\n', "\n  ");
        format!(
            "{{\n  default: _withCtx(() => {}),\n  _: 1 /* STABLE */\n}}",
            rendered_children
        )
    }

    fn render_slot_as_vnode_fallback(&self, slot: &vuec_ast::Vue3SsrSlot) -> bool {
        !slot.inner
    }

    fn render_vnode_fallback_slot(
        &self,
        slot: &vuec_ast::Vue3SsrSlot,
        scope: &RenderScope,
    ) -> String {
        let slots = if uses_prefixed_identifiers(self.options) {
            "_ctx.$slots"
        } else {
            "$slots"
        };
        let mut args = vec![slots.to_string(), self.render_slot_name(&slot.name, scope)];
        let props = self.render_slot_props(&slot.props, scope);
        if props != "{}" || !slot.fallback.is_empty() || !self.options.slotted {
            args.push(props);
        }
        if !slot.fallback.is_empty() || !self.options.slotted {
            let fallback = if slot.fallback.is_empty() {
                "undefined".into()
            } else {
                let rendered = slot
                    .fallback
                    .iter()
                    .filter_map(|child_id| self.render_vnode_fallback_node(*child_id, scope))
                    .collect::<Vec<_>>();
                format!("() => {}", render_array(&rendered))
            };
            args.push(fallback);
        }
        if !self.options.slotted {
            args.push("true".into());
        }
        format!("_renderSlot({})", args.join(", "))
    }

    fn render_vnode_fallback_static_props(
        &self,
        entries: &[(String, Option<String>)],
    ) -> Option<String> {
        let rendered = entries
            .iter()
            .filter(|(name, _)| !self.is_vnode_fallback_scope_attr(name))
            .map(|(name, value)| {
                format!(
                    "{}: {}",
                    json_key(name),
                    quote_string(value.as_deref().unwrap_or(""))
                )
            })
            .collect::<Vec<_>>();
        if entries
            .iter()
            .any(|(name, _)| name == "srcset" || name == "sizes")
        {
            Some(render_object(&rendered))
        } else {
            self.render_plain_props(&rendered)
        }
    }

    fn is_vnode_fallback_scope_attr(&self, name: &str) -> bool {
        self.options
            .scope_id
            .as_ref()
            .is_some_and(|scope_id| name == scope_id || name == format!("{scope_id}-s"))
    }

    fn render_vnode_fallback_dynamic_props(
        &self,
        props: &Vue3DomProps,
        scope: &RenderScope,
    ) -> Option<String> {
        if props.segments.is_empty()
            && props.dynamic_bindings.len() == 1
            && props.dynamic_bindings[0].name == "srcset"
            && props.static_attrs.is_empty()
            && props.events.is_empty()
            && props.object_bindings.is_empty()
            && props.object_listeners.is_empty()
            && !props.normalize.normalize_props
        {
            let binding = &props.dynamic_bindings[0];
            return Some(render_object(
                &[self.render_dynamic_binding(binding, scope)],
            ));
        }
        if props.segments.len() == 1
            && props.dynamic_bindings.len() == 1
            && props.dynamic_bindings[0].name == "srcset"
            && props.static_attrs.is_empty()
            && props.events.is_empty()
            && props.object_bindings.is_empty()
            && props.object_listeners.is_empty()
            && !props.normalize.normalize_props
            && matches!(
                &props.segments[0],
                Vue3DomPropSegment::DynamicBinding(binding) if binding.name == "srcset"
            )
        {
            let binding = &props.dynamic_bindings[0];
            return Some(render_object(
                &[self.render_dynamic_binding(binding, scope)],
            ));
        }
        self.render_ordered_props(props, scope)
            .map(|rendered| self.render_normalized_props(props, rendered))
    }

    fn merge_vnode_fallback_props(
        &self,
        static_props: Option<String>,
        dynamic_props: Option<String>,
    ) -> Option<String> {
        match (static_props, dynamic_props) {
            (Some(static_props), Some(dynamic_props)) => {
                Some(format!("_mergeProps({static_props}, {dynamic_props})"))
            }
            (Some(props), None) | (None, Some(props)) => Some(props),
            (None, None) => None,
        }
    }

    fn merge_vnode_fallback_key_prop(
        &self,
        props: Option<String>,
        key: Option<usize>,
    ) -> Option<String> {
        let key = key?;
        let key_props = format!("{{ key: {key} }}");
        Some(match props {
            Some(props) => format!("_mergeProps({key_props}, {props})"),
            None => key_props,
        })
    }

    fn render_vnode_fallback_call(
        &self,
        tag: &str,
        props: Option<String>,
        children: Vec<String>,
    ) -> String {
        let tag = if tag.starts_with("_component_") {
            tag.to_string()
        } else if tag.starts_with('_') {
            tag.to_string()
        } else {
            quote_string(tag)
        };
        if children.is_empty() {
            if let Some(props) = props {
                format!("_createVNode({tag}, {props})")
            } else {
                format!("_createVNode({tag})")
            }
        } else {
            let rendered_children =
                if children.len() == 1 && vue3_expression_is_string_literal(children[0].trim()) {
                    children[0].clone()
                } else {
                    render_array(&children)
                };
            format!(
                "_createVNode({tag}, {}, {})",
                props.unwrap_or_else(|| "null".into()),
                rendered_children
            )
        }
    }

    fn render_vnode_fallback_block_call(
        &self,
        tag: &str,
        props: Option<String>,
        children: Vec<String>,
    ) -> String {
        let tag = if tag.starts_with("_component_") || tag.starts_with('_') {
            tag.to_string()
        } else {
            quote_string(tag)
        };
        let open = if tag.starts_with("_component_") || tag.starts_with('_') {
            "_openBlock()"
        } else {
            "_openBlock()"
        };
        match (props, children.is_empty()) {
            (None, true) => format!("({open}, _createBlock({tag}))"),
            (Some(props), true) => format!("({open}, _createBlock({tag}, {props}))"),
            (props, false) => format!(
                "({open}, _createBlock({}, {}, {}))",
                tag,
                props.unwrap_or_else(|| "null".into()),
                render_array(&children).replace('\n', "\n    ")
            ),
        }
    }

    fn render_component_props(&self, props: &Vue3DomProps, scope: &RenderScope) -> String {
        self.render_ordered_props(props, scope)
            .map(|rendered| self.render_normalized_props(props, rendered))
            .unwrap_or_else(|| "null".into())
    }

    fn render_component_props_with_root_attrs(
        &self,
        props: &Vue3DomProps,
        directives: &[Vue3DomDirective],
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
    ) -> String {
        let rendered = self.render_component_props(props, scope);
        let mut merge_args = Vec::new();
        if rendered != "null" {
            merge_args.push(rendered);
        }
        if let Some(root_attrs) = root_attrs {
            if let Some(attrs) = &root_attrs.attrs {
                merge_args.push(attrs.clone());
            }
            if let Some(css_vars) = &root_attrs.css_vars {
                merge_args.push(css_vars.clone());
            }
        }
        merge_args.extend(
            directives
                .iter()
                .map(|directive| self.render_ssr_directive_props(directive, scope)),
        );
        match merge_args.len() {
            0 => "null".into(),
            1 => merge_args.pop().unwrap_or_else(|| "null".into()),
            _ => format!("_mergeProps({})", merge_args.join(", ")),
        }
    }

    fn render_root_attrs_expr_with_static(
        &self,
        root_attrs: &SsrRootAttrs,
        static_entries: &[(String, Option<String>)],
    ) -> String {
        let props = if root_attrs.attrs.is_some() {
            self.root_static_props(static_entries)
        } else {
            None
        };
        let merged = self
            .merge_root_attrs_with_props(props, root_attrs)
            .unwrap_or_else(|| "{}".into());
        format!("_ssrRenderAttrs({merged})")
    }

    fn root_attrs_props(&self, root_attrs: Option<&SsrRootAttrs>) -> Option<String> {
        let root_attrs = root_attrs?;
        let mut parts = Vec::new();
        if let Some(attrs) = &root_attrs.attrs {
            parts.push(attrs.clone());
        }
        if let Some(css_vars) = &root_attrs.css_vars {
            parts.push(css_vars.clone());
        }
        match parts.len() {
            0 => None,
            1 => parts.into_iter().next(),
            _ => Some(format!("_mergeProps({})", parts.join(", "))),
        }
    }

    fn render_root_element_attrs_expr_with_static(
        &self,
        attrs: &Vue3SsrAttrs,
        root_attrs: &SsrRootAttrs,
        scope: &RenderScope,
        tag: &str,
        static_entries: &[(String, Option<String>)],
    ) -> String {
        if attrs.v_model.is_none() && self.ssr_attrs_use_rebuilt_element_attrs(attrs) {
            if let Some(rendered) = self.render_ssr_render_attrs_call(
                attrs,
                scope,
                self.root_attrs_props(Some(root_attrs)),
                Some(tag),
            ) {
                return rendered;
            }
        }
        let static_props = if root_attrs.attrs.is_some() {
            self.root_static_props(static_entries)
        } else {
            None
        };
        let dynamic_props = self
            .render_ordered_props(&attrs.props, scope)
            .map(|rendered| self.render_normalized_props(&attrs.props, rendered));
        let props = match (static_props, dynamic_props) {
            (Some(static_props), Some(dynamic_props)) => {
                Some(format!("_mergeProps({static_props}, {dynamic_props})"))
            }
            (Some(props), None) | (None, Some(props)) => Some(props),
            (None, None) => None,
        };
        let merged = self
            .merge_root_attrs_with_props(props, root_attrs)
            .unwrap_or_else(|| "{}".into());
        format!("_ssrRenderAttrs({merged})")
    }

    fn render_forced_ssr_attrs_expr(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        extra_props: Option<String>,
    ) -> Option<String> {
        let mut props = self
            .render_ordered_props(&attrs.props, scope)
            .map(|rendered| self.render_normalized_props(&attrs.props, rendered));
        if let Some(extra_props) = extra_props {
            props = Some(match props {
                Some(props) => format!("_mergeProps({props}, {extra_props})"),
                None => extra_props,
            });
        }
        props.map(|props| format!("_ssrRenderAttrs({props})"))
    }

    fn render_root_element_v_show_attrs_expr_with_static(
        &self,
        attrs: &Vue3SsrAttrs,
        root_attrs: &SsrRootAttrs,
        scope: &RenderScope,
        static_entries: &[(String, Option<String>)],
    ) -> String {
        let style = self.render_v_show_style_property_value(attrs.v_show.expect("v-show"), scope);
        let mut merge_args = self.render_root_v_show_props_args(
            &attrs.props,
            scope,
            root_attrs.attrs.is_some().then_some(static_entries),
        );
        if let Some(attrs) = &root_attrs.attrs {
            merge_args.push(attrs.clone());
        }
        if let Some(css_vars) = &root_attrs.css_vars {
            merge_args.push(css_vars.clone());
        }
        merge_args.push(render_object(&[format!("style: {style}")]));
        let merged = if merge_args.len() == 1 {
            merge_args.pop().unwrap_or_else(|| "{}".into())
        } else {
            format!("_mergeProps({})", merge_args.join(", "))
        };
        format!("_ssrRenderAttrs({merged})")
    }

    fn merge_root_attrs_with_props(
        &self,
        props: Option<String>,
        root_attrs: &SsrRootAttrs,
    ) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(props) = props {
            parts.push(props);
        }
        if let Some(attrs) = &root_attrs.attrs {
            parts.push(attrs.clone());
        }
        if let Some(css_vars) = &root_attrs.css_vars {
            parts.push(css_vars.clone());
        }
        match parts.len() {
            0 => None,
            1 => parts.into_iter().next(),
            _ => Some(format!("_mergeProps({})", parts.join(", "))),
        }
    }

    fn render_teleport(
        &self,
        node_id: NodeId,
        teleport: &Vue3SsrTeleport,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        writer.push_line("_ssrRenderTeleport(_push, (_push) => {");
        writer.indent();
        self.render_children(node_id, scope, writer);
        writer.dedent();
        writer.push_line(&format!(
            "}}, {}, {}, _parent)",
            self.render_mir_expr(&teleport.target, scope),
            self.render_mir_expr(&teleport.disabled, scope)
        ));
    }

    fn render_suspense(
        &self,
        suspense: &Vue3SsrSuspense,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        writer.push_line("_ssrRenderSuspense(_push, {");
        writer.indent();
        for slot in &suspense.slots.slots {
            self.render_suspense_slot(slot, scope, writer);
        }
        writer.push_line(&format!(
            "_: {}",
            vue3_slot_flag_with_comment(suspense.slots.flag)
        ));
        writer.dedent();
        writer.push_line("})");
    }

    fn render_suspense_slot(
        &self,
        slot: &vuec_ast::Vue3DomSlot,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        let child_scope = slot
            .params
            .map(|params| self.scope_with_pattern(scope, params))
            .unwrap_or_else(|| scope.clone());
        writer.push_line(&format!("{}: () => {{", json_key(&slot.name)));
        writer.indent();
        let root_attrs = self.ssr_css_vars_only_root_attrs();
        self.render_child_slice(
            &slot.children,
            &child_scope,
            None,
            root_attrs.as_ref(),
            writer,
        );
        writer.dedent();
        writer.push_line("},");
    }

    fn render_slot(
        &self,
        slot: &vuec_ast::Vue3SsrSlot,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        let props = self.render_slot_props(&slot.props, scope);
        let fallback = if slot.fallback.is_empty() {
            "null".to_string()
        } else {
            let mut fallback = CodeWriter::new();
            fallback.push_line("() => {");
            fallback.indent();
            let mut index = 0usize;
            while index < slot.fallback.len() {
                if let Some((html, next_index)) =
                    self.render_ssr_template_literal_slice(&slot.fallback, index, scope, None, None)
                {
                    fallback.push_line(&format!("_push({html})"));
                    index = next_index;
                    continue;
                }
                self.render_node(slot.fallback[index], scope, None, &mut fallback);
                index += 1;
            }
            fallback.dedent();
            fallback.push_line("}");
            fallback.finish().trim_end().to_string()
        };
        let scope_id = self.render_slot_scope_id_arg(slot, scope);
        let helper = if slot.inner {
            "_ssrRenderSlotInner"
        } else {
            "_ssrRenderSlot"
        };
        let mut args = vec![
            "_ctx.$slots".to_string(),
            self.render_slot_name(&slot.name, scope),
            props,
            fallback,
            "_push".into(),
            "_parent".into(),
        ];
        if let Some(scope_id) = scope_id {
            args.push(scope_id);
        }
        if slot.inner {
            if args.len() == 6 {
                args.push("null".into());
            }
            args.push("true".into());
        }
        writer.push_line(&format!("{}({})", helper, args.join(", ")));
    }

    fn render_slot_scope_id_arg(
        &self,
        slot: &vuec_ast::Vue3SsrSlot,
        scope: &RenderScope,
    ) -> Option<String> {
        if !self.options.slotted {
            return None;
        }
        let local_scope = scope
            .locals
            .iter()
            .any(|local| local == "_scopeId")
            .then_some("_scopeId");
        let scope_id = self
            .options
            .scope_id
            .as_ref()
            .map(|scope_id| format!("{}-s", scope_id));
        match (scope_id, local_scope) {
            (Some(scope_id), Some(local_scope)) => {
                Some(format!("{} + {}", quote_string(&scope_id), local_scope))
            }
            (Some(scope_id), None) => Some(quote_string(&scope_id)),
            (None, Some(local_scope)) if self.render_slot_as_vnode_fallback(slot) => {
                Some(local_scope.to_string())
            }
            (None, Some(local_scope)) if slot.inner => Some(local_scope.to_string()),
            _ => None,
        }
    }

    fn render_content(
        &self,
        content: &Vue3SsrContent,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        match content {
            Vue3SsrContent::Html { expression } => {
                writer.push_line(&format!(
                    "_push(({}) ?? '');",
                    self.render_js_expr(*expression, scope)
                ));
            }
            Vue3SsrContent::Text { expression } => {
                writer.push_line(&format!(
                    "_push(_ssrInterpolate({}));",
                    self.render_js_expr(*expression, scope)
                ));
            }
        }
    }

    fn render_attrs_with_root_attrs(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        if let Some(root_attrs) = root_attrs.filter(|root_attrs| root_attrs.attrs.is_none()) {
            let _ = (attrs, scope, root_attrs);
            return;
        }
        if attrs.v_show.is_some()
            && matches!(
                attrs.v_model.as_ref().map(|model| &model.kind),
                Some(Vue3SsrModelKind::InputDynamicProps)
            )
        {
            self.render_attrs_with_v_show_and_dynamic_model_props(attrs, scope, root_attrs, writer);
            return;
        }
        if attrs.v_show.is_some() {
            self.render_attrs_with_v_show(attrs, scope, root_attrs, writer);
            return;
        }
        if attrs.force_render_attrs {
            if let Some(rendered) =
                self.render_forced_ssr_attrs_expr(attrs, scope, self.root_attrs_props(root_attrs))
            {
                writer.push_line(&format!("_push({rendered});"));
            }
            return;
        }
        if matches!(
            attrs.v_model.as_ref().map(|model| &model.kind),
            Some(Vue3SsrModelKind::InputDynamicProps)
        ) {
            self.render_attrs_with_dynamic_model_props(attrs, scope, root_attrs, writer);
            return;
        }
        if let Some(root_attrs) = root_attrs {
            if matches!(
                attrs.v_model.as_ref().map(|model| &model.kind),
                Some(Vue3SsrModelKind::InputDynamicProps)
            ) {
                let rendered = format!(
                    "_ssrRenderAttrs({})",
                    self.render_dynamic_model_props_expr(attrs, scope, Some(root_attrs))
                );
                writer.push_line(&format!("_push({rendered});"));
                return;
            }
            let rendered =
                self.render_root_element_attrs_expr_with_static(attrs, root_attrs, scope, "", &[]);
            writer.push_line(&format!("_push({rendered});"));
            return;
        }
        self.render_attrs(attrs, scope, writer);
    }

    fn render_attrs(&self, attrs: &Vue3SsrAttrs, scope: &RenderScope, writer: &mut CodeWriter) {
        if attrs.v_show.is_some()
            && matches!(
                attrs.v_model.as_ref().map(|model| &model.kind),
                Some(Vue3SsrModelKind::InputDynamicProps)
            )
        {
            self.render_attrs_with_v_show_and_dynamic_model_props(attrs, scope, None, writer);
            return;
        }
        if attrs.v_show.is_some() {
            self.render_attrs_with_v_show(attrs, scope, None, writer);
            return;
        }
        if attrs.force_render_attrs {
            if let Some(rendered) = self.render_forced_ssr_attrs_expr(attrs, scope, None) {
                writer.push_line(&format!("_push({rendered});"));
            }
            return;
        }
        if matches!(
            attrs.v_model.as_ref().map(|model| &model.kind),
            Some(Vue3SsrModelKind::InputDynamicProps)
        ) {
            self.render_attrs_with_dynamic_model_props(attrs, scope, None, writer);
            return;
        }
        let props = &attrs.props;
        if props.segments.is_empty() {
            for binding in &props.dynamic_bindings {
                writer.push_line(&format!(
                    "_push({});",
                    self.render_ssr_binding(binding, scope)
                ));
            }
            for binding in &props.object_bindings {
                writer.push_line(&format!(
                    "_push(_ssrRenderAttrs({}));",
                    self.render_js_expr(binding.value, scope)
                ));
            }
            if let Some(model) = &attrs.v_model {
                self.render_v_model_attr(model, scope, writer);
            }
            return;
        }

        for segment in &props.segments {
            match segment {
                Vue3DomPropSegment::StaticAttr(_) => {}
                Vue3DomPropSegment::DynamicBinding(binding) => {
                    writer.push_line(&format!(
                        "_push({});",
                        self.render_ssr_binding(binding, scope)
                    ));
                }
                Vue3DomPropSegment::Content(_) => {}
                Vue3DomPropSegment::Model(_) => {}
                Vue3DomPropSegment::ObjectBinding(binding) => {
                    writer.push_line(&format!(
                        "_push(_ssrRenderAttrs({}));",
                        self.render_js_expr(binding.value, scope)
                    ));
                }
                Vue3DomPropSegment::Event(_) | Vue3DomPropSegment::ObjectListeners(_) => {}
            }
        }
        if let Some(model) = &attrs.v_model {
            self.render_v_model_attr(model, scope, writer);
        }
    }

    fn render_attrs_with_dynamic_model_props(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        let rendered = self.render_dynamic_model_props_expr(attrs, scope, root_attrs);
        writer.push_line(&format!("_push(_ssrRenderAttrs({rendered}));"));
    }

    fn render_dynamic_model_props_expr(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
    ) -> String {
        let Some(model) = &attrs.v_model else {
            return "{}".into();
        };
        let model_expr = self.render_js_expr(model.expression, scope);
        let props_expr = self.render_dynamic_model_props_base(&attrs.props, scope, root_attrs);
        format!(
            "(_temp0 = {props_expr}, _mergeProps(_temp0, _ssrGetDynamicModelProps(_temp0, {model_expr})))"
        )
    }

    fn render_dynamic_model_props_base(
        &self,
        props: &Vue3DomProps,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
    ) -> String {
        let ordered = if self.props_base_only_feed_dynamic_model_temp(props) {
            self.render_ordered_props_without_normalization(props, scope)
        } else {
            self.render_ordered_props(props, scope)
        };
        let rendered = ordered
            .map(|rendered| {
                if self.props_base_only_feed_dynamic_model_temp(props) {
                    rendered
                } else {
                    self.render_normalized_props(props, rendered)
                }
            })
            .unwrap_or_else(|| "{}".into());
        root_attrs
            .and_then(|root_attrs| {
                self.merge_root_attrs_with_props(Some(rendered.clone()), root_attrs)
            })
            .unwrap_or(rendered)
    }

    fn render_ordered_props_without_normalization(
        &self,
        props: &Vue3DomProps,
        scope: &RenderScope,
    ) -> Option<String> {
        let mut merge_args = Vec::new();
        let mut object_entries = Vec::new();
        if props.segments.is_empty() {
            for attr in &props.static_attrs {
                object_entries.push(self.render_static_attr(attr));
            }
            for binding in &props.dynamic_bindings {
                object_entries.push(self.render_dynamic_binding(binding, scope));
            }
            return self.render_plain_props(&object_entries);
        }
        for segment in &props.segments {
            match segment {
                Vue3DomPropSegment::StaticAttr(attr) => {
                    object_entries.push(self.render_static_attr(attr));
                }
                Vue3DomPropSegment::DynamicBinding(binding) => {
                    object_entries.push(
                        self.render_dynamic_model_base_binding_without_normalization(
                            binding, scope,
                        ),
                    );
                }
                Vue3DomPropSegment::ObjectBinding(binding) => {
                    self.push_merge_object_arg(&mut merge_args, &mut object_entries);
                    merge_args.push(self.render_js_expr(binding.value, scope));
                }
                Vue3DomPropSegment::Content(_)
                | Vue3DomPropSegment::Model(_)
                | Vue3DomPropSegment::Event(_)
                | Vue3DomPropSegment::ObjectListeners(_) => {}
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

    fn props_base_only_feed_dynamic_model_temp(&self, props: &Vue3DomProps) -> bool {
        props.object_listeners.is_empty() && props.events.is_empty()
    }

    fn render_dynamic_model_base_binding_without_normalization(
        &self,
        binding: &Vue3DomBinding,
        scope: &RenderScope,
    ) -> String {
        let value = self.render_js_expr(binding.value, scope);
        if binding.dynamic_arg {
            let key = binding
                .dynamic_name
                .map(|id| self.render_js_expr(id, scope))
                .unwrap_or_else(|| binding.name.clone());
            let key = if binding.camel {
                format!("_camelize({})", render_dynamic_prop_key(&key))
            } else {
                render_dynamic_prop_key(&key)
            };
            format!("[{key}]: {value}")
        } else if binding.name == "class" {
            format!("class: {}", value)
        } else if binding.name == "style" {
            format!("style: {}", value)
        } else {
            format!(
                "{}: {}",
                json_key(&render_vue3_dom_binding_static_key(binding, false)),
                value
            )
        }
    }

    fn render_attrs_with_v_show_and_dynamic_model_props(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        let Some(model) = &attrs.v_model else {
            return;
        };
        let style = self.render_v_show_style(&attrs.props, attrs.v_show.expect("v-show"), scope);
        let props_expr = self.render_v_show_merged_attrs(&attrs.props, &style, scope, root_attrs);
        let model_expr = self.render_js_expr(model.expression, scope);
        writer.push_line(&format!(
            "_push(_ssrRenderAttrs((_temp0 = {props_expr}, _mergeProps(_temp0, _ssrGetDynamicModelProps(_temp0, {model_expr})))));"
        ));
    }

    fn render_v_model_attr(
        &self,
        model: &Vue3SsrModel,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        let Some(rendered) = self.render_v_model_attr_expr(model, scope) else {
            return;
        };
        writer.push_line(&format!("_push({rendered});"));
    }

    fn render_v_model_attr_expr(
        &self,
        model: &Vue3SsrModel,
        scope: &RenderScope,
    ) -> Option<String> {
        let expression = self.render_js_expr(model.expression, scope);
        match &model.kind {
            Vue3SsrModelKind::InputValue => {
                Some(format!("_ssrRenderAttr(\"value\", {expression})"))
            }
            Vue3SsrModelKind::InputRadio { value } => Some(format!(
                "(_ssrIncludeBooleanAttr(_ssrLooseEqual({expression}, {}))) ? \" checked\" : \"\"",
                self.render_mir_expr(value, scope)
            )),
            Vue3SsrModelKind::InputCheckbox { value } => Some(format!(
                "(_ssrIncludeBooleanAttr((Array.isArray({expression}))\n  ? _ssrLooseContain({expression}, {})\n  : {expression})) ? \" checked\" : \"\"",
                self.render_mir_expr(value, scope)
            )),
            Vue3SsrModelKind::InputCheckboxTrueValue { true_value } => Some(format!(
                "(_ssrIncludeBooleanAttr(_ssrLooseEqual({expression}, {}))) ? \" checked\" : \"\"",
                self.render_mir_expr(true_value, scope)
            )),
            Vue3SsrModelKind::InputDynamicType { type_expr, value } => Some(format!(
                "_ssrRenderDynamicModel({}, {expression}, {})",
                self.render_js_expr(*type_expr, scope),
                self.render_mir_expr(value, scope)
            )),
            Vue3SsrModelKind::SelectOption { value } => {
                let value = self.render_mir_expr(value, scope);
                Some(format!(
                    "(_ssrIncludeBooleanAttr((Array.isArray({expression}))\n  ? _ssrLooseContain({expression}, {value})\n  : _ssrLooseEqual({expression}, {value}))) ? \" selected\" : \"\""
                ))
            }
            Vue3SsrModelKind::InputDynamicProps | Vue3SsrModelKind::Textarea => None,
        }
    }

    fn render_attrs_with_v_show(
        &self,
        attrs: &Vue3SsrAttrs,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        let style = self.render_v_show_style(&attrs.props, attrs.v_show.expect("v-show"), scope);
        if ssr_attrs_has_object_binding(&attrs.props) || root_attrs.is_some() {
            let attrs_expr =
                self.render_v_show_merged_attrs(&attrs.props, &style, scope, root_attrs);
            writer.push_line(&format!("_push(_ssrRenderAttrs({attrs_expr}));"));
            return;
        }

        if attrs.props.segments.is_empty() {
            for binding in &attrs.props.dynamic_bindings {
                if binding.name != "style" {
                    writer.push_line(&format!(
                        "_push({});",
                        self.render_ssr_binding(binding, scope)
                    ));
                }
            }
        } else {
            for segment in &attrs.props.segments {
                if let Vue3DomPropSegment::DynamicBinding(binding) = segment {
                    if binding.name != "style" {
                        writer.push_line(&format!(
                            "_push({});",
                            self.render_ssr_binding(binding, scope)
                        ));
                    }
                }
            }
        }
        writer.push_line(&format!("_push({});", self.render_ssr_style_attr(&style)));
        if let Some(model) = &attrs.v_model {
            self.render_v_model_attr(model, scope, writer);
        }
    }

    fn render_v_show_merged_attrs(
        &self,
        props: &Vue3DomProps,
        style: &str,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
    ) -> String {
        let mut merge_args = Vec::new();
        let mut object_entries = Vec::new();
        if props.segments.is_empty() {
            for binding in &props.dynamic_bindings {
                if binding.name != "style" {
                    object_entries.push(self.render_ssr_object_binding(binding, scope));
                }
            }
            self.push_merge_object_arg(&mut merge_args, &mut object_entries);
            for binding in &props.object_bindings {
                merge_args.push(self.render_js_expr(binding.value, scope));
            }
        } else {
            for segment in &props.segments {
                match segment {
                    Vue3DomPropSegment::DynamicBinding(binding) if binding.name != "style" => {
                        object_entries.push(self.render_ssr_object_binding(binding, scope));
                    }
                    Vue3DomPropSegment::Content(_) => {}
                    Vue3DomPropSegment::Model(_) => {}
                    Vue3DomPropSegment::ObjectBinding(binding) => {
                        self.push_merge_object_arg(&mut merge_args, &mut object_entries);
                        merge_args.push(self.render_js_expr(binding.value, scope));
                    }
                    Vue3DomPropSegment::StaticAttr(_)
                    | Vue3DomPropSegment::DynamicBinding(_)
                    | Vue3DomPropSegment::Event(_)
                    | Vue3DomPropSegment::ObjectListeners(_) => {}
                }
            }
            self.push_merge_object_arg(&mut merge_args, &mut object_entries);
        }
        merge_args.push(render_object(&[format!("style: {style}")]));
        if let Some(root_attrs) = root_attrs {
            if let Some(attrs) = &root_attrs.attrs {
                merge_args.push(attrs.clone());
            }
            if let Some(css_vars) = &root_attrs.css_vars {
                merge_args.push(css_vars.clone());
            }
        }
        if merge_args.len() == 1 {
            merge_args.pop().unwrap_or_else(|| "{}".into())
        } else {
            format!("_mergeProps({})", merge_args.join(", "))
        }
    }

    fn render_root_v_show_props_args(
        &self,
        props: &Vue3DomProps,
        scope: &RenderScope,
        static_entries: Option<&[(String, Option<String>)]>,
    ) -> Vec<String> {
        let static_entries = static_entries
            .map(|entries| {
                self.root_static_merge_entries(entries)
                    .into_iter()
                    .filter(|(name, _)| name.as_str() != "style")
                    .map(|(name, value)| {
                        (
                            name.clone(),
                            value
                                .as_ref()
                                .map(|value| decode_vue3_ssr_escaped_attr(value)),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut merge_args = Vec::new();
        let mut object_entries = Vec::new();
        let style_parts = self.collect_v_show_existing_style_parts(props, scope);
        if props.segments.is_empty() {
            for (name, value) in static_entries {
                object_entries.push(Self::render_ssr_static_prop_entry(&name, value.as_deref()));
            }
            if !style_parts.is_empty() {
                self.push_v_show_style_merge_arg(
                    &mut merge_args,
                    &mut object_entries,
                    &style_parts,
                );
            }
            for binding in &props.dynamic_bindings {
                if binding.name != "style" {
                    object_entries.push(self.render_ssr_object_binding(binding, scope));
                }
            }
            self.push_merge_object_arg(&mut merge_args, &mut object_entries);
            for binding in &props.object_bindings {
                merge_args.push(self.render_js_expr(binding.value, scope));
            }
            return merge_args;
        }
        let mut style_pushed = false;
        for segment in &props.segments {
            match segment {
                Vue3DomPropSegment::StaticAttr(attr) if attr.name == "style" => {
                    if !style_pushed {
                        self.push_v_show_style_merge_arg(
                            &mut merge_args,
                            &mut object_entries,
                            &style_parts,
                        );
                        style_pushed = true;
                    }
                }
                Vue3DomPropSegment::StaticAttr(attr) => {
                    object_entries.push(Self::render_ssr_static_prop_entry(
                        &attr.name,
                        Some(&attr.value),
                    ));
                }
                Vue3DomPropSegment::DynamicBinding(binding) if binding.name == "style" => {
                    if !style_pushed {
                        self.push_v_show_style_merge_arg(
                            &mut merge_args,
                            &mut object_entries,
                            &style_parts,
                        );
                        style_pushed = true;
                    }
                }
                Vue3DomPropSegment::DynamicBinding(binding) => {
                    object_entries.push(self.render_ssr_object_binding(binding, scope));
                }
                Vue3DomPropSegment::ObjectBinding(binding) => {
                    self.push_merge_object_arg(&mut merge_args, &mut object_entries);
                    merge_args.push(self.render_js_expr(binding.value, scope));
                }
                Vue3DomPropSegment::Content(_)
                | Vue3DomPropSegment::Model(_)
                | Vue3DomPropSegment::Event(_)
                | Vue3DomPropSegment::ObjectListeners(_) => {}
            }
        }
        self.push_merge_object_arg(&mut merge_args, &mut object_entries);
        merge_args
    }

    fn push_v_show_style_merge_arg(
        &self,
        merge_args: &mut Vec<String>,
        object_entries: &mut Vec<String>,
        style_parts: &[String],
    ) {
        if style_parts.is_empty() {
            return;
        }
        let style = format!("style: {}", self.render_style_parts(style_parts));
        if style.contains('\n') && object_entries.is_empty() {
            merge_args.push(render_object(&[style]));
        } else {
            object_entries.push(style);
        }
    }

    fn render_style_parts(&self, parts: &[String]) -> String {
        match parts {
            [] => "null".into(),
            [single] => single.clone(),
            _ => render_array(parts),
        }
    }

    fn render_ssr_static_prop_entry(name: &str, value: Option<&str>) -> String {
        if name == "style" {
            format!(
                "style: {}",
                vue3_static_style_object_expr(value.unwrap_or_default())
            )
        } else {
            format!(
                "{}: {}",
                json_key(name),
                quote_string(value.unwrap_or_default())
            )
        }
    }

    fn render_ssr_object_binding(&self, binding: &Vue3DomBinding, scope: &RenderScope) -> String {
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
        } else {
            format!(
                "{}: {}",
                json_key(&render_vue3_dom_binding_static_key(binding, true)),
                value
            )
        }
    }

    fn render_v_show_style(
        &self,
        props: &Vue3DomProps,
        v_show: JsExprId,
        scope: &RenderScope,
    ) -> String {
        let parts = self.collect_v_show_style_parts(props, v_show, scope);
        if parts.len() == 1 {
            parts.into_iter().next().unwrap_or_else(|| "null".into())
        } else {
            render_array(&parts)
        }
    }

    fn render_v_show_style_property_value(&self, v_show: JsExprId, scope: &RenderScope) -> String {
        format!(
            "({}) ? null : {{ display: \"none\" }}",
            self.render_js_expr(v_show, scope)
        )
    }

    fn collect_v_show_style_parts(
        &self,
        props: &Vue3DomProps,
        v_show: JsExprId,
        scope: &RenderScope,
    ) -> Vec<String> {
        let mut parts = Vec::new();
        if props.segments.is_empty() {
            for attr in &props.static_attrs {
                if attr.name == "style" {
                    parts.push(vue3_static_style_object_expr(&attr.value));
                }
            }
            for binding in &props.dynamic_bindings {
                if binding.name == "style" {
                    parts.push(self.render_js_expr(binding.value, scope));
                }
            }
        } else {
            for segment in &props.segments {
                match segment {
                    Vue3DomPropSegment::StaticAttr(attr) if attr.name == "style" => {
                        parts.push(vue3_static_style_object_expr(&attr.value));
                    }
                    Vue3DomPropSegment::DynamicBinding(binding) if binding.name == "style" => {
                        parts.push(self.render_js_expr(binding.value, scope));
                    }
                    Vue3DomPropSegment::StaticAttr(_)
                    | Vue3DomPropSegment::Content(_)
                    | Vue3DomPropSegment::Model(_)
                    | Vue3DomPropSegment::DynamicBinding(_)
                    | Vue3DomPropSegment::Event(_)
                    | Vue3DomPropSegment::ObjectBinding(_)
                    | Vue3DomPropSegment::ObjectListeners(_) => {}
                }
            }
        }
        parts.push(self.render_v_show_style_property_value(v_show, scope));
        parts
    }

    fn collect_v_show_existing_style_parts(
        &self,
        props: &Vue3DomProps,
        scope: &RenderScope,
    ) -> Vec<String> {
        let mut parts = Vec::new();
        if props.segments.is_empty() {
            for attr in &props.static_attrs {
                if attr.name == "style" {
                    parts.push(vue3_static_style_object_expr(&attr.value));
                }
            }
            for binding in &props.dynamic_bindings {
                if binding.name == "style" {
                    parts.push(self.render_js_expr(binding.value, scope));
                }
            }
            return parts;
        }
        for segment in &props.segments {
            match segment {
                Vue3DomPropSegment::StaticAttr(attr) if attr.name == "style" => {
                    parts.push(vue3_static_style_object_expr(&attr.value));
                }
                Vue3DomPropSegment::DynamicBinding(binding) if binding.name == "style" => {
                    parts.push(self.render_js_expr(binding.value, scope));
                }
                Vue3DomPropSegment::StaticAttr(_)
                | Vue3DomPropSegment::Content(_)
                | Vue3DomPropSegment::Model(_)
                | Vue3DomPropSegment::DynamicBinding(_)
                | Vue3DomPropSegment::Event(_)
                | Vue3DomPropSegment::ObjectBinding(_)
                | Vue3DomPropSegment::ObjectListeners(_) => {}
            }
        }
        parts
    }

    fn render_ssr_style_attr(&self, style: &str) -> String {
        format!("` style=\"${{_ssrRenderStyle({style})}}\"`")
    }

    fn render_ssr_binding(&self, binding: &Vue3DomBinding, scope: &RenderScope) -> String {
        let value = self.render_js_expr(binding.value, scope);
        if binding.dynamic_arg {
            let name = render_vue3_dom_binding_dynamic_key(
                binding,
                binding
                    .dynamic_name
                    .map(|id| self.render_js_expr(id, scope))
                    .unwrap_or_else(|| binding.name.clone()),
                false,
            );
            format!("_ssrRenderDynamicAttr({name}, {value})")
        } else if binding.name == "class" {
            format!("` class=\"${{_ssrRenderClass({value})}}\"`")
        } else if binding.name == "style" {
            format!("` style=\"${{_ssrRenderStyle({value})}}\"`")
        } else {
            format!(
                "_ssrRenderAttr({}, {value})",
                quote_string(&render_vue3_dom_binding_static_key(binding, false))
            )
        }
    }

    fn render_if(
        &self,
        node_id: NodeId,
        condition: Option<JsExprId>,
        comment: bool,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        let Some(condition) = condition else {
            self.render_if_branch_children(node_id, scope, root_attrs, writer);
            return;
        };
        writer.push_line(&format!(
            "if ({}) {{",
            self.render_js_expr(condition, scope)
        ));
        writer.indent();
        let alternate = self.render_if_branch_children(node_id, scope, root_attrs, writer);
        writer.dedent();
        if let Some(alternate) = alternate {
            self.render_if_alternate(alternate, scope, root_attrs, writer);
        } else if comment {
            writer.push_line("} else {");
            writer.indent();
            writer.push_line("_push(`<!---->`)");
            writer.dedent();
            writer.push_line("}");
        } else {
            writer.push_line("}");
        }
    }

    fn render_if_branch_children(
        &self,
        node_id: NodeId,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<NodeId> {
        let Some(node) = self.mir.node(node_id) else {
            return None;
        };
        let (primary_children, alternate) = self.split_if_children(&node.children);
        let branch_root_attrs = self.root_attrs_for_branch_children(&primary_children, root_attrs);
        let scope_id = scope
            .locals
            .iter()
            .any(|local| local == "_scopeId")
            .then_some("_scopeId");
        self.render_child_slice(
            &primary_children,
            scope,
            scope_id,
            branch_root_attrs.as_ref(),
            writer,
        );
        alternate
    }

    fn render_if_alternate(
        &self,
        alternate: NodeId,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        let Some(node) = self.mir.node(alternate) else {
            writer.push_line("} else {");
            writer.indent();
            writer.push_line("_push(`<!---->`)");
            writer.dedent();
            writer.push_line("}");
            return;
        };
        match node.kind {
            Vue3SsrMirKind::If {
                condition: Some(condition),
                comment,
            } => {
                writer.push_line(&format!(
                    "}} else if ({}) {{",
                    self.render_js_expr(condition, scope)
                ));
                writer.indent();
                let nested_alternate =
                    self.render_if_branch_children(alternate, scope, root_attrs, writer);
                writer.dedent();
                if let Some(nested_alternate) = nested_alternate {
                    self.render_if_alternate(nested_alternate, scope, root_attrs, writer);
                } else if comment {
                    writer.push_line("} else {");
                    writer.indent();
                    writer.push_line("_push(`<!---->`)");
                    writer.dedent();
                    writer.push_line("}");
                } else {
                    writer.push_line("}");
                }
            }
            Vue3SsrMirKind::If {
                condition: None, ..
            } => {
                writer.push_line("} else {");
                writer.indent();
                self.render_if_branch_children(alternate, scope, root_attrs, writer);
                writer.dedent();
                writer.push_line("}");
            }
            _ => {
                writer.push_line("} else {");
                writer.indent();
                self.render_node(alternate, scope, root_attrs, writer);
                writer.dedent();
                writer.push_line("}");
            }
        }
    }

    fn split_if_children(&self, children: &[NodeId]) -> (Vec<NodeId>, Option<NodeId>) {
        let mut alternate = None;
        let mut primary_children = Vec::new();
        for child_id in children {
            if matches!(
                self.mir.node(*child_id).map(|child| &child.kind),
                Some(Vue3SsrMirKind::If { .. })
            ) && alternate.is_none()
            {
                alternate = Some(*child_id);
                continue;
            }
            primary_children.push(*child_id);
        }
        (primary_children, alternate)
    }

    fn render_for(
        &self,
        node_id: NodeId,
        for_mir: &Vue3SsrFor,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        if for_mir.fragment {
            writer.push_line("_push(`<!--[-->`)");
        }
        self.render_for_list_with_optional_fragment(node_id, for_mir, scope, writer);
        if for_mir.fragment {
            writer.push_line("_push(`<!--]-->`)");
        }
    }

    fn render_for_list_with_optional_fragment(
        &self,
        node_id: NodeId,
        for_mir: &Vue3SsrFor,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        let Some(node) = self.mir.node(node_id) else {
            return;
        };
        if matches!(
            node.children.as_slice(),
            [child]
                if matches!(
                    self.mir.node(*child).map(|node| &node.kind),
                    Some(Vue3SsrMirKind::If { .. })
                )
        ) {
            self.render_for_list_with_fragment(node_id, for_mir, scope, writer);
        } else {
            self.render_for_list(node_id, for_mir, scope, writer);
        }
    }

    fn render_for_list_with_fragment(
        &self,
        node_id: NodeId,
        for_mir: &Vue3SsrFor,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        let source = self.render_js_expr(for_mir.source, scope);
        let params = self.render_for_params(for_mir);
        let child_scope = self.scope_with_for_mir(scope, for_mir);
        writer.push_line(&format!("_ssrRenderList({source}, ({params}) => {{"));
        writer.indent();
        writer.push_line("_push(`<!--[-->`)");
        let children = self
            .mir
            .node(node_id)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        let scope_id = scope
            .locals
            .iter()
            .any(|local| local == "_scopeId")
            .then_some("_scopeId");
        self.render_child_slice(&children, &child_scope, scope_id, None, writer);
        writer.push_line("_push(`<!--]-->`)");
        writer.dedent();
        writer.push_line("})");
    }

    fn render_for_list(
        &self,
        node_id: NodeId,
        for_mir: &Vue3SsrFor,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        let source = self.render_js_expr(for_mir.source, scope);
        let params = self.render_for_params(for_mir);
        let child_scope = self.scope_with_for_mir(scope, for_mir);
        writer.push_line(&format!("_ssrRenderList({source}, ({params}) => {{"));
        writer.indent();
        let children = self
            .mir
            .node(node_id)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        let scope_id = scope
            .locals
            .iter()
            .any(|local| local == "_scopeId")
            .then_some("_scopeId");
        self.render_child_slice(&children, &child_scope, scope_id, None, writer);
        writer.dedent();
        writer.push_line("})");
    }

    fn render_for_params(&self, for_mir: &Vue3SsrFor) -> String {
        let mut params = vec![self.render_js_pattern(for_mir.value_alias)];
        if let Some(key) = for_mir.key_alias {
            params.push(self.render_js_pattern(key));
        }
        if let Some(index) = for_mir.index_alias {
            params.push(self.render_js_pattern(index));
        }
        params.join(", ")
    }

    fn render_slot_name(&self, name: &Vue3DomSlotName, scope: &RenderScope) -> String {
        match name {
            Vue3DomSlotName::Static(name) => quote_string(name),
            Vue3DomSlotName::Dynamic(name) => self.render_js_expr(*name, scope),
        }
    }

    fn render_slot_props(&self, props: &Vue3DomProps, scope: &RenderScope) -> String {
        let rendered = self
            .render_ordered_props(props, scope)
            .unwrap_or_else(|| "{}".into());
        self.render_normalized_props(props, rendered)
    }

    fn render_ordered_props(&self, props: &Vue3DomProps, scope: &RenderScope) -> Option<String> {
        if props.segments.is_empty() {
            let mut entries = Vec::new();
            for attr in &props.static_attrs {
                entries.push(self.render_static_attr(attr));
            }
            for binding in &props.dynamic_bindings {
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
                    object_entries.push(self.render_dynamic_binding(binding, scope));
                }
                Vue3DomPropSegment::Content(_) => {}
                Vue3DomPropSegment::Model(_) => {}
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

    fn render_event(&self, event: &Vue3DomEvent, scope: &RenderScope) -> String {
        let handler = self.render_guarded_event_handler(event, scope);
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

    fn render_mir_expr(&self, expr: &MirExpr, scope: &RenderScope) -> String {
        match expr {
            MirExpr::String(value) => quote_string(value),
            MirExpr::Bool(value) => value.to_string(),
            MirExpr::Null => "null".into(),
            MirExpr::JsExpr(expr) => self.render_js_expr(*expr, scope),
            MirExpr::Helper(helper) => helper_reference(*helper),
        }
    }

    fn render_js_expr(&self, id: JsExprId, scope: &RenderScope) -> String {
        self.js
            .expressions()
            .get(id.0 as usize)
            .map(|entry| rewrite_expression_with_scope(&entry.source, self.options, scope))
            .unwrap_or_else(|| "undefined".into())
    }

    fn render_js_stmt(&self, id: vuec_ast::JsStmtId, scope: &RenderScope) -> String {
        self.js
            .statements()
            .get(id.0 as usize)
            .map(|entry| rewrite_handler_expression_with_scope(&entry.source, self.options, scope))
            .unwrap_or_else(|| "undefined".into())
    }

    fn render_js_pattern(&self, id: JsPatternId) -> String {
        self.js
            .patterns()
            .get(id.0 as usize)
            .map(|entry| entry.source.to_string())
            .unwrap_or_else(|| "_item".into())
    }

    fn scope_with_pattern(&self, scope: &RenderScope, pattern: JsPatternId) -> RenderScope {
        scope.with_locals(extract_v_for_alias_locals(&self.render_js_pattern(pattern)))
    }

    fn scope_with_for_mir(&self, scope: &RenderScope, for_mir: &Vue3SsrFor) -> RenderScope {
        let mut locals = extract_v_for_alias_locals(&self.render_js_pattern(for_mir.value_alias));
        if let Some(key) = for_mir.key_alias {
            locals.extend(extract_v_for_alias_locals(&self.render_js_pattern(key)));
        }
        if let Some(index) = for_mir.index_alias {
            locals.extend(extract_v_for_alias_locals(&self.render_js_pattern(index)));
        }
        scope.with_locals(locals)
    }

    fn render_dom_for_slot_params(&self, slot: &vuec_ast::Vue3DomForSlot) -> String {
        let mut params = vec![self.render_js_pattern(slot.value_alias)];
        if let Some(key) = slot.key_alias {
            params.push(self.render_js_pattern(key));
        }
        if let Some(index) = slot.index_alias {
            if slot.key_alias.is_none() {
                params.push("_".into());
            }
            params.push(self.render_js_pattern(index));
        }
        params.join(", ")
    }

    fn scope_with_dom_for_slot(
        &self,
        scope: &RenderScope,
        slot: &vuec_ast::Vue3DomForSlot,
    ) -> RenderScope {
        let mut locals = extract_v_for_alias_locals(&self.render_js_pattern(slot.value_alias));
        if let Some(key) = slot.key_alias {
            locals.extend(extract_v_for_alias_locals(&self.render_js_pattern(key)));
        }
        if let Some(index) = slot.index_alias {
            locals.extend(extract_v_for_alias_locals(&self.render_js_pattern(index)));
        }
        scope.with_locals(locals)
    }
}

pub(crate) fn vue3_ssr_helper_order() -> &'static [RuntimeHelper] {
    &[
        RuntimeHelper::Vue3SsrRenderClass,
        RuntimeHelper::Vue3SsrRenderStyle,
        RuntimeHelper::Vue3SsrInterpolate,
        RuntimeHelper::Vue3SsrRenderAttr,
        RuntimeHelper::Vue3SsrRenderDynamicAttr,
        RuntimeHelper::Vue3SsrIncludeBooleanAttr,
        RuntimeHelper::Vue3SsrLooseContain,
        RuntimeHelper::Vue3SsrLooseEqual,
        RuntimeHelper::Vue3SsrRenderDynamicModel,
        RuntimeHelper::Vue3SsrRenderAttrs,
        RuntimeHelper::Vue3SsrGetDynamicModelProps,
        RuntimeHelper::Vue3SsrGetDirectiveProps,
        RuntimeHelper::Vue3SsrRenderVNode,
        RuntimeHelper::Vue3SsrRenderComponent,
        RuntimeHelper::Vue3SsrRenderSlot,
        RuntimeHelper::Vue3SsrRenderList,
        RuntimeHelper::Vue3SsrRenderTeleport,
        RuntimeHelper::Vue3SsrRenderSuspense,
    ]
}

pub(crate) enum SsrTemplatePart {
    Static(String),
    Expr(String),
}

pub(crate) fn render_ssr_template_literal(parts: &[SsrTemplatePart]) -> String {
    let parts = merge_adjacent_ssr_template_static_parts(parts);
    let mut output = String::from("`");
    let multiline_exprs = parts.len() > 3;
    for part in &parts {
        match part {
            SsrTemplatePart::Static(value) => {
                output.push_str(&escape_template_literal_static(value));
            }
            SsrTemplatePart::Expr(value) => {
                if multiline_exprs {
                    output.push_str("${\n");
                    output.push_str(&indent_lines(value, 2));
                    output.push_str("\n}");
                } else {
                    output.push_str("${");
                    output.push_str(value);
                    output.push('}');
                }
            }
        }
    }
    output.push('`');
    output
}

pub(crate) fn merge_adjacent_ssr_template_static_parts(
    parts: &[SsrTemplatePart],
) -> Vec<SsrTemplatePart> {
    let mut merged = Vec::new();
    for part in parts {
        match (merged.last_mut(), part) {
            (Some(SsrTemplatePart::Static(previous)), SsrTemplatePart::Static(value)) => {
                previous.push_str(value);
            }
            (_, SsrTemplatePart::Static(value)) => {
                merged.push(SsrTemplatePart::Static(value.clone()));
            }
            (_, SsrTemplatePart::Expr(value)) => {
                merged.push(SsrTemplatePart::Expr(value.clone()));
            }
        }
    }
    merged
}

pub(crate) fn append_static_to_ssr_template_literal(mut literal: String, value: &str) -> String {
    if literal.pop() == Some('`') {
        literal.push_str(&escape_template_literal_static(value));
        literal.push('`');
        literal
    } else {
        literal
    }
}

pub(crate) fn escape_template_literal_static(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('$', "\\$")
}

pub(crate) fn parse_ssr_open_tag_start(
    value: &str,
) -> Option<(String, Vec<(String, Option<String>)>)> {
    let rest = value.strip_prefix('<')?;
    let tag_end = rest
        .char_indices()
        .find_map(|(index, ch)| (is_vue3_html_whitespace(ch) || ch == '>').then_some(index))
        .unwrap_or(rest.len());
    let tag = rest.get(..tag_end)?.to_string();
    if tag.is_empty() || tag.starts_with('/') || tag.starts_with('!') {
        return None;
    }
    let mut attrs = Vec::new();
    let mut input = rest.get(tag_end..).unwrap_or("").trim_start();
    while !input.is_empty() {
        let name_end = input
            .char_indices()
            .find_map(|(index, ch)| (is_vue3_html_whitespace(ch) || ch == '=').then_some(index))
            .unwrap_or(input.len());
        let name = input.get(..name_end)?.to_string();
        if name.is_empty() {
            break;
        }
        input = input.get(name_end..).unwrap_or("").trim_start();
        if let Some(after_eq) = input.strip_prefix('=') {
            input = after_eq.trim_start();
            if let Some(after_quote) = input.strip_prefix('"') {
                if let Some(end_quote) = after_quote.find('"') {
                    let value = after_quote.get(..end_quote)?.to_string();
                    attrs.push((name, Some(value)));
                    input = after_quote.get(end_quote + 1..).unwrap_or("").trim_start();
                    continue;
                }
            }
            attrs.push((name, Some(String::new())));
            break;
        }
        attrs.push((name, None));
    }
    Some((tag, attrs))
}
