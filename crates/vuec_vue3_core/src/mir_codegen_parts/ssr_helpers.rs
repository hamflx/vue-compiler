impl<'a> Vue3SsrMirCodegen<'a> {
    fn vue_helpers(&self) -> Vec<RuntimeHelper> {
        let mut helpers = Vec::new();
        let root_children = self.root_children();
        let root_attrs = self.root_attrs_for_children(root_children);
        let root_attr_node = root_attrs
            .as_ref()
            .and_then(|root_attrs| self.root_attr_node_for_children(root_children, root_attrs));
        if let Some(root_attrs) = &root_attrs {
            if self.root_attrs_need_merge_props(self.root_children(), root_attrs) {
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
                Vue3SsrMirKind::RenderSlot(slot)
                    if self.render_slot_as_vnode_fallback(slot) =>
                {
                    push_unique_helper(helpers, RuntimeHelper::Vue3RenderSlot);
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

}
