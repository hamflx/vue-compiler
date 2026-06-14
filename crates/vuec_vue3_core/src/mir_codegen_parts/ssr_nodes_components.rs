impl<'a> Vue3SsrMirCodegen<'a> {
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

}
