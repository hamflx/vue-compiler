    #[test]
    fn vue3_dom_conformance_shims_use_dom_vitest_glob() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-dom-shims-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        write_vue3_core_source_shims(&temp).unwrap();
        let index_spec = temp
            .join("packages")
            .join("compiler-dom")
            .join("__tests__")
            .join("index.spec.ts");
        fs::create_dir_all(index_spec.parent().unwrap()).unwrap();
        fs::write(&index_spec, "import { compile } from '../src'\n").unwrap();
        write_vue3_dom_conformance_shims(&temp).unwrap();

        let config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(!config.contains("vitest/config"));
        assert!(config.contains("include: ['packages/compiler-dom/__tests__/**/*.spec.ts']"));
        let index_spec = fs::read_to_string(index_spec).unwrap();
        assert!(index_spec.contains("import { compile } from '@vue/compiler-dom'"));

        let transform_style = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("transformStyle.ts"),
        )
        .unwrap();
        assert!(transform_style.contains("export { transformStyle } from '@vue/compiler-dom'"));

        let stringify_static = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("stringifyStatic.ts"),
        )
        .unwrap();
        assert!(stringify_static.contains("__vuecRuntime"));
        assert!(stringify_static.contains("@vue/compiler-core"));
        assert!(stringify_static.contains("StringifyThresholds"));
        assert!(stringify_static.contains("stringifyStatic"));

        let v_html = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("vHtml.ts"),
        )
        .unwrap();
        assert!(v_html.contains("__vuecRuntime"));
        assert!(v_html.contains("@vue/compiler-dom"));
        assert!(v_html.contains("transformVHtml"));

        let v_text = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("vText.ts"),
        )
        .unwrap();
        assert!(v_text.contains("__vuecRuntime"));
        assert!(v_text.contains("@vue/compiler-dom"));
        assert!(v_text.contains("transformVText"));

        let v_show = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("vShow.ts"),
        )
        .unwrap();
        assert!(v_show.contains("__vuecRuntime"));
        assert!(v_show.contains("@vue/compiler-dom"));
        assert!(v_show.contains("transformShow"));

        let v_on = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("vOn.ts"),
        )
        .unwrap();
        assert!(v_on.contains("__vuecRuntime"));
        assert!(v_on.contains("@vue/compiler-dom"));
        assert!(v_on.contains("transformOn"));
        assert!(v_on.contains("V_ON_WITH_MODIFIERS"));

        let v_model = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("vModel.ts"),
        )
        .unwrap();
        assert!(v_model.contains("__vuecRuntime"));
        assert!(v_model.contains("@vue/compiler-dom"));
        assert!(v_model.contains("transformModel"));
        assert!(v_model.contains("V_MODEL_TEXT"));
        let transition = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("Transition.ts"),
        )
        .unwrap();
        assert!(transition.contains("__vuecRuntime"));
        assert!(transition.contains("@vue/compiler-dom"));
        assert!(transition.contains("transformTransition"));
        assert!(transition.contains("TRANSITION"));
        let ignore_side_effect_tags = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("ignoreSideEffectTags.ts"),
        )
        .unwrap();
        assert!(ignore_side_effect_tags.contains("__vuecRuntime"));
        assert!(ignore_side_effect_tags.contains("@vue/compiler-dom"));
        assert!(ignore_side_effect_tags.contains("ignoreSideEffectTags"));
        let decode_html_browser = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("decodeHtmlBrowser.ts"),
        )
        .unwrap();
        assert!(decode_html_browser.contains("__vuecRuntime"));
        assert!(decode_html_browser.contains("@vue/compiler-dom"));
        assert!(decode_html_browser.contains("decodeHtmlBrowser"));
        let validate_html_nesting = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("transforms")
                .join("validateHtmlNesting.ts"),
        )
        .unwrap();
        assert!(validate_html_nesting.contains("__vuecRuntime"));
        assert!(validate_html_nesting.contains("validateHtmlNesting"));
        let html_nesting = fs::read_to_string(
            temp.join("packages")
                .join("compiler-dom")
                .join("src")
                .join("htmlNesting.ts"),
        )
        .unwrap();
        assert!(html_nesting.contains("__vuecRuntime"));
        assert!(html_nesting.contains("isValidHTMLNesting"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformVHtml"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformVText"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformShow"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformOn"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformModel"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.transformTransition"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.ignoreSideEffectTags"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.decodeHtmlBrowser"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.validateHtmlNesting"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.dom.isValidHTMLNesting"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3.core.stringifyStatic"));
        let _ = fs::remove_dir_all(temp);
    }
