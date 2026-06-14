    #[test]
    fn transform_model_projection_selects_text_runtime_and_filters_model_value() {
        let projection = model_projection("input", vec![]);

        assert_eq!(projection["errors"], json!([]));
        assert_eq!(projection["needRuntime"], json!("V_MODEL_TEXT"));
        assert_eq!(projection["props"].as_array().unwrap().len(), 1);
        assert_eq!(
            projection["props"][0]["key"],
            json!({ "kind": "static", "content": "onUpdate:modelValue" })
        );
    }

    #[test]
    fn transform_model_projection_selects_native_input_helpers() {
        let radio = model_projection(
            "input",
            vec![json!({
                "type": 6,
                "name": "type",
                "value": { "content": "radio" },
            })],
        );
        assert_eq!(radio["needRuntime"], json!("V_MODEL_RADIO"));

        let checkbox = model_projection(
            "input",
            vec![json!({
                "type": 6,
                "name": "type",
                "value": { "content": "checkbox" },
            })],
        );
        assert_eq!(checkbox["needRuntime"], json!("V_MODEL_CHECKBOX"));

        let dynamic = model_projection(
            "input",
            vec![json!({
                "type": 7,
                "name": "bind",
                "arg": { "type": 4, "content": "type", "isStatic": true },
                "exp": { "type": 4, "content": "kind" },
            })],
        );
        assert_eq!(dynamic["needRuntime"], json!("V_MODEL_DYNAMIC"));

        let static_type_wins_over_dynamic_bind = model_projection(
            "input",
            vec![
                json!({
                    "type": 6,
                    "name": "type",
                    "value": { "content": "radio" },
                }),
                json!({
                    "type": 7,
                    "name": "bind",
                    "arg": null,
                    "exp": { "type": 4, "content": "attrs" },
                }),
            ],
        );
        assert_eq!(
            static_type_wins_over_dynamic_bind["needRuntime"],
            json!("V_MODEL_RADIO")
        );
    }

    #[test]
    fn transform_model_projection_selects_select_textarea_and_custom_helpers() {
        let select = model_projection("select", vec![]);
        assert_eq!(select["needRuntime"], json!("V_MODEL_SELECT"));

        let textarea = model_projection("textarea", vec![]);
        assert_eq!(textarea["needRuntime"], json!("V_MODEL_TEXT"));

        let custom = transform_model_projection(&json!({
            "dir": model_dir(),
            "node": model_node("my-input", vec![]),
            "context": { "isCustomElement": true },
        }));
        assert_eq!(custom["errors"], json!([]));
        assert_eq!(custom["needRuntime"], json!("V_MODEL_TEXT"));
    }

    #[test]
    fn transform_model_projection_reports_dom_model_errors() {
        let file = model_projection(
            "input",
            vec![json!({
                "type": 6,
                "name": "type",
                "value": { "content": "file" },
            })],
        );
        assert_eq!(file["errors"][0]["code"], json!(60));
        assert!(file.get("needRuntime").is_none());

        let invalid = model_projection("span", vec![]);
        assert_eq!(invalid["errors"][0]["code"], json!(58));

        let with_arg = transform_model_projection(&json!({
            "dir": {
                "name": "model",
                "exp": {
                    "type": 4,
                    "content": "model",
                    "loc": { "source": "model" }
                },
                "arg": {
                    "type": 4,
                    "content": "value",
                    "isStatic": true,
                    "loc": { "source": "value" }
                },
                "modifiers": [],
                "loc": { "source": "v-model:value=\"model\"" }
            },
            "node": model_node("input", vec![]),
            "context": {},
        }));
        assert_eq!(with_arg["errors"][0]["code"], json!(59));
        assert_eq!(with_arg["errors"][0]["loc"]["source"], json!("value"));
        assert_eq!(with_arg["props"].as_array().unwrap().len(), 2);

        let dynamic_value = model_projection(
            "input",
            vec![json!({
                "type": 7,
                "name": "bind",
                "arg": { "type": 4, "content": "value", "isStatic": true },
                "exp": { "type": 4, "content": "model" },
                "loc": { "source": ":value=\"model\"" },
            })],
        );
        assert_eq!(dynamic_value["errors"][0]["code"], json!(61));
        assert_eq!(
            dynamic_value["errors"][0]["loc"]["source"],
            json!(":value=\"model\"")
        );

        let static_value = model_projection(
            "input",
            vec![json!({
                "type": 6,
                "name": "value",
                "value": { "content": "model" },
            })],
        );
        assert_eq!(static_value["errors"], json!([]));
    }
