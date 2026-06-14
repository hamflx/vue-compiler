use crate::*;

pub(crate) fn apply_bridge_dom_parser_defaults(
    core: &mut Vue3CompilerOptions,
    options: Option<&Value>,
) {
    let explicit_void_tags = bridge_option_has(options, "__vuecVoidTags");
    let explicit_pre_tags = bridge_option_has(options, "__vuecPreTags");
    let explicit_ignore_newline_tags = bridge_option_has(options, "__vuecIgnoreNewlineTags");
    let explicit_native_tags = bridge_option_has(options, "__vuecNativeTags");
    let void_tags = core.void_tags.clone();
    let pre_tags = core.pre_tags.clone();
    let ignore_newline_tags = core.ignore_newline_tags.clone();
    let native_tags = core.native_tags.clone();

    vuec_vue3_dom::apply_dom_parser_defaults(core);

    if explicit_void_tags {
        core.void_tags = void_tags;
    }
    if explicit_pre_tags {
        core.pre_tags = pre_tags;
    }
    if explicit_ignore_newline_tags {
        core.ignore_newline_tags = ignore_newline_tags;
    }
    if explicit_native_tags {
        core.native_tags = native_tags;
    }
}

pub(crate) fn bridge_option_has(options: Option<&Value>, name: &str) -> bool {
    options.is_some_and(|options| options.get(name).is_some())
}

pub(crate) fn string_field(payload: &Value, name: &str) -> String {
    string_field_or(payload, name, "")
}

pub(crate) fn string_field_or(payload: &Value, name: &str, fallback: &str) -> String {
    payload
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

pub(crate) fn usize_field(payload: &Value, name: &str) -> usize {
    payload
        .get(name)
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize
}

pub(crate) fn template_source(payload: &Value) -> TemplateSource {
    let filename = template_filename(payload);
    if let Some(source) = template_source_from_ast_payload(payload, filename.clone()) {
        return source;
    }
    TemplateSource {
        filename,
        source: string_field(payload, "source"),
        file_id: FileId(0),
        base_offset: 0,
    }
}

pub(crate) fn template_filename(payload: &Value) -> String {
    payload
        .get("filename")
        .or_else(|| {
            payload
                .get("options")
                .and_then(|options| options.get("filename"))
        })
        .and_then(Value::as_str)
        .unwrap_or("anonymous.vue")
        .to_string()
}

pub(crate) fn template_source_from_ast_payload(
    payload: &Value,
    filename: String,
) -> Option<TemplateSource> {
    let ast = payload.get("ast")?;
    let children = ast.get("children").and_then(Value::as_array)?;
    let source = ast
        .get("source")
        .or_else(|| payload.get("source"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if children.is_empty() {
        return Some(TemplateSource {
            filename,
            source: String::new(),
            file_id: FileId(0),
            base_offset: 0,
        });
    }
    let mut start = usize::MAX;
    let mut end = 0usize;
    for child in children {
        if let Some((child_start, child_end)) =
            child.get("loc").and_then(|loc| loc_byte_range(source, loc))
        {
            start = start.min(child_start);
            end = end.max(child_end);
        }
    }
    if start == usize::MAX || end < start {
        return None;
    }
    Some(TemplateSource {
        filename,
        source: source.get(start..end).unwrap_or_default().to_string(),
        file_id: FileId(0),
        base_offset: start,
    })
}

pub(crate) fn template_source_from_transformed_sfc_ast_payload(
    payload: &Value,
    filename: String,
) -> Option<TemplateSource> {
    let ast = payload.get("ast")?;
    if ast.get("transformed").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let source = ast.get("source").and_then(Value::as_str)?;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename.clone(), source);
    let template = descriptor.template.as_ref()?;
    Some(TemplateSource {
        filename,
        source: template.content.clone(),
        file_id: descriptor.source_file,
        base_offset: template.content_start,
    })
}

pub(crate) fn loc_byte_range(source: &str, loc: &Value) -> Option<(usize, usize)> {
    let start = loc_offset(loc, "start")?;
    let end = loc_offset(loc, "end")?;
    if end < start {
        return None;
    }
    let loc_source = loc.get("source").and_then(Value::as_str);
    let byte_range = source.get(start..end).map(|slice| ((start, end), slice));
    if let Some(((byte_start, byte_end), slice)) = byte_range {
        if loc_source.is_none_or(|expected| expected == slice) {
            return Some((byte_start, byte_end));
        }
    }
    let utf16_range =
        utf16_offset_to_byte_index(source, start).zip(utf16_offset_to_byte_index(source, end));
    if let Some((utf16_start, utf16_end)) = utf16_range {
        if utf16_end >= utf16_start {
            if let Some(slice) = source.get(utf16_start..utf16_end) {
                if loc_source.is_none_or(|expected| expected == slice) {
                    return Some((utf16_start, utf16_end));
                }
            }
        }
    }
    byte_range
        .map(|((byte_start, byte_end), _)| (byte_start, byte_end))
        .or(utf16_range.filter(|(utf16_start, utf16_end)| utf16_end >= utf16_start))
}

pub(crate) fn loc_offset(loc: &Value, name: &str) -> Option<usize> {
    loc.get(name)?
        .get("offset")?
        .as_u64()
        .map(|offset| offset as usize)
}

pub(crate) fn utf16_offset_to_byte_index(source: &str, offset: usize) -> Option<usize> {
    let mut utf16_units = 0usize;
    for (byte_index, ch) in source.char_indices() {
        if utf16_units == offset {
            return Some(byte_index);
        }
        if utf16_units > offset {
            return None;
        }
        utf16_units += ch.len_utf16();
    }
    (utf16_units == offset).then_some(source.len())
}
