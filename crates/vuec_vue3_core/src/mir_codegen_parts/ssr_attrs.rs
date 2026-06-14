impl<'a> Vue3SsrMirCodegen<'a> {
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

}
