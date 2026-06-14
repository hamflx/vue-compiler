    #[test]
    fn compiles_vars_modules_and_map() {
        let result = compile_style(
            ".a { color: v-bind(color); }",
            StyleCompileOptions {
                id: Some("data-v-x".into()),
                scoped: true,
                modules: true,
                source_map: true,
                ..StyleCompileOptions::default()
            },
        );
        assert!(result.code.contains("[data-v-x]"));
        assert!(result.code.contains("var(--x-color)"));
        let modules = result.modules.expect("css modules map");
        assert!(modules.get("a").is_some_and(|value| value.contains("_a_")));
        assert_eq!(result.vars, vec!["color"]);
        assert!(result.map.is_some());
    }

    #[test]
    fn compiles_css_modules_default_local_and_global_pseudo() {
        let result = compile_style(
            ".red { color: red }\n.green { color: green }\n:global(.blue) { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(modules
            .get("red")
            .is_some_and(|value| value.contains("_red_")));
        assert!(modules
            .get("green")
            .is_some_and(|value| value.contains("_green_")));
        assert!(!modules.contains_key("blue"));
        assert!(result.code.contains(".blue { color: blue }"));
    }

    #[test]
    fn compiles_css_modules_global_scope_with_local_and_camel_case_only() {
        let result = compile_style(
            ":local(.foo-bar) { color: red }\n.baz-qux { color: green }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    scope_behaviour: "global".into(),
                    generate_scoped_name: Some("[name]__[local]__[hash:base64:5]".into()),
                    locals_convention: "camelCaseOnly".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(modules
            .get("fooBar")
            .is_some_and(|value| value.contains("__foo-bar__")));
        assert!(!modules.contains_key("foo-bar"));
        assert!(!modules.contains_key("bazQux"));
        assert!(result.code.contains(".baz-qux { color: green }"));
    }

    #[test]
    fn compiles_css_modules_leaves_class_attribute_selectors_global() {
        let result = compile_style(
            "[class=\"btn\"] { color: red }\n:local([class='forced']) { color: blue }\n[class~=tag] { color: green }\n.btn { color: black }",
            StyleCompileOptions {
                filename: Some("src/Attr.vue".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(modules
            .get("btn")
            .is_some_and(|value| value.contains("_btn_")));
        assert!(!modules.contains_key("forced"));
        assert!(!modules.contains_key("tag"));
        assert!(result.code.contains("[class=\"btn\"] { color: red }"));
        assert!(result.code.contains("[class='forced'] { color: blue }"));
        assert!(result.code.contains("[class~=tag] { color: green }"));
        assert!(result.code.contains("._btn_"));
        assert!(result.code.contains("{ color: black }"));
    }

    #[test]
    fn compiles_css_modules_global_module_paths_for_matching_file() {
        let result = compile_style(
            ".button { color: red }\n:local(.forced) { color: blue }",
            StyleCompileOptions {
                filename: Some("src/theme.global.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    global_module_paths: vec![r"global\.css$".into()],
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(!modules.contains_key("button"));
        assert!(modules
            .get("forced")
            .is_some_and(|value| value.contains("_forced_")));
        assert!(result.code.contains(".button { color: red }"));
        assert!(result.code.contains("._forced_"));
    }

    #[test]
    fn compiles_css_modules_global_module_paths_uses_entry_scope_for_imported_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("theme.global.css");
        std::fs::write(
            &dep,
            ".dep { color: blue; }\n:local(.forced) { color: green; }",
        )
        .expect("write dep");

        let result = compile_style(
            ".button { composes: forced from \"./theme.global.css\"; color: red; }",
            StyleCompileOptions {
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                modules_options: CssModulesOptions {
                    global_module_paths: vec![r"global\.css$".into()],
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(!modules.contains_key("dep"));
        assert!(button.contains("_button_"));
        assert!(button.contains("_forced_"));
        assert!(result.code.contains("._dep_"));
        assert!(result.code.contains("._forced_"));
    }

    #[test]
    fn compiles_css_modules_id_selectors_like_official() {
        let result = compile_style(
            "#panel { color: red }\n.button#item { color: blue }",
            StyleCompileOptions {
                filename: Some("src/Selectors.vue".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(
            modules.get("panel").map(String::as_str),
            Some("_panel_aau0c_1")
        );
        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("_button_aau0c_2")
        );
        assert_eq!(
            modules.get("item").map(String::as_str),
            Some("_item_aau0c_1")
        );
        assert!(result.code.contains("#_panel_aau0c_1"));
        assert!(result.code.contains("._button_aau0c_2#_item_aau0c_1"));
    }

    #[test]
    fn compiles_css_modules_global_scope_local_id_only() {
        let result = compile_style(
            ":local(#panel) #plain { color: red }",
            StyleCompileOptions {
                filename: Some("src/Selectors.vue".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    scope_behaviour: "global".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(
            modules.get("panel").map(String::as_str),
            Some("_panel_qt8vi_1")
        );
        assert!(!modules.contains_key("plain"));
        assert!(result.code.contains("#_panel_qt8vi_1 #plain"));
    }

    #[test]
    fn compiles_css_modules_export_global_ids() {
        let result = compile_style(
            ":global(#panel) { color: red }",
            StyleCompileOptions {
                filename: Some("src/Selectors.vue".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    export_globals: true,
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("panel").map(String::as_str), Some("panel"));
        assert!(result.code.contains("#panel"));
    }

    #[test]
    fn compiles_css_modules_generate_scoped_name_hash_prefix_like_official() {
        let result = compile_style(
            ".button { color: red }",
            StyleCompileOptions {
                id: Some("data-v-probe".into()),
                filename: Some("src/Comp.vue".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    generate_scoped_name: Some("[local]__[hash:base64:5]".into()),
                    hash_prefix: "alpha".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("button__2G66Z")
        );
        assert!(result.code.contains(".button__2G66Z"));
    }

    #[test]
    fn ignores_css_modules_hash_prefix_for_default_scoped_names_like_official() {
        let base = compile_style(
            ".button { color: red }",
            StyleCompileOptions {
                id: Some("data-v-probe".into()),
                filename: Some("src/Comp.vue".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let prefixed = compile_style(
            ".button { color: red }",
            StyleCompileOptions {
                id: Some("data-v-probe".into()),
                filename: Some("src/Comp.vue".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    hash_prefix: "alpha".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(base.code, prefixed.code);
        assert_eq!(base.modules, prefixed.modules);
    }

    #[test]
    fn compiles_css_modules_keyframes_and_animation_names_like_official() {
        let result = compile_style(
            "@keyframes fade { from { opacity: 0 } to { opacity: 1 } }\n.button { animation-name: fade; }",
            StyleCompileOptions {
                filename: Some("src/Anim.vue".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

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
    fn compiles_css_modules_animation_shorthand_keywords_like_official() {
        let result = compile_style(
            ".button { animation: infinite infinite, ease ease, none 1s, fade 1s; }",
            StyleCompileOptions {
                filename: Some("src/Anim.vue".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("_button_11sc8_1")
        );
        assert_eq!(
            modules.get("infinite").map(String::as_str),
            Some("_infinite_11sc8_1")
        );
        assert_eq!(
            modules.get("ease").map(String::as_str),
            Some("_ease_11sc8_1")
        );
        assert_eq!(
            modules.get("fade").map(String::as_str),
            Some("_fade_11sc8_1")
        );
        assert!(!modules.contains_key("none"));
        assert!(result.code.contains(
            "animation: infinite _infinite_11sc8_1, ease _ease_11sc8_1, none 1s, _fade_11sc8_1 1s"
        ));
    }

    #[test]
    fn compiles_css_modules_global_scope_local_keyframes_only() {
        let result = compile_style(
            "@keyframes :local(fade) { from { opacity: 0 } to { opacity: 1 } }\n.button { animation-name: fade; }",
            StyleCompileOptions {
                filename: Some("src/Anim.vue".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    scope_behaviour: "global".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(
            modules.get("fade").map(String::as_str),
            Some("_fade_3cm9u_1")
        );
        assert!(!modules.contains_key("button"));
        assert!(result.code.contains("@keyframes _fade_3cm9u_1"));
        assert!(result.code.contains(".button { animation-name: fade"));
    }

    #[test]
    fn compiles_css_modules_dashes_locals_convention() {
        let result = compile_style(
            ".foo-bar { color: red }\n.foo_bar { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    locals_convention: "dashes".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let foo_bar_scoped = modules.get("foo-bar").expect("original dashed export");

        assert_eq!(modules.get("fooBar"), Some(foo_bar_scoped));
        assert!(modules
            .get("foo_bar")
            .is_some_and(|value| value.contains("_foo_bar_")));
        assert_ne!(modules.get("fooBar"), modules.get("foo_bar"));
        assert!(result.code.contains("._foo-bar_"));
        assert!(result.code.contains("._foo_bar_"));
    }

    #[test]
    fn compiles_css_modules_dashes_only_locals_convention() {
        let result = compile_style(
            ".foo-bar { color: red }\n.foo_bar { color: blue }",
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

        assert!(modules
            .get("fooBar")
            .is_some_and(|value| value.contains("_foo-bar_")));
        assert!(!modules.contains_key("foo-bar"));
        assert!(modules
            .get("foo_bar")
            .is_some_and(|value| value.contains("_foo_bar_")));
        assert_ne!(modules.get("fooBar"), modules.get("foo_bar"));
        assert!(result.code.contains("._foo-bar_"));
        assert!(result.code.contains("._foo_bar_"));
    }

    #[test]
    fn compiles_css_modules_locals_convention_alias_collisions_like_official() {
        for locals_convention in ["camelCase", "dashes"] {
            let result = compile_style(
                ".foo-bar { color: red }\n.fooBar { color: blue }",
                StyleCompileOptions {
                    id: Some("test".into()),
                    filename: Some("test.css".into()),
                    modules: true,
                    modules_options: CssModulesOptions {
                        locals_convention: locals_convention.into(),
                        ..CssModulesOptions::default()
                    },
                    ..StyleCompileOptions::default()
                },
            );
            let modules = result.modules.expect("css modules map");

            assert!(modules
                .get("foo-bar")
                .is_some_and(|value| value.contains("_foo-bar_")));
            assert!(modules
                .get("fooBar")
                .is_some_and(|value| value.contains("_fooBar_")));
        }
    }

    #[test]
    fn compiles_css_modules_export_globals() {
        let result = compile_style(
            ".local :global(.global) { color: red }\n:global(.blue) { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    export_globals: true,
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(modules
            .get("local")
            .is_some_and(|value| value.contains("_local_")));
        assert_eq!(modules.get("global").map(String::as_str), Some("global"));
        assert_eq!(modules.get("blue").map(String::as_str), Some("blue"));
        assert!(result.code.contains("._local_"));
        assert!(result.code.contains(".global"));
        assert!(result.code.contains(".blue { color: blue }"));
    }

    #[test]
    fn compiles_css_modules_export_globals_with_global_scope_and_convention() {
        let result = compile_style(
            ".foo-bar .foo_bar { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    scope_behaviour: "global".into(),
                    locals_convention: "dashesOnly".into(),
                    export_globals: true,
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("fooBar").map(String::as_str), Some("foo-bar"));
        assert_eq!(modules.get("foo_bar").map(String::as_str), Some("foo_bar"));
        assert!(!modules.contains_key("foo-bar"));
        assert_eq!(result.code, ".foo-bar .foo_bar { color: blue }");
    }

    #[test]
    fn compiles_css_modules_local_composes() {
        let result = compile_style(
            ".base { color: blue }\n.button { composes: base; color: red }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let base = modules.get("base").expect("base export");
        let button = modules.get("button").expect("button export");

        assert!(base.contains("_base_"));
        assert!(button.contains("_button_"));
        assert!(button.contains(base));
        assert!(!result.code.contains("composes"));
        assert!(result.code.contains("._button_"));
    }

    #[test]
    fn compiles_css_modules_global_and_chained_composes() {
        let result = compile_style(
            ".base { composes: global(reset); }\n.button { composes: base global(extra); }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let base = modules.get("base").expect("base export");
        let button = modules.get("button").expect("button export");

        assert!(base.contains("_base_"));
        assert!(base.contains("reset"));
        assert!(button.contains("_button_"));
        assert!(button.contains(base));
        assert!(button.contains("extra"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn reports_css_modules_composes_on_complex_selector() {
        let result = compile_style(
            ".button.extra { composes: base; }\n.next { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(
            result.errors,
            vec![
                "composition is only allowed when selector is single :local class name not in \":local(.button):local(.extra)\""
            ]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        assert!(result.code.is_empty());
        assert!(result.modules.is_none());
    }

    #[test]
    fn reports_css_modules_missing_composes_class() {
        let result = compile_style(
            ".button { composes: missing; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                source_map_file_id: Some(FileId(11)),
                source_map_base_offset: 20,
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(
            result.errors,
            vec!["referenced class name \"missing\" in composes not found"]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        let start = ".button { composes: ".len();
        assert_eq!(
            result.diagnostics[0].span,
            Some(Span::new(
                FileId(11),
                20 + start,
                20 + start + "missing".len()
            ))
        );
        assert!(result.code.is_empty());
        assert!(result.modules.is_none());
    }

    #[test]
    fn reports_css_modules_late_composes_class() {
        let result = compile_style(
            ".button { composes: next; }\n.next { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(
            result.errors,
            vec!["referenced class name \"next\" in composes not found"]
        );
        assert!(result.code.is_empty());
        assert!(result.modules.is_none());
    }

    #[test]
    fn compiles_css_modules_missing_external_composes_class_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }").expect("write dep");

        let result = compile_style(
            ".button { composes: missing from \"./dep.css\"; }",
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
        assert!(result.diagnostics.is_empty());
        assert!(button.contains("undefined"));
        assert!(!result.code.contains("composes"));
    }
