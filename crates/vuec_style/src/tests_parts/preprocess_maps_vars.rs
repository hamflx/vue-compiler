    #[test]
    fn compiles_css_modules_relative_imports_before_locals_convention_projection() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(
            &dep,
            ".foo-bar { color: blue; }\n:export { theme-color: green; }",
        )
        .expect("write dep");

        let result = compile_style(
            ":import(\"./dep.css\") { shade: theme-color; }\n.button { composes: foo-bar from \"./dep.css\"; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                modules_options: CssModulesOptions {
                    locals_convention: "dashesOnly".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_foo-bar_"));
        assert!(result.code.contains("color: green"));
        assert!(!modules.contains_key("foo-bar"));
        assert!(!modules.contains_key("fooBar"));
    }

    #[test]
    fn source_map_tracks_original_style_source_lines() {
        let source = ".a { color: red; }\n.b { color: blue; }";
        let result = compile_style(
            source,
            StyleCompileOptions {
                filename: Some("component.vue".into()),
                source_map_source: Some(format!("<style>\n{source}\n</style>")),
                source_map_file_id: Some(FileId(7)),
                source_map_base_offset: "<style>\n".len(),
                source_map: true,
                ..StyleCompileOptions::default()
            },
        );
        let map = result.map.expect("style source map");

        assert_eq!(map.sources, vec!["component.vue"]);
        assert_eq!(
            map.sources_content
                .as_ref()
                .and_then(|sources| sources[0].as_ref()),
            Some(&format!("<style>\n{source}\n</style>"))
        );
        let first = map
            .original_position(vuec_source::GeneratedPosition::new(0, 0))
            .unwrap()
            .expect("first mapping");
        assert_eq!(first.source, "component.vue");
        assert_eq!(first.line, 1);
        assert_eq!(first.column, 0);
        let second = map
            .original_position(vuec_source::GeneratedPosition::new(1, 0))
            .unwrap()
            .expect("second mapping");
        assert_eq!(second.line, 2);
        assert_eq!(second.column, 0);
    }

    #[test]
    fn preserves_plain_css_imports_without_missing_import_diagnostics() {
        let source = ".a { color: red; }\n@import \"./not-missing.css\";\n@import \"missing.css\";";
        let result = compile_style(source, StyleCompileOptions::default());

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.code.contains("@import \"./not-missing.css\";"));
        assert!(result.code.contains("@import \"missing.css\";"));
    }

    #[test]
    fn less_unresolved_import_reports_resolver_source_span() {
        let source = ".a { color: red; }\n  @import \"./theme\";\n.b { color: blue; }";
        let result = compile_style(
            source,
            StyleCompileOptions {
                preprocess_lang: Some("less".into()),
                source_map_file_id: Some(FileId(9)),
                source_map_base_offset: 100,
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(
            result.errors,
            vec!["Less import could not be resolved: ./theme"]
        );
        assert_eq!(result.diagnostics.len(), 1);
        let diagnostic = &result.diagnostics[0];
        assert_eq!(diagnostic.code, "VUEC_STYLE_IMPORT_RESOLVE");
        let start = ".a { color: red; }\n  ".len();
        let end = start + "@import \"./theme\";".len();
        assert_eq!(
            diagnostic.span,
            Some(Span::new(FileId(9), 100 + start, 100 + end))
        );
    }

    #[test]
    fn preprocesses_vue27_style_languages_before_css_transforms() {
        let less = compile_style(
            "@red: rgb(255, 0, 0);\n.color { color: @red; }",
            StyleCompileOptions {
                preprocess_lang: Some("less".into()),
                source_map: true,
                ..StyleCompileOptions::default()
            },
        );
        assert!(less.errors.is_empty());
        assert!(less.code.contains("color: #ff0000;"));
        assert!(less.map.is_some());

        let scss = compile_style(
            "$red: red;\n.color { color: $red; .child { width: 1px; } }",
            StyleCompileOptions {
                preprocess_lang: Some("scss".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(scss.code.contains("color: red;"));
        assert!(scss.code.contains(".color .child"));

        let sass = compile_style(
            "$red: red\n.color\n  color: $red",
            StyleCompileOptions {
                preprocess_lang: Some("sass".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(sass.code.contains("color: red;"));

        let stylus = compile_style(
            "red-color = rgb(255, 0, 0);\n.color\n  color: red-color",
            StyleCompileOptions {
                preprocess_lang: Some("styl".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(stylus.code.contains("color: #f00;"));
    }

    #[test]
    fn preprocesses_less_variables_nested_selectors_and_media() {
        let result = compile_style(
            r#"
@red: rgb(255, 0, 0);
.card, .panel {
  @gap: 8px;
  color: @red;
  padding: @gap;
  &:hover {
    color: blue;
  }
  .title {
    margin: @gap;
  }
  @media (min-width: 600px) {
    display: block;
    .title {
      color: @red;
    }
  }
}
.other {
  color: @red;
}
"#,
            StyleCompileOptions {
                preprocess_lang: Some("less".into()),
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".card, .panel {"));
        assert!(result.code.contains("color: #ff0000;"));
        assert!(result.code.contains("padding: 8px;"));
        assert!(result.code.contains(".card:hover, .panel:hover {"));
        assert!(result.code.contains(".card .title, .panel .title {"));
        assert!(result.code.contains("@media (min-width: 600px) {"));
        assert!(result.code.contains("display: block;"));
        assert!(result.code.contains(".other {"));
        assert!(!result.code.contains("@red"));
        assert!(!result.code.contains("@gap"));
    }

    #[test]
    fn preprocesses_less_additional_data_imports_and_load_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let shared_dir = dir.path().join("shared");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&shared_dir).expect("shared dir");
        let base = src_dir.join("component.less");
        let local_import = src_dir.join("local.less");
        let load_path_import = shared_dir.join("tokens.less");
        std::fs::write(
            &local_import,
            r#"
.imported {
  border-color: @brand;
}
"#,
        )
        .expect("write local import");
        std::fs::write(
            &load_path_import,
            r#"
@space: 12px;
.shared {
  margin: @space;
}
"#,
        )
        .expect("write load path import");

        let result = compile_style(
            r#"
@import "./local.less";
@import "tokens";
@import "https://example.com/reset.css";
.root {
  color: @brand;
  padding: @space;
}
"#,
            StyleCompileOptions {
                filename: Some(base.to_string_lossy().into_owned()),
                preprocess_lang: Some("less".into()),
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("@brand: red;".into()),
                    load_paths: vec![shared_dir.to_string_lossy().into_owned()],
                },
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result
            .code
            .contains("@import \"https://example.com/reset.css\";"));
        assert!(result.code.contains(".imported {"));
        assert!(result.code.contains("border-color: red;"));
        assert!(result.code.contains(".shared {"));
        assert!(result.code.contains("margin: 12px;"));
        assert!(result.code.contains("padding: 12px;"));
        let mut expected = vec![
            std::fs::canonicalize(local_import)
                .expect("canonical local import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
            std::fs::canonicalize(load_path_import)
                .expect("canonical load path import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
        ];
        expected.sort();
        assert_eq!(result.dependencies, expected);
    }

    #[test]
    fn preprocesses_stylus_variables_nested_selectors_and_media() {
        let result = compile_style(
            r#"
red-color = rgb(255, 0, 0)
gap = 8px
.card, .panel
  color red-color
  padding: gap
  &:hover
    color blue
  .title
    margin gap
  @media (min-width: 600px)
    display block
    .title
      color red-color
.other
  color red-color
"#,
            StyleCompileOptions {
                preprocess_lang: Some("styl".into()),
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".card, .panel {"));
        assert!(result.code.contains("color: #f00;"));
        assert!(result.code.contains("padding: 8px;"));
        assert!(result.code.contains(".card:hover, .panel:hover {"));
        assert!(result.code.contains(".card .title, .panel .title {"));
        assert!(result.code.contains("@media (min-width: 600px) {"));
        assert!(result.code.contains("display: block;"));
        assert!(result.code.contains(".other {"));
        assert!(!result.code.contains("red-color"));
        assert!(!result.code.contains("gap"));
    }

    #[test]
    fn lightweight_preprocessor_variables_respect_tokens_and_lexical_context() {
        fn environment(entries: &[(&str, &str)]) -> StyleVariableEnvironment {
            std::sync::Arc::new(
                entries
                    .iter()
                    .map(|(name, value)| ((*name).to_string(), std::sync::Arc::from(*value)))
                    .collect(),
            )
        }

        let stylus = environment(&[("tone", "red"), ("$tone", "blue"), ("tone-dark", "navy")]);
        let mut stylus_evaluator =
            StyleVariableEvaluator::new(stylus.as_ref(), StyleVariableSyntax::StylusBare);
        let mut stylus_budget = StyleVariableExpansionBudget::default();
        assert_eq!(
            stylus_evaluator
                .resolve(
                    "tone $tone tone-dark undertone tone_ 'tone $tone' /* tone */ // tone",
                    &mut stylus_budget,
                )
                .expect("resolve Stylus variables"),
            "red blue navy undertone tone_ 'tone $tone' /* tone */ // tone"
        );
        assert_eq!(
            stylus_evaluator
                .resolve("étone tone", &mut stylus_budget)
                .expect("resolve Unicode token boundaries"),
            "étone red"
        );

        let less = environment(&[("tone", "red"), ("tone-dark", "blue")]);
        let mut less_evaluator =
            StyleVariableEvaluator::new(less.as_ref(), StyleVariableSyntax::LessAt);
        let mut less_budget = StyleVariableExpansionBudget::default();
        assert_eq!(
            less_evaluator
                .resolve(
                    "@tone @tone-dark @undertone '@tone' \"@{tone}\" /* @tone */ // @tone",
                    &mut less_budget,
                )
                .expect("resolve Less variables"),
            "red blue @undertone '@tone' \"red\" /* @tone */ // @tone"
        );
        assert_eq!(
            less_evaluator
                .resolve("@toneé @tone", &mut less_budget)
                .expect("resolve Unicode Less token boundaries"),
            "@toneé red"
        );

        let stylus_result = compile_style(
            "tone = red\n$tone = blue\n.example\n  color tone\n  border-color $tone\n  content 'tone'\n",
            StyleCompileOptions {
                preprocess_lang: Some("styl".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(stylus_result.errors.is_empty(), "{:?}", stylus_result.errors);
        assert!(stylus_result.code.contains("color: red;"));
        assert!(stylus_result.code.contains("border-color: blue;"));
        assert!(stylus_result.code.contains("content: 'tone';"));

        let less_result = compile_style(
            "@breakpoint: 600px;\n.lazy { width: @var; @a: 9%; content: '@a'; note: \"@{a}\"; @media (min-width: @breakpoint) { height: @a; } }\n@var: @a;\n@a: 100%;",
            StyleCompileOptions {
                preprocess_lang: Some("less".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(less_result.errors.is_empty(), "{:?}", less_result.errors);
        assert!(less_result.code.contains("width: 9%;"));
        assert!(less_result.code.contains("content: '@a';"));
        assert!(less_result.code.contains("note: \"9%\";"));
        assert!(less_result.code.contains("@media (min-width: 600px)"));
        assert!(less_result.code.contains("height: 9%;"));
    }

    #[test]
    fn lightweight_preprocessor_variable_resolution_detects_cycles_and_depth() {
        fn environment(entries: &[(&str, &str)]) -> StyleVariableEnvironment {
            std::sync::Arc::new(
                entries
                    .iter()
                    .map(|(name, value)| ((*name).to_string(), std::sync::Arc::from(*value)))
                    .collect(),
            )
        }

        let recursive = environment(&[("a", "a a")]);
        let mut evaluator =
            StyleVariableEvaluator::new(recursive.as_ref(), StyleVariableSyntax::StylusBare);
        let error = evaluator
            .resolve("a", &mut StyleVariableExpansionBudget::default())
            .expect_err("recursive Stylus variable must fail");
        assert_eq!(error.code, "VUEC_STYLE_VARIABLE_RESOLVE");
        assert_eq!(
            error.message,
            "recursive style preprocessor variable reference: a"
        );

        let mutual = environment(&[("a", "@b"), ("b", "@a")]);
        let mut evaluator =
            StyleVariableEvaluator::new(mutual.as_ref(), StyleVariableSyntax::LessAt);
        let error = evaluator
            .resolve("@a", &mut StyleVariableExpansionBudget::default())
            .expect_err("mutually recursive Less variables must fail");
        assert_eq!(error.code, "VUEC_STYLE_VARIABLE_RESOLVE");

        let quoted = environment(&[("a", "'a'")]);
        let mut evaluator =
            StyleVariableEvaluator::new(quoted.as_ref(), StyleVariableSyntax::StylusBare);
        assert_eq!(
            evaluator
                .resolve("a", &mut StyleVariableExpansionBudget::default())
                .expect("quoted identifier is not recursive"),
            "'a'"
        );

        let chain = environment(&[("a", "b"), ("b", "c"), ("c", "red")]);
        let mut exact_budget = StyleVariableExpansionBudget {
            limits: StyleVariableExpansionLimits {
                max_depth: 3,
                ..StyleVariableExpansionLimits::default()
            },
            ..StyleVariableExpansionBudget::default()
        };
        let mut evaluator =
            StyleVariableEvaluator::new(chain.as_ref(), StyleVariableSyntax::StylusBare);
        assert_eq!(
            evaluator
                .resolve("a", &mut exact_budget)
                .expect("exact variable depth"),
            "red"
        );

        let mut shallow_budget = StyleVariableExpansionBudget {
            limits: StyleVariableExpansionLimits {
                max_depth: 2,
                ..StyleVariableExpansionLimits::default()
            },
            ..StyleVariableExpansionBudget::default()
        };
        let mut evaluator =
            StyleVariableEvaluator::new(chain.as_ref(), StyleVariableSyntax::StylusBare);
        let error = evaluator
            .resolve("a", &mut shallow_budget)
            .expect_err("variable beyond depth limit must fail");
        assert_eq!(error.code, "VUEC_STYLE_VARIABLE_LIMIT");
        assert_eq!(
            error.message,
            "style preprocessor variable references exceed the maximum depth of 2"
        );

        let compiled = compile_style(
            "@a: @a @a;\n.example { color: @a; }",
            StyleCompileOptions {
                preprocess_lang: Some("less".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert_eq!(compiled.diagnostics.len(), 1);
        assert_eq!(
            compiled.diagnostics[0].code,
            "VUEC_STYLE_VARIABLE_RESOLVE"
        );
    }

    #[test]
    fn lightweight_preprocessor_variable_resolution_enforces_work_and_byte_budgets() {
        let variables = std::sync::Arc::new(BTreeMap::from([(
            "value".to_string(),
            std::sync::Arc::<str>::from("1234"),
        )]));

        let mut exact = StyleVariableExpansionBudget {
            limits: StyleVariableExpansionLimits {
                max_depth: 1,
                max_steps: 1,
                max_value_bytes: 4,
                max_total_bytes: 8,
            },
            ..StyleVariableExpansionBudget::default()
        };
        let mut evaluator =
            StyleVariableEvaluator::new(variables.as_ref(), StyleVariableSyntax::StylusBare);
        assert_eq!(
            evaluator
                .resolve("value", &mut exact)
                .expect("exact variable budgets"),
            "1234"
        );
        assert_eq!(exact.steps, 1);
        assert_eq!(exact.total_bytes, 8);

        let mut short_value = StyleVariableExpansionBudget {
            limits: StyleVariableExpansionLimits {
                max_value_bytes: 3,
                ..StyleVariableExpansionLimits::default()
            },
            ..StyleVariableExpansionBudget::default()
        };
        let mut evaluator =
            StyleVariableEvaluator::new(variables.as_ref(), StyleVariableSyntax::StylusBare);
        let error = evaluator
            .resolve("value", &mut short_value)
            .expect_err("variable beyond value byte limit must fail");
        assert_eq!(error.code, "VUEC_STYLE_VARIABLE_LIMIT");

        let mut short_total = StyleVariableExpansionBudget {
            limits: StyleVariableExpansionLimits {
                max_total_bytes: 7,
                ..StyleVariableExpansionLimits::default()
            },
            ..StyleVariableExpansionBudget::default()
        };
        let mut evaluator =
            StyleVariableEvaluator::new(variables.as_ref(), StyleVariableSyntax::StylusBare);
        let error = evaluator
            .resolve("value", &mut short_total)
            .expect_err("variable beyond total byte limit must fail");
        assert_eq!(error.code, "VUEC_STYLE_VARIABLE_LIMIT");
        assert_eq!(
            error.message,
            "style preprocessor variable expansion exceeds the maximum total of 7 bytes"
        );

        let chain = std::sync::Arc::new(BTreeMap::from([
            ("a".to_string(), std::sync::Arc::<str>::from("b")),
            ("b".to_string(), std::sync::Arc::<str>::from("red")),
        ]));
        let mut one_step = StyleVariableExpansionBudget {
            limits: StyleVariableExpansionLimits {
                max_steps: 1,
                ..StyleVariableExpansionLimits::default()
            },
            ..StyleVariableExpansionBudget::default()
        };
        let mut evaluator =
            StyleVariableEvaluator::new(chain.as_ref(), StyleVariableSyntax::StylusBare);
        let error = evaluator
            .resolve("a", &mut one_step)
            .expect_err("variable beyond work limit must fail");
        assert_eq!(error.code, "VUEC_STYLE_VARIABLE_LIMIT");
        assert_eq!(
            error.message,
            "style preprocessor variable expansion exceeds the maximum step count of 1"
        );
    }

    #[test]
    fn lightweight_preprocessor_variable_scopes_share_and_scale() {
        let inherited = std::sync::Arc::new(BTreeMap::from([(
            "inherited".to_string(),
            std::sync::Arc::<str>::from("red"),
        )]));
        let declaration_only = vec![LessNode::Declaration {
            name: "color".to_string(),
            value: "inherited".to_string(),
        }];
        let shared = less_scope_variables(&declaration_only, &inherited);
        assert!(std::sync::Arc::ptr_eq(&shared, &inherited));

        let mut nodes = (0..1_024)
            .map(|index| LessNode::Variable {
                name: format!("shared-prefix-{index:04}"),
                value: format!("{index}px"),
            })
            .collect::<Vec<_>>();
        nodes.push(LessNode::Variable {
            name: "alias".to_string(),
            value: "shared-prefix-0000".to_string(),
        });
        let variables = less_scope_variables(&nodes, &inherited);
        assert_eq!(variables.len(), 1_026);
        assert_eq!(
            variables.get("shared-prefix-0000").map(AsRef::as_ref),
            Some("0px")
        );
        assert_eq!(
            variables.get("shared-prefix-1023").map(AsRef::as_ref),
            Some("1023px")
        );
        assert_eq!(variables.get("inherited").map(AsRef::as_ref), Some("red"));
        assert_eq!(
            variables.get("alias").map(AsRef::as_ref),
            Some("shared-prefix-0000")
        );
    }

    #[test]
    fn lightweight_stylus_variables_follow_assignment_order_and_scope() {
        let result = compile_style(
            r#"
tone = red
.first
  color tone
tone = blue
.second
  color tone

forward = later
.forward
  color forward
later = green
.direct-future
  color future-tone
future-tone = purple

alias = dependency
dependency = red
.cached
  color alias
dependency = blue
.cached-again
  color alias

.local
  color tone
  tone = green
  border-color tone
  .nested
    color tone
.after-local
  color tone

breakpoint = 600px
@media (min-width: breakpoint)
  .media-first
    color red
breakpoint = 700px
@media (min-width: breakpoint)
  .media-second
    color blue
"#,
            StyleCompileOptions {
                preprocess_lang: Some("styl".into()),
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".first {\n  color: red;"));
        assert!(result.code.contains(".second {\n  color: blue;"));
        assert!(result.code.contains(".forward {\n  color: later;"));
        assert!(
            result
                .code
                .contains(".direct-future {\n  color: future-tone;")
        );
        assert!(result.code.contains(".cached {\n  color: red;"));
        assert!(result.code.contains(".cached-again {\n  color: red;"));
        assert!(result.code.contains(
            ".local {\n  color: blue;\n  border-color: green;"
        ));
        assert!(result.code.contains(".local .nested {\n  color: green;"));
        assert!(result.code.contains(".after-local {\n  color: blue;"));
        assert!(result.code.contains("@media (min-width: 600px)"));
        assert!(result.code.contains("@media (min-width: 700px)"));
    }

    #[test]
    fn lightweight_preprocessors_bound_syntax_nesting() {
        fn nested_less(depth: usize) -> String {
            let mut source = String::new();
            for index in 0..depth {
                source.push_str(&format!(".level-{index} {{"));
            }
            source.push_str("color: red;");
            source.push_str(&"}".repeat(depth));
            source
        }

        fn nested_stylus(depth: usize) -> String {
            let mut source = String::new();
            for index in 0..depth {
                source.push_str(&"  ".repeat(index));
                source.push_str(&format!(".level-{index}\n"));
            }
            source.push_str(&"  ".repeat(depth));
            source.push_str("color red\n");
            source
        }

        assert!(parse_less_nodes(&nested_less(STYLE_PREPROCESS_MAX_NESTING_DEPTH)).is_ok());
        assert!(parse_stylus_nodes(&nested_stylus(STYLE_PREPROCESS_MAX_NESTING_DEPTH)).is_ok());

        let less_error = parse_less_nodes(&nested_less(
            STYLE_PREPROCESS_MAX_NESTING_DEPTH.saturating_add(1),
        ))
        .expect_err("overly nested Less must fail");
        let stylus_error = parse_stylus_nodes(&nested_stylus(
            STYLE_PREPROCESS_MAX_NESTING_DEPTH.saturating_add(1),
        ))
        .expect_err("overly nested Stylus must fail");
        assert_eq!(less_error, STYLE_PREPROCESS_NESTING_ERROR);
        assert_eq!(stylus_error, STYLE_PREPROCESS_NESTING_ERROR);
    }

    #[test]
    fn lightweight_preprocessors_bound_import_depth_and_restore_active_paths() {
        fn context_with_depth(max_depth: usize) -> StyleImportContext {
            let mut context = StyleImportContext::new(&StyleCompileOptions::default());
            context.limits = StyleImportLimits {
                max_depth,
                max_files: 8,
                max_file_bytes: 1_024,
                max_total_bytes: 8_192,
            };
            context
        }

        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("a.less"),
            "@import \"./b.less\";\n.a { color: red; }",
        )
        .expect("write a.less");
        std::fs::write(
            dir.path().join("b.less"),
            "@import \"./c.less\";\n.b { color: blue; }",
        )
        .expect("write b.less");
        std::fs::write(dir.path().join("c.less"), ".c { color: green; }")
            .expect("write c.less");
        std::fs::write(
            dir.path().join("a.styl"),
            "@import \"./b.styl\"\n.a\n  color red\n",
        )
        .expect("write a.styl");
        std::fs::write(
            dir.path().join("b.styl"),
            "@import \"./c.styl\"\n.b\n  color blue\n",
        )
        .expect("write b.styl");
        std::fs::write(dir.path().join("c.styl"), ".c\n  color green\n")
            .expect("write c.styl");

        let mut exact_less = context_with_depth(3);
        let less = inline_less_imports(
            "@import \"./a.less\";",
            Some(dir.path()),
            &mut exact_less,
            true,
        )
        .expect("exact Less import depth");
        assert!(less.contains(".a { color: red; }"));
        assert!(less.contains(".b { color: blue; }"));
        assert!(less.contains(".c { color: green; }"));
        assert!(exact_less.active_paths.is_empty());

        let mut shallow_less = context_with_depth(2);
        let less_error = inline_less_imports(
            "@import \"./a.less\";",
            Some(dir.path()),
            &mut shallow_less,
            true,
        )
        .expect_err("Less import beyond the depth limit must fail");
        assert_eq!(less_error.code, "VUEC_STYLE_IMPORT_LIMIT");
        assert_eq!(
            less_error.message,
            "Less import nesting exceeds the maximum depth of 2"
        );
        assert_eq!(shallow_less.imported_files, 2);
        assert!(shallow_less.active_paths.is_empty());

        let mut exact_stylus = context_with_depth(3);
        let stylus = inline_stylus_imports(
            "@import \"./a.styl\"\n",
            Some(dir.path()),
            &mut exact_stylus,
            true,
        )
        .expect("exact Stylus import depth");
        assert!(stylus.contains(".a\n  color red"));
        assert!(stylus.contains(".b\n  color blue"));
        assert!(stylus.contains(".c\n  color green"));
        assert!(exact_stylus.active_paths.is_empty());

        let mut shallow_stylus = context_with_depth(2);
        let stylus_error = inline_stylus_imports(
            "@import \"./a.styl\"\n",
            Some(dir.path()),
            &mut shallow_stylus,
            true,
        )
        .expect_err("Stylus import beyond the depth limit must fail");
        assert_eq!(stylus_error.code, "VUEC_STYLE_IMPORT_LIMIT");
        assert_eq!(
            stylus_error.message,
            "Stylus import nesting exceeds the maximum depth of 2"
        );
        assert_eq!(shallow_stylus.imported_files, 2);
        assert!(shallow_stylus.active_paths.is_empty());
    }

    #[test]
    fn lightweight_preprocessors_bound_import_count_without_charging_cycles() {
        fn context_with_file_limit(max_files: usize) -> StyleImportContext {
            let mut context = StyleImportContext::new(&StyleCompileOptions::default());
            context.limits = StyleImportLimits {
                max_depth: 4,
                max_files,
                max_file_bytes: 1_024,
                max_total_bytes: 1_024,
            };
            context
        }

        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("first.less"), "").expect("write first.less");
        std::fs::write(dir.path().join("second.less"), "").expect("write second.less");
        let source = "@import \"./first.less\";@import \"./second.less\";";

        let mut exact = context_with_file_limit(2);
        exact.limits.max_file_bytes = 0;
        exact.limits.max_total_bytes = 0;
        inline_less_imports(source, Some(dir.path()), &mut exact, true)
            .expect("exact import file count");
        assert_eq!(exact.imported_files, 2);
        assert_eq!(exact.imported_bytes, 0);

        let mut limited = context_with_file_limit(1);
        limited.limits.max_file_bytes = 0;
        limited.limits.max_total_bytes = 0;
        let error = inline_less_imports(source, Some(dir.path()), &mut limited, true)
            .expect_err("import beyond the file count limit must fail");
        assert_eq!(error.code, "VUEC_STYLE_IMPORT_LIMIT");
        assert_eq!(
            error.message,
            "Less imports exceed the maximum file count of 1"
        );
        assert_eq!(limited.imported_files, 1);
        assert!(limited.active_paths.is_empty());

        let cycle_source = "@import \"./cycle.less\";\n.cycle { color: red; }";
        std::fs::write(dir.path().join("cycle.less"), cycle_source).expect("write cycle.less");
        let mut cycle = context_with_file_limit(1);
        cycle.limits.max_depth = 1;
        let output = inline_less_imports(
            "@import \"./cycle.less\";",
            Some(dir.path()),
            &mut cycle,
            true,
        )
        .expect("an active import cycle must be skipped");
        assert!(output.contains(".cycle { color: red; }"));
        assert_eq!(cycle.imported_files, 1);
        assert_eq!(cycle.imported_bytes, cycle_source.len());
        assert!(cycle.active_paths.is_empty());

        std::fs::write(dir.path().join("empty.less"), "").expect("write empty.less");
        std::fs::write(
            dir.path().join("nested.less"),
            "@import \"./empty.less\";\n.nested { color: blue; }",
        )
        .expect("write nested.less");
        let mut nested = context_with_file_limit(2);
        let nested_output = inline_less_imports(
            "\n@import \"./nested.less\";",
            Some(dir.path()),
            &mut nested,
            true,
        )
        .expect("nested empty import");
        assert_eq!(
            nested_output,
            "\n\n\n.nested { color: blue; }\n"
        );
    }

    #[test]
    fn lightweight_preprocessors_bound_import_file_and_total_bytes() {
        fn context_with_bytes(
            max_file_bytes: usize,
            max_total_bytes: usize,
        ) -> StyleImportContext {
            let mut context = StyleImportContext::new(&StyleCompileOptions::default());
            context.limits = StyleImportLimits {
                max_depth: 4,
                max_files: 4,
                max_file_bytes,
                max_total_bytes,
            };
            context
        }

        let dir = tempfile::tempdir().expect("temp dir");
        let payload = ".payload { color: red; }";
        std::fs::write(dir.path().join("payload.less"), payload).expect("write payload.less");
        let source = "@import \"./payload.less\";";

        let mut exact_file = context_with_bytes(payload.len(), payload.len());
        inline_less_imports(source, Some(dir.path()), &mut exact_file, true)
            .expect("exact import file byte limit");
        assert_eq!(exact_file.imported_bytes, payload.len());

        let mut short_file = context_with_bytes(payload.len() - 1, payload.len());
        let file_error = inline_less_imports(source, Some(dir.path()), &mut short_file, true)
            .expect_err("import beyond the per-file byte limit must fail");
        assert_eq!(file_error.code, "VUEC_STYLE_IMPORT_LIMIT");
        assert_eq!(
            file_error.message,
            format!(
                "Less import exceeds the maximum of {} bytes: ./payload.less",
                payload.len() - 1
            )
        );
        assert_eq!(file_error.span, Some((0, source.len())));
        assert_eq!(short_file.imported_files, 0);
        assert_eq!(short_file.imported_bytes, 0);

        std::fs::write(dir.path().join("first.styl"), "a").expect("write first.styl");
        std::fs::write(dir.path().join("second.styl"), "bc").expect("write second.styl");
        let stylus_source = "@import \"./first.styl\"\n@import \"./second.styl\"\n";

        let mut exact_total = context_with_bytes(2, 3);
        inline_stylus_imports(stylus_source, Some(dir.path()), &mut exact_total, true)
            .expect("exact total import byte limit");
        assert_eq!(exact_total.imported_files, 2);
        assert_eq!(exact_total.imported_bytes, 3);

        let mut short_total = context_with_bytes(2, 2);
        let total_error = inline_stylus_imports(
            stylus_source,
            Some(dir.path()),
            &mut short_total,
            true,
        )
        .expect_err("imports beyond the total byte limit must fail");
        assert_eq!(total_error.code, "VUEC_STYLE_IMPORT_LIMIT");
        assert_eq!(
            total_error.message,
            "Stylus imports exceed the maximum total of 2 bytes"
        );
        assert_eq!(short_total.imported_files, 1);
        assert_eq!(short_total.imported_bytes, 1);
        assert!(short_total.active_paths.is_empty());
    }

    #[test]
    fn preprocesses_stylus_additional_data_imports_and_load_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let shared_dir = dir.path().join("shared");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&shared_dir).expect("shared dir");
        let base = src_dir.join("component.styl");
        let local_import = src_dir.join("local.styl");
        let load_path_import = shared_dir.join("tokens.styl");
        std::fs::write(
            &local_import,
            r#"
.imported
  border-color brand
"#,
        )
        .expect("write local import");
        std::fs::write(
            &load_path_import,
            r#"
space = 12px
.shared
  margin space
"#,
        )
        .expect("write load path import");

        let result = compile_style(
            r#"
@import "./local"
@import "tokens"
@import "https://example.com/reset.css"
.root
  color brand
  padding space
"#,
            StyleCompileOptions {
                filename: Some(base.to_string_lossy().into_owned()),
                preprocess_lang: Some("stylus".into()),
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("brand = red".into()),
                    load_paths: vec![shared_dir.to_string_lossy().into_owned()],
                },
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result
            .code
            .contains("@import \"https://example.com/reset.css\";"));
        assert!(result.code.contains(".imported {"));
        assert!(result.code.contains("border-color: red;"));
        assert!(result.code.contains(".shared {"));
        assert!(result.code.contains("margin: 12px;"));
        assert!(result.code.contains("padding: 12px;"));
        let mut expected = vec![
            std::fs::canonicalize(local_import)
                .expect("canonical local import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
            std::fs::canonicalize(load_path_import)
                .expect("canonical load path import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
        ];
        expected.sort();
        assert_eq!(result.dependencies, expected);
    }

    #[test]
    fn preprocesses_scss_additional_data_and_import_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let base = dir.path().join("test.scss");
        let import = dir.path().join("import.scss");
        std::fs::write(&import, ".imported { color: $red; }\n").expect("write import");

        let result = compile_style(
            r#"
@import "./import.scss";
.square {
  @include square(100px);
}
"#,
            StyleCompileOptions {
                filename: Some(base.to_string_lossy().into_owned()),
                preprocess_lang: Some("scss".into()),
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some(
                        r#"
$red: red;
@mixin square($size) {
  width: $size;
  height: $size;
}
"#
                        .into(),
                    ),
                    ..StylePreprocessOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("color: red;"));
        assert!(result.code.contains("width: 100px;"));
        let resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .to_string();
        assert_eq!(
            result.dependencies,
            vec![normalize_native_dependency_path(Path::new(
                &resolved_import
            ))]
        );
    }

    #[test]
    fn collects_css_vars_like_vue27() {
        let vars = collect_css_vars(
            r#"
            /* color: v-bind(ignored); */
            div {
              color: v-bind(color);
              width: v-bind('font.size');
              top: v-bind((a + b) / 2 + 'px');
              height: v-bind("count.toString(");
              border: v-bind(color);
            }
            "#,
        );

        assert_eq!(
            vars,
            vec![
                "color",
                "font.size",
                "(a + b) / 2 + 'px'",
                "count.toString("
            ]
        );
    }

    #[test]
    fn collects_css_vars_like_vue3_with_line_comments() {
        let vars = collect_css_vars_with_options(
            r#"
            // color: v-bind(ignored);
            div {
              color: v-bind(color);
              width: v-bind('font.size');
              top: v-bind    ((a + b) / 2 + 'px' );
              height: v-bind("count.toString(");
            }
            "#,
            CssVarCollectOptions {
                ignore_line_comments: true,
            },
        );

        assert_eq!(
            vars,
            vec![
                "color",
                "font.size",
                "(a + b) / 2 + 'px'",
                "count.toString("
            ]
        );
    }

    #[test]
    fn collects_css_vars_across_interstitial_block_comments() {
        let vars = collect_css_vars_with_options(
            concat!(
                ".foo { color: v-bind/**/(color); ",
                "font-size: v-bind /*x*/ ('font.size'); ",
                "width: v-bind/**/ (size); }"
            ),
            CssVarCollectOptions {
                ignore_line_comments: true,
            },
        );

        assert_eq!(vars, vec!["color", "font.size", "size"]);
    }

    #[test]
    fn rewrites_css_vars_with_vue27_names() {
        let code = rewrite_css_vars(
            ".foo { color: v-bind(color); font-size: v-bind('font.size'); }",
            "test",
            false,
        );
        assert!(code.contains("var(--test-color)"));
        assert!(code.contains("var(--test-font_size)"));
        assert_eq!(gen_css_var_name("xxxxxxxx", "color", true), "4003f1a6");
        assert_eq!(gen_css_var_name("xxxxxxxx", "font.size", true), "41b6490a");
    }

    #[test]
    fn rewrites_css_vars_across_comment_separated_call_gaps() {
        let code = rewrite_css_vars_with_options(
            concat!(
                ".foo { color: v-bind /*x*/ (color); ",
                "font-size: v-bind /**/ /**/ ('font.size'); ",
                "width: v-bind/**/ (size); ",
                "height: v-bind/**/(height); }"
            ),
            "test",
            CssVarRewriteOptions {
                is_prod: false,
                name_style: CssVarNameStyle::Vue3Escaped,
                ignore_line_comments: true,
            },
        );

        assert!(code.contains("var(--test-color)"));
        assert!(code.contains(r"var(--test-font\.size)"));
        assert!(code.contains("var(--test-size)"));
        assert!(code.contains("v-bind/**/(height)"));
    }

    #[test]
    fn rewrites_css_vars_with_vue3_escaped_names() {
        let code = rewrite_css_vars_with_options(
            concat!(
                ".foo { color: v-bind(color); font-size: v-bind('font.size'); ",
                "font-weight: v-bind(_φ); width: calc(v-bind(foo + 'px') - 3px); }\n",
                "// color: v-bind(ignored)\n",
                ".bar { width: v-bind(width); }"
            ),
            "test",
            CssVarRewriteOptions {
                is_prod: false,
                name_style: CssVarNameStyle::Vue3Escaped,
                ignore_line_comments: true,
            },
        );

        assert!(code.contains("var(--test-color)"));
        assert!(code.contains(r"var(--test-font\.size)"));
        assert!(code.contains("var(--test-_φ)"));
        assert!(code.contains(r"var(--test-foo\ \+\ \'px\')"));
        assert!(code.contains("// color: v-bind(ignored)"));
        assert!(code.contains("var(--test-width)"));
        assert_eq!(
            gen_css_var_name_with_style("xxxxxxxx", "color", true, CssVarNameStyle::Vue3Escaped),
            "v4003f1a6"
        );
    }
