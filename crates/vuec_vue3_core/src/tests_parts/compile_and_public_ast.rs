    use crate::*;
    use vuec_ast::PublicProjection;

    #[test]
    fn parse_transform_generate_roundtrip() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div>hello</div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(source, Vue3CompilerOptions::default());
        assert!(result.code.contains("render"));
        assert!(result.ast_summary.contains("nodes="));
    }

    #[test]
    fn ssr_wraps_code() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = compile_ssr(source, Vue3CompilerOptions::default());
        assert!(result.code.starts_with("/* ssr */"));
    }

    #[test]
    fn generate_public_ast_emits_root_imports() {
        let ast = json!({
            "type": 0,
            "helpers": ["openBlock", "createElementBlock"],
            "components": [],
            "directives": [],
            "hoists": [],
            "imports": [{
                "exp": {
                    "type": 4,
                    "content": "_imports_0",
                    "isStatic": false,
                    "constType": 3
                },
                "path": "./logo.png"
            }],
            "cached": [],
            "temps": 0,
            "codegenNode": {
                "type": 13,
                "tag": "\"img\"",
                "props": {
                    "type": 15,
                    "properties": [{
                        "type": 16,
                        "key": {
                            "type": 4,
                            "content": "src",
                            "isStatic": true
                        },
                        "value": {
                            "type": 4,
                            "content": "_imports_0",
                            "isStatic": false
                        }
                    }]
                },
                "children": null,
                "isBlock": true,
                "isComponent": false
            }
        });

        let result = generate_public_ast(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains("import _imports_0 from './logo.png'"));
        assert!(result
            .code
            .contains("import _imports_0 from './logo.png'\n\n\nexport function render"));
        assert!(result.code.contains("src: _imports_0"));
        assert!(!result.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn generate_public_ast_separates_root_imports_from_hoists() {
        let ast = json!({
            "type": 0,
            "helpers": ["openBlock", "createElementBlock", "createElementVNode", "Fragment"],
            "components": [],
            "directives": [],
            "hoists": [{
                "type": 4,
                "content": "_imports_0 + '#fragment'",
                "isStatic": false,
                "constType": 3
            }],
            "imports": [{
                "exp": {
                    "type": 4,
                    "content": "_imports_0",
                    "isStatic": false,
                    "constType": 3
                },
                "path": "./icons.svg"
            }],
            "cached": [],
            "temps": 0,
            "codegenNode": {
                "type": 13,
                "tag": "_Fragment",
                "props": null,
                "children": [{
                    "type": 13,
                    "tag": "\"use\"",
                    "props": {
                        "type": 15,
                        "properties": [{
                            "type": 16,
                            "key": {
                                "type": 4,
                                "content": "href",
                                "isStatic": true
                            },
                            "value": {
                                "type": 4,
                                "content": "_hoisted_1",
                                "isStatic": false
                            }
                        }]
                    },
                    "children": null,
                    "isBlock": false,
                    "isComponent": false
                }],
                "patchFlag": "64",
                "isBlock": true,
                "isComponent": false
            }
        });

        let result = generate_public_ast(
            &ast,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(result.code.contains(
            "import _imports_0 from './icons.svg'\n\n\nconst _hoisted_1 = _imports_0 + '#fragment'\n\nexport function render"
        ));
    }

    #[test]
    fn template_base_offset_maps_nodes_to_original_file_spans() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div>{{ msg }}</div>".into(),
            file_id: FileId(7),
            base_offset: 42,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        assert_eq!(root.span.source(), Some(Span::new(FileId(7), 42, 62)));
        let element = ast.node(root.children[0]).expect("element");
        assert_eq!(element.span.source(), Some(Span::new(FileId(7), 42, 62)));
        let interpolation = ast.node(element.children[0]).expect("interpolation child");
        assert_eq!(
            interpolation.span.source(),
            Some(Span::new(FileId(7), 47, 56))
        );
    }
