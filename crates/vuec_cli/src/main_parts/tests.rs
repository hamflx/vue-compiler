#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_exits_successfully() {
        let output = run_with_args(["vuec", "--help"]).expect("run");
        assert_eq!(output.code, 0);
        assert!(output.stdout.contains("compile-template"));
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn compiles_vue2_template_json() {
        let path = write_temp("vuec-cli-vue2.html", "<div>{{ msg }}</div>");
        let output = run_with_args([
            "vuec",
            "compile-template",
            "--target",
            "vue2",
            "--json",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("vue2-template"));
        assert!(value["render"].as_str().unwrap().contains("_c('div'"));
    }

    #[test]
    fn compiles_vue3_template_json() {
        let path = write_temp("vuec-cli-vue3.html", "<div>{{ msg }}</div>");
        let output = run_with_args([
            "vuec",
            "compile-template",
            "--target",
            "vue3",
            "--json",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("vue3-template"));
        assert!(value["code"].as_str().unwrap().contains("function render"));
    }

    #[test]
    fn validates_vue3_template_mode() {
        let path = write_temp("vuec-cli-vue3-mode.html", "<div>{{ msg }}</div>");

        for mode in ["function", "module"] {
            let output = run_with_args([
                "vuec",
                "compile-template",
                "--target",
                "vue3",
                "--mode",
                mode,
                path.to_str().unwrap(),
            ])
            .expect("run");
            assert_eq!(output.code, 0, "mode {mode}");
            assert!(output.stdout.contains("function render"), "mode {mode}");
        }

        let output = run_with_args([
            "vuec",
            "compile-template",
            "--target",
            "vue3",
            "--mode",
            "invalid",
            path.to_str().unwrap(),
        ])
        .expect("run");
        assert_eq!(output.code, 2);
        assert!(output.stderr.contains("invalid value 'invalid'"));
        assert!(output.stderr.contains("function"));
        assert!(output.stderr.contains("module"));
    }

    #[test]
    fn compiles_vue3_sfc_json() {
        let path = write_temp(
            "vuec-cli-sfc.vue",
            "<template><div>{{ msg }}</div></template><script setup>const msg = 'hi'</script>",
        );
        let output =
            run_with_args(["vuec", "compile-sfc", "--json", path.to_str().unwrap()]).expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("vue3-sfc"));
        assert!(value["template"]["code"]
            .as_str()
            .unwrap()
            .contains("function render"));
        assert!(value["script"]["content"]
            .as_str()
            .unwrap()
            .contains("setup"));
    }

    #[test]
    fn compiles_vue3_sfc_props_destructure_option_json() {
        let path = write_temp(
            "vuec-cli-sfc-props-destructure.vue",
            concat!(
                "<script setup>",
                "const { foo, bar: baz } = defineProps(['foo', 'bar'])\n",
                "const message = foo + baz",
                "</script>"
            ),
        );
        let output = run_with_args([
            "vuec",
            "compile-sfc",
            "--json",
            "--props-destructure",
            "disabled",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        let content = value["script"]["content"].as_str().unwrap_or_default();

        assert_eq!(value["kind"], json!("vue3-sfc"));
        assert!(content.contains("const { foo, bar: baz } = __props"));
        assert!(content.contains("const message = foo + baz"));
        assert!(!content.contains("__props.foo + __props.bar"));
    }

    #[test]
    fn compiles_vue3_sfc_global_type_files_json() {
        let sfc = write_temp(
            "vuec-cli-sfc-global-types.vue",
            concat!(
                "<script setup lang=\"ts\">",
                "defineProps<GlobalProps>()\n",
                "defineModel<GlobalModel>()",
                "</script>"
            ),
        );
        let global = write_temp(
            "vuec-cli-sfc-global-types.d.ts",
            concat!(
                "declare interface GlobalProps { msg: string }\n",
                "declare type GlobalModel = boolean | string"
            ),
        );
        let output = run_with_args([
            "vuec",
            "compile-sfc",
            "--json",
            "--global-type-file",
            global.to_str().unwrap(),
            sfc.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        let content = value["script"]["content"].as_str().unwrap_or_default();
        let deps = value["script"]["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        let expected_dep = global.to_string_lossy().replace('\\', "/");

        assert_eq!(value["kind"], json!("vue3-sfc"));
        assert!(content.contains("msg: { type: String, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert_eq!(deps, vec![expected_dep]);
    }

    #[test]
    fn compiles_vue3_sfc_inline_ssr_json() {
        let path = write_temp(
            "vuec-cli-sfc-inline-ssr.vue",
            concat!(
                "<script setup>import { ref } from 'vue'; const count = ref(0)</script>",
                "<template><div>{{ count }}</div></template>"
            ),
        );
        let output = run_with_args([
            "vuec",
            "compile-sfc",
            "--json",
            "--inline-template",
            "--ssr",
            "--id",
            "xxxxxxxx",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        let content = value["script"]["content"].as_str().unwrap_or_default();

        assert_eq!(value["kind"], json!("vue3-sfc-ssr"));
        assert!(content.contains("__ssrInlineRender: true,"));
        assert!(content.contains("return (_ctx, _push, _parent, _attrs) => {"));
        assert!(content.contains("_ssrInterpolate(count.value)"));
    }

    #[test]
    fn compiles_vue3_sfc_script_source_map_json() {
        let source = "<script setup>\nconst msg = 'hi'\n</script>";
        let path = write_temp("vuec-cli-sfc-script-map.vue", source);
        let output = run_with_args([
            "vuec",
            "compile-sfc",
            "--json",
            "--source-map",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");

        assert_eq!(value["kind"], json!("vue3-sfc"));
        assert_eq!(value["script"]["map"]["version"], json!(3));
        assert_eq!(value["script"]["map"]["sourcesContent"][0], json!(source));
        assert!(value["script"]["map"]["mappings"]
            .as_str()
            .is_some_and(|mappings| !mappings.is_empty()));
    }

    #[test]
    fn prints_sfc_style_diagnostic_source_range() {
        let source =
            "<template><div/></template>\n<style lang=\"less\">\n@import \"./theme\";\n</style>";
        let path = write_temp("vuec-cli-style-diagnostic.vue", source);
        let output = run_with_args([
            "vuec",
            "compile-sfc",
            "--diagnostics",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let import_start = source.find("@import").expect("import start");
        let import_end = import_start + "@import \"./theme\";".len();

        assert!(output.stderr.contains("VUEC_STYLE_IMPORT_RESOLVE"));
        assert!(output
            .stderr
            .contains(&format!("@{import_start}-{import_end}")));
        assert_eq!(output.code, 1);
    }

    #[test]
    fn compiles_vue3_ssr_json() {
        let path = write_temp("vuec-cli-ssr.html", "<div>{{ msg }}</div>");
        let output =
            run_with_args(["vuec", "compile-ssr", "--json", path.to_str().unwrap()]).expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("vue3-ssr-template"));
        assert!(value["code"].as_str().unwrap().contains("ssrRender"));
    }

    #[test]
    fn parses_sfc_json() {
        let path = write_temp("vuec-cli-parse.vue", "<template><p/></template>");
        let output =
            run_with_args(["vuec", "parse-sfc", "--json", path.to_str().unwrap()]).expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("sfc-descriptor"));
        assert!(value["descriptor"]["template"].is_object());
    }

    #[test]
    fn parse_sfc_reports_descriptor_diagnostics_and_exits_non_zero() {
        let source = "<template><div/></template><template><span/></template><script>const one = 1</script><script>const two = 2</script>";
        let path = write_temp("vuec-cli-parse-errors.vue", source);
        let output =
            run_with_args(["vuec", "parse-sfc", "--json", path.to_str().unwrap()]).expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");

        assert_eq!(output.code, 1);
        assert_eq!(value["diagnostics"][0]["code"], json!("VUEC_SFC_PARSE"));
        assert_eq!(
            value["diagnostics"][0]["message"],
            json!("Single file component can contain only one <template> element")
        );
        assert_eq!(
            value["diagnostics"][1]["message"],
            json!("Single file component can contain only one <script> element")
        );
        assert!(output.stderr.contains("VUEC_SFC_PARSE"));
    }

    #[test]
    fn compile_sfc_reports_descriptor_diagnostics_once() {
        let source = "<template><div/></template><template><span/></template><script>const one = 1</script><script>const two = 2</script>";
        let path = write_temp("vuec-cli-compile-errors.vue", source);
        let output =
            run_with_args(["vuec", "compile-sfc", "--json", path.to_str().unwrap()]).expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");

        assert_eq!(output.code, 1);
        assert_eq!(value["diagnostics"].as_array().unwrap().len(), 2);
        assert_eq!(value["diagnostics"][0]["code"], json!("VUEC_SFC_PARSE"));
        assert_eq!(value["diagnostics"][1]["code"], json!("VUEC_SFC_PARSE"));
    }

    #[test]
    fn compile_ssr_sfc_reports_descriptor_diagnostics_once() {
        let source = "<template><div/></template><template><span/></template>";
        let path = write_temp("vuec-cli-ssr-sfc-errors.vue", source);
        let output = run_with_args([
            "vuec",
            "compile-ssr",
            "--sfc",
            "--json",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");

        assert_eq!(output.code, 1);
        assert_eq!(value["diagnostics"].as_array().unwrap().len(), 1);
        assert_eq!(value["diagnostics"][0]["code"], json!("VUEC_SFC_PARSE"));
    }

    #[test]
    fn benchmarks_vue3_template_json() {
        let path = write_temp("vuec-cli-bench.html", "<div/>");
        let output = run_with_args([
            "vuec",
            "bench",
            "--target",
            "vue3-template",
            "--iterations",
            "1",
            "--json",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("bench"));
        assert_eq!(value["iterations"], json!(1));
    }

    #[test]
    fn compiles_batch_in_input_order() {
        let first = write_temp("vuec-cli-batch-first.html", "<div>{{ first }}</div>");
        let second = write_temp(
            "vuec-cli-batch-second.html",
            "<section>{{ second }}</section>",
        );
        let output = run_with_args([
            "vuec",
            "compile-batch",
            "--target",
            "vue3-template",
            "--jobs",
            "2",
            "--json",
            first.to_str().unwrap(),
            second.to_str().unwrap(),
        ])
        .expect("run");
        assert_eq!(output.code, 0);
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("compile-batch"));
        assert_eq!(value["target"], json!("vue3-template"));
        assert_eq!(value["jobs"], json!(2));
        let results = value["results"].as_array().expect("results");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["index"], json!(0));
        assert_eq!(results[1]["index"], json!(1));
        assert!(results[0]["input"]
            .as_str()
            .unwrap()
            .contains("vuec-cli-batch-first.html"));
        assert!(results[1]["input"]
            .as_str()
            .unwrap()
            .contains("vuec-cli-batch-second.html"));
        assert!(results[0]["result"]["code"]
            .as_str()
            .unwrap()
            .contains("first"));
        assert!(results[1]["result"]["code"]
            .as_str()
            .unwrap()
            .contains("second"));
    }

    #[test]
    fn compile_batch_sfc_reports_descriptor_diagnostics() {
        let source = "<template><div/></template><template><span/></template>";
        let path = write_temp("vuec-cli-batch-sfc-errors.vue", source);
        let output = run_with_args([
            "vuec",
            "compile-batch",
            "--target",
            "vue3-sfc",
            "--json",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");

        assert_eq!(output.code, 1);
        assert_eq!(value["results"][0]["status"], json!("ok"));
        assert_eq!(
            value["results"][0]["result"]["diagnostics"][0]["code"],
            json!("VUEC_SFC_PARSE")
        );
    }

    #[test]
    fn compile_batch_reports_read_errors() {
        let missing = std::env::temp_dir().join(format!(
            "vuec-cli-batch-missing-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        let output = run_with_args([
            "vuec",
            "compile-batch",
            "--target",
            "vue3-template",
            "--jobs",
            "8",
            "--json",
            missing.to_str().unwrap(),
        ])
        .expect("run");
        assert_eq!(output.code, 1);
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["jobs"], json!(1));
        assert_eq!(value["results"][0]["status"], json!("error"));
        assert!(value["results"][0]["error"]
            .as_str()
            .unwrap()
            .contains("failed to read"));
    }

    #[test]
    fn compile_template_exits_non_zero_for_error_diagnostics() {
        let path = write_temp("vuec-cli-error-diagnostic.html", r#"<div v-model="baz"/>"#);
        let output = run_with_args([
            "vuec",
            "compile-template",
            "--target",
            "vue3",
            "--json",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");

        assert_eq!(output.code, 1);
        assert_eq!(value["diagnostics"][0]["severity"], json!("error"));
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn compile_batch_exits_non_zero_for_error_diagnostics() {
        let path = write_temp(
            "vuec-cli-batch-error-diagnostic.html",
            r#"<div v-model="baz"/>"#,
        );
        let output = run_with_args([
            "vuec",
            "compile-batch",
            "--target",
            "vue3-template",
            "--json",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");

        assert_eq!(output.code, 1);
        assert_eq!(value["results"][0]["status"], json!("ok"));
        assert_eq!(
            value["results"][0]["result"]["diagnostics"][0]["severity"],
            json!("error")
        );
    }

    #[test]
    fn writes_source_map_for_vue3_template() {
        let path = write_temp("vuec-cli-map.html", "<div>{{ msg }}</div>");
        let map_path = write_temp("vuec-cli-map.json", "");
        let output = run_with_args([
            "vuec",
            "compile-template",
            "--target",
            "vue3",
            "--source-map",
            "--map-out",
            map_path.to_str().unwrap(),
            path.to_str().unwrap(),
        ])
        .expect("run");
        assert!(output.stdout.contains("function render"));
        let map = fs::read_to_string(map_path).expect("map");
        let value: Value = serde_json::from_str(&map).expect("map json");
        assert_eq!(value["version"], json!(3));
        assert!(value["sources"].as_array().unwrap()[0]
            .as_str()
            .unwrap()
            .contains("vuec-cli-map.html"));
    }

    #[test]
    fn prints_diagnostics_to_stderr_when_requested() {
        let path = write_temp("vuec-cli-diagnostic.html", r#"<div v-model="baz"/>"#);
        let output = run_with_args([
            "vuec",
            "compile-template",
            "--target",
            "vue3",
            "--diagnostics",
            path.to_str().unwrap(),
        ])
        .expect("run");
        assert_eq!(output.code, 1);
        assert!(output.stderr.contains("[error]"));
        assert!(output.stderr.contains("v-model can only be used"));
    }

    fn write_temp(name: &str, content: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "{}-{}-{}",
            name,
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        );
        path.push(unique);
        fs::write(&path, content).expect("write temp");
        path
    }
}
