impl<'a> Vue3SsrMirCodegen<'a> {
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

}
