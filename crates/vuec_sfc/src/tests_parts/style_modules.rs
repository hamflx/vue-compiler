    #[test]
    fn compile_wrappers_return_shapes() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><div/></template><script lang="ts">export default {}</script><script setup lang="ts">const x = 1</script><style scoped src="./base.css">@import "./dep.css"; .a{ color: v-bind(color); }</style>"#,
        );
        let template = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());
        assert!(template.code.contains("render"));
        assert!(template.ast_summary.starts_with("dom:"));
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
        assert_eq!(script.errors.len(), 0);
        assert!(script.setup);
        assert_eq!(script.lang.as_deref(), Some("ts"));
        assert_eq!(
            script.bindings.get("x").map(String::as_str),
            Some("setup-const")
        );
        assert!(script.content.contains("_defineComponent"));
        assert!(script.content.contains("__returned__ = { x }"));
        assert_eq!(script.script_ast.len(), 1);
        let script_statement = &script.script_ast[0];
        assert_eq!(script_statement["type"], json!("ExportDefaultDeclaration"));
        assert_eq!(script_statement["start"], json!(0));
        assert_eq!(script_statement["end"], json!("export default {}".len()));
        assert_eq!(script_statement["source"], json!("export default {}"));
        assert_eq!(script_statement["loc"]["start"]["offset"], json!(0));
        assert_eq!(
            script_statement["loc"]["end"]["offset"],
            json!("export default {}".len())
        );
        assert_eq!(
            script_statement["declaration"]["type"],
            json!("ObjectExpression")
        );

        assert_eq!(script.script_setup_ast.len(), 1);
        let setup_statement = &script.script_setup_ast[0];
        assert_eq!(setup_statement["type"], json!("VariableDeclaration"));
        assert_eq!(setup_statement["kind"], json!("const"));
        assert_eq!(setup_statement["source"], json!("const x = 1"));
        assert_eq!(setup_statement["loc"]["start"]["offset"], json!(0));
        assert_eq!(setup_statement["declarations"][0]["id"]["name"], json!("x"));
        assert_eq!(
            setup_statement["declarations"][0]["init"]["value"],
            json!(1.0)
        );
        let script_json = serde_json::to_value(&script).expect("script json");
        assert!(script_json.get("scriptAst").is_some());
        assert!(script_json.get("scriptSetupAst").is_some());
        assert_eq!(
            script_json.get("type").and_then(|value| value.as_str()),
            Some("script")
        );
        let style = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        assert!(style.errors.is_empty());
        assert!(style.map.is_none());
        assert!(style.code.contains("var(--color)"));
        assert_eq!(style.dependencies, vec!["./base.css", "./dep.css"]);
        assert_eq!(style.raw_result.len(), 1);
        let style_json = serde_json::to_value(&style).expect("style json");
        assert!(style_json.get("rawResult").is_some());
    }

    #[test]
    fn compile_style_returns_css_module_exports() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.red { color: red }\n:global(.blue) { color: blue }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("red")
            .is_some_and(|value| value.contains("_red_")));
        assert!(!modules.contains_key("blue"));
        assert!(result.code.contains(".blue { color: blue }"));
    }

    #[test]
    fn compile_style_returns_css_modules_values() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>@value primary: red; @value query: (min-width: 1px); @media query { .button { color: primary; } }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert_eq!(
            modules.get("query").map(String::as_str),
            Some("(min-width: 1px)")
        );
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains("@value"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("color: red"));
    }

    #[test]
    fn compile_style_returns_css_modules_imported_values() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tokens.css"),
            "@value primary: red; .remote { color: primary; }",
        )
        .expect("write dep");
        let filename = dir.path().join("modules.vue");
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>@value primary, remote as external from "./tokens.css"; .button { composes: external; color: primary; } .external { border-color: primary; }</style>"#;
        let descriptor = compiler.parse(filename.to_string_lossy().to_string(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let external = modules.get("external").expect("external export");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert!(external.contains("_remote_"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_") && value.contains(external)));
        assert!(!result.code.contains("@value"));
        assert!(!result.code.contains("_external_"));
        assert!(!result.code.contains("; }"));
        assert!(result.code.contains("color: red"));
        assert!(result.code.contains("border-color: red"));
    }

    #[test]
    fn compile_style_returns_css_modules_missing_imported_value_composes() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tokens.css"),
            "@value primary: red; .remote { color: primary; }",
        )
        .expect("write dep");
        let filename = dir.path().join("modules.vue");
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>@value missing from "./tokens.css"; .button { composes: missing; color: missing; }</style>"#;
        let descriptor = compiler.parse(filename.to_string_lossy().to_string(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty());
        assert_eq!(
            modules.get("missing").map(String::as_str),
            Some("undefined")
        );
        assert!(button.contains("_button_"));
        assert!(button.contains("undefined"));
        assert!(!button.contains("i__const_missing_0"));
        assert!(!result.code.contains("@value"));
        assert!(result.code.contains("color: i__const_missing_0"));
    }

    #[test]
    fn compile_style_forwards_css_modules_dashes_convention() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.foo-bar { color: red }\n.foo_bar { color: blue }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    locals_convention: "dashesOnly".into(),
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("fooBar")
            .is_some_and(|value| value.contains("_foo-bar_")));
        assert!(!modules.contains_key("foo-bar"));
        assert!(modules
            .get("foo_bar")
            .is_some_and(|value| value.contains("_foo_bar_")));
    }

    #[test]
    fn compile_style_forwards_css_modules_hash_prefix() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.button { color: red }</style>"#;
        let descriptor = compiler.parse("src/Comp.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    generate_scoped_name: Some("[local]__[hash:base64:5]".into()),
                    hash_prefix: "alpha".into(),
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("button__2G66Z")
        );
        assert!(result.code.contains(".button__2G66Z"));
    }

    #[test]
    fn compile_style_forwards_css_modules_global_module_paths() {
        let mut compiler = SfcCompiler::new();
        let source =
            r#"<style module>.button { color: red }:local(.forced) { color: blue }</style>"#;
        let descriptor = compiler.parse("src/theme.global.css", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    global_module_paths: vec![r"global\.css$".into()],
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert!(!modules.contains_key("button"));
        assert!(modules
            .get("forced")
            .is_some_and(|value| value.contains("_forced_")));
        assert!(result.code.contains(".button { color: red }"));
        assert!(result.code.contains("._forced_"));
    }

    #[test]
    fn compile_style_returns_css_modules_id_exports() {
        let source = r#"<style module>#panel { color: red }.button#item { color: blue }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("src/Selectors.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(
            modules.get("panel").map(String::as_str),
            Some("_panel_7jaos_1")
        );
        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("_button_7jaos_1")
        );
        assert_eq!(
            modules.get("item").map(String::as_str),
            Some("_item_7jaos_1")
        );
        assert!(result.code.contains("#_panel_7jaos_1"));
        assert!(result.code.contains("._button_7jaos_1#_item_7jaos_1"));
    }

    #[test]
    fn compile_style_leaves_css_modules_class_attribute_selectors_global() {
        let source = r#"<style module>[class="btn"] { color: red }:local([class='forced']) { color: blue }.btn { color: black }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("src/Attr.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("btn")
            .is_some_and(|value| value.contains("_btn_")));
        assert!(!modules.contains_key("forced"));
        assert!(result.code.contains("[class=\"btn\"] { color: red }"));
        assert!(result.code.contains("[class='forced'] { color: blue }"));
        assert!(result.code.contains("._btn_"));
    }

    #[test]
    fn compile_style_returns_css_modules_keyframe_exports() {
        let source = r#"<style module>@keyframes fade { from { opacity: 0 } to { opacity: 1 } }
.button { animation-name: fade; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("src/Anim.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(
            modules.get("fade").map(String::as_str),
            Some("_fade_17sru_1")
        );
        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("_button_17sru_2")
        );
        assert!(result.code.contains("@keyframes _fade_17sru_1"));
        assert!(result.code.contains("animation-name: _fade_17sru_1"));
    }

    #[test]
    fn compile_style_forwards_css_modules_export_globals() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.local :global(.global) { color: red }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    export_globals: true,
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("local")
            .is_some_and(|value| value.contains("_local_")));
        assert_eq!(modules.get("global").map(String::as_str), Some("global"));
    }

    #[test]
    fn compile_style_returns_css_modules_composes_exports() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.base { color: blue }.button { composes: base global(extra); color: red }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let base = modules.get("base").expect("base export");
        let button = modules.get("button").expect("button export");

        assert!(button.contains(base));
        assert!(button.contains("extra"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_returns_css_modules_icss_exports() {
        let mut compiler = SfcCompiler::new();
        let source =
            r#"<style module>:export { primary: red; }.button { color: primary; }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains(":export"));
    }

    #[test]
    fn compile_style_rewrites_css_modules_icss_import_symbols() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        std::fs::write(
            dir.path().join("dep.css"),
            ".dep { color: blue; }\n:export { token: green; query: (min-width: 1px); }",
        )
        .expect("write dep");
        let source = r#"<style module>:import("./dep.css") { imported: dep; shade: token; mq: query; }.shade { color: shade; }.imported { color: shade; }@media mq { .button { color: shade; } }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert!(!modules.contains_key("shade"));
        assert!(!modules.contains_key("imported"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains(":import"));
        assert!(!result.code.contains("_shade_"));
        assert!(!result.code.contains("_imported_"));
        assert!(result.code.contains(".green"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("color: green"));
    }

    #[test]
    fn compile_style_preserves_empty_css_modules_for_missing_icss_imports() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        std::fs::write(dir.path().join("dep.css"), ":export { token: green; }").expect("write dep");
        let source = r#"<style module>:import("./dep.css") { shade: missing; }.shade { color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("empty css modules map");

        assert!(modules.is_empty());
        assert!(result.errors.is_empty());
        assert!(!result.code.contains(":import"));
        assert!(result.code.contains(".shade { color: red"));
        assert!(!result.code.contains("_shade_"));
    }

    #[test]
    fn compile_style_rewrites_css_modules_native_nested_rules() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let source = r#"<style module>.foo { color: blue; .bar { color: red; } &.active { color: green; } @media (min-width: 1px) { :global(.global) { color: black; } :local(.inner) { color: white; } } color: yellow; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        for key in ["foo", "bar", "active", "inner"] {
            assert!(
                modules.get(key).is_some_and(|value| value.contains('_')),
                "missing module key {key}: {modules:?}"
            );
        }
        assert!(!modules.contains_key("global"));
        assert!(result.code.contains("{ color: blue;\n"));
        assert!(result.code.contains("\n._bar_"));
        assert!(result.code.contains("\n&._active_"));
        assert!(result.code.contains("@media (min-width: 1px) {\n.global"));
        assert!(result.code.contains("\n._inner_"));
        assert!(result.code.contains("} color: yellow;"));
        assert!(!result.code.contains("\n.bar {"));
        assert!(!result.code.contains("\n&.active {"));
        assert!(!result.code.contains(":local(.inner)"));
        assert!(!result.code.contains(":global(.global)"));
    }

    #[test]
    fn compile_style_reports_css_modules_native_nested_composes() {
        let source = r#"<style module>.foo { .bar { composes: foo; color: red; } }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());

        assert_eq!(
            result.errors,
            vec![
                "composition is not allowed in nested rule \n\n:local(.bar) { composes: foo; color: red;\n}"
            ]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        assert!(result.code.is_empty());
        assert!(result.modules.is_none());
    }

    #[test]
    fn compile_style_returns_css_modules_external_composes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }\n:export { token: green; }")
            .expect("write dep");
        let source =
            r#"<style module>.button { composes: dep from "./dep.css"; color: token; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_returns_css_modules_node_modules_composes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&dist_dir).expect("dist dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(dist_dir.join("theme.css"), ".dep { color: blue; }").expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./theme.css":"./dist/theme.css"}}"#,
        )
        .expect("write package");
        let source = r#"<style module>.button { composes: dep from "vuec-css-fixture/theme.css"; color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_returns_css_modules_composes_from_global() {
        let source =
            r#"<style module>.button { composes: reset utility from global; color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("reset"));
        assert!(button.contains("utility"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_maps_css_modules_composes_diagnostics_to_vue_source() {
        let source = r#"<template></template>
<style module>.button { composes: missing; color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let missing_start = source.find("missing").expect("missing token");

        assert_eq!(
            result.errors,
            vec!["referenced class name \"missing\" in composes not found"]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        assert_eq!(
            result.diagnostics[0].span,
            Some(vuec_source::Span::new(
                descriptor.source_file,
                missing_start,
                missing_start + "missing".len()
            ))
        );
    }

    #[test]
    fn compile_style_reports_css_modules_complex_composes_selector() {
        let source =
            r#"<style module>.button.extra { composes: base; }.base { color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());

        assert_eq!(
            result.errors,
            vec![
                "composition is only allowed when selector is single :local class name not in \":local(.button):local(.extra)\""
            ]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_forwards_scss_preprocess_options_and_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let import = dir.path().join("import.scss");
        std::fs::write(&import, ".imported { color: $red; }\n").expect("write import");
        let source = r#"<style lang="scss">
@import "./import.scss";
.square { @include square(10px); }
</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some(
                        "$red: red;\n@mixin square($size) { width: $size; height: $size; }".into(),
                    ),
                    ..StylePreprocessOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("width: 10px;"));
        let mut resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .to_string();
        if let Some(stripped) = resolved_import.strip_prefix(r"\\?\") {
            resolved_import = stripped.to_string();
        } else if let Some(stripped) = resolved_import.strip_prefix("//?/") {
            resolved_import = stripped.to_string();
        }
        assert_eq!(result.dependencies, vec![resolved_import]);
    }

    #[test]
    fn compile_style_forwards_less_preprocess_options_and_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let import = dir.path().join("tokens.less");
        std::fs::write(&import, "@space: 6px;\n.imported { margin: @space; }\n")
            .expect("write import");
        let source = r#"<style lang="less">
@import "./tokens.less";
.card {
  color: @brand;
  .title {
    padding: @space;
  }
}
</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("@brand: red;".into()),
                    ..StylePreprocessOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("margin: 6px;"));
        assert!(result.code.contains(".card .title"));
        assert!(result.code.contains("padding: 6px;"));
        assert!(result.code.contains("color: red;"));
        let resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("//?/")
            .to_string();
        assert_eq!(result.dependencies, vec![resolved_import]);
    }

    #[test]
    fn compile_style_forwards_stylus_preprocess_options_and_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let import = dir.path().join("tokens.styl");
        std::fs::write(&import, "space = 6px\n.imported\n  margin space\n").expect("write import");
        let source = r#"<style lang="stylus">
@import "./tokens"
.card
  color brand
  .title
    padding space
</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("brand = red".into()),
                    ..StylePreprocessOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("margin: 6px;"));
        assert!(result.code.contains(".card .title"));
        assert!(result.code.contains("padding: 6px;"));
        assert!(result.code.contains("color: red;"));
        let resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("//?/")
            .to_string();
        assert_eq!(result.dependencies, vec![resolved_import]);
    }
