impl<'a> Vue3SsrMirCodegen<'a> {
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

}
