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

        assert_eq!(
            replace_css_module_values(source, &values),
            r#"A X Y xalpha alpha2 1alpha -alpha Z Q /* alpha */ "X" \X"#
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

        assert_eq!(replace_css_module_values(&source, &values), source);
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
