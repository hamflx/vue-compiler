    #[test]
    fn compiles_css_modules_icss_exports() {
        let result = compile_style(
            ":export { primary: red; spacing: 1px; }\n.button { color: primary; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert_eq!(modules.get("spacing").map(String::as_str), Some("1px"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains(":export"));
        assert!(result.code.contains("color: primary"));
    }

    #[test]
    fn compiles_css_modules_local_values_like_official() {
        let result = compile_style(
            r#"@value primary: red; @value accent: primary; @value query: (min-width: 1px);
@media query { .button::before { content: "accent"; /* accent */ color: accent; } }
.accent { border-color: accent; }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("value.module.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert_eq!(modules.get("accent").map(String::as_str), Some("red"));
        assert_eq!(
            modules.get("query").map(String::as_str),
            Some("(min-width: 1px)")
        );
        assert!(modules
            .get("red")
            .is_some_and(|value| value.contains("_red_")));
        assert!(!result.code.contains("@value"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("content: \"red\""));
        assert!(result.code.contains("/* accent */ color: red"));
        assert!(result.code.contains("border-color: red"));
    }

    #[test]
    fn compiles_css_modules_values_as_single_pass_replacements() {
        let result = compile_style(
            "@value accent: primary; @value primary: red; .button { color: accent; background: primary; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("value-order.module.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("accent").map(String::as_str), Some("primary"));
        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert!(result.code.contains("color: primary"));
        assert!(result.code.contains("background: red"));
    }

    #[test]
    fn css_module_values_replace_complete_identifier_tokens() {
        let values = BTreeMap::from([
            ("a".to_string(), "A".to_string()),
            ("alpha".to_string(), "X".to_string()),
            ("alpha-beta".to_string(), "Y".to_string()),
            ("value_2".to_string(), "Q".to_string()),
            ("eclair".to_string(), "Z".to_string()),
        ]);
        let source = r#"a alpha alpha-beta xalpha alpha2 1alpha -alpha eclair value_2 /* alpha */ "alpha" \alpha"#;
        let mut load_state =
            CssModulesImportState::new(CssModulesImportLimits::default());

        assert_eq!(
            replace_css_module_values(
                source,
                &values,
                &mut load_state,
                CSS_MODULES_MAX_VALUE_OUTPUT_BYTES,
            )
            .as_deref(),
            Some(r#"A X Y xalpha alpha2 1alpha -alpha Z Q /* alpha */ "X" \X"#)
        );
    }

    #[test]
    fn css_module_values_handle_large_shared_prefix_sets() {
        let values = (0..1024)
            .map(|index| {
                (
                    format!("shared_prefix_{index}"),
                    format!("replacement_{index}"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let source = std::iter::repeat_n("shared_prefix_missing", 1024)
            .collect::<Vec<_>>()
            .join(" ");
        let mut load_state =
            CssModulesImportState::new(CssModulesImportLimits::default());

        assert_eq!(
            replace_css_module_values(
                &source,
                &values,
                &mut load_state,
                CSS_MODULES_MAX_VALUE_OUTPUT_BYTES,
            ),
            Some(source)
        );
    }

    #[test]
    fn compiles_css_modules_value_imports_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let dep = dir.path().join("tokens.css");
        std::fs::write(
            &dep,
            "@value primary: red; @value query: (min-width: 1px); .remote { color: primary; }",
        )
        .expect("write dep");
        let entry = dir.path().join("entry.css");
        let result = compile_style(
            r#"@value primary, query, remote as external, missing from "./tokens.css";
@value accent: primary;
@media query { .button { composes: external; color: accent; outline-color: missing; } }
.external { border-color: primary; }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(entry.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let external = modules.get("external").expect("external export");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert_eq!(modules.get("accent").map(String::as_str), Some("red"));
        assert_eq!(
            modules.get("query").map(String::as_str),
            Some("(min-width: 1px)")
        );
        assert_eq!(
            modules.get("missing").map(String::as_str),
            Some("undefined")
        );
        assert!(external.contains("_remote_"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_") && value.contains(external)));
        assert!(!result.code.contains("@value"));
        assert!(!result.code.contains("_external_"));
        assert!(!result.code.contains("; }"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("color: red"));
        assert!(result.code.contains("outline-color: i__const_missing_3"));
        assert!(result.code.contains("border-color: red"));
    }

    #[test]
    fn css_modules_value_placeholders_replace_complete_tokens_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let dep = dir.path().join("tokens.css");
        std::fs::write(&dep, ":export { token: red; }").expect("write dep");
        let entry = dir.path().join("entry.css");

        let result = compile_style(
            r#"@value token from "./tokens.css";
.button { color: token; --literal: __vuec_value_0suffix; }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(entry.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty());
        assert!(result.code.contains("color: red"));
        assert!(result.code.contains("--literal: __vuec_value_0suffix"));
    }

    #[test]
    fn css_modules_value_placeholder_replacement_scales_linearly() {
        let options = StyleCompileOptions::default();
        let mut load_state =
            CssModulesImportState::new(CssModulesImportLimits::default());
        let mut context = CssModulesContext::new(
            &options,
            "test.css".into(),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut load_state,
        );
        let mut source = String::new();
        let mut expected = String::new();
        for index in 0..4096 {
            if index > 0 {
                source.push(' ');
                expected.push(' ');
            }
            let placeholder = format!("__vuec_value_{index}");
            let replacement = format!("value_{index}");
            source.push_str(&placeholder);
            expected.push_str(&replacement);
            context
                .value_placeholders
                .insert(placeholder, replacement);
        }
        source.push_str(" __vuec_value_1suffix");
        expected.push_str(" __vuec_value_1suffix");

        assert_eq!(context.replace_value_placeholders(source), expected);
    }

    #[test]
    fn compiles_css_modules_missing_value_import_composes_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let dep = dir.path().join("tokens.css");
        std::fs::write(&dep, "@value primary: red; .remote { color: primary; }")
            .expect("write dep");
        let entry = dir.path().join("entry.css");
        let result = compile_style(
            r#"@value missing from "./tokens.css";
.button { composes: missing; color: missing; }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(entry.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
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
    fn compiles_css_modules_icss_exports_with_locals_convention() {
        let result = compile_style(
            ":export { theme-color: red; }\n.button { color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    locals_convention: "dashesOnly".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("themeColor").map(String::as_str), Some("red"));
        assert!(!modules.contains_key("theme-color"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_relative_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }\n:export { token: green; }")
            .expect("write dep");

        let result = compile_style(
            ".button { composes: dep from \"./dep.css\"; color: token; }\n.plain { color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(modules
            .get("plain")
            .is_some_and(|value| value.contains("_plain_")));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._button_"));
        assert!(!result.code.contains("composes"));
        assert!(!result.code.contains(":export"));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_node_modules_subpath() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._button_"));
        assert!(!result.code.contains("composes"));
        assert!(!result.code.contains(":export"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_package_main() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","main":"theme.css"}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("color: green"));
        assert!(!result.code.contains(":import"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_package_exports_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":"./theme.css"}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("color: green"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_conditional_exports_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{".":{"default":"./theme.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("color: green"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_exports_require_condition_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("require.css"),
            ".dep { color: blue; }\n:export { token: requireGreen; }",
        )
        .expect("write require dep");
        std::fs::write(
            package_dir.join("default.css"),
            ".dep { color: red; }\n:export { token: defaultRed; }",
        )
        .expect("write default dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{".":{"require":"./require.css","default":"./default.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.contains("color: requireGreen"));
        assert!(!result.code.contains("defaultRed"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_exports_condition_order_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("node.css"),
            ".dep { color: blue; }\n:export { token: nodePurple; }",
        )
        .expect("write node dep");
        std::fs::write(
            package_dir.join("require.css"),
            ".dep { color: red; }\n:export { token: requireGreen; }",
        )
        .expect("write require dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{".":{"node":"./node.css","require":"./require.css","default":"./require.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains("color: nodePurple"));
        assert!(!result.code.contains("requireGreen"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_nested_exports_conditions_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("require.css"),
            ".dep { color: blue; }\n:export { token: nestedRequire; }",
        )
        .expect("write require dep");
        std::fs::write(
            package_dir.join("default.css"),
            ".dep { color: red; }\n:export { token: fallbackDefault; }",
        )
        .expect("write default dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{".":{"node":{"require":"./require.css","default":"./default.css"},"default":"./default.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains("color: nestedRequire"));
        assert!(!result.code.contains("fallbackDefault"));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_node_modules_package_exports_subpath() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&dist_dir).expect("dist dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            dist_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./theme.css":"./dist/theme.css"}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._button_"));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_node_modules_conditional_exports_subpath() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&dist_dir).expect("dist dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            dist_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./theme.css":{"default":"./dist/theme.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._button_"));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_node_modules_exports_require_condition_subpath()
    {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&dist_dir).expect("dist dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            dist_dir.join("require.css"),
            ".dep { color: requireGreen; }\n:export { token: requireGreen; }",
        )
        .expect("write require dep");
        std::fs::write(
            dist_dir.join("default.css"),
            ".dep { color: defaultRed; }\n:export { token: defaultRed; }",
        )
        .expect("write default dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./theme.css":{"require":"./dist/require.css","default":"./dist/default.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("requireGreen"));
        assert!(!result.code.contains("defaultRed"));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_node_modules_wildcard_exports_subpath() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&dist_dir).expect("dist dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            dist_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./*.css":"./dist/*.css"}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._button_"));
    }

    #[test]
    fn css_modules_node_modules_exports_blocks_unexported_subpath_fallback() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(package_dir.join("theme.css"), ".dep { color: blue; }").expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./other.css":"./theme.css"}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(!button.contains("_dep_"));
        assert!(result.code.contains("composes"));
    }

    #[test]
    fn compiles_css_modules_multiple_external_composes_from_relative_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }\n.extra { color: green; }").expect("write dep");

        let result = compile_style(
            ".button { composes: dep extra from \"./dep.css\"; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(button.contains("_extra_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._extra_"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compiles_css_modules_composes_from_global() {
        let result = compile_style(
            ".button { composes: reset utility from global; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("reset"));
        assert!(button.contains("utility"));
        assert!(!result.code.contains("composes"));
        assert!(result.code.contains("color: red"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_relative_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }\n:export { token: green; }")
            .expect("write dep");

        let result = compile_style(
            ":import(\"./dep.css\") { imported: dep; shade: token; }\n.button { color: shade; }\n.other { composes: imported; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let other = modules.get("other").expect("other export");

        assert!(other.contains("_other_"));
        assert!(other.contains("_dep_"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("color: green"));
        assert!(!result.code.contains(":import"));
    }

    #[test]
    fn compiles_css_modules_icss_import_symbols_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(
            &dep,
            ".dep { color: blue; }\n:export { token: green; query: (min-width: 1px); }",
        )
        .expect("write dep");

        let result = compile_style(
            r#":import("./dep.css") { imported: dep; shade: token; mq: query; }
.shade { color: red; }
.imported { border-color: shade; }
@media mq { .button::before { content: "shade"; color: shade; } }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(!modules.contains_key("shade"));
        assert!(!modules.contains_key("imported"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains(":import"));
        assert!(!result.code.contains("_shade_"));
        assert!(!result.code.contains("_imported_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains(".green { color: red"));
        assert!(result.code.contains("._dep_"));
        assert!(result.code.contains("border-color: green"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("content: \"green\""));
        assert!(result.code.contains("color: green"));
    }

    #[test]
    fn compiles_css_modules_missing_icss_import_symbols_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }\n:export { token: green; }")
            .expect("write dep");

        let result = compile_style(
            r#":import("./dep.css") { imported: dep; shade: nope; color: token; mq: missing; }
.shade { color: red; }
.imported { border-color: color; }
.button { composes: shade; color: shade; }
@media mq { .panel { color: shade; } }
:export { out: shade; importedOut: color; }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty());
        assert!(!modules.contains_key("shade"));
        assert!(!modules.contains_key("imported"));
        assert!(button.contains("_button_"));
        assert!(button.contains("undefined"));
        assert_eq!(modules.get("out").map(String::as_str), Some("undefined"));
        assert_eq!(
            modules.get("importedOut").map(String::as_str),
            Some("green")
        );
        assert!(modules
            .get("panel")
            .is_some_and(|value| value.contains("_panel_")));
        assert!(!result.code.contains(":import"));
        assert!(!result.code.contains(":export"));
        assert!(!result.code.contains("composes"));
        assert!(!result.code.contains("_shade_"));
        assert!(!result.code.contains("_imported_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains(".shade { color: red"));
        assert!(result.code.contains("border-color: green"));
        assert!(result.code.contains("color: shade"));
        assert!(result.code.contains("@media mq"));
    }

    #[test]
    fn compiles_css_modules_native_nested_rules_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");

        let result = compile_style(
            r#".foo { color: blue; .bar { color: red; } &.active { color: green; } @media (min-width: 1px) { :global(.global) { color: black; } :local(.inner) { color: white; } } color: yellow; }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

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
    fn reports_css_modules_native_nested_composes_like_official() {
        let result = compile_style(
            ".foo { .bar { composes: foo; color: red; } }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                source_map_file_id: Some(FileId(7)),
                source_map_base_offset: 10,
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(
            result.errors,
            vec![
                "composition is not allowed in nested rule \n\n:local(.bar) { composes: foo; color: red;\n}"
            ]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        let start = ".foo { .bar {".len();
        assert_eq!(
            result.diagnostics[0].span,
            Some(Span::new(
                FileId(7),
                10 + start,
                10 + start + " composes: foo".len()
            ))
        );
        assert!(result.code.is_empty());
        assert!(result.modules.is_none());
    }

    fn css_modules_test_import_limits() -> CssModulesImportLimits {
        CssModulesImportLimits {
            max_depth: 8,
            max_files: 8,
            max_file_bytes: 1024,
            max_total_bytes: 4096,
            max_metadata_file_bytes: 1024,
            max_metadata_bytes: 4096,
            max_path_bytes: 4096,
            max_path_probes: 128,
            max_value_bytes: 1024,
            max_total_value_bytes: 4096,
            max_value_output_bytes: 4096,
            max_generated_bytes: 16 * 1024,
            max_replacement_steps: 4096,
            max_export_values: 4096,
            max_export_bytes: 16 * 1024,
            max_value_comparisons: 4096,
            max_syntax_depth: 16,
            max_rewrite_work_bytes: 16 * 1024,
            max_scoped_name_pattern_bytes: 4096,
            max_scoped_name_bytes: 4096,
            max_scoped_name_hash_input_bytes: 4096,
            max_default_name_work_bytes: 16 * 1024,
        }
    }

    fn css_modules_test_options(filename: &Path) -> StyleCompileOptions {
        StyleCompileOptions {
            id: Some("test".into()),
            filename: Some(filename.to_string_lossy().to_string()),
            modules: true,
            ..StyleCompileOptions::default()
        }
    }

    #[test]
    fn css_modules_value_expansion_enforces_exact_budgets() {
        let source = "@value v0: x; @value v1: v0 v0; @value v2: v1 v1;";
        let options = StyleCompileOptions {
            id: Some("test".into()),
            filename: Some("values.css".into()),
            modules: true,
            ..StyleCompileOptions::default()
        };
        let exact_limits = CssModulesImportLimits {
            max_value_bytes: 7,
            max_total_value_bytes: 33,
            max_generated_bytes: 22,
            max_replacement_steps: 8,
            ..css_modules_test_import_limits()
        };

        let exact = compile_css_modules_with_limits(source, source, &options, exact_limits);
        assert!(exact.diagnostics.is_empty(), "{:?}", exact.diagnostics);
        assert_eq!(exact.modules.get("v2").map(String::as_str), Some("x x x x"));

        let over_limits = [
            CssModulesImportLimits {
                max_value_bytes: 6,
                ..exact_limits
            },
            CssModulesImportLimits {
                max_total_value_bytes: 32,
                ..exact_limits
            },
            CssModulesImportLimits {
                max_generated_bytes: 21,
                ..exact_limits
            },
            CssModulesImportLimits {
                max_replacement_steps: 7,
                ..exact_limits
            },
        ];
        for limits in over_limits {
            let result = compile_css_modules_with_limits(source, source, &options, limits);
            assert_eq!(result.diagnostics.len(), 1);
            assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
            assert!(result.code.is_empty());
            assert!(result.raw_modules.is_empty());
            assert!(result.modules.is_empty());
        }
    }

    #[test]
    fn css_modules_composition_enforces_exact_export_budgets() {
        let source = ".c0 {} .c1 { composes: c0; } .c2 { composes: c1; }";
        let options = StyleCompileOptions {
            id: Some("test".into()),
            filename: Some("compose.css".into()),
            modules: true,
            modules_options: CssModulesOptions {
                generate_scoped_name: Some("[local]".into()),
                ..CssModulesOptions::default()
            },
            ..StyleCompileOptions::default()
        };
        let exact_limits = CssModulesImportLimits {
            max_export_values: 6,
            max_export_bytes: 12,
            max_value_comparisons: 5,
            ..css_modules_test_import_limits()
        };

        let exact = compile_css_modules_with_limits(source, source, &options, exact_limits);
        assert!(exact.diagnostics.is_empty(), "{:?}", exact.diagnostics);
        assert_eq!(exact.raw_modules.get("c2").map(String::as_str), Some("c2 c1 c0"));

        for limits in [
            CssModulesImportLimits {
                max_export_values: 5,
                ..exact_limits
            },
            CssModulesImportLimits {
                max_export_bytes: 11,
                ..exact_limits
            },
            CssModulesImportLimits {
                max_value_comparisons: 4,
                ..exact_limits
            },
        ] {
            let result = compile_css_modules_with_limits(source, source, &options, limits);
            assert_eq!(result.diagnostics.len(), 1);
            assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
            assert!(result.code.is_empty());
            assert!(result.raw_modules.is_empty());
            assert!(result.modules.is_empty());
        }
    }

    #[test]
    fn css_modules_export_accounting_is_cumulative_and_deduplicated() {
        let options = StyleCompileOptions::default();
        let mut state = CssModulesImportState::new(CssModulesImportLimits {
            max_export_values: 5,
            max_export_bytes: 5,
            max_value_comparisons: 3,
            ..css_modules_test_import_limits()
        });
        let mut context = CssModulesContext::new(
            &options,
            "test.css".into(),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut state,
        );

        assert!(context.push_raw_export_value("a", "x"));
        assert!(context.push_raw_export_value("a", "y"));
        assert!(context.push_raw_export_value("a", "y"));
        context.set_raw_export_values("a", vec!["z".into()]);
        context.set_raw_export_values("a", vec!["w".into(), "q".into()]);

        assert_eq!(context.raw_modules().get("a").map(String::as_str), Some("w q"));
        assert_eq!(
            (
                context.load_state.export_values,
                context.load_state.export_bytes,
                context.load_state.value_comparisons,
            ),
            (5, 5, 3)
        );
        assert!(context.load_state.error.is_none());
    }

    #[test]
    fn css_modules_export_budget_is_shared_across_imports() {
        let dir = tempfile::tempdir().expect("temp dir");
        let entry = dir.path().join("entry.css");
        let dependency = dir.path().join("dep.css");
        let source = ".root { composes: dep from \"./dep.css\"; }";
        std::fs::write(&entry, source).expect("write entry");
        std::fs::write(&dependency, ".dep {}").expect("write dependency");
        let mut options = css_modules_test_options(&entry);
        options.modules_options.generate_scoped_name = Some("[local]".into());
        let exact_limits = CssModulesImportLimits {
            max_export_values: 3,
            max_export_bytes: 10,
            max_value_comparisons: 1,
            ..css_modules_test_import_limits()
        };

        let exact = compile_css_modules_with_limits(source, source, &options, exact_limits);
        assert!(exact.diagnostics.is_empty(), "{:?}", exact.diagnostics);
        assert_eq!(
            exact.raw_modules.get("root").map(String::as_str),
            Some("root dep")
        );

        let over = compile_css_modules_with_limits(
            source,
            source,
            &options,
            CssModulesImportLimits {
                max_export_values: 2,
                ..exact_limits
            },
        );
        assert_eq!(over.diagnostics.len(), 1);
        assert_eq!(over.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
        assert!(over.code.is_empty());
        assert!(over.raw_modules.is_empty());
        assert!(over.modules.is_empty());
    }

    #[test]
    fn css_modules_syntax_depth_has_exact_boundary() {
        let source = ".a { .b { .c { color: red; } } }";
        let options = StyleCompileOptions {
            id: Some("test".into()),
            filename: Some("nested.css".into()),
            modules: true,
            modules_options: CssModulesOptions {
                generate_scoped_name: Some("[local]".into()),
                ..CssModulesOptions::default()
            },
            ..StyleCompileOptions::default()
        };
        let exact_limits = CssModulesImportLimits {
            max_syntax_depth: 3,
            ..css_modules_test_import_limits()
        };

        let exact = compile_css_modules_with_limits(source, source, &options, exact_limits);
        assert!(exact.diagnostics.is_empty(), "{:?}", exact.diagnostics);
        assert!(exact.code.contains(".c"));

        let over = compile_css_modules_with_limits(
            source,
            source,
            &options,
            CssModulesImportLimits {
                max_syntax_depth: 2,
                ..exact_limits
            },
        );
        assert_eq!(over.diagnostics.len(), 1);
        assert_eq!(over.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
        assert!(over.diagnostics[0].message.contains("maximum depth of 2"));
        assert!(over.code.is_empty());
        assert!(over.raw_modules.is_empty());
        assert!(over.modules.is_empty());

        let adversarial = format!("{}{}", ".a{".repeat(4_096), "}".repeat(4_096));
        let bounded = compile_css_modules_with_limits(
            &adversarial,
            &adversarial,
            &options,
            CssModulesImportLimits {
                max_syntax_depth: 8,
                max_value_output_bytes: adversarial.len(),
                max_generated_bytes: adversarial.len() * 2,
                ..exact_limits
            },
        );
        assert_eq!(bounded.diagnostics.len(), 1);
        assert_eq!(bounded.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
        assert!(
            bounded.diagnostics[0].message.contains("maximum depth of 8"),
            "{}",
            bounded.diagnostics[0].message
        );
    }

    #[test]
    fn css_modules_syntax_depth_restores_between_sibling_blocks() {
        let options = StyleCompileOptions {
            id: Some("test".into()),
            filename: Some("siblings.css".into()),
            modules: true,
            ..StyleCompileOptions::default()
        };
        let exact_limits = CssModulesImportLimits {
            max_syntax_depth: 1,
            ..css_modules_test_import_limits()
        };
        for source in [
            ".a {} .b {}",
            "@media screen {} @supports (display: grid) {}",
        ] {
            let result = compile_css_modules_with_limits(source, source, &options, exact_limits);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        }

        let nested_at_rules = "@media screen { @supports (display: grid) { .a {} } }";
        let exact = compile_css_modules_with_limits(
            nested_at_rules,
            nested_at_rules,
            &options,
            CssModulesImportLimits {
                max_syntax_depth: 3,
                ..exact_limits
            },
        );
        assert!(exact.diagnostics.is_empty(), "{:?}", exact.diagnostics);

        let zero = compile_css_modules_with_limits(
            ".a {}",
            ".a {}",
            &options,
            CssModulesImportLimits {
                max_syntax_depth: 0,
                ..exact_limits
            },
        );
        assert_eq!(zero.diagnostics.len(), 1);
        assert_eq!(zero.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
    }

    #[test]
    fn css_modules_syntax_depth_is_shared_across_imports() {
        let dir = tempfile::tempdir().expect("temp dir");
        let entry = dir.path().join("entry.css");
        let dependency = dir.path().join("dep.css");
        let source =
            "@media screen { .root { composes: dep from \"./dep.css\"; color: red; } }";
        std::fs::write(&entry, source).expect("write entry");
        std::fs::write(&dependency, ".dep {}").expect("write dependency");
        let options = css_modules_test_options(&entry);
        let exact_limits = CssModulesImportLimits {
            max_syntax_depth: 3,
            ..css_modules_test_import_limits()
        };

        let exact = compile_css_modules_with_limits(source, source, &options, exact_limits);
        assert!(exact.diagnostics.is_empty(), "{:?}", exact.diagnostics);
        assert!(exact.raw_modules.contains_key("root"));

        let over = compile_css_modules_with_limits(
            source,
            source,
            &options,
            CssModulesImportLimits {
                max_syntax_depth: 2,
                ..exact_limits
            },
        );
        assert_eq!(over.diagnostics.len(), 1);
        assert_eq!(over.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
        assert!(over.code.is_empty());
        assert!(over.raw_modules.is_empty());
        assert!(over.modules.is_empty());
    }

    #[test]
    fn css_modules_rewrite_work_is_shared_across_imports() {
        let dir = tempfile::tempdir().expect("temp dir");
        let entry = dir.path().join("entry.css");
        let dependency = dir.path().join("dep.css");
        let source = ".root { composes: dep from \"./dep.css\"; }";
        std::fs::write(&entry, source).expect("write entry");
        std::fs::write(&dependency, ".dep { color: red; }").expect("write dependency");
        let mut options = css_modules_test_options(&entry);
        options.modules_options.generate_scoped_name = Some("[local]".into());
        let exact_limits = CssModulesImportLimits {
            max_rewrite_work_bytes: 14,
            ..css_modules_test_import_limits()
        };

        let exact = compile_css_modules_with_limits(source, source, &options, exact_limits);
        assert!(exact.diagnostics.is_empty(), "{:?}", exact.diagnostics);
        assert!(exact.code.contains("color: red"));

        let over = compile_css_modules_with_limits(
            source,
            source,
            &options,
            CssModulesImportLimits {
                max_rewrite_work_bytes: 13,
                ..exact_limits
            },
        );
        assert_eq!(over.diagnostics.len(), 1);
        assert_eq!(over.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
        assert!(over.code.is_empty());
        assert!(over.raw_modules.is_empty());
        assert!(over.modules.is_empty());
    }

    #[test]
    fn css_modules_rewrite_work_has_exact_boundary_and_checked_accounting() {
        let source = ".a { color: red; }";
        let options = StyleCompileOptions {
            id: Some("test".into()),
            filename: Some("work.css".into()),
            modules: true,
            modules_options: CssModulesOptions {
                generate_scoped_name: Some("[local]".into()),
                ..CssModulesOptions::default()
            },
            ..StyleCompileOptions::default()
        };
        let exact_limits = CssModulesImportLimits {
            max_rewrite_work_bytes: 13,
            ..css_modules_test_import_limits()
        };

        let exact = compile_css_modules_with_limits(source, source, &options, exact_limits);
        assert!(exact.diagnostics.is_empty(), "{:?}", exact.diagnostics);
        assert_eq!(exact.code, source);

        let over = compile_css_modules_with_limits(
            source,
            source,
            &options,
            CssModulesImportLimits {
                max_rewrite_work_bytes: 12,
                ..exact_limits
            },
        );
        assert_eq!(over.diagnostics.len(), 1);
        assert_eq!(over.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
        assert!(over.code.is_empty());
        assert!(over.raw_modules.is_empty());
        assert!(over.modules.is_empty());

        let mut overflow = CssModulesImportState::new(CssModulesImportLimits {
            max_rewrite_work_bytes: usize::MAX,
            ..css_modules_test_import_limits()
        });
        overflow.rewrite_work_bytes = usize::MAX;
        assert!(!overflow.claim_rewrite_work_bytes(1));
        assert!(overflow.error.is_some());
    }

    #[test]
    fn css_modules_scoped_name_template_has_exact_budgets() {
        let options = StyleCompileOptions {
            modules_options: CssModulesOptions {
                generate_scoped_name: Some("[name]__[local]__[hash:base64:5]".into()),
                hash_prefix: "alpha".into(),
                ..CssModulesOptions::default()
            },
            ..StyleCompileOptions::default()
        };
        let exact_limits = CssModulesImportLimits {
            max_scoped_name_pattern_bytes: 32,
            max_scoped_name_bytes: 30,
            max_scoped_name_hash_input_bytes: 24,
            max_generated_bytes: 159,
            max_replacement_steps: 3,
            ..css_modules_test_import_limits()
        };
        let run = |limits| {
            let mut state = CssModulesImportState::new(limits);
            let output = {
                let mut context = CssModulesContext::new(
                    &options,
                    "src/Comp.vue".into(),
                    String::new(),
                    CssModulesScopeBehaviour::Local,
                    false,
                    &mut state,
                );
                context.scoped_name("button")
            };
            (output, state)
        };

        let (exact, exact_state) = run(exact_limits);
        assert_eq!(exact, "Comp__button__2G66Z");
        assert_eq!(exact_state.generated_bytes, 159);
        assert_eq!(exact_state.replacement_steps, 3);
        assert!(exact_state.error.is_none());

        for limits in [
            CssModulesImportLimits {
                max_scoped_name_pattern_bytes: 31,
                ..exact_limits
            },
            CssModulesImportLimits {
                max_scoped_name_bytes: 29,
                ..exact_limits
            },
            CssModulesImportLimits {
                max_scoped_name_hash_input_bytes: 23,
                ..exact_limits
            },
            CssModulesImportLimits {
                max_generated_bytes: 158,
                ..exact_limits
            },
            CssModulesImportLimits {
                max_replacement_steps: 2,
                ..exact_limits
            },
        ] {
            let (output, state) = run(limits);
            assert!(output.is_empty());
            assert!(state.error.is_some());
        }
    }

    #[test]
    fn css_modules_scoped_name_cache_counts_each_output_use() {
        let options = StyleCompileOptions {
            modules_options: CssModulesOptions {
                generate_scoped_name: Some("[local]".into()),
                ..CssModulesOptions::default()
            },
            ..StyleCompileOptions::default()
        };
        let exact_limits = CssModulesImportLimits {
            max_scoped_name_pattern_bytes: 7,
            max_scoped_name_bytes: 7,
            max_generated_bytes: 35,
            max_replacement_steps: 1,
            ..css_modules_test_import_limits()
        };

        let mut exact_state = CssModulesImportState::new(exact_limits);
        let mut exact = CssModulesContext::new(
            &options,
            "test.css".into(),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut exact_state,
        );
        assert_eq!(exact.scoped_name("name"), "name");
        assert_eq!(exact.scoped_name("name"), "name");
        assert_eq!(
            (
                exact.load_state.generated_bytes,
                exact.load_state.replacement_steps,
                exact.scoped_names.len(),
            ),
            (35, 1, 1)
        );

        let mut over_state = CssModulesImportState::new(CssModulesImportLimits {
            max_generated_bytes: 34,
            ..exact_limits
        });
        let mut over = CssModulesContext::new(
            &options,
            "test.css".into(),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut over_state,
        );
        assert_eq!(over.scoped_name("name"), "name");
        assert!(over.scoped_name("name").is_empty());
        assert!(over.load_state.error.is_some());
    }

    #[test]
    fn css_modules_default_scoped_names_cache_hashes_and_searches() {
        let source = ".a {}\n.b {}";
        let options = StyleCompileOptions::default();
        let exact_limits = CssModulesImportLimits {
            max_default_name_work_bytes: 39,
            ..css_modules_test_import_limits()
        };
        let mut exact_state = CssModulesImportState::new(exact_limits);
        let mut exact = CssModulesContext::new(
            &options,
            "test.css".into(),
            source.into(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut exact_state,
        );
        let first = exact.scoped_name("a");
        assert_eq!(exact.scoped_name("a"), first);
        assert!(exact.scoped_name("b").ends_with("_2"));
        assert_eq!(exact.load_state.default_name_work_bytes, 39);
        assert_eq!(exact.scoped_names.len(), 2);
        assert!(exact.load_state.error.is_none());

        let mut over_state = CssModulesImportState::new(CssModulesImportLimits {
            max_default_name_work_bytes: 38,
            ..exact_limits
        });
        let mut over = CssModulesContext::new(
            &options,
            "test.css".into(),
            source.into(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut over_state,
        );
        assert!(!over.scoped_name("a").is_empty());
        assert!(over.scoped_name("b").is_empty());
        assert!(over.load_state.error.is_some());
    }

    #[test]
    fn css_modules_default_hash_preserves_unicode_utf16_semantics() {
        let source = "a😀b\u{0085}c";
        let codes = source.encode_utf16().collect::<Vec<_>>();
        let mut legacy = 5381u32;
        for code in codes.iter().rev() {
            legacy = legacy.wrapping_mul(33) ^ (*code as u32);
        }
        let mut expected = encode_base36_u32(legacy);
        expected.truncate(5);

        assert_eq!(css_module_default_hash(source), expected);
    }

    #[test]
    fn css_modules_scoped_name_preserves_staged_template_expansion() {
        let options = StyleCompileOptions {
            modules_options: CssModulesOptions {
                generate_scoped_name: Some("[name]".into()),
                ..CssModulesOptions::default()
            },
            ..StyleCompileOptions::default()
        };
        let mut state = CssModulesImportState::new(css_modules_test_import_limits());
        let mut context = CssModulesContext::new(
            &options,
            "src/[local][local].css".into(),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut state,
        );
        assert_eq!(context.scoped_name("x"), "xx");
        assert_eq!(context.load_state.replacement_steps, 3);

        let mut hash_state = CssModulesImportState::new(css_modules_test_import_limits());
        let mut hash_context = CssModulesContext::new(
            &options,
            "src/[hash:hex:4].css".into(),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut hash_state,
        );
        let hash = xxhash64(b"src/[hash:hex:4].css\0x");
        let hash = css_module_template_hash(hash, "hex", Some(4));
        let expected = if hash.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
            format!("_{hash}")
        } else {
            hash
        };
        assert_eq!(hash_context.scoped_name("x"), expected);
    }

    #[test]
    fn css_modules_scoped_name_rejects_multiplicative_templates_before_allocation() {
        let pattern = "[name]".repeat(5_000);
        let file_stem = "x".repeat(30_000);
        let options = StyleCompileOptions {
            modules_options: CssModulesOptions {
                generate_scoped_name: Some(pattern.clone()),
                ..CssModulesOptions::default()
            },
            ..StyleCompileOptions::default()
        };
        let mut state = CssModulesImportState::new(CssModulesImportLimits {
            max_scoped_name_pattern_bytes: pattern.len(),
            max_scoped_name_bytes: 1024,
            ..css_modules_test_import_limits()
        });
        let mut context = CssModulesContext::new(
            &options,
            format!("{file_stem}.css"),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut state,
        );
        assert!(context.scoped_name("a").is_empty());
        assert!(context
            .load_state
            .error
            .as_ref()
            .is_some_and(|error| error.message.contains("scoped name")));

        let source = ".a {}";
        let compile_options = StyleCompileOptions {
            id: Some("test".into()),
            filename: Some("test.css".into()),
            modules: true,
            modules_options: CssModulesOptions {
                generate_scoped_name: Some(pattern.clone()),
                ..CssModulesOptions::default()
            },
            ..StyleCompileOptions::default()
        };
        let result = compile_css_modules_with_limits(
            source,
            source,
            &compile_options,
            CssModulesImportLimits {
                max_scoped_name_pattern_bytes: pattern.len() - 1,
                ..css_modules_test_import_limits()
            },
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
        assert!(result.code.is_empty());
        assert!(result.raw_modules.is_empty());
        assert!(result.modules.is_empty());
    }

    #[test]
    fn css_modules_hash_prefix_is_ignored_without_hash_tokens() {
        let options = StyleCompileOptions {
            modules_options: CssModulesOptions {
                generate_scoped_name: Some("[local]".into()),
                hash_prefix: "x".repeat(8_192),
                ..CssModulesOptions::default()
            },
            ..StyleCompileOptions::default()
        };
        let mut state = CssModulesImportState::new(CssModulesImportLimits {
            max_scoped_name_hash_input_bytes: 0,
            ..css_modules_test_import_limits()
        });
        let mut context = CssModulesContext::new(
            &options,
            "test.css".into(),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut state,
        );

        assert_eq!(context.scoped_name("button"), "button");
        assert!(context.scoped_name_hash_resource_path.is_none());
        assert!(context.load_state.error.is_none());
    }

    #[test]
    fn css_modules_template_hash_is_reused_across_digest_tokens() {
        let pattern = "[hash:hex:4]-[hash:base64:4]";
        let hash = xxhash64(b"x.css\0a");
        let raw = format!(
            "{}-{}",
            css_module_template_hash(hash, "hex", Some(4)),
            css_module_template_hash(hash, "base64", Some(4))
        );
        let needs_prefix = raw.chars().next().is_some_and(|ch| ch.is_ascii_digit());
        let expected = if needs_prefix {
            format!("_{raw}")
        } else {
            raw.clone()
        };
        let exact_generated = pattern.len() * 2 + 7 + raw.len() + expected.len() * 3;
        let options = StyleCompileOptions {
            modules_options: CssModulesOptions {
                generate_scoped_name: Some(pattern.into()),
                ..CssModulesOptions::default()
            },
            ..StyleCompileOptions::default()
        };
        let exact_limits = CssModulesImportLimits {
            max_scoped_name_hash_input_bytes: 7,
            max_generated_bytes: exact_generated,
            max_replacement_steps: 2,
            ..css_modules_test_import_limits()
        };

        let mut exact_state = CssModulesImportState::new(exact_limits);
        let mut exact = CssModulesContext::new(
            &options,
            "x.css".into(),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut exact_state,
        );
        assert_eq!(exact.scoped_name("a"), expected);
        assert_eq!(exact.load_state.generated_bytes, exact_generated);
        assert_eq!(exact.load_state.replacement_steps, 2);

        let mut over_state = CssModulesImportState::new(CssModulesImportLimits {
            max_generated_bytes: exact_generated - 1,
            ..exact_limits
        });
        let mut over = CssModulesContext::new(
            &options,
            "x.css".into(),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut over_state,
        );
        assert!(over.scoped_name("a").is_empty());
        assert!(over.load_state.error.is_some());
    }

    #[test]
    fn css_modules_hash_pattern_parser_preserves_supported_forms() {
        for (token, digest, length) in [
            ("hash", "hex", None),
            ("contenthash:base64", "base64", None),
            ("hash:hex:5", "hex", Some(5)),
            ("xxhash64:hash", "hex", None),
            ("XXHASH64:contenthash:base64", "base64", None),
            ("xxhash64:hash:hex:7", "hex", Some(7)),
        ] {
            let parsed = parse_css_module_hash_pattern(token).expect("supported hash token");
            assert_eq!((parsed.digest, parsed.max_length), (digest, length));
        }
        for token in ["sha256:hash", "hash:a:b:c", "xxhash64:hash:a:b:c"] {
            assert!(parse_css_module_hash_pattern(token).is_none(), "{token}");
        }
    }

    #[test]
    fn css_modules_value_replacement_bounds_output_before_append() {
        let values = BTreeMap::from([("x".to_string(), "1234".to_string())]);
        let source = "x x x";
        let mut exact = CssModulesImportState::new(CssModulesImportLimits {
            max_generated_bytes: 14,
            max_replacement_steps: 3,
            ..css_modules_test_import_limits()
        });
        assert_eq!(
            replace_css_module_values(source, &values, &mut exact, 14).as_deref(),
            Some("1234 1234 1234")
        );
        assert_eq!((exact.generated_bytes, exact.replacement_steps), (14, 3));
        assert!(exact.error.is_none());

        let mut over = CssModulesImportState::new(CssModulesImportLimits {
            max_generated_bytes: 14,
            max_replacement_steps: 3,
            ..css_modules_test_import_limits()
        });
        assert!(replace_css_module_values(source, &values, &mut over, 13).is_none());
        assert!(over.generated_bytes <= 13);
        assert!(over.error.is_some());
    }

    #[test]
    fn css_modules_icss_symbol_replacement_enforces_exact_output() {
        let options = StyleCompileOptions::default();
        let mut exact_state = CssModulesImportState::new(CssModulesImportLimits {
            max_value_output_bytes: 21,
            max_generated_bytes: 36,
            max_replacement_steps: 3,
            ..css_modules_test_import_limits()
        });
        let mut exact = CssModulesContext::new(
            &options,
            "test.css".into(),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut exact_state,
        );
        exact.import_symbols.insert(
            "x".into(),
            CssModuleImportSymbol::Found("1234".into()),
        );
        assert_eq!(
            replace_css_module_import_symbols("color: x x x", &mut exact),
            "color: 1234 1234 1234"
        );
        assert_eq!(
            (
                exact.load_state.generated_bytes,
                exact.load_state.replacement_steps,
            ),
            (36, 3)
        );
        assert!(exact.load_state.error.is_none());

        let mut over_state = CssModulesImportState::new(CssModulesImportLimits {
            max_value_output_bytes: 20,
            max_generated_bytes: 36,
            max_replacement_steps: 3,
            ..css_modules_test_import_limits()
        });
        let mut over = CssModulesContext::new(
            &options,
            "test.css".into(),
            String::new(),
            CssModulesScopeBehaviour::Local,
            false,
            &mut over_state,
        );
        over.import_symbols.insert(
            "x".into(),
            CssModuleImportSymbol::Found("1234".into()),
        );
        assert!(replace_css_module_import_symbols("color: x x x", &mut over).is_empty());
        assert!(over.load_state.error.is_some());
    }

    #[test]
    fn css_modules_import_state_enforces_file_budgets_atomically() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first = dir.path().join("first.css");
        let second = dir.path().join("second.css");
        let invalid = dir.path().join("invalid.css");
        std::fs::write(&first, b"ab").expect("write first");
        std::fs::write(&second, b"cd").expect("write second");
        std::fs::write(&invalid, [0xff]).expect("write invalid");

        let exact_limits = CssModulesImportLimits {
            max_files: 3,
            max_file_bytes: 2,
            max_total_bytes: 5,
            ..css_modules_test_import_limits()
        };
        let mut exact = CssModulesImportState::new(exact_limits);
        assert_eq!(exact.read_module(&first).as_deref(), Some("ab"));
        assert_eq!(exact.read_module(&second).as_deref(), Some("cd"));
        assert!(exact.read_module(&invalid).is_none());
        assert_eq!((exact.imported_files, exact.imported_bytes), (3, 5));
        assert!(exact.error.is_none());

        let mut file_over = CssModulesImportState::new(CssModulesImportLimits {
            max_file_bytes: 1,
            ..css_modules_test_import_limits()
        });
        assert!(file_over.read_module(&first).is_none());
        assert_eq!((file_over.imported_files, file_over.imported_bytes), (0, 0));
        assert!(file_over.error.is_some());

        let mut total_over = CssModulesImportState::new(CssModulesImportLimits {
            max_file_bytes: 2,
            max_total_bytes: 3,
            ..css_modules_test_import_limits()
        });
        assert_eq!(total_over.read_module(&first).as_deref(), Some("ab"));
        assert!(total_over.read_module(&second).is_none());
        assert_eq!(
            (total_over.imported_files, total_over.imported_bytes),
            (1, 2)
        );
        assert!(total_over.error.is_some());

        let mut count_over = CssModulesImportState::new(CssModulesImportLimits {
            max_files: 1,
            ..css_modules_test_import_limits()
        });
        assert_eq!(count_over.read_module(&first).as_deref(), Some("ab"));
        assert!(count_over.read_module(&second).is_none());
        assert_eq!(
            (count_over.imported_files, count_over.imported_bytes),
            (1, 2)
        );
        assert!(count_over.error.is_some());
    }

    #[test]
    fn css_modules_import_state_enforces_metadata_budgets_atomically() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first = dir.path().join("first.json");
        let second = dir.path().join("second.json");
        let invalid = dir.path().join("invalid.json");
        std::fs::write(&first, b"{}").expect("write first");
        std::fs::write(&second, b"[]").expect("write second");
        std::fs::write(&invalid, [0xff]).expect("write invalid");

        let mut exact = CssModulesImportState::new(CssModulesImportLimits {
            max_metadata_file_bytes: 2,
            max_metadata_bytes: 5,
            ..css_modules_test_import_limits()
        });
        assert_eq!(exact.read_metadata(&first).as_deref(), Some("{}"));
        assert_eq!(exact.read_metadata(&second).as_deref(), Some("[]"));
        assert!(exact.read_metadata(&invalid).is_none());
        assert_eq!(exact.metadata_bytes, 5);
        assert!(exact.error.is_none());

        let mut file_over = CssModulesImportState::new(CssModulesImportLimits {
            max_metadata_file_bytes: 1,
            ..css_modules_test_import_limits()
        });
        assert!(file_over.read_metadata(&first).is_none());
        assert_eq!(file_over.metadata_bytes, 0);
        assert!(file_over.error.is_some());

        let mut total_over = CssModulesImportState::new(CssModulesImportLimits {
            max_metadata_file_bytes: 2,
            max_metadata_bytes: 3,
            ..css_modules_test_import_limits()
        });
        assert_eq!(total_over.read_metadata(&first).as_deref(), Some("{}"));
        assert!(total_over.read_metadata(&second).is_none());
        assert_eq!(total_over.metadata_bytes, 2);
        assert!(total_over.error.is_some());
    }

    #[test]
    fn css_modules_import_state_bounds_path_probes_and_path_bytes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("dep.css");
        std::fs::write(&file, ".dep {}").expect("write dep");
        let path_bytes = file.as_os_str().as_encoded_bytes().len();

        let mut probes = CssModulesImportState::new(CssModulesImportLimits {
            max_path_bytes: path_bytes,
            max_path_probes: 1,
            ..css_modules_test_import_limits()
        });
        assert!(probes.is_file(&file));
        assert!(!probes.is_file(&file));
        assert_eq!(probes.path_probes, 1);
        assert!(probes.error.is_some());

        let mut path = CssModulesImportState::new(CssModulesImportLimits {
            max_path_bytes: path_bytes.saturating_sub(1),
            ..css_modules_test_import_limits()
        });
        assert!(!path.is_file(&file));
        assert_eq!(path.path_probes, 0);
        assert!(path.error.is_some());
    }

    #[test]
    fn css_modules_import_limits_report_stable_diagnostics() {
        let dir = tempfile::tempdir().expect("temp dir");
        let entry = dir.path().join("entry.css");
        let dep = dir.path().join("dep.css");
        let source = ".button { composes: dep from \"./dep.css\"; }";
        let dependency = ".dep { color: red; }";
        std::fs::write(&entry, source).expect("write entry");
        std::fs::write(&dep, dependency).expect("write dep");
        let options = css_modules_test_options(&entry);

        let result = compile_css_modules_with_limits(
            source,
            source,
            &options,
            CssModulesImportLimits {
                max_file_bytes: dependency.len() - 1,
                ..css_modules_test_import_limits()
            },
        );

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
        assert!(result.diagnostics[0].message.contains("maximum"));
        assert!(result.code.is_empty());
        assert!(result.raw_modules.is_empty());
        assert!(result.modules.is_empty());

        let package = dir.path().join("node_modules").join("vuec-css-budget");
        std::fs::create_dir_all(&package).expect("create package");
        let package_json = r#"{"main":"index.css"}"#;
        std::fs::write(package.join("package.json"), package_json).expect("write package json");
        std::fs::write(package.join("index.css"), dependency).expect("write package css");
        let package_source = ".button { composes: dep from \"vuec-css-budget\"; }";
        let metadata_result = compile_css_modules_with_limits(
            package_source,
            package_source,
            &options,
            CssModulesImportLimits {
                max_metadata_file_bytes: package_json.len() - 1,
                ..css_modules_test_import_limits()
            },
        );
        assert_eq!(metadata_result.diagnostics.len(), 1);
        assert_eq!(
            metadata_result.diagnostics[0].code,
            "VUEC_STYLE_MODULE_LIMIT"
        );
        assert!(metadata_result.diagnostics[0]
            .message
            .contains("package metadata"));
    }

    #[test]
    fn css_modules_import_depth_has_exact_boundary() {
        let dir = tempfile::tempdir().expect("temp dir");
        let entry = dir.path().join("entry.css");
        let middle = dir.path().join("middle.css");
        let leaf = dir.path().join("leaf.css");
        let source = ".root { composes: middle from \"./middle.css\"; }";
        std::fs::write(&entry, source).expect("write entry");
        std::fs::write(
            &middle,
            ".middle { composes: leaf from \"./leaf.css\"; }",
        )
        .expect("write middle");
        std::fs::write(&leaf, ".leaf { color: red; }").expect("write leaf");
        let options = css_modules_test_options(&entry);

        let exact = compile_css_modules_with_limits(
            source,
            source,
            &options,
            CssModulesImportLimits {
                max_depth: 2,
                ..css_modules_test_import_limits()
            },
        );
        assert!(exact.diagnostics.is_empty(), "{:?}", exact.diagnostics);
        assert!(exact
            .modules
            .get("root")
            .is_some_and(|value| value.contains("_leaf_")));

        let over = compile_css_modules_with_limits(
            source,
            source,
            &options,
            CssModulesImportLimits {
                max_depth: 1,
                ..css_modules_test_import_limits()
            },
        );
        assert_eq!(over.diagnostics.len(), 1);
        assert_eq!(over.diagnostics[0].code, "VUEC_STYLE_MODULE_LIMIT");
        assert!(over.diagnostics[0].message.contains("maximum depth of 1"));
    }

    #[test]
    fn css_modules_external_composes_loads_each_source_once() {
        let dir = tempfile::tempdir().expect("temp dir");
        let entry = dir.path().join("entry.css");
        let dep = dir.path().join("dep.css");
        let source = ".root { composes: a b c from \"./dep.css\"; }";
        std::fs::write(&entry, source).expect("write entry");
        std::fs::write(&dep, ".a {} .b {} .c {}").expect("write dep");
        let options = css_modules_test_options(&entry);

        let result = compile_css_modules_with_limits(
            source,
            source,
            &options,
            CssModulesImportLimits {
                // One root canonicalization plus one file check and canonicalization.
                max_path_probes: 3,
                ..css_modules_test_import_limits()
            },
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let root = result.modules.get("root").expect("root module");
        for name in ["_a_", "_b_", "_c_"] {
            assert!(root.contains(name), "missing {name}: {root}");
        }
    }

    #[test]
    fn css_modules_package_self_reference_uses_physical_cycle_identity() {
        let dir = tempfile::tempdir().expect("temp dir");
        let entry = dir.path().join("entry.css");
        let package = dir.path().join("node_modules").join("vuec-css-self");
        std::fs::create_dir_all(&package).expect("create package");
        let source = ".entry { composes: loop from \"vuec-css-self\"; }";
        std::fs::write(&entry, source).expect("write entry");
        std::fs::write(
            package.join("index.css"),
            ".loop { composes: loop from \"vuec-css-self\"; }",
        )
        .expect("write package css");
        let options = css_modules_test_options(&entry);

        let result = compile_css_modules_with_limits(
            source,
            source,
            &options,
            CssModulesImportLimits {
                max_files: 1,
                ..css_modules_test_import_limits()
            },
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    }

    #[test]
    fn css_modules_package_resolution_is_bounded_and_cannot_escape() {
        assert!(split_css_module_package_specifier("pkg/../outside.css").is_none());
        assert!(split_css_module_package_specifier("@scope/pkg/../../outside.css").is_none());
        assert!(split_css_module_package_specifier("pkg\\outside.css").is_none());

        let dir = tempfile::tempdir().expect("temp dir");
        let entry = dir.path().join("entry.css");
        std::fs::create_dir(dir.path().join("pkg")).expect("create fallback directory");
        std::fs::write(dir.path().join("outside.css"), ".outside {}").expect("write outside");
        let mut fallback_state = CssModulesImportState::new(css_modules_test_import_limits());
        assert!(resolve_css_module_import(
            "pkg/../outside.css",
            &entry.to_string_lossy(),
            &mut fallback_state,
        )
        .is_none());
        assert!(fallback_state.error.is_none());

        let package = dir.path().join("package");
        std::fs::create_dir_all(&package).expect("create package");
        let mut state = CssModulesImportState::new(css_modules_test_import_limits());
        assert!(css_module_package_export_target(
            &package,
            "./../outside.css",
            &mut state,
        )
        .is_none());
        assert!(state.error.is_none());

        let package_json: CssModulePackageJson = serde_json::from_str(
            r#"{"exports":{"./*":"./********"}}"#,
        )
        .expect("parse package json");
        let exports = package_json.exports.as_ref().expect("exports");
        let mut exact = CssModulesImportState::new(CssModulesImportLimits {
            max_path_bytes: 66,
            ..css_modules_test_import_limits()
        });
        let target = css_module_package_exports_subpath_target(
            exports,
            "./12345678",
            &mut exact,
        )
        .expect("exact target");
        assert_eq!(target.len(), 66);
        assert!(exact.error.is_none());

        let mut over = CssModulesImportState::new(CssModulesImportLimits {
            max_path_bytes: 65,
            ..css_modules_test_import_limits()
        });
        assert!(css_module_package_exports_subpath_target(
            exports,
            "./12345678",
            &mut over,
        )
        .is_none());
        assert!(over.error.is_some());
    }
