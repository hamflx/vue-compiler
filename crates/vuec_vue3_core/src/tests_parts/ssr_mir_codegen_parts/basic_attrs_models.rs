    #[test]
    fn generate_vue3_ssr_mir_emits_basic_pushes_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div>Hello {{ msg }}</div>"#.into(),
            file_id: FileId(43),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert_eq!(generated.ast_summary, "vue3-ssr-mir-nodes=6");
        assert!(generated.code.contains("ssrInterpolate as _ssrInterpolate"));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated
            .code
            .contains("export function ssrRender(_ctx, _push, _parent, _attrs)"));
        assert!(generated.code.contains("<div${"));
        assert!(generated.code.contains("_ssrRenderAttrs(_attrs)"));
        assert!(generated.code.contains("}>Hello ${"));
        assert!(generated.code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(generated.code.contains("}</div>`)"));
        assert!(!generated.code.contains("Vue3Ast"));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_root_text_and_interpolation_slices() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"foo {{ bar }} baz"#.into(),
            file_id: FileId(43),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrInterpolate: _ssrInterpolate"));
        assert!(generated.code.contains("_push(`foo ${"));
        assert!(generated.code.contains("_ssrInterpolate(_ctx.bar)"));
        assert!(generated.code.contains("} baz`)"));
        assert!(generated
            .code
            .contains(r#"_push(`foo ${_ssrInterpolate(_ctx.bar)} baz`)"#));
        assert!(!generated.code.contains("<!--[-->"));
        assert!(!generated.code.contains("_push(\"foo \")"));
        assert!(!generated.code.contains("_push(_ssrInterpolate(_ctx.bar));"));
    }

    #[test]
    fn generate_vue3_ssr_mir_escapes_text_for_static_html_and_template_literals() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div>&lt;foo&gt;\$bar</div>"#.into(),
            file_id: FileId(43),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("&lt;foo&gt;\\\\\\$bar</div>`"));
        assert!(generated.code.contains("_ssrRenderAttrs(_attrs)"));
        assert!(!generated.code.contains("<foo>"));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_static_root_fragments() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"foo<!--x--><div></div><span>bar</span>"#.into(),
            file_id: FileId(43),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("_push(`<!--[-->foo<!--x--><div></div><span>bar</span><!--]-->`)"));
        assert!(!generated.code.contains("_push(\"foo\")"));
        assert!(!generated.code.contains("_push(\"<!--x-->\")"));
        assert!(!generated.code.contains("_push(\"<div\")"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_asset_imports_from_mir_root() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<img :src="_imports_0">"#.into(),
                file_id: FileId(73),
                base_offset: 0,
            },
            &Vue3CompilerOptions::default(),
        );
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
            }
        }

        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(generated
            .code
            .contains("_ssrRenderAttrs(_mergeProps({ src: _imports_0 }, _attrs))"));
        assert!(!generated.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_attrs_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div id="app" :class="klass" :style="style" :title="title" :[name]="value" v-bind="extra">Hi</div>"#.into(),
            file_id: FileId(46),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let attrs = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            })
            .expect("ssr attrs");
        assert_eq!(attrs.props.dynamic_bindings.len(), 4);
        assert_eq!(attrs.props.object_bindings.len(), 1);
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["klass", "style", "title", "name", "value", "extra"]
        );
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(!generated.code.contains("normalizeClass as _normalizeClass"));
        assert!(!generated.code.contains("normalizeProps as _normalizeProps"));
        assert!(!generated
            .code
            .contains("guardReactiveProps as _guardReactiveProps"));
        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated.code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(generated.code.contains("id: \"app\""));
        assert!(generated.code.contains("class: _ctx.klass"));
        assert!(generated.code.contains("style: _ctx.style"));
        assert!(generated.code.contains("title: _ctx.title"));
        assert!(generated.code.contains("[_ctx.name || \"\"]: _ctx.value"));
        assert!(generated.code.contains("_ctx.extra"));
        assert!(generated.code.contains("_attrs"));
        assert!(generated.code.contains(">Hi</div>`)"));
    }

    #[test]
    fn generate_vue3_ssr_mir_ignores_dom_bind_prefix_modifiers() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div :foo-bar.camel="foo" :id.prop="id" :role.attr="role" :[name].camel="value">Hi</div>"#.into(),
            file_id: FileId(70),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("camelize as _camelize"));
        assert!(!generated.code.contains("normalizeProps as _normalizeProps"));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated.code.contains("fooBar: _ctx.foo"));
        assert!(generated.code.contains("\".id\": _ctx.id"));
        assert!(generated.code.contains("\"^role\": _ctx.role"));
        assert!(generated
            .code
            .contains("[_camelize(_ctx.name || \"\")]: _ctx.value"));
        assert!(generated.code.contains("_attrs"));
    }

    #[test]
    fn generate_vue3_ssr_mir_rebuilds_element_attrs_like_public_ssr() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><div key="1" ref="el"></div><div class="foo" :class="bar"></div><input type="checkbox" :checked="checked"><div class="foo" v-bind:[key]="value"></div></div>"#.into(),
            file_id: FileId(74),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(!generated.code.contains(" key="));
        assert!(!generated.code.contains(" ref="));
        assert!(generated
            .code
            .contains(r#"_ssrRenderClass([_ctx.bar, "foo"])"#));
        assert!(generated
            .code
            .contains(r#"(_ssrIncludeBooleanAttr(_ctx.checked)) ? " checked" : """#));
        assert!(generated.code.contains("class: \"foo\""));
        assert!(generated.code.contains("[_ctx.key || \"\"]: _ctx.value"));
        assert!(!generated.code.contains("<div class=\"foo\"${"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_directive_attrs_and_textarea_value_fallback() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source:
                r#"<div><textarea v-bind="obj">fallback</textarea><section v-xxx:x.y="z"/></div>"#
                    .into(),
            file_id: FileId(75),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("resolveDirective: _resolveDirective"));
        assert!(generated
            .code
            .contains("ssrGetDirectiveProps: _ssrGetDirectiveProps"));
        assert!(generated.code.contains("let _temp0"));
        assert!(generated
            .code
            .contains(r#"_ssrRenderAttrs(_temp0 = _ctx.obj, "textarea")"#));
        assert!(generated
            .code
            .contains(r#"_ssrInterpolate(("value" in _temp0) ? _temp0.value : "fallback")"#));
        assert!(generated
            .code
            .contains(r#"_ssrGetDirectiveProps(_ctx, _directive_xxx, _ctx.z, "x", { y: true })"#));
        assert!(generated.code.contains(
            r#"("textContent" in _temp0) ? _ssrInterpolate(_temp0.textContent) : _temp0.innerHTML ?? ''"#
        ));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_v_show_style_payload() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div id="app" style="color:red" :style="style" v-show="ok">Hi</div>"#.into(),
            file_id: FileId(47),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let attrs = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            })
            .expect("ssr attrs");
        assert_eq!(attrs.v_show, Some(JsExprId(1)));
        assert_eq!(attrs.props.static_attrs.len(), 1);
        assert_eq!(attrs.props.static_attrs[0].name, "style");
        assert_eq!(attrs.props.static_attrs[0].value, "color:red");
        assert_eq!(attrs.props.dynamic_bindings.len(), 1);
        assert_eq!(attrs.props.dynamic_bindings[0].name, "style");
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["style", "ok"]
        );
        let pushes = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(pushes, vec!["<div id=\"app\"", ">", "Hi", "</div>"]);
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_v_model_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><input v-model="bar"><input type="radio" value="foo" v-model="bar"><input type="checkbox" :true-value="foo" v-model="baz"><input :type="kind" :value="value" v-model="model"><textarea v-model="text">old</textarea><select v-model="picked"><option value="1"></option></select></div>"#.into(),
            file_id: FileId(50),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let models = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => attrs.v_model.as_ref(),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(models.len(), 6);
        assert!(matches!(models[0].kind, Vue3SsrModelKind::InputValue));
        assert!(matches!(
            models[1].kind,
            Vue3SsrModelKind::InputRadio {
                value: MirExpr::String(ref value)
            } if value == "foo"
        ));
        assert!(matches!(
            models[2].kind,
            Vue3SsrModelKind::InputCheckboxTrueValue {
                true_value: MirExpr::JsExpr(JsExprId(2))
            }
        ));
        assert!(matches!(
            models[3].kind,
            Vue3SsrModelKind::InputDynamicType {
                type_expr: JsExprId(4),
                value: MirExpr::JsExpr(JsExprId(5))
            }
        ));
        assert!(matches!(models[4].kind, Vue3SsrModelKind::Textarea));
        assert!(matches!(
            models[5].kind,
            Vue3SsrModelKind::SelectOption {
                value: MirExpr::String(ref value)
            } if value == "1"
        ));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["bar", "bar", "foo", "baz", "kind", "value", "model", "text", "picked"]
        );
        let pushes = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!pushes.contains(&"old"));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_content_override_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><section v-html="raw">old</section><p v-text="msg"/><span>keep</span></div>"#.into(),
            file_id: FileId(56),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert!(result.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue3SsrMirKind::RenderContent(Vue3SsrContent::Html {
                expression: JsExprId(0)
            })
        )));
        assert!(result.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue3SsrMirKind::RenderContent(Vue3SsrContent::Text {
                expression: JsExprId(1)
            })
        )));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["raw", "msg"]
        );
        let pushes = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!pushes.contains(&"old"));
        assert!(pushes.contains(&"<p"));
        assert!(pushes.contains(&">"));
        assert!(pushes.contains(&"</p>"));
        assert!(pushes.contains(&"keep"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_v_show_style_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><span style="color:red" :style="style" v-show="ok"/></div>"#.into(),
            file_id: FileId(48),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrRenderStyle as _ssrRenderStyle"));
        assert!(!generated.code.contains("mergeProps as _mergeProps"));
        assert!(!generated.code.contains("style=\\\"color:red\\\""));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated.code.contains("_push(`<div${"));
        assert!(generated.code.contains("<span style=\"${"));
        assert!(generated.code.contains("_ssrRenderStyle([\n      {\"color\":\"red\"},\n      _ctx.style,\n      (_ctx.ok) ? null : { display: \"none\" }\n    ])"));
        assert!(!generated.code.contains("_push(\"<span\");"));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_v_show_style_after_object_bindings() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source:
                r#"<div><span v-bind="extra" :title="title" style="display:flex" :style="style" v-show="ok"/></div>"#
                    .into(),
            file_id: FileId(49),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(!generated
            .code
            .contains("guardReactiveProps as _guardReactiveProps"));
        assert!(!generated.code.contains("_guardReactiveProps("));
        assert!(!generated.code.contains("ssrRenderStyle as _ssrRenderStyle"));
        assert!(!generated.code.contains("ssrRenderAttr as _ssrRenderAttr"));
        assert!(!generated.code.contains("style=\\\"display:flex\\\""));
        assert!(generated.code.contains("_push(`<div${"));
        assert!(generated.code.contains("_ssrRenderAttrs(_mergeProps(_ctx.extra, { title: _ctx.title }, {\n      style: [\n        {\"display\":\"flex\"},\n        _ctx.style,\n        (_ctx.ok) ? null : { display: \"none\" }\n      ]\n    }))"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_v_model_input_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><input v-model="bar"><input type="radio" value="foo" v-model="bar"><input type="checkbox" :true-value="foo" v-model="baz"><input :type="kind" v-model="foo" :value="value"></div>"#.into(),
            file_id: FileId(51),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrRenderAttr as _ssrRenderAttr"));
        assert!(generated
            .code
            .contains("ssrIncludeBooleanAttr as _ssrIncludeBooleanAttr"));
        assert!(generated.code.contains("ssrLooseEqual as _ssrLooseEqual"));
        assert!(generated
            .code
            .contains("ssrRenderDynamicModel as _ssrRenderDynamicModel"));
        assert!(generated
            .code
            .contains("_ssrRenderAttr(\"value\", _ctx.bar)"));
        assert!(generated.code.contains(
            "(_ssrIncludeBooleanAttr(_ssrLooseEqual(_ctx.bar, \"foo\"))) ? \" checked\" : \"\""
        ));
        assert!(generated.code.contains(
            "(_ssrIncludeBooleanAttr(_ssrLooseEqual(_ctx.baz, _ctx.foo))) ? \" checked\" : \"\""
        ));
        assert!(generated
            .code
            .contains("_ssrRenderDynamicModel(_ctx.kind, _ctx.foo, _ctx.value)"));
        assert!(generated
            .code
            .contains("_ssrRenderAttr(\"value\", _ctx.value)"));
        assert!(!generated.code.contains("true-value"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_textarea_and_select_v_model_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><textarea v-model="text">old</textarea><select v-model="model"><option value="1"></option><option v-for="item in items" :value="item">{{ item }}</option></select></div>"#.into(),
            file_id: FileId(52),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrInterpolate as _ssrInterpolate"));
        assert!(generated
            .code
            .contains("ssrRenderAttrs as _ssrRenderAttrs, ssrInterpolate as _ssrInterpolate"));
        assert!(generated
            .code
            .contains("ssrIncludeBooleanAttr as _ssrIncludeBooleanAttr"));
        assert!(generated
            .code
            .contains("ssrLooseContain as _ssrLooseContain"));
        assert!(generated.code.contains("ssrLooseEqual as _ssrLooseEqual"));
        assert!(generated.code.contains("_push(`<div${"));
        assert!(generated.code.contains("_ssrInterpolate(_ctx.text)"));
        assert!(!generated
            .code
            .contains("_push(_ssrInterpolate(_ctx.text));"));
        assert!(!generated.code.contains("_push(\"old\");"));
        assert!(generated.code.contains(
            "(_ssrIncludeBooleanAttr((Array.isArray(_ctx.model))\n      ? _ssrLooseContain(_ctx.model, \"1\")\n      : _ssrLooseEqual(_ctx.model, \"1\"))) ? \" selected\" : \"\""
        ));
        assert!(generated
            .code
            .contains("_ssrRenderList(_ctx.items, (item) => {"));
        assert!(generated.code.contains(
            "(_ssrIncludeBooleanAttr((Array.isArray(_ctx.model))\n        ? _ssrLooseContain(_ctx.model, item)\n        : _ssrLooseEqual(_ctx.model, item))) ? \" selected\" : \"\""
        ));
        assert!(!generated.code.contains("_ctx.item)"));
        assert!(!generated.code.contains("_ctx.item,"));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_input_object_v_model_props() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<input id="x" v-bind="obj" v-model="foo" class="y">"#.into(),
            file_id: FileId(53),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated
            .code
            .contains("ssrGetDynamicModelProps as _ssrGetDynamicModelProps"));
        assert!(generated.code.contains("let _temp0"));
        assert!(!generated
            .code
            .contains("_push(\"<input id=\\\"x\\\" class=\\\"y\\\"\");"));
        assert!(generated.code.contains(
            "_push(_ssrRenderAttrs((_temp0 = _mergeProps(_mergeProps({ id: \"x\" }, _ctx.obj, { class: \"y\" }), _attrs), _mergeProps(_temp0, _ssrGetDynamicModelProps(_temp0, _ctx.foo)))));"
        ));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_dynamic_key_input_v_model_props() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<input :[name]="value" v-model="foo">"#.into(),
            file_id: FileId(54),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(!generated.code.contains("normalizeProps as _normalizeProps"));
        assert!(generated
            .code
            .contains("ssrGetDynamicModelProps as _ssrGetDynamicModelProps"));
        assert!(generated
            .code
            .contains("_temp0 = _mergeProps({ [_ctx.name || \"\"]: _ctx.value }, _attrs)"));
        assert!(generated
            .code
            .contains("{ [_ctx.name || \"\"]: _ctx.value }"));
        assert!(generated.code.contains(", _attrs)"));
        assert!(generated
            .code
            .contains("_mergeProps(_temp0, _ssrGetDynamicModelProps(_temp0, _ctx.foo))"));
    }

    #[test]
    fn generate_vue3_ssr_mir_merges_v_show_input_object_v_model_props() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<input v-bind="obj" style="color:red" v-show="ok" v-model="foo">"#.into(),
            file_id: FileId(55),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated
            .code
            .contains("ssrGetDynamicModelProps as _ssrGetDynamicModelProps"));
        assert!(generated.code.contains("_push(_ssrRenderAttrs((_temp0 = _mergeProps(_ctx.obj, {\n    style: [\n      {\"color\":\"red\"},\n      (_ctx.ok) ? null : { display: \"none\" }\n    ]\n  }, _attrs), _mergeProps(_temp0, _ssrGetDynamicModelProps(_temp0, _ctx.foo)))));"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_content_override_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source:
                r#"<div><section v-html="raw">old</section><p v-text="msg"/><span>keep</span></div>"#
                    .into(),
            file_id: FileId(57),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrInterpolate as _ssrInterpolate"));
        assert!(generated.code.contains("(_ctx.raw) ?? ''"));
        assert!(generated.code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(generated.code.contains("<p>${"));
        assert!(generated.code.contains("}</p>"));
        assert!(!generated.code.contains("_push(\"old\");"));
        assert!(generated.code.contains("<span>keep</span>"));
    }
