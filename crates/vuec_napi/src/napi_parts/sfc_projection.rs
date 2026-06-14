fn vue3_sfc_attach_template_ast(
    descriptor_value: &mut Value,
    descriptor: &vuec_sfc::SfcDescriptor,
    parse_options: &Value,
) {
    let Some(template) = descriptor.template.as_ref() else {
        return;
    };
    if template.attrs.has_src_attr() {
        return;
    }
    let ast = vue3_sfc_template_ast_value(descriptor, template, parse_options);
    if let Some(template_value) = descriptor_value
        .get_mut("template")
        .and_then(Value::as_object_mut)
    {
        template_value.insert("ast".into(), ast);
    }
}

fn vue3_sfc_template_ast_value(
    descriptor: &vuec_sfc::SfcDescriptor,
    template: &vuec_sfc::SfcBlock,
    parse_options: &Value,
) -> Value {
    if sfc_template_is_plain_text(template) {
        return vue3_sfc_plain_template_ast_value(descriptor, template);
    }
    let null = Value::Null;
    let template_options = parse_options.get("templateParseOptions").unwrap_or(&null);
    let mut core = vue3_options(Some(template_options));
    core.prefix_identifiers = true;
    apply_napi_dom_parser_defaults(&mut core, Some(template_options));
    let default_options = DomCompilerOptions::default();
    let dom_options = DomCompilerOptions {
        core,
        transform_asset_urls: false,
        asset_url_options: default_options.asset_url_options.clone(),
        decode_entities: bool_option(
            template_options,
            "decodeEntities",
            default_options.decode_entities,
        ),
        is_custom_element: Vec::new(),
    };
    let source = TemplateSource {
        filename: descriptor.filename.clone(),
        source: template.content.clone(),
        file_id: descriptor.source_file,
        base_offset: template.content_start,
    };
    let ast = parse_dom(source, &dom_options);
    let mut value = vue3_public_parse_ast(&ast, &descriptor.source, 0, &dom_options.core);
    if let Some(object) = value.as_object_mut() {
        object.insert("source".into(), json!(descriptor.source));
        object.insert("loc".into(), vue3_loc_stub_value());
        object.remove("__vuecDiagnostics");
    }
    value
}

fn sfc_template_is_plain_text(template: &vuec_sfc::SfcBlock) -> bool {
    template
        .attrs
        .lang
        .as_deref()
        .is_some_and(|lang| !lang.is_empty() && lang != "html")
}

fn vue3_sfc_plain_template_ast_value(
    descriptor: &vuec_sfc::SfcDescriptor,
    template: &vuec_sfc::SfcBlock,
) -> Value {
    let raw_content = descriptor
        .source
        .get(template.content_start..template.content_end)
        .unwrap_or(&template.content);
    json!({
        "type": 0,
        "source": descriptor.source,
        "children": [{
            "type": 2,
            "content": raw_content,
            "loc": vue3_source_loc_value(
                &descriptor.source,
                template.content_start,
                template.content_end,
            ),
        }],
        "helpers": [],
        "components": [],
        "directives": [],
        "hoists": [],
        "imports": [],
        "cached": [],
        "temps": 0,
        "codegenNode": Value::Null,
        "loc": vue3_loc_stub_value(),
    })
}

fn vue3_public_parse_ast(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    options: &Vue3CompilerOptions,
) -> Value {
    json!({
        "type": 0,
        "source": source,
        "children": vue3_public_children(ast, ast.root, source, base_offset, options),
        "helpers": [],
        "components": [],
        "directives": [],
        "hoists": [],
        "imports": vue3_public_root_imports(ast),
        "cached": [],
        "temps": 0,
        "codegenNode": Value::Null,
        "loc": ast.root_node().map(|node| vue3_loc_value(source, base_offset, &node.span)).unwrap_or_else(vue3_loc_stub_value),
        "__vuecDiagnostics": vue3_parse_diagnostics(ast, source, base_offset, options),
    })
}

fn vue3_parse_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    options: &Vue3CompilerOptions,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    collect_html_parse_error_diagnostics(source, options, &mut diagnostics);
    collect_invalid_lt_diagnostics(ast, source, base_offset, options, &mut diagnostics);
    collect_missing_interpolation_end_diagnostics(source, options, &mut diagnostics);
    collect_invalid_end_tag_diagnostics(ast, source, base_offset, options, &mut diagnostics);
    collect_missing_directive_name_diagnostics(ast, source, base_offset, &mut diagnostics);
    diagnostics
}
