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
            context,
        );
        let code = projection_code(&destructure);
        assert!(code.contains("({ count: count.value } = val)"), "{code}");
        assert!(code.contains("[maybe.value] = val"), "{code}");
        assert!(code.contains("({ lett: lett } = val)"), "{code}");
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
