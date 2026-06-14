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
