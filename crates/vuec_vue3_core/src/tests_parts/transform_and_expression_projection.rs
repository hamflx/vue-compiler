    #[test]
    fn transform_collects_root_helpers_components_and_directives() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Child v-focus><slot/><Transition/><component :is="view"/><div v-show="ok" :class="klass">{{ msg }}</div></Child>"#.into(),
            file_id: FileId(20),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions::default();
        let mut ast = Vue3Dialect::base_parse(source, &options);
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);

        let root = ast
            .root_node()
            .and_then(|node| match &node.kind {
                Vue3AstKind::Root(root) => Some(root),
                _ => None,
            })
            .expect("Vue3 root");

        assert!(root.components.contains("Child"));
        assert_eq!(root.directives.iter().collect::<Vec<_>>(), vec!["focus"]);
        for helper in [
            RuntimeHelper::Vue3OpenBlock,
            RuntimeHelper::Vue3CreateElementBlock,
            RuntimeHelper::Vue3CreateElementVNode,
            RuntimeHelper::Vue3RenderSlot,
            RuntimeHelper::Vue3ResolveComponent,
            RuntimeHelper::Vue3ResolveDynamicComponent,
            RuntimeHelper::Vue3Transition,
            RuntimeHelper::Vue3WithDirectives,
            RuntimeHelper::Vue3ResolveDirective,
            RuntimeHelper::Vue3VShow,
            RuntimeHelper::Vue3NormalizeClass,
            RuntimeHelper::Vue3ToDisplayString,
        ] {
            assert!(
                root.helpers.contains(&helper),
                "missing root helper {helper:?}"
            );
            assert!(
                ctx.helpers.contains(&helper),
                "missing ctx helper {helper:?}"
            );
        }

        let projected = ast.project_public();
        let projected_root = match &projected.kind {
            Vue3AstKind::Root(root) => root,
            _ => panic!("projected root"),
        };
        assert_eq!(projected_root.components, root.components);
        assert_eq!(projected_root.directives, root.directives);
        assert_eq!(projected_root.helpers, root.helpers);
    }

    #[test]
    fn transform_root_collection_keeps_structural_directives_out_of_runtime_assets() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<ul><li v-for="item in list" v-if="item.ok" v-once v-memo="[item.id]">{{ item.name }}</li></ul>"#.into(),
            file_id: FileId(21),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions::default();
        let mut ast = Vue3Dialect::base_parse(source, &options);
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);

        let root = ast
            .root_node()
            .and_then(|node| match &node.kind {
                Vue3AstKind::Root(root) => Some(root),
                _ => None,
            })
            .expect("Vue3 root");

        assert!(root.components.is_empty());
        assert!(root.directives.is_empty());
        for helper in [
            RuntimeHelper::Vue3Fragment,
            RuntimeHelper::Vue3RenderList,
            RuntimeHelper::Vue3CreateCommentVNode,
            RuntimeHelper::Vue3WithMemo,
            RuntimeHelper::Vue3IsMemoSame,
        ] {
            assert!(
                root.helpers.contains(&helper),
                "missing root helper {helper:?}"
            );
            assert!(
                ctx.helpers.contains(&helper),
                "missing ctx helper {helper:?}"
            );
        }
        assert!(!root.helpers.contains(&RuntimeHelper::Vue3ResolveDirective));
        assert!(!root.helpers.contains(&RuntimeHelper::Vue3WithDirectives));
    }

    #[test]
    fn base_compile_wraps_multiple_root_children_in_fragment_block() {
        let result = base_compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div/><span/>"#.into(),
                file_id: FileId(92),
                base_offset: 0,
            },
            Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("Fragment as _Fragment"));
        assert!(result
            .code
            .contains("return (_openBlock(), _createElementBlock(_Fragment, null, ["));
        assert!(result.code.contains("_createElementVNode(\"div\")"));
        assert!(result.code.contains("_createElementVNode(\"span\")"));
        assert!(result.code.contains("64 /* STABLE_FRAGMENT */"));
    }

    #[test]
    fn generate_consumes_transformed_root_component_state() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><Child/><Transition/><component :is="view"/></div>"#.into(),
            file_id: FileId(22),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions::default();
        let mut ast = Vue3Dialect::base_parse(source, &options);
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);

        let result = Vue3Dialect::generate(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
            &TransformContext::default(),
        );

        assert!(result
            .code
            .contains("resolveComponent as _resolveComponent"));
        assert!(result
            .code
            .contains("resolveDynamicComponent as _resolveDynamicComponent"));
        assert!(result.code.contains("Transition as _Transition"));
        assert!(result
            .code
            .contains("const _component_Child = _resolveComponent(\"Child\")"));
        assert!(!result.code.contains("_component_Transition"));
        assert!(result.code.contains("_createVNode(_Transition"));
        assert!(result
            .code
            .contains("_createVNode(_resolveDynamicComponent(_ctx.view)"));
    }

    #[test]
    fn generate_preserves_raw_ast_component_scan_fallback() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<Child/>".into(),
            file_id: FileId(23),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::generate(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
            &TransformContext::default(),
        );

        assert!(result
            .code
            .contains("resolveComponent as _resolveComponent"));
        assert!(result
            .code
            .contains("const _component_Child = _resolveComponent(\"Child\")"));
        assert!(result.code.contains("_createBlock(_component_Child"));
    }

    #[test]
    fn generate_preserves_raw_ast_runtime_directive_scan_fallback() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-focus:foo.bar="value"/>"#.into(),
            file_id: FileId(24),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::generate(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
            &TransformContext::default(),
        );

        assert!(result.code.contains("withDirectives as _withDirectives"));
        assert!(result
            .code
            .contains("resolveDirective as _resolveDirective"));
        assert!(result
            .code
            .contains("const _directive_focus = _resolveDirective(\"focus\")"));
        assert!(result
            .code
            .contains("[_directive_focus, _ctx.value, \"foo\", {"));
        assert!(result.code.contains("512 /* NEED_PATCH */"));
    }

    #[test]
    fn generate_consumes_transformed_root_runtime_directive_state() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-focus:foo.bar="value"><span v-show="ok"/></div>"#.into(),
            file_id: FileId(25),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions::default();
        let mut ast = Vue3Dialect::base_parse(source, &options);
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);

        let result = Vue3Dialect::generate(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
            &TransformContext::default(),
        );

        assert!(result.code.contains("withDirectives as _withDirectives"));
        assert!(result
            .code
            .contains("resolveDirective as _resolveDirective"));
        assert!(result.code.contains("vShow as _vShow"));
        assert!(result
            .code
            .contains("const _directive_focus = _resolveDirective(\"focus\")"));
        assert!(result
            .code
            .contains("[_directive_focus, _ctx.value, \"foo\", {"));
        assert!(result.code.contains("bar: true"));
        assert!(result.code.contains("[_vShow, _ctx.ok]"));
        assert!(result.code.contains("512 /* NEED_PATCH */"));
    }

    fn process_expression_test_projection(content: &str, context: Value) -> Value {
        process_expression_projection(&json!({
            "node": {
                "type": 4,
                "content": content,
                "isStatic": false,
                "loc": {
                    "start": { "offset": 0, "line": 1, "column": 1 },
                    "end": { "offset": content.len(), "line": 1, "column": content.len() + 1 },
                    "source": content
                }
            },
            "context": context
        }))
    }

    fn process_expression_test_statement_projection(content: &str, context: Value) -> Value {
        process_expression_projection(&json!({
            "node": {
                "type": 4,
                "content": content,
                "isStatic": false,
                "loc": {
                    "start": { "offset": 0, "line": 1, "column": 1 },
                    "end": { "offset": content.len(), "line": 1, "column": content.len() + 1 },
                    "source": content
                }
            },
            "context": context,
            "asRawStatements": true
        }))
    }

    fn projection_code(value: &Value) -> String {
        match json_str(value, "kind") {
            Some("simple") => json_str(value, "content").unwrap_or("").to_string(),
            Some("compound") => value
                .get("children")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .map(projection_code)
                .collect::<String>(),
            _ => value.as_str().unwrap_or_default().to_string(),
        }
    }

    #[test]
    fn process_expression_projection_prefixes_simple_identifier() {
        let projection = process_expression_test_projection(
            "foo",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );

        assert_eq!(projection["kind"], json!("simple"));
        assert_eq!(projection["content"], json!("_ctx.foo"));
        assert_eq!(projection["constType"], json!(0));
    }

    #[test]
    fn vue3_utils_projection_advances_positions_with_utf16_offsets() {
        let projection = advance_position_with_clone_projection(&json!({
            "pos": { "offset": 2, "line": 3, "column": 4 },
            "source": "a😏\nb",
            "numberOfCharacters": 4,
        }));

        assert_eq!(projection["offset"], json!(6));
        assert_eq!(projection["line"], json!(4));
        assert_eq!(projection["column"], json!(1));
    }

    #[test]
    fn vue3_utils_projection_generates_official_asset_ids() {
        assert_eq!(
            to_valid_asset_id_projection(&json!({
                "name": "test-测试-1",
                "type": "component",
            }))["id"],
            json!("_component_test_2797935797_1")
        );
        assert_eq!(
            to_valid_asset_id_projection(&json!({
                "name": "a😏-b",
                "type": "directive",
            }))["id"],
            json!("_directive_a5535756847_b")
        );
    }

    #[test]
    fn process_expression_projection_prefixes_object_shorthand_value() {
        let projection = process_expression_test_projection(
            "{ foo }",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(projection["children"][0], json!("{ foo: "));
        assert_eq!(projection["children"][1]["content"], json!("_ctx.foo"));
        assert_eq!(projection["children"][2], json!(" }"));
    }

    #[test]
    fn process_expression_projection_keeps_static_object_literal_simple() {
        let projection = process_expression_test_projection(
            "{ foo: true }",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );

        assert_eq!(projection["kind"], json!("setConstType"));
        assert_eq!(projection["constType"], json!(3));
    }

    #[test]
    fn process_expression_projection_rewrites_template_literal_expressions() {
        let projection = process_expression_test_projection(
            r#"`outer ${`inner ${value}` + /[}]/.test(other)}`"#,
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(
            projection_code(&projection),
            r#"`outer ${`inner ${_ctx.value}` + /[}]/.test(_ctx.other)}`"#,
        );
    }

    #[test]
    fn process_expression_projection_preserves_regular_expressions_and_comments() {
        let projection = process_expression_test_projection(
            r#"/[a-z/]+/.test(value) /* outside */ + next"#,
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(
            projection_code(&projection),
            r#"/[a-z/]+/.test(_ctx.value) /* outside */ + _ctx.next"#,
        );
    }

    #[test]
    fn process_expression_projection_materializes_static_member_identifiers() {
        let context =
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} });
        let member = process_expression_test_projection("foo + bar(baz.qux)", context.clone());
        assert_eq!(member["children"][0]["content"], json!("_ctx.foo"));
        assert_eq!(member["children"][1], json!(" + "));
        assert_eq!(member["children"][2]["content"], json!("_ctx.bar"));
        assert_eq!(member["children"][3], json!("("));
        assert_eq!(member["children"][4]["content"], json!("_ctx.baz"));
        assert_eq!(member["children"][5], json!("."));
        assert_eq!(member["children"][6]["content"], json!("qux"));
        assert_eq!(member["children"][6]["constType"], json!(0));
        assert_eq!(member["children"][7], json!(")"));

        let global = process_expression_test_projection("Math.max(1, 2)", context.clone());
        assert_eq!(global["children"][0]["content"], json!("Math"));
        assert_eq!(global["children"][0]["constType"], json!(0));
        assert_eq!(global["children"][1], json!("."));
        assert_eq!(global["children"][2]["content"], json!("max"));
        assert_eq!(global["children"][2]["constType"], json!(0));
        assert_eq!(global["children"][3], json!("(1, 2)"));

        let constructor = process_expression_test_projection("new Date().getFullYear()", context);
        assert_eq!(constructor["children"][0], json!("new "));
        assert_eq!(constructor["children"][1]["content"], json!("Date"));
        assert_eq!(constructor["children"][1]["constType"], json!(0));
        assert_eq!(constructor["children"][2], json!("()."));
        assert_eq!(constructor["children"][3]["content"], json!("getFullYear"));
        assert_eq!(constructor["children"][3]["constType"], json!(0));
        assert_eq!(constructor["children"][4], json!("()"));
    }

    #[test]
    fn process_expression_projection_matches_babel_optional_chain_parent_kinds() {
        let context =
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} });

        let optional = process_expression_test_projection("foo?.bar", context.clone());
        assert_eq!(optional["children"][0]["content"], json!("_ctx.foo"));
        assert_eq!(optional["children"][1], json!("?."));
        assert_eq!(optional["children"][2]["content"], json!("bar"));
        assert_eq!(optional["children"][2]["constType"], json!(3));

        let optional_tail =
            process_expression_test_projection("foo.bar?.baz", context.clone());
        assert_eq!(optional_tail["children"][2]["content"], json!("bar"));
        assert_eq!(optional_tail["children"][2]["constType"], json!(0));
        assert_eq!(optional_tail["children"][4]["content"], json!("baz"));
        assert_eq!(optional_tail["children"][4]["constType"], json!(3));

        let optional_head =
            process_expression_test_projection("foo?.bar.baz", context.clone());
        assert_eq!(optional_head["children"][2]["content"], json!("bar"));
        assert_eq!(optional_head["children"][2]["constType"], json!(3));
        assert_eq!(optional_head["children"][4]["content"], json!("baz"));
        assert_eq!(optional_head["children"][4]["constType"], json!(3));

        let optional_call =
            process_expression_test_projection("foo?.bar(Math)", context.clone());
        assert_eq!(optional_call["children"][2]["constType"], json!(3));
        assert_eq!(optional_call["children"][4]["content"], json!("Math"));
        assert_eq!(optional_call["children"][4]["constType"], json!(3));

        let terminated = process_expression_test_projection("(foo?.bar).baz", context);
        assert_eq!(terminated["children"][3]["content"], json!("bar"));
        assert_eq!(terminated["children"][3]["constType"], json!(3));
        assert_eq!(terminated["children"][5]["content"], json!("baz"));
        assert_eq!(terminated["children"][5]["constType"], json!(0));
    }

    #[test]
    fn process_expression_projection_segments_keyword_and_global_static_members() {
        let context =
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} });
        for property in ["default", "true", "Math"] {
            let projection =
                process_expression_test_projection(&format!("foo.{property}"), context.clone());
            assert_eq!(projection["children"][0]["content"], json!("_ctx.foo"));
            assert_eq!(projection["children"][1], json!("."));
            assert_eq!(projection["children"][2]["content"], json!(property));
            assert_eq!(projection["children"][2]["constType"], json!(0));
        }

        let escaped =
            process_expression_test_projection(r"foo.\u0064efault", context);
        assert_eq!(escaped["children"][2]["content"], json!("default"));
        assert_eq!(escaped["children"][2]["constType"], json!(0));
        assert_eq!(escaped["children"][2]["loc"]["source"], json!(r"\u0064efault"));
    }

    #[test]
    fn process_expression_projection_matches_babel_constant_parent_boundaries() {
        let context =
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} });

        let argument = process_expression_test_projection("foo(Math)", context.clone());
        assert_eq!(argument["children"][2]["content"], json!("Math"));
        assert_eq!(argument["children"][2]["constType"], json!(0));

        let constructor = process_expression_test_projection("new Foo(Math)", context.clone());
        assert_eq!(constructor["children"][3]["content"], json!("Math"));
        assert_eq!(constructor["children"][3]["constType"], json!(0));

        let computed = process_expression_test_projection("obj[Math]", context.clone());
        assert_eq!(computed["children"][2]["content"], json!("Math"));
        assert_eq!(computed["children"][2]["constType"], json!(0));

        let standalone = process_expression_test_projection("Math + foo", context.clone());
        assert_eq!(standalone["children"][0]["content"], json!("Math"));
        assert_eq!(standalone["children"][0]["constType"], json!(3));

        let parenthesized = process_expression_test_projection("(Math).max", context.clone());
        assert_eq!(parenthesized["children"][1]["content"], json!("Math"));
        assert_eq!(parenthesized["children"][1]["constType"], json!(0));
        assert_eq!(parenthesized["children"][3]["content"], json!("max"));
        assert_eq!(parenthesized["children"][3]["constType"], json!(0));

        let parameter = process_expression_test_projection("(x)=>x.foo", context);
        assert_eq!(parameter["children"][1]["content"], json!("x"));
        assert_eq!(parameter["children"][1]["constType"], json!(3));
        assert_eq!(parameter["children"][3]["content"], json!("x"));
        assert_eq!(parameter["children"][3]["constType"], json!(0));
        assert_eq!(parameter["children"][5]["content"], json!("foo"));
        assert_eq!(parameter["children"][5]["constType"], json!(0));
    }

    #[test]
    fn process_expression_projection_uses_parsed_identifier_roles() {
        let comment = process_expression_test_projection(
            "/* (value) => */ value + outside",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );
        assert_eq!(
            projection_code(&comment),
            "/* (value) => */ _ctx.value + _ctx.outside",
        );

        let labels = process_expression_test_statement_projection(
            "outer: while (ready) { if (skip) continue outer; break outer }",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );
        assert_eq!(
            projection_code(&labels),
            "outer: while (_ctx.ready) { if (_ctx.skip) continue outer; break outer }",
        );

        let unicode = process_expression_test_projection(
            "{ 用户 }",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );
        assert_eq!(projection_code(&unicode), "{ 用户: _ctx.用户 }");

        let spread = process_expression_test_projection(
            "{ ...items, values: [...others] }",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );
        assert_eq!(
            projection_code(&spread),
            "{ ..._ctx.items, values: [..._ctx.others] }",
        );
    }

    #[test]
    fn process_expression_projection_decodes_escaped_identifiers() {
        let context =
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} });
        let reference = process_expression_test_projection(r"\u0061 + outside", context.clone());
        assert_eq!(
            projection_code(&reference),
            "_ctx.a + _ctx.outside",
        );
        assert_eq!(reference["children"][0]["loc"]["source"], json!(r"\u0061"));

        let scoped = process_expression_test_projection(
            r"(\u0061) => \u0061 + outside",
            context.clone(),
        );
        assert_eq!(projection_code(&scoped), "(a) => a + _ctx.outside");

        let properties = process_expression_test_projection(
            r"obj.\u0061 + ({ \u0062: value, [\u0063]: other, \u0064 })",
            context,
        );
        assert_eq!(
            projection_code(&properties),
            r"_ctx.obj.a + ({ \u0062: _ctx.value, [_ctx.c]: _ctx.other, d: _ctx.d })",
        );
        assert_eq!(properties["children"][2]["loc"]["source"], json!(r"\u0061"));

        let labels = process_expression_test_statement_projection(
            r"\u006futer: while (ready) { break \u006futer }",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );
        assert_eq!(
            projection_code(&labels),
            r"\u006futer: while (_ctx.ready) { break \u006futer }",
        );

        let inline = process_expression_test_projection(
            r"({ \u0063ount })",
            json!({
                "prefixIdentifiers": true,
                "inline": true,
                "identifiers": {},
                "bindingMetadata": { "count": "setup-ref" }
            }),
        );
        assert_eq!(projection_code(&inline), "({ count: count.value })");
    }

    #[test]
    fn process_expression_projection_rejects_unavailable_escaped_identifier_ast() {
        let context =
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} });
        for invalid in [r"\u{110000} + outside", r"\u0069f + outside"] {
            let projection = process_expression_test_projection(invalid, context.clone());
            assert_eq!(projection["kind"], json!("error"));
            assert_eq!(projection["code"], json!(46));
        }

        let long = format!(r"\u0061 + {}", "outside + ".repeat(450));
        assert!(long.len() > PROCESS_EXPRESSION_MAX_SAFE_AST_BYTES);
        let projection = process_expression_test_projection(&long, context);
        assert_eq!(projection["kind"], json!("error"));
        assert_eq!(projection["code"], json!(46));
        assert_eq!(
            projection["message"],
            json!(PROCESS_EXPRESSION_AST_LIMIT_MESSAGE),
        );
    }

    #[test]
    fn process_expression_projection_preserves_slot_params_as_bindings() {
        let projection = process_expression_projection(&json!({
            "node": {
                "type": 4,
                "content": "{ foo }",
                "isStatic": false,
                "loc": {
                    "start": { "offset": 0, "line": 1, "column": 1 },
                    "end": { "offset": 7, "line": 1, "column": 8 },
                    "source": "{ foo }"
                }
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} },
            "asParams": true
        }));

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(projection["children"][0], json!("{ "));
        assert_eq!(projection["children"][1]["content"], json!("foo"));
        assert_eq!(projection["children"][2], json!(" }"));
        assert_eq!(projection["identifiers"], json!(["foo"]));
    }

    #[test]
    fn process_expression_projection_decodes_escaped_slot_params() {
        let content = r"{ \u0061, value: \u0062 = outside }";
        let projection = process_expression_projection(&json!({
            "node": {
                "type": 4,
                "content": content,
                "isStatic": false,
                "loc": {
                    "start": { "offset": 0, "line": 1, "column": 1 },
                    "end": { "offset": content.len(), "line": 1, "column": content.len() + 1 },
                    "source": content
                }
            },
            "context": {
                "prefixIdentifiers": true,
                "identifiers": {},
                "bindingMetadata": {}
            },
            "asParams": true
        }));

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(
            projection_code(&projection),
            "{ a, value: b = _ctx.outside }",
        );
        assert_eq!(projection["identifiers"], json!(["a", "b"]));

        let typed = r"\u0061: Item = outside";
        let typed_projection = process_expression_projection(&json!({
            "node": {
                "type": 4,
                "content": typed,
                "isStatic": false,
                "loc": {
                    "start": { "offset": 0, "line": 1, "column": 1 },
                    "end": { "offset": typed.len(), "line": 1, "column": typed.len() + 1 },
                    "source": typed
                }
            },
            "context": {
                "prefixIdentifiers": true,
                "identifiers": {},
                "bindingMetadata": {},
                "isTS": true
            },
            "asParams": true
        }));
        assert_eq!(
            projection_code(&typed_projection),
            "a: Item = _ctx.outside",
        );
        assert_eq!(typed_projection["identifiers"], json!(["a"]));
    }

    #[test]
    fn process_expression_projection_keeps_arrow_params_scoped_to_arrow_body() {
        let projection = process_expression_test_projection(
            "{ a: foo => foo, b: foo }",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );
        let children = &projection["children"];

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(children[0], json!("{ a: "));
        assert_eq!(children[1]["content"], json!("foo"));
        assert_eq!(children[2], json!(" => "));
        assert_eq!(children[3]["content"], json!("foo"));
        assert_eq!(children[4], json!(", b: "));
        assert_eq!(children[5]["content"], json!("_ctx.foo"));
        assert_eq!(children[6], json!(" }"));
    }

    #[test]
    fn process_expression_projection_scopes_function_declarations() {
        let projection = process_expression_test_statement_projection(
            "function named(value = seed) { return named(value) + outside }; named(input); value",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(
            projection_code(&projection),
            "function named(value = _ctx.seed) { return named(value) + _ctx.outside }; named(_ctx.input); _ctx.value",
        );
    }

    #[test]
    fn process_expression_projection_recovers_function_bindings_with_external_syntax() {
        let projection = process_expression_test_projection(
            "function named(value) { return value |> transform }",
            json!({
                "prefixIdentifiers": true,
                "identifiers": {},
                "bindingMetadata": {},
                "expressionPlugins": ["pipelineOperator"]
            }),
        );

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(
            projection_code(&projection),
            "function named(value) { return value |> _ctx.transform }",
        );
    }

    #[test]
    fn process_expression_projection_recovers_hack_pipeline_topic_bindings() {
        for topic in ["%", "#", "^", "@@", "^^"] {
            let source = format!(
                "function named(value) {{ const local = value; return local |> {topic} + outside }}"
            );
            let projection = process_expression_test_projection(
                &source,
                json!({
                    "prefixIdentifiers": true,
                    "identifiers": {},
                    "bindingMetadata": {},
                    "expressionPlugins": [[
                        "pipelineOperator",
                        { "proposal": "hack", "topicToken": topic }
                    ]]
                }),
            );

            assert_eq!(projection["kind"], json!("compound"), "topic {topic}");
            assert_eq!(
                projection_code(&projection),
                format!(
                    "function named(value) {{ const local = value; return local |> {topic} + _ctx.outside }}"
                ),
                "topic {topic}",
            );
        }
    }

    #[test]
    fn process_expression_projection_skips_typescript_type_positions() {
        let projection = process_expression_test_projection(
            "(value: External): Result<External> => factory<Generic>(value as Cast, outside satisfies Shape)",
            json!({
                "prefixIdentifiers": true,
                "identifiers": {},
                "bindingMetadata": {},
                "expressionPlugins": ["typescript"]
            }),
        );

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(
            projection_code(&projection),
            "(value: External): Result<External> => _ctx.factory<Generic>(value as Cast, _ctx.outside satisfies Shape)",
        );
    }

    #[test]
    fn process_expression_projection_supports_jsx_and_tsx_plugins() {
        let jsx = process_expression_test_projection(
            "(item) => <><div title=\"raw\">hello {item}</div><Comp {...props}>{outside}</Comp></>",
            json!({
                "prefixIdentifiers": true,
                "identifiers": {},
                "bindingMetadata": {},
                "expressionPlugins": [["jsx", {}]]
            }),
        );

        assert_eq!(jsx["kind"], json!("compound"));
        assert_eq!(
            projection_code(&jsx),
            "(item) => <><div title=\"raw\">hello {item}</div><Comp {..._ctx.props}>{_ctx.outside}</Comp></>",
        );

        let tsx = process_expression_test_projection(
            "(item: Item) => <Comp value={item as Item}>{outside}</Comp>",
            json!({
                "prefixIdentifiers": true,
                "identifiers": {},
                "bindingMetadata": {},
                "expressionPlugins": ["typescript", "jsx"]
            }),
        );

        assert_eq!(tsx["kind"], json!("compound"));
        assert_eq!(
            projection_code(&tsx),
            "(item: Item) => <Comp value={item as Item}>{_ctx.outside}</Comp>",
        );
    }

    #[test]
    fn process_expression_projection_rejects_jsx_above_the_safe_ast_limit() {
        let source = format!("<div>{}{{outside}}</div>", "plain text ".repeat(450));
        assert!(source.len() > PROCESS_EXPRESSION_MAX_SAFE_AST_BYTES);
        let projection = process_expression_test_projection(
            &source,
            json!({
                "prefixIdentifiers": true,
                "identifiers": {},
                "bindingMetadata": {},
                "expressionPlugins": ["jsx"]
            }),
        );

        assert_eq!(projection["kind"], json!("error"));
        assert_eq!(projection["code"], json!(46));
        assert_eq!(
            projection["message"],
            json!(PROCESS_EXPRESSION_AST_LIMIT_MESSAGE),
        );
    }

    #[test]
    fn process_expression_projection_scopes_local_declarations() {
        let projection = process_expression_test_statement_projection(
            "function run(input) { use(hoisted); var hoisted = source; { let block = hoisted; use(block) } try { throw input } catch ({ message }) { use(message) } for (const item of items) { use(item) } return hoisted }; hoisted + block + message + item",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(
            projection_code(&projection),
            "function run(input) { _ctx.use(hoisted); var hoisted = _ctx.source; { let block = hoisted; _ctx.use(block) } try { throw input } catch ({ message }) { _ctx.use(message) } for (const item of _ctx.items) { _ctx.use(item) } return hoisted }; _ctx.hoisted + _ctx.block + _ctx.message + _ctx.item",
        );

        let no_call = process_expression_test_statement_projection(
            "let local = source; local",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );
        assert_eq!(
            projection_code(&no_call),
            "let local = _ctx.source; local",
        );

        let using = process_expression_test_statement_projection(
            "using resource = source; resource; using + source",
            json!({ "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }),
        );
        assert_eq!(
            projection_code(&using),
            "using resource = _ctx.source; resource; _ctx.using + _ctx.source",
        );
    }

    #[test]
    fn process_expression_projection_rewrites_setup_let_assignment_rhs() {
        let projection = process_expression_test_projection(
            "(async () => { x = await bar })()",
            json!({
                "prefixIdentifiers": true,
                "inline": true,
                "identifiers": {},
                "bindingMetadata": {
                    "x": "setup-let",
                    "bar": "setup-const"
                }
            }),
        );

        assert_eq!(projection["kind"], json!("compound"));
        assert_eq!(
            projection["children"][1]["content"],
            json!("_isRef(x) ? x.value = await bar : x")
        );
        assert_eq!(projection["children"][3]["content"], json!("bar"));
        assert_eq!(projection["helpers"], json!(["IS_REF"]));

        let scoped = process_expression_test_statement_projection(
            "function run(input) { target = input + hoisted + outside; var hoisted = source }",
            json!({
                "prefixIdentifiers": true,
                "inline": true,
                "identifiers": {},
                "bindingMetadata": { "target": "setup-let" }
            }),
        );
        let target = scoped["children"]
            .as_array()
            .and_then(|children| {
                children.iter().find(|child| {
                    child
                        .get("content")
                        .and_then(Value::as_str)
                        .is_some_and(|content| content.starts_with("_isRef(target)"))
                })
            })
            .expect("setup-let replacement");
        assert_eq!(
            target["content"],
            json!("_isRef(target) ? target.value = input + hoisted + _ctx.outside : target"),
        );

        let arrow = process_expression_test_projection(
            "(input) => { target = input + outside }",
            json!({
                "prefixIdentifiers": true,
                "inline": true,
                "identifiers": {},
                "bindingMetadata": { "target": "setup-let" }
            }),
        );
        let target = arrow["children"]
            .as_array()
            .and_then(|children| {
                children.iter().find(|child| {
                    child
                        .get("content")
                        .and_then(Value::as_str)
                        .is_some_and(|content| content.starts_with("_isRef(target)"))
                })
            })
            .expect("arrow setup-let replacement");
        assert_eq!(
            target["content"],
            json!("_isRef(target) ? target.value = input + _ctx.outside : target"),
        );

        let template = process_expression_test_statement_projection(
            r#"function run(input) { target = `${input}:${outside}` }"#,
            json!({
                "prefixIdentifiers": true,
                "inline": true,
                "identifiers": {},
                "bindingMetadata": { "target": "setup-let" }
            }),
        );
        let target = template["children"]
            .as_array()
            .and_then(|children| {
                children.iter().find(|child| {
                    child
                        .get("content")
                        .and_then(Value::as_str)
                        .is_some_and(|content| content.starts_with("_isRef(target)"))
                })
            })
            .expect("template setup-let replacement");
        assert_eq!(
            target["content"],
            json!("_isRef(target) ? target.value = `${input}:${_ctx.outside}` : target"),
        );
    }

    #[test]
    fn process_expression_projection_rewrites_inline_assignment_update_and_destructure() {
        let context = json!({
            "prefixIdentifiers": true,
            "inline": true,
            "identifiers": {},
            "bindingMetadata": {
                "count": "setup-ref",
                "maybe": "setup-maybe-ref",
                "lett": "setup-let",
                "key": "setup-let",
                "rest": "setup-ref",
                "val": "setup-const"
            }
        });

        let assignment = process_expression_test_statement_projection(
            "count = 1; maybe = count; lett += count",
            context.clone(),
        );
        let code = projection_code(&assignment);
        assert!(code.contains("count.value = 1"), "{code}");
        assert!(code.contains("maybe.value = count.value"), "{code}");
        assert!(
            code.contains("_isRef(lett) ? lett.value += count.value : lett += count.value"),
            "{code}"
        );
        assert_eq!(assignment["helpers"], json!(["IS_REF"]));

        let update = process_expression_test_statement_projection(
            "count++; --maybe; lett--",
            context.clone(),
        );
        let code = projection_code(&update);
        assert!(code.contains("count.value++"), "{code}");
        assert!(code.contains("--maybe.value"), "{code}");
        assert!(
            code.contains("_isRef(lett) ? lett.value-- : lett--"),
            "{code}"
        );

        let destructure = process_expression_test_statement_projection(
            "({ count } = val); [maybe] = val; ({ lett } = val)",
            context.clone(),
        );
        let code = projection_code(&destructure);
        assert!(code.contains("({ count: count.value } = val)"), "{code}");
        assert!(code.contains("[maybe.value] = val"), "{code}");
        assert!(code.contains("({ lett: lett } = val)"), "{code}");

        let nested = process_expression_test_statement_projection(
            "({ nested: { maybe = fallback }, [key]: lett, ...rest } = val)",
            context,
        );
        assert_eq!(
            projection_code(&nested),
            "({ nested: { maybe: maybe.value = _ctx.fallback }, [_unref(key)]: lett, ...rest.value } = val)",
        );
    }

    #[test]
    fn js_ast_extract_identifiers_projects_destructure_patterns() {
        let pattern = json!({
            "type": "ObjectPattern",
            "properties": [{
                "type": "ObjectProperty",
                "key": { "type": "Identifier", "name": "foo", "start": 8, "end": 11 },
                "value": {
                    "type": "AssignmentPattern",
                    "left": { "type": "Identifier", "name": "bar", "start": 15, "end": 18 },
                    "right": { "type": "Identifier", "name": "baz", "start": 21, "end": 24 }
                },
                "computed": false,
                "shorthand": false
            }, {
                "type": "RestElement",
                "argument": { "type": "Identifier", "name": "rest", "start": 29, "end": 33 }
            }]
        });
        let projection = extract_identifiers_projection(&json!({ "node": pattern }));
        let names = projection["identifiers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["bar", "rest"]);
    }

    #[test]
    fn js_ast_function_type_projection_recognizes_babel_function_nodes() {
        for kind in [
            "FunctionDeclaration",
            "FunctionExpression",
            "ArrowFunctionExpression",
            "ObjectMethod",
            "ClassMethod",
            "ClassPrivateMethod",
        ] {
            let projection = is_function_type_projection(&json!({
                "node": { "type": kind }
            }));
            assert_eq!(projection["isFunctionType"], json!(true), "{kind}");
        }

        let projection = is_function_type_projection(&json!({
            "node": { "type": "ObjectProperty" }
        }));
        assert_eq!(projection["isFunctionType"], json!(false));
    }

    #[test]
    fn js_ast_reference_projection_excludes_declarations_and_params() {
        let parent_stack = vec![json!({
            "type": "VariableDeclaration",
            "kind": "const"
        })];
        let parent = json!({
            "type": "VariableDeclarator",
            "id": { "type": "Identifier", "name": "foo" },
            "init": { "type": "Identifier", "name": "bar" }
        });
        let id = json!({ "type": "Identifier", "name": "foo" });
        assert!(!js_ast_is_referenced_identifier(
            &id,
            &parent,
            &parent_stack,
            Some("id")
        ));
        let id = json!({ "type": "Identifier", "name": "bar" });
        assert!(js_ast_is_referenced_identifier(
            &id,
            &parent,
            &parent_stack,
            Some("init")
        ));

        let function = json!({
            "type": "FunctionDeclaration",
            "id": { "type": "Identifier", "name": "test" },
            "params": [{ "type": "Identifier", "name": "foo" }],
            "body": { "type": "BlockStatement", "body": [] }
        });
        let id = json!({ "type": "Identifier", "name": "foo" });
        assert!(!js_ast_is_referenced_identifier(
            &id,
            &function,
            &[],
            Some("params")
        ));
    }

    #[test]
    fn js_ast_reference_projection_excludes_destructured_function_params() {
        let id = json!({ "type": "Identifier", "name": "title" });
        let property = json!({
            "type": "ObjectProperty",
            "key": { "type": "Identifier", "name": "title" },
            "value": id,
            "computed": false,
            "shorthand": true
        });
        let pattern = json!({
            "type": "ObjectPattern",
            "properties": [property]
        });
        let function = json!({
            "type": "ArrowFunctionExpression",
            "params": [pattern],
            "body": { "type": "ArrayExpression", "elements": [] }
        });
        let statement = json!({
            "type": "ExpressionStatement",
            "expression": function
        });
        let parent = &statement["expression"]["params"][0]["properties"][0];
        let parent_stack = vec![
            statement.clone(),
            statement["expression"].clone(),
            statement["expression"]["params"][0].clone(),
            parent.clone(),
        ];

        assert!(!js_ast_is_referenced_identifier(
            &statement["expression"]["params"][0]["properties"][0]["key"],
            parent,
            &parent_stack,
            Some("key")
        ));
        assert!(!js_ast_is_referenced_identifier(
            &statement["expression"]["params"][0]["properties"][0]["value"],
            parent,
            &parent_stack,
            Some("value")
        ));
    }

    #[test]
    fn js_ast_walk_identifiers_respects_function_and_for_locals() {
        let root = json!({
            "type": "Program",
            "body": [{
                "type": "FunctionDeclaration",
                "id": { "type": "Identifier", "name": "test" },
                "params": [{ "type": "Identifier", "name": "foo", "start": 14, "end": 17 }],
                "body": {
                    "type": "BlockStatement",
                    "body": [{
                        "type": "ExpressionStatement",
                        "expression": {
                            "type": "CallExpression",
                            "callee": { "type": "Identifier", "name": "console", "start": 27, "end": 34 },
                            "arguments": [{ "type": "Identifier", "name": "foo", "start": 39, "end": 42 }]
                        }
                    }]
                }
            }, {
                "type": "ForStatement",
                "init": {
                    "type": "VariableDeclaration",
                    "kind": "let",
                    "declarations": [{
                        "type": "VariableDeclarator",
                        "id": { "type": "Identifier", "name": "i", "start": 55, "end": 56 },
                        "init": { "type": "NumericLiteral", "value": 0 }
                    }]
                },
                "test": {
                    "type": "BinaryExpression",
                    "left": { "type": "Identifier", "name": "i", "start": 63, "end": 64 },
                    "right": { "type": "Identifier", "name": "len", "start": 67, "end": 70 }
                },
                "update": {
                    "type": "UpdateExpression",
                    "argument": { "type": "Identifier", "name": "i", "start": 72, "end": 73 },
                    "operator": "++",
                    "prefix": false
                },
                "body": { "type": "BlockStatement", "body": [] }
            }]
        });
        let projection = walk_identifiers_projection(&json!({ "root": root }));
        let names = projection["identifiers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["console", "len"]);
    }

    #[test]
    fn js_ast_destructure_assignment_detects_object_pattern_assignment() {
        let parent = json!({
            "type": "ObjectProperty",
            "key": { "type": "Identifier", "name": "foo" },
            "value": { "type": "Identifier", "name": "foo" },
            "computed": false,
            "shorthand": true
        });
        let stack = vec![
            json!({
                "type": "AssignmentExpression",
                "left": {
                    "type": "ObjectPattern",
                    "properties": [parent.clone()]
                }
            }),
            json!({
                "type": "ObjectPattern",
                "properties": [parent.clone()]
            }),
        ];
        assert!(js_ast_is_in_destructure_assignment(&parent, &stack));
    }

    #[test]
    fn transform_expression_projection_processes_interpolation_content() {
        let projection = transform_expression_projection(&json!({
            "node": {
                "type": 5,
                "content": {
                    "type": 4,
                    "content": "foo",
                    "isStatic": false
                }
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        assert_eq!(projection["operations"][0]["path"], json!(["content"]));
        assert_eq!(
            projection["operations"][0]["projection"]["content"],
            json!("_ctx.foo")
        );
    }

    #[test]
    fn transform_expression_projection_processes_slot_params_but_skips_on_handlers() {
        let projection = transform_expression_projection(&json!({
            "node": {
                "type": 1,
                "props": [
                    {
                        "type": 7,
                        "name": "slot",
                        "arg": { "type": 4, "content": "foo", "isStatic": true },
                        "exp": { "type": 4, "content": "{ bar }", "isStatic": false }
                    },
                    {
                        "type": 7,
                        "name": "on",
                        "arg": { "type": 4, "content": "click", "isStatic": true },
                        "exp": { "type": 4, "content": "submit", "isStatic": false }
                    },
                    {
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "name", "isStatic": false },
                        "exp": { "type": 4, "content": "value", "isStatic": false }
                    }
                ]
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        let operations = projection["operations"].as_array().expect("operations");
        assert_eq!(operations.len(), 3);
        assert_eq!(operations[0]["path"], json!(["props", "0", "exp"]));
        assert_eq!(operations[0]["projection"]["identifiers"], json!(["bar"]));
        assert_eq!(operations[1]["path"], json!(["props", "2", "exp"]));
        assert_eq!(operations[2]["path"], json!(["props", "2", "arg"]));
    }

    #[test]
    fn transform_expression_projection_skips_v_memo_key_expression() {
        let projection = transform_expression_projection(&json!({
            "node": {
                "type": 1,
                "props": [
                    { "type": 7, "name": "memo" },
                    {
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "key", "isStatic": true },
                        "exp": { "type": 4, "content": "item.id", "isStatic": false }
                    }
                ]
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        assert_eq!(projection["operations"], json!([]));
    }

    #[test]
    fn transform_once_projection_enters_once_node() {
        let projection = transform_once_projection(&json!({
            "node": {
                "type": 1,
                "props": [{ "type": 7, "name": "once" }]
            },
            "context": {}
        }));

        assert_eq!(projection["kind"], json!("enter"));
        assert_eq!(projection["helper"], json!("SET_BLOCK_TRACKING"));
        assert_eq!(projection["exit"]["cacheCodegen"], json!(true));
        assert_eq!(projection["exit"]["inVOnce"], json!(true));
    }

    #[test]
    fn transform_once_projection_skips_seen_nested_and_ssr_nodes() {
        let node = json!({
            "type": 1,
            "props": [{ "type": 7, "name": "once" }]
        });

        assert_eq!(
            transform_once_projection(&json!({ "node": node, "context": {}, "seen": true }))
                ["kind"],
            json!("noop")
        );
        assert_eq!(
            transform_once_projection(&json!({ "node": node, "context": { "inVOnce": true } }))
                ["kind"],
            json!("noop")
        );
        assert_eq!(
            transform_once_projection(&json!({ "node": node, "context": { "inSSR": true } }))
                ["kind"],
            json!("noop")
        );
    }

    #[test]
    fn transform_memo_projection_enters_plain_element() {
        let projection = transform_memo_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 0,
                "props": [{
                    "type": 7,
                    "name": "memo",
                    "exp": { "type": 4, "content": "[x]", "isStatic": false }
                }]
            },
            "context": { "cachedLength": 3 }
        }));

        assert_eq!(projection["kind"], json!("enter"));
        assert_eq!(projection["exit"]["helper"], json!("WITH_MEMO"));
        assert_eq!(projection["exit"]["convertToBlock"], json!(true));
        assert_eq!(projection["exit"]["cacheIndex"], json!(3));
        assert_eq!(projection["exit"]["exp"]["content"], json!("[x]"));
    }

    #[test]
    fn transform_memo_projection_keeps_component_vnode_shape() {
        let projection = transform_memo_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [{
                    "type": 7,
                    "name": "memo",
                    "exp": { "type": 4, "content": "[x]", "isStatic": false }
                }]
            },
            "context": {}
        }));

        assert_eq!(projection["kind"], json!("enter"));
        assert_eq!(projection["exit"]["convertToBlock"], json!(false));
    }

    #[test]
    fn transform_memo_projection_skips_seen_ssr_and_empty_nodes() {
        let node = json!({
            "type": 1,
            "tagType": 0,
            "props": [{
                "type": 7,
                "name": "memo",
                "exp": { "type": 4, "content": "[x]", "isStatic": false }
            }]
        });
        assert_eq!(
            transform_memo_projection(&json!({ "node": node, "context": {}, "seen": true }))
                ["kind"],
            json!("noop")
        );
        assert_eq!(
            transform_memo_projection(&json!({ "node": node, "context": { "inSSR": true } }))
                ["kind"],
            json!("noop")
        );
        assert_eq!(
            transform_memo_projection(&json!({
                "node": { "type": 1, "tagType": 0, "props": [{ "type": 7, "name": "memo" }] },
                "context": {}
            }))["kind"],
            json!("noop")
        );
    }
