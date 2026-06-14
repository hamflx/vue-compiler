    #[test]
    fn base_compile_stringifies_static_child_tree_when_threshold_matches() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div><div>{}</div></div>",
                    r#"<span class="foo"/>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("createStaticVNode"));
        assert!(result.code.contains("_createStaticVNode(\"<div><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span></div>\", 1)"));
    }

    #[test]
    fn base_compile_stringifies_multiple_adjacent_static_nodes() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!("<div>{}</div>", r#"<span class="foo"/>"#.repeat(5)),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
    }

    #[test]
    fn base_compile_stringifies_multiple_static_chunks_around_dynamic_child() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div>{}{{{{ msg }}}}{}</div>",
                    r#"<span class="foo"></span>"#.repeat(5),
                    r#"<span class="bar"></span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert_eq!(result.code.matches("_createStaticVNode(").count(), 2);
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
        assert!(result
            .code
            .contains("_createTextVNode(_toDisplayString(_ctx.msg), 1 /* TEXT */)"));
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span>\", 5)"));
        assert!(!result.code.contains("class: \"foo\""));
        assert!(!result.code.contains("class: \"bar\""));
    }

    #[test]
    fn base_compile_bails_stringify_static_invalid_p_child_placement() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div><p>{}</p></div>",
                    r#"<span class="inline"></span>"#.repeat(5)
                        + "<span><div class=\"block\"></div></span>"
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(!result.code.contains("_createStaticVNode"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
        assert!(result.code.contains("_createElementVNode(\"p\""));
        assert!(result
            .code
            .contains("_createElementVNode(\"div\", { class: \"block\" })"));
    }

    #[test]
    fn base_compile_stringifies_static_p_with_phrasing_children() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div><p>{}</p></div>",
                    r#"<span class="inline"></span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result.code.contains("_createStaticVNode(\"<p><span class=\\\"inline\\\"></span><span class=\\\"inline\\\"></span><span class=\\\"inline\\\"></span><span class=\\\"inline\\\"></span><span class=\\\"inline\\\"></span></p>\", 1)"));
    }

    #[test]
    fn base_compile_stringifies_static_html_escaping_and_void_tags() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div>{}{}</div>",
                    r#"<span title="foo>bar">&amp; &lt;</span>"#.repeat(5),
                    r#"<img title="foo>bar"/>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result
            .code
            .contains(r#"<span title=\"foo&gt;bar\">&amp; &lt;</span>"#));
        assert!(result.code.contains(r#"<img title=\"foo&gt;bar\">"#));
        assert!(!result.code.contains("</img>"));
    }

    #[test]
    fn base_compile_stringifies_static_html_with_scope_id() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div>{}</div>",
                    r#"<span class="foo"><i>ok</i></span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                hoist_static: true,
                stringify_static: true,
                scope_id: Some("data-v-test".into()),
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result
            .code
            .contains(r#"<span class=\"foo\" data-v-test><i data-v-test>ok</i></span>"#));
    }

    #[test]
    fn base_compile_stringifies_static_constant_bindings_and_interpolations() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    r#"<div><div :style="{{ color: 'red' }}">{}</div></div>"#,
                    r#"<span :class="[{ foo: true }, { bar: true }]">{{ 1 }} + {{ false }}</span>"#
                        .repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result
            .code
            .contains(r#"<div style=\"color:red;\"><span class=\"foo bar\">1 + false</span>"#));
        assert!(!result.code.contains("_normalizeClass"));
    }

    #[test]
    fn base_compile_stringifies_static_asset_import_bindings() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: format!(
                r#"<div><img :src="_imports_0" :srcset="_imports_0 + ', ' + _imports_1 + '#heart 2x'" />{}</div>"#,
                r#"<span class="foo"></span>"#.repeat(5)
            ),
            file_id: FileId(0),
            base_offset: 0,
        };
        let mut ast = Vue3Dialect::base_parse(
            source.clone(),
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_1".into(),
                    path: "./icons.svg".into(),
                });
            }
        }
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::finish_compile(
            ast,
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
            ctx,
        );

        assert!(result.code.contains("import _imports_0 from './logo.png'"));
        assert!(result.code.contains("import _imports_1 from './icons.svg'"));
        assert!(
            result.code.contains("_createStaticVNode"),
            "{}",
            result.code
        );
        assert!(result
            .code
            .contains("const _hoisted_1 = _imports_0 + ', ' + _imports_1 + '#heart 2x'"));
        assert!(result.code.contains(
            r##"_createStaticVNode("<img src=\"" + _imports_0 + "\" srcset=\"" + _hoisted_1 + "\"><span class=\"foo\"></span>"##
        ));
        assert!(!result.code.contains("_ctx._imports_0"));
        assert!(!result.code.contains("_ctx._imports_1"));
    }

    #[test]
    fn base_compile_stringifies_constant_binding_removals_and_escape() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div>{}{}</div>",
                    r#"<span :title="null" :class="'foo' + '&gt;ar'">{{ '<' }}</span>"#.repeat(5),
                    r#"<button :disabled="false">enable</button>"#.repeat(16)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result
            .code
            .contains(r#"<span class=\"foo&gt;ar\">&lt;</span>"#));
        assert!(result.code.contains(r#"<button>enable</button>"#));
        assert!(!result.code.contains("title="));
        assert!(!result.code.contains("disabled="));
    }

    #[test]
    fn base_compile_stringifies_static_svg_namespace_children() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    r#"<div><svg width="50" height="50" viewBox="0 0 50 50" fill="none" xmlns="http://www.w3.org/2000/svg">{}</svg></div>"#,
                    r##"<rect width="50" height="50" fill="#C4C4C4"></rect>"##.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                dom_namespaces: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result.code.contains(r#"<svg width=\"50\" height=\"50\" viewBox=\"0 0 50 50\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">"#));
        assert!(result
            .code
            .contains(r##"<rect width=\"50\" height=\"50\" fill=\"#C4C4C4\"></rect>"##));
    }

    #[test]
    fn base_compile_stringifies_static_mathml_namespace_children() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    r#"<div><math xmlns="http://www.w3.org/1998/Math/MathML">{}</math></div>"#,
                    r#"<ms>1</ms>"#.repeat(20)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                dom_namespaces: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result
            .code
            .contains(r#"<math xmlns=\"http://www.w3.org/1998/Math/MathML\"><ms>1</ms>"#));
    }

    #[test]
    fn base_compile_bails_stringify_static_option_constant_value_binding() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: format!(
                    "<div><select>{}</select></div>",
                    r#"<option :value="1"/>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(!result.code.contains("_createStaticVNode"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
    }

    #[test]
    fn base_compile_keeps_static_cache_array_below_stringify_threshold() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: "<div><div><div>hello</div><div>hello</div></div></div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                prefix_identifiers: true,
                hoist_static: true,
                stringify_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(!result.code.contains("_createStaticVNode"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
        assert!(result.code.contains("-1 /* CACHED */"));
    }

    #[test]
    fn base_compile_wraps_named_component_slots_with_ctx() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Child>
        <template #foo="{ msg }">{{ msg }}</template>
        <template #bar><div/></template>
      </Child>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                scope_id: Some("test".into()),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("foo: _withCtx(({ msg }) => ["));
        assert!(result
            .code
            .contains("_createTextVNode(_toDisplayString(msg), 1 /* TEXT */)"));
        assert!(result.code.contains("bar: _withCtx(() => ["));
        assert!(result.code.contains("_: 1 /* STABLE */"));
    }

    #[test]
    fn base_compile_wraps_dynamic_component_slots_with_create_slots() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Child>
        <template #foo v-if="ok"><div/></template>
        <template v-for="i in list" #[i]><div/></template>
      </Child>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                scope_id: Some("test".into()),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result
            .code
            .contains("_createSlots({ _: 2 /* DYNAMIC */ }, ["));
        assert!(result.code.contains("name: \"foo\""));
        assert!(result.code.contains("fn: _withCtx(() => ["));
        assert!(result.code.contains("name: i"));
        assert!(result.code.contains(", 1024 /* DYNAMIC_SLOTS */"));
    }

    #[test]
    fn root_codegen_projection_uses_child_for_slot_outlet() {
        let root = json!({
            "children": [{
                "type": 1,
                "tagType": 2,
                "codegenNode": { "type": 14 }
            }]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "child", "index": 0 })
        );
    }

    #[test]
    fn root_codegen_projection_uses_single_element_codegen_as_block() {
        let root = json!({
            "children": [{
                "type": 1,
                "tagType": 0,
                "codegenNode": { "type": 13 }
            }]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "childCodegen", "index": 0, "asBlock": true })
        );
    }

    #[test]
    fn root_codegen_projection_preserves_non_element_child() {
        let root = json!({ "children": [{ "type": 11, "codegenNode": { "type": 13 } }] });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "child", "index": 0 })
        );
    }
