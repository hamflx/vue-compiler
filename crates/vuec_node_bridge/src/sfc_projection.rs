use crate::*;

pub(crate) fn vue27_parse_component_value(
    descriptor: &SfcDescriptor,
    errors: &[vuec_sfc::Vue27SfcParseError],
    output_source_range: bool,
) -> Value {
    let mut value = vue27_descriptor_value(descriptor);
    value["errors"] = vue27_parse_errors_value(errors, output_source_range);
    value
}

pub(crate) fn vue27_parse_errors_value(
    errors: &[vuec_sfc::Vue27SfcParseError],
    output_source_range: bool,
) -> Value {
    if output_source_range {
        json!(errors)
    } else {
        json!(errors
            .iter()
            .map(|error| error.msg.clone())
            .collect::<Vec<_>>())
    }
}

pub(crate) fn vue27_descriptor_value(descriptor: &SfcDescriptor) -> Value {
    json!({
        "source": descriptor.source,
        "filename": descriptor.filename,
        "template": descriptor.template.as_ref().map(|block| vue27_block_value(descriptor, block)),
        "script": descriptor.script.as_ref().map(|block| vue27_block_value(descriptor, block)),
        "scriptSetup": descriptor.script_setup.as_ref().map(|block| vue27_block_value(descriptor, block)),
        "styles": descriptor.styles.iter().map(|block| vue27_style_block_value(descriptor, block)).collect::<Vec<_>>(),
        "customBlocks": descriptor.custom_blocks.iter().map(|block| vue27_block_value(descriptor, block)).collect::<Vec<_>>(),
        "cssVars": vue27_css_vars(descriptor),
        "errors": [],
        "shouldForceReload": null,
    })
}

pub(crate) fn vue27_block_value(descriptor: &SfcDescriptor, block: &SfcBlock) -> Value {
    let mut value = json!({
        "type": block.type_name,
        "content": block.content,
        "start": block.content_start,
        "end": block.content_end,
        "attrs": vue27_attrs_value(&block.attrs),
    });
    if matches!(block.type_name.as_str(), "script" | "style") {
        value["map"] = vue27_block_map(descriptor);
    }
    if block.attrs.setup {
        value["setup"] = json!(true);
    }
    if let Some(lang) = block.attrs.lang.as_ref() {
        value["lang"] = json!(lang);
    }
    if let Some(src) = block.attrs.src.as_ref() {
        value["src"] = json!(src);
    }
    if let Some(module) = block.attrs.module.as_ref() {
        if module.is_empty() {
            value["module"] = json!(true);
        } else {
            value["module"] = json!(module);
        }
    }
    value
}

pub(crate) fn vue27_style_block_value(descriptor: &SfcDescriptor, block: &SfcBlock) -> Value {
    let mut value = vue27_block_value(descriptor, block);
    if block.attrs.scoped {
        value["scoped"] = json!(true);
    }
    value
}

pub(crate) fn vue27_block_map(descriptor: &SfcDescriptor) -> Value {
    json!({
        "version": 3,
        "sources": [descriptor.filename],
        "names": [],
        "mappings": "AAAA",
        "file": descriptor.filename,
        "sourceRoot": "",
        "sourcesContent": [descriptor.source],
    })
}

pub(crate) fn vue27_attrs_value(attrs: &SfcBlockAttrs) -> Value {
    let mut object = serde_json::Map::new();
    for (name, value) in &attrs.raw {
        object.insert(
            name.clone(),
            match value {
                SfcAttrValue::Bool(value) => json!(value),
                SfcAttrValue::String(value) => json!(value),
            },
        );
    }
    if attrs.scoped {
        object.insert("scoped".into(), json!(true));
    }
    if attrs.setup {
        object.insert("setup".into(), json!(true));
    }
    if let Some(lang) = attrs.lang.as_ref() {
        object.insert("lang".into(), json!(lang));
    }
    if let Some(src) = attrs.src.as_ref() {
        object.insert("src".into(), json!(src));
    }
    if let Some(module) = attrs.module.as_ref() {
        if module.is_empty() {
            object.insert("module".into(), json!(true));
        } else {
            object.insert("module".into(), json!(module));
        }
    }
    Value::Object(object)
}

pub(crate) fn vue27_css_vars(descriptor: &SfcDescriptor) -> Vec<String> {
    let mut vars = Vec::new();
    for style in &descriptor.styles {
        for var in vuec_style::collect_css_vars(&style.content) {
            if !vars.iter().any(|existing| existing == &var) {
                vars.push(var);
            }
        }
    }
    vars
}

pub(crate) fn vue3_sfc_parse_options(value: Option<&Value>) -> Vue3SfcParseOptions {
    let mut options = Vue3SfcParseOptions::default();
    let Some(value) = value else {
        return options;
    };
    options.ignore_empty = bool_option(value, "ignoreEmpty", options.ignore_empty);
    options.pad = vue3_sfc_pad_option(value.get("pad"));
    options
}

pub(crate) fn vue3_sfc_parse_projection_options(
    value: Option<&Value>,
    parse_options: &Vue3SfcParseOptions,
) -> Vue3SfcParseProjectionOptions {
    match value {
        Some(value) => Vue3SfcParseProjectionOptions {
            pad: parse_options.pad.clone(),
            source_map: bool_option(value, "sourceMap", true),
            source_root: value
                .get("sourceRoot")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        },
        None => Vue3SfcParseProjectionOptions {
            pad: parse_options.pad.clone(),
            ..Vue3SfcParseProjectionOptions::default()
        },
    }
}

pub(crate) fn vue3_sfc_pad_option(value: Option<&Value>) -> Vue3SfcPad {
    match value {
        Some(Value::Bool(true)) => Vue3SfcPad::Line,
        Some(Value::String(value)) if value == "line" => Vue3SfcPad::Line,
        Some(Value::String(value)) if value == "space" => Vue3SfcPad::Space,
        _ => Vue3SfcPad::False,
    }
}

pub(crate) fn vue3_sfc_attach_template_ast(
    result: &mut Value,
    descriptor: &SfcDescriptor,
    parse_options: Option<&Value>,
) {
    let Some(template) = descriptor.template.as_ref() else {
        return;
    };
    if template.attrs.has_src_attr() {
        return;
    }
    let ast = vue3_sfc_template_ast_value(descriptor, template, parse_options);
    if let Some(template_value) = result
        .get_mut("descriptor")
        .and_then(|descriptor| descriptor.get_mut("template"))
        .and_then(Value::as_object_mut)
    {
        template_value.insert("ast".into(), ast);
    }
}

pub(crate) fn vue3_sfc_template_ast_value(
    descriptor: &SfcDescriptor,
    template: &SfcBlock,
    parse_options: Option<&Value>,
) -> Value {
    if sfc_template_is_plain_text(template) {
        return vue3_sfc_plain_template_ast_value(descriptor, template);
    }
    let null = Value::Null;
    let template_options = parse_options
        .and_then(|options| options.get("templateParseOptions"))
        .unwrap_or(&null);
    let mut core = vue3_options(Some(template_options));
    core.prefix_identifiers = true;
    apply_bridge_dom_parser_defaults(&mut core, Some(template_options));
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
    let ast = vuec_vue3_dom::parse(source, &dom_options);
    let mut value = vue3_parse_value(&ast, &descriptor.source, 0, false, &dom_options.core, false);
    if let Some(object) = value.as_object_mut() {
        object.insert("source".into(), json!(descriptor.source));
        object.insert("loc".into(), vue3_loc_stub_value());
        object.remove("__vuecDiagnostics");
    }
    value
}

pub(crate) fn sfc_template_is_plain_text(template: &SfcBlock) -> bool {
    template
        .attrs
        .lang
        .as_deref()
        .is_some_and(|lang| !lang.is_empty() && lang != "html")
}

pub(crate) fn vue3_sfc_plain_template_ast_value(
    descriptor: &SfcDescriptor,
    template: &SfcBlock,
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

pub(crate) fn is_simple_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

pub(crate) fn vue27_template_ast_value(compiled: &vuec_vue2::Vue2CompiledResult) -> Value {
    match compiled.element_ast.as_ref() {
        Some(element) => vue27_element_ast_value(element),
        None => Value::Null,
    }
}

pub(crate) fn vue27_element_ast_value(element: &vuec_vue2::Vue2Element) -> Value {
    json!({
        "type": 1,
        "tag": element.tag,
        "attrsList": element.attrs_list,
        "attrsMap": element.attrs_map,
        "rawAttrsMap": element.raw_attrs_map,
        "children": element.children.iter().map(vue27_node_ast_value).collect::<Vec<_>>(),
        "plain": element.plain,
        "static": element.static_node,
        "staticRoot": element.static_root,
    })
}

pub(crate) fn vue27_node_ast_value(node: &vuec_vue2::Vue2Node) -> Value {
    match node {
        vuec_vue2::Vue2Node::Element(element) => vue27_element_ast_value(element),
        vuec_vue2::Vue2Node::Text(text) if text.expression.is_some() => json!({
            "type": 2,
            "expression": text.expression,
            "tokens": [{"@binding": vue27_binding_from_expression(text.expression.as_deref().unwrap_or_default())}],
            "text": text.text,
            "static": text.static_node,
        }),
        vuec_vue2::Vue2Node::Text(text) => json!({
            "type": if text.is_comment { 3 } else { 2 },
            "text": text.text,
            "static": text.static_node,
        }),
    }
}

pub(crate) fn vue27_binding_from_expression(expression: &str) -> String {
    expression
        .strip_prefix("_s(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(expression)
        .to_string()
}

pub(crate) fn vue27_script_value(script: &SfcScriptBlock) -> Value {
    let mut value = serde_json::to_value(script).expect("script block is serializable");
    if let Some(object) = value.as_object_mut() {
        object.remove("errors");
        object.remove("deps");
        object.insert("content".into(), json!(script.content.clone()));
        object.insert(
            "start".into(),
            json!(script
                .loc
                .as_ref()
                .map(block_content_start_from_loc)
                .unwrap_or(0)),
        );
        object.insert(
            "end".into(),
            json!(script
                .loc
                .as_ref()
                .map(block_content_end_from_loc)
                .unwrap_or(0)),
        );
        object["bindings"] = json!(script.bindings);
        object["imports"] = json!({});
    }
    value
}

pub(crate) fn block_content_start_from_loc(loc: &vuec_sfc::SfcBlockLocation) -> usize {
    loc.start
}

pub(crate) fn block_content_end_from_loc(loc: &vuec_sfc::SfcBlockLocation) -> usize {
    loc.end
}
