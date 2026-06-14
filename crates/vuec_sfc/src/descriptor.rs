use crate::*;

#[derive(Clone, Debug)]
pub(crate) struct ExtractedSfcBlocks {
    pub(crate) blocks: Vec<SfcBlock>,
    pub(crate) vue3_errors: Vec<Vue3SfcParseError>,
    pub(crate) errors: Vec<Vue27SfcParseError>,
}

#[derive(Clone, Copy)]
pub(crate) enum SfcBlockContentMode<'a> {
    Vue3 {
        options: &'a Vue3SfcParseOptions,
    },
    Vue27 {
        options: &'a Vue27ParseComponentOptions,
    },
}

impl SfcBlockContentMode<'_> {
    pub(crate) fn is_vue3(&self) -> bool {
        matches!(self, SfcBlockContentMode::Vue3 { .. })
    }

    pub(crate) fn decodes_attr_entities(&self) -> bool {
        matches!(self, SfcBlockContentMode::Vue3 { .. })
    }

    pub(crate) fn is_void_tag(&self, name: &str) -> bool {
        self.is_vue3() && vue3_sfc_dom_void_tag(name)
    }
}

pub(crate) struct OpenSfcBlock {
    pub(crate) type_name: String,
    pub(crate) attrs: SfcBlockAttrs,
    pub(crate) start: usize,
    pub(crate) open_end: usize,
    pub(crate) self_closing: bool,
}

pub(crate) fn vue3_descriptor_from_blocks(
    filename: String,
    source: &str,
    source_file: FileId,
    blocks: Vec<SfcBlock>,
    options: &Vue3SfcParseOptions,
) -> Vue3SfcParseResult {
    let mut descriptor = SfcDescriptor {
        filename,
        source: source.to_string(),
        source_file,
        template: None,
        script: None,
        script_setup: None,
        styles: Vec::new(),
        custom_blocks: Vec::new(),
    };
    let mut errors = Vec::new();
    let mut has_template_or_script_candidate = false;
    let mut has_script_setup_candidate = false;

    for block in blocks {
        errors.extend(block.attrs.duplicate_attr_errors(source_file));
        if options.ignore_empty
            && block.type_name != "template"
            && !block.attrs.has_src_attr()
            && block.content.trim().is_empty()
            && !block.preserve_empty
        {
            continue;
        }
        match block.type_name.as_str() {
            "template" => {
                has_template_or_script_candidate = true;
                if descriptor.template.is_some() {
                    errors.push(vue3_sfc_parse_block_error(
                        "Single file component can contain only one <template> element",
                        &block,
                    ));
                } else {
                    if let Some(error) = vue3_sfc_functional_template_error(&block) {
                        errors.push(error);
                    }
                    descriptor.template = Some(block);
                }
            }
            "script" => {
                has_template_or_script_candidate = true;
                if block.attrs.setup {
                    if descriptor.script_setup.is_some() {
                        errors.push(vue3_sfc_parse_block_error(
                            "Single file component can contain only one <script setup> element",
                            &block,
                        ));
                    } else {
                        has_script_setup_candidate = true;
                        descriptor.script_setup = Some(block);
                    }
                } else if descriptor.script.is_some() {
                    errors.push(vue3_sfc_parse_block_error(
                        "Single file component can contain only one <script> element",
                        &block,
                    ));
                } else {
                    descriptor.script = Some(block);
                }
            }
            "style" => descriptor.styles.push(block),
            _ => descriptor.custom_blocks.push(block),
        }
    }

    if descriptor
        .script_setup
        .as_ref()
        .is_some_and(|script_setup| script_setup.attrs.has_non_empty_src())
    {
        errors.push(vue3_sfc_parse_error(
            "<script setup> cannot use the \"src\" attribute because its syntax will be ambiguous outside of the component.",
        ));
        descriptor.script_setup = None;
    }
    if has_script_setup_candidate
        && descriptor
            .script
            .as_ref()
            .is_some_and(|script| script.attrs.has_non_empty_src())
    {
        errors.push(vue3_sfc_parse_error(
            "<script> cannot use the \"src\" attribute when <script setup> is also present because they must be processed together.",
        ));
        descriptor.script = None;
    }
    if !has_template_or_script_candidate {
        errors.push(vue3_sfc_parse_error(format!(
            "At least one <template> or <script> is required in a single file component. {}",
            descriptor.filename
        )));
    }
    vue3_dedent_pug_template(&mut descriptor);

    Vue3SfcParseResult { descriptor, errors }
}

pub(crate) fn vue3_sfc_parse_error(message: impl Into<String>) -> Vue3SfcParseError {
    Vue3SfcParseError {
        message: message.into(),
        loc: None,
    }
}

pub(crate) fn vue3_sfc_parse_syntax_error(
    message: impl Into<String>,
    offset: usize,
    source_file: FileId,
) -> Vue3SfcParseError {
    Vue3SfcParseError {
        message: message.into(),
        loc: Some(SfcBlockLocation {
            start: offset,
            end: offset,
            source_file,
        }),
    }
}

pub(crate) fn vue3_sfc_missing_end_tag_error(
    start: usize,
    source_file: FileId,
) -> Vue3SfcParseError {
    vue3_sfc_parse_syntax_error("Element is missing end tag.", start, source_file)
}

pub(crate) fn vue3_sfc_invalid_end_tag_error(
    start: usize,
    source_file: FileId,
) -> Vue3SfcParseError {
    vue3_sfc_parse_syntax_error("Invalid end tag.", start, source_file)
}

pub(crate) fn vue3_sfc_cdata_error(start: usize, source_file: FileId) -> Vue3SfcParseError {
    vue3_sfc_parse_syntax_error(
        "CDATA section is allowed only in XML context.",
        start,
        source_file,
    )
}

pub(crate) fn vue3_sfc_parse_block_error(
    message: impl Into<String>,
    block: &SfcBlock,
) -> Vue3SfcParseError {
    Vue3SfcParseError {
        message: message.into(),
        loc: Some(block.loc.clone()),
    }
}

pub(crate) fn vue3_sfc_functional_template_error(block: &SfcBlock) -> Option<Vue3SfcParseError> {
    if !block.attrs.raw.contains_key("functional") {
        return None;
    }
    Some(Vue3SfcParseError {
        message: "<template functional> is no longer supported in Vue 3, since functional components no longer have significant performance difference from stateful ones. Just use a normal <template> instead.".into(),
        loc: block
            .attrs
            .attr_location("functional", block.loc.source_file),
    })
}

pub(crate) fn vue3_dedent_pug_template(descriptor: &mut SfcDescriptor) {
    let Some(template) = descriptor.template.as_mut() else {
        return;
    };
    if !matches!(template.attrs.lang.as_deref(), Some("pug" | "jade")) {
        return;
    }
    let (content, column_offset) = vue3_dedent_template_content(&template.content);
    template.content = content;
    template.source_map_column_offset = column_offset;
}

pub(crate) fn vue3_dedent_template_content(source: &str) -> (String, usize) {
    let lines = source.split('\n').collect::<Vec<_>>();
    let mut min_indent = usize::MAX;
    for line in &lines {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
        min_indent = min_indent.min(indent);
    }
    if min_indent == usize::MAX || min_indent == 0 {
        return (source.to_string(), 0);
    }
    (
        lines
            .iter()
            .map(|line| strip_chars(line, min_indent))
            .collect::<Vec<_>>()
            .join("\n"),
        min_indent,
    )
}

pub(crate) fn descriptor_from_blocks(
    filename: String,
    source: &str,
    source_file: FileId,
    blocks: Vec<SfcBlock>,
) -> SfcDescriptor {
    let mut descriptor = SfcDescriptor {
        filename,
        source: source.to_string(),
        source_file,
        template: None,
        script: None,
        script_setup: None,
        styles: Vec::new(),
        custom_blocks: Vec::new(),
    };

    for block in blocks {
        match block.type_name.as_str() {
            "template" => descriptor.template = Some(block),
            "script" => {
                if block.attrs.setup {
                    descriptor.script_setup = Some(block);
                } else {
                    descriptor.script = Some(block);
                }
            }
            "style" => descriptor.styles.push(block),
            _ => descriptor.custom_blocks.push(block),
        }
    }

    descriptor
}

/// Projects a Rust SFC descriptor into the Vue 3 public `parse()` result shape.
pub fn vue3_sfc_parse_result_value(
    result: &Vue3SfcParseResult,
    options: &Vue3SfcParseProjectionOptions,
) -> serde_json::Value {
    json!({
        "descriptor": vue3_sfc_descriptor_value(&result.descriptor, options),
        "errors": result.errors.iter().map(|error| vue3_sfc_parse_error_value(&result.descriptor, error)).collect::<Vec<_>>(),
    })
}

/// Projects a Rust SFC descriptor into the Vue 3 public descriptor shape.
pub fn vue3_sfc_descriptor_value(
    descriptor: &SfcDescriptor,
    options: &Vue3SfcParseProjectionOptions,
) -> serde_json::Value {
    json!({
        "filename": descriptor.filename,
        "source": descriptor.source,
        "template": descriptor.template.as_ref().map(|block| vue3_sfc_block_value(descriptor, block, options, true)),
        "script": descriptor.script.as_ref().map(|block| vue3_sfc_block_value(descriptor, block, options, true)),
        "scriptSetup": descriptor.script_setup.as_ref().map(|block| vue3_sfc_block_value(descriptor, block, options, false)),
        "styles": descriptor.styles.iter().map(|block| vue3_sfc_block_value(descriptor, block, options, true)).collect::<Vec<_>>(),
        "customBlocks": descriptor.custom_blocks.iter().map(|block| vue3_sfc_block_value(descriptor, block, options, true)).collect::<Vec<_>>(),
        "cssVars": descriptor_css_vars(descriptor, CssVarCollectOptions::default()),
        "slotted": vue3_sfc_descriptor_has_slotted_styles(descriptor),
        "shouldForceReload": serde_json::Value::Null,
    })
}

pub(crate) fn vue3_sfc_block_value(
    descriptor: &SfcDescriptor,
    block: &SfcBlock,
    options: &Vue3SfcParseProjectionOptions,
    include_map: bool,
) -> serde_json::Value {
    let mut value = serde_json::Map::new();
    value.insert("type".into(), json!(block.type_name));
    value.insert("content".into(), json!(block.content));
    value.insert("loc".into(), vue3_sfc_block_loc_value(descriptor, block));
    value.insert("attrs".into(), vue3_sfc_attrs_value(&block.attrs));

    if block.type_name == "script" && block.attrs.setup {
        let setup = block
            .attrs
            .raw
            .get("setup")
            .unwrap_or(&SfcAttrValue::Bool(true));
        value.insert("setup".into(), vue3_sfc_attr_value(setup));
    }
    if let Some(lang) = block.attrs.lang.as_ref() {
        value.insert("lang".into(), json!(lang));
    }
    if let Some(src) = block.attrs.src.as_ref() {
        value.insert("src".into(), json!(src));
    }
    if block.type_name == "style" && block.attrs.scoped {
        value.insert("scoped".into(), json!(true));
    }
    if block.type_name == "style" {
        if let Some(module) = block.attrs.raw.get("module") {
            value.insert("module".into(), vue3_sfc_attr_value(module));
        } else if let Some(module) = block.attrs.module.as_ref() {
            value.insert(
                "module".into(),
                if module.is_empty() {
                    json!(true)
                } else {
                    json!(module)
                },
            );
        }
    }
    if options.source_map && include_map && !block.attrs.has_src_attr() {
        value.insert(
            "map".into(),
            vue3_sfc_block_map_value(descriptor, block, options),
        );
    }

    serde_json::Value::Object(value)
}

pub(crate) fn vue3_sfc_attrs_value(attrs: &SfcBlockAttrs) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for (name, value) in &attrs.raw {
        object.insert(name.clone(), vue3_sfc_attr_value(value));
    }
    serde_json::Value::Object(object)
}

pub(crate) fn vue3_sfc_attr_value(value: &SfcAttrValue) -> serde_json::Value {
    match value {
        SfcAttrValue::Bool(value) => json!(value),
        SfcAttrValue::String(value) if value.is_empty() => json!(true),
        SfcAttrValue::String(value) => json!(value),
    }
}

pub(crate) fn vue3_sfc_block_loc_value(
    descriptor: &SfcDescriptor,
    block: &SfcBlock,
) -> serde_json::Value {
    let start = block.content_start.min(descriptor.source.len());
    let end = block.content_end.min(descriptor.source.len()).max(start);
    json!({
        "start": vue3_sfc_position_value(&descriptor.source, start),
        "end": vue3_sfc_position_value(&descriptor.source, end),
        "source": descriptor.source.get(start..end).unwrap_or(&block.content),
    })
}

pub(crate) fn vue3_sfc_position_value(source: &str, offset: usize) -> serde_json::Value {
    let mut line = 1usize;
    let mut column = 1usize;
    let mut byte_index = 0usize;
    let mut utf16_offset = 0usize;
    for ch in source.chars() {
        if byte_index >= offset {
            break;
        }
        byte_index += ch.len_utf8();
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += ch.len_utf16();
        }
        utf16_offset += ch.len_utf16();
    }
    if offset > byte_index {
        let extra = offset - byte_index;
        column += extra;
        utf16_offset += extra;
    }
    json!({
        "column": column,
        "line": line,
        "offset": utf16_offset,
    })
}

pub(crate) fn vue3_sfc_block_map_value(
    descriptor: &SfcDescriptor,
    block: &SfcBlock,
    options: &Vue3SfcParseProjectionOptions,
) -> serde_json::Value {
    let filename = descriptor.filename.replace('\\', "/");
    let mut builder = SourceMapBuilder::new().file(filename.clone());
    builder.add_source_content(filename.clone(), descriptor.source.clone());
    let block_start = vue3_sfc_position_value(&descriptor.source, block.content_start);
    let line_offset = if !options.pad.is_enabled() || block.type_name == "template" {
        block_start
            .get("line")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1)
            .saturating_sub(1) as usize
    } else {
        0
    };
    for (line_index, line) in block.content.split('\n').enumerate() {
        if vue3_sfc_source_map_line_is_empty(line) {
            continue;
        }
        let original_line = line_index + 1 + line_offset;
        let mut generated_column = 0usize;
        for ch in line.chars() {
            if !ch.is_whitespace() {
                let original_column = generated_column + block.source_map_column_offset;
                if let Some(absolute) = byte_offset_at_utf16_line_column(
                    &descriptor.source,
                    original_line,
                    original_column,
                ) {
                    builder.add_mapping(
                        line_index + 1,
                        generated_column,
                        Some(Span::new(descriptor.source_file, absolute, absolute)),
                        Some(filename.clone()),
                    );
                }
            }
            generated_column += ch.len_utf16();
        }
    }
    let mut value = serde_json::to_value(builder.build()).unwrap_or_else(|_| {
        json!({
            "version": 3,
            "sources": [filename],
            "names": [],
            "mappings": "",
            "file": descriptor.filename.replace('\\', "/"),
            "sourcesContent": [descriptor.source],
        })
    });
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "sourceRoot".into(),
            json!(options.source_root.replace('\\', "/")),
        );
    }
    value
}

pub(crate) fn vue3_sfc_source_map_line_is_empty(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed == "//"
}

pub(crate) fn byte_offset_at_utf16_line_column(
    source: &str,
    line: usize,
    column: usize,
) -> Option<usize> {
    if line == 0 {
        return None;
    }
    let mut current_line = 1usize;
    let mut line_start = 0usize;
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while current_line < line && index < source.len() {
        match bytes[index] {
            b'\r' => {
                if index + 1 < source.len() && bytes[index + 1] == b'\n' {
                    index += 2;
                } else {
                    index += 1;
                }
                current_line += 1;
                line_start = index;
            }
            b'\n' => {
                index += 1;
                current_line += 1;
                line_start = index;
            }
            _ => index += 1,
        }
    }
    if current_line != line {
        return None;
    }
    let line_end = source[line_start..]
        .find(['\r', '\n'])
        .map(|offset| line_start + offset)
        .unwrap_or(source.len());
    let mut current_column = 0usize;
    let mut cursor = line_start;
    while cursor <= line_end {
        if current_column == column {
            return Some(cursor);
        }
        if cursor == line_end {
            break;
        }
        let ch = source[cursor..line_end].chars().next()?;
        current_column += ch.len_utf16();
        cursor += ch.len_utf8();
        if current_column > column {
            return None;
        }
    }
    (current_column == column).then_some(cursor)
}

pub(crate) fn vue3_sfc_descriptor_has_slotted_styles(descriptor: &SfcDescriptor) -> bool {
    descriptor.styles.iter().any(|style| {
        style.attrs.scoped
            && (style.content.contains(":slotted(") || style.content.contains("::v-slotted("))
    })
}

pub(crate) fn vue3_sfc_parse_error_value(
    descriptor: &SfcDescriptor,
    error: &Vue3SfcParseError,
) -> serde_json::Value {
    let mut value = serde_json::Map::new();
    value.insert("message".into(), json!(error.message));
    if let Some(loc) = error.loc.as_ref() {
        let start = loc.start.min(descriptor.source.len());
        let end = if loc.end == 0 {
            start
        } else {
            loc.end.min(descriptor.source.len()).max(start)
        };
        value.insert(
            "loc".into(),
            json!({
                "start": vue3_sfc_position_value(&descriptor.source, start),
                "end": vue3_sfc_position_value(&descriptor.source, end),
                "source": descriptor.source.get(start..end).unwrap_or_default(),
            }),
        );
    }
    serde_json::Value::Object(value)
}

pub(crate) fn project_vue27_errors(
    errors: Vec<Vue27SfcParseError>,
    output_source_range: bool,
) -> Vec<Vue27SfcParseError> {
    if output_source_range {
        return errors;
    }
    errors
        .into_iter()
        .map(|error| Vue27SfcParseError {
            msg: error.msg,
            start: None,
            end: None,
        })
        .collect()
}

pub(crate) fn extract_sfc_blocks(
    source: &str,
    source_file: FileId,
    mode: SfcBlockContentMode<'_>,
) -> ExtractedSfcBlocks {
    let mut blocks = Vec::new();
    let mut vue3_errors = Vec::new();
    let mut errors = Vec::new();
    let mut stack: Vec<(String, usize, usize)> = Vec::new();
    let mut current_block: Option<OpenSfcBlock> = None;
    let mut depth = 0usize;
    let mut malformed_tail_start = None;
    let mut vue3_terminal_root_cdata_start = None;
    let mut vue3_terminal_root_invalid_end_start = None;
    let mut tokenizer = HtmlTokenizer::new(source);

    loop {
        let token = tokenizer.next_token();
        match token.kind {
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let is_vue3_template_content = current_block
                    .as_ref()
                    .is_some_and(|block| block.type_name == "template");
                if mode.is_vue3() && (depth == 0 || is_vue3_template_content) {
                    vue3_collect_sfc_attr_syntax_errors(
                        &attributes,
                        source_file,
                        depth > 0 && is_vue3_template_content,
                        &mut vue3_errors,
                    );
                    vue3_terminal_root_cdata_start = None;
                    vue3_terminal_root_invalid_end_start = None;
                }
                if depth == 0 {
                    current_block = Some(OpenSfcBlock {
                        type_name: name.clone(),
                        attrs: attrs_from_html(&attributes, mode.decodes_attr_entities()),
                        start: token.start,
                        open_end: token.end,
                        self_closing,
                    });
                }

                if !self_closing && !mode.is_void_tag(&name) {
                    if depth == 0 && is_plain_text_sfc_tag(&name) {
                        consume_plain_text_element(
                            source,
                            source_file,
                            mode,
                            &mut tokenizer,
                            &mut blocks,
                            &mut vue3_errors,
                            &mut current_block,
                            token.end,
                        );
                        depth = 0;
                    } else {
                        stack.push((name, token.start, token.end));
                        depth += 1;
                    }
                } else if depth == 0 {
                    if let Some(open) = current_block.take() {
                        blocks.push(finish_sfc_block(
                            source,
                            source_file,
                            mode,
                            open,
                            0,
                            token.end,
                            false,
                        ));
                    }
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if depth == 0 {
                    if mode.is_vue3() && !name.is_empty() {
                        vue3_errors.push(vue3_sfc_invalid_end_tag_error(token.start, source_file));
                    }
                    continue;
                }
                let Some(pos) = matching_open_pos(&stack, &name, mode.is_vue3()) else {
                    if name.is_empty() {
                        malformed_tail_start.get_or_insert(token.start);
                    } else if name.eq_ignore_ascii_case("br") && depth == 0 {
                        current_block = Some(OpenSfcBlock {
                            type_name: name,
                            attrs: SfcBlockAttrs::default(),
                            start: token.start,
                            open_end: token.end,
                            self_closing: true,
                        });
                    } else if mode.is_vue3()
                        && current_block
                            .as_ref()
                            .is_some_and(|block| block.type_name == "template")
                    {
                        vue3_errors.push(vue3_sfc_invalid_end_tag_error(token.start, source_file));
                        if depth == 1 {
                            vue3_terminal_root_invalid_end_start = Some(token.start);
                            vue3_terminal_root_cdata_start = None;
                        }
                    }
                    continue;
                };
                let mut emitted_vue3_missing_child = false;
                while stack.len() > pos + 1 {
                    if let Some((tag, start, end)) = stack.pop() {
                        if mode.is_vue3()
                            && current_block
                                .as_ref()
                                .is_some_and(|block| block.type_name == "template")
                            && !emitted_vue3_missing_child
                        {
                            vue3_errors.push(vue3_sfc_missing_end_tag_error(start, source_file));
                            emitted_vue3_missing_child = true;
                        }
                        errors.push(Vue27SfcParseError {
                            msg: format!("tag <{tag}> has no matching end tag."),
                            start: Some(start),
                            end: Some(end),
                        });
                        depth = depth.saturating_sub(1);
                    }
                }
                stack.pop();
                if depth == 1 {
                    if let Some(open) = current_block.take() {
                        let content_end =
                            if mode.is_vue3() && open.type_name == "template" && pos == 0 {
                                vue3_terminal_root_cdata_start
                                    .take()
                                    .or_else(|| vue3_terminal_root_invalid_end_start.take())
                                    .unwrap_or(token.start)
                            } else {
                                token.start
                            };
                        blocks.push(finish_sfc_block(
                            source,
                            source_file,
                            mode,
                            open,
                            content_end,
                            token.end,
                            false,
                        ));
                    }
                }
                depth = depth.saturating_sub(1);
            }
            HtmlTokenKind::BogusQuestionTag => {
                if mode.is_vue3()
                    && current_block
                        .as_ref()
                        .is_none_or(|block| block.type_name == "template")
                {
                    vue3_errors.push(vue3_sfc_parse_syntax_error(
                        "'<?' is allowed only in XML context.",
                        token.start.saturating_add(1),
                        source_file,
                    ));
                }
            }
            HtmlTokenKind::Cdata(_) => {
                if mode.is_vue3()
                    && current_block
                        .as_ref()
                        .is_some_and(|block| block.type_name == "template")
                {
                    vue3_errors.push(vue3_sfc_cdata_error(token.start, source_file));
                    if depth == 1 {
                        vue3_terminal_root_cdata_start = Some(token.start);
                        vue3_terminal_root_invalid_end_start = None;
                    }
                }
            }
            HtmlTokenKind::Eof => {
                let is_vue3_template = mode.is_vue3()
                    && current_block
                        .as_ref()
                        .is_some_and(|block| block.type_name == "template");
                let is_vue3 = mode.is_vue3();
                while let Some((tag, start, end)) = stack.pop() {
                    if is_vue3 && (is_vue3_template || stack.is_empty()) {
                        vue3_errors.push(vue3_sfc_missing_end_tag_error(start, source_file));
                    }
                    errors.push(Vue27SfcParseError {
                        msg: format!("tag <{tag}> has no matching end tag."),
                        start: Some(start),
                        end: Some(end),
                    });
                    if stack.is_empty() {
                        if let Some(open) = current_block.take() {
                            let fallback_end = if mode.is_vue3() {
                                open.open_end
                            } else {
                                malformed_tail_start.unwrap_or_else(|| {
                                    malformed_tail_content_end(source, &open, token.start)
                                })
                            };
                            blocks.push(finish_sfc_block(
                                source,
                                source_file,
                                mode,
                                open,
                                fallback_end,
                                token.end,
                                mode.is_vue3(),
                            ));
                        }
                    }
                }
                break;
            }
            HtmlTokenKind::Text(_) | HtmlTokenKind::Comment(_) | HtmlTokenKind::Doctype(_) => {
                vue3_terminal_root_cdata_start = None;
                vue3_terminal_root_invalid_end_start = None;
            }
        }
    }

    blocks.sort_by_key(|block| block.loc.start);
    ExtractedSfcBlocks {
        blocks,
        vue3_errors,
        errors,
    }
}

pub(crate) fn consume_plain_text_element(
    source: &str,
    source_file: FileId,
    mode: SfcBlockContentMode<'_>,
    tokenizer: &mut HtmlTokenizer<'_>,
    blocks: &mut Vec<SfcBlock>,
    vue3_errors: &mut Vec<Vue3SfcParseError>,
    current_block: &mut Option<OpenSfcBlock>,
    content_start: usize,
) {
    let Some(open) = current_block.take() else {
        return;
    };
    let lower_name = open.type_name.to_ascii_lowercase();
    let rest = &source[content_start..];
    let needle = format!("</{lower_name}");
    if let Some(close_offset) = find_ascii_case_insensitive(rest, &needle) {
        let close_start = content_start + close_offset;
        let close_end = source[close_start..]
            .find('>')
            .map(|offset| close_start + offset + 1)
            .unwrap_or(source.len());
        tokenizer.set_cursor(close_end);
        blocks.push(finish_sfc_block(
            source,
            source_file,
            mode,
            open,
            close_start,
            close_end,
            false,
        ));
    } else {
        tokenizer.set_cursor(source.len());
        let content_end = if mode.is_vue3() {
            vue3_errors.push(vue3_sfc_missing_end_tag_error(open.start, source_file));
            open.open_end
        } else {
            source.len()
        };
        blocks.push(finish_sfc_block(
            source,
            source_file,
            mode,
            open,
            content_end,
            source.len(),
            mode.is_vue3(),
        ));
    }
}

pub(crate) fn finish_sfc_block(
    source: &str,
    source_file: FileId,
    mode: SfcBlockContentMode<'_>,
    open: OpenSfcBlock,
    content_end: usize,
    close_end: usize,
    preserve_empty: bool,
) -> SfcBlock {
    let content_start = open.open_end.min(source.len());
    let raw_end = content_end.min(source.len()).max(content_start);
    let mut content = source[content_start..raw_end].to_string();
    match mode {
        SfcBlockContentMode::Vue3 { options } => {
            if open.type_name != "template" && options.pad.is_enabled() {
                content = vue3_pad_content(source, &open, &options.pad) + &content;
            }
        }
        SfcBlockContentMode::Vue27 { options } => {
            if should_vue27_deindent(&open, options) {
                content = deindent(&content);
            }
            if open.type_name != "template" && options.pad.is_enabled() {
                content = vue27_pad_content(source, &open, &options.pad) + &content;
            }
        }
    }

    SfcBlock {
        type_name: open.type_name,
        content,
        attrs: open.attrs,
        loc: SfcBlockLocation {
            start: open.start,
            end: if open.self_closing { 0 } else { close_end },
            source_file,
        },
        content_start,
        content_end: raw_end,
        source_map_column_offset: 0,
        preserve_empty,
    }
}

pub(crate) fn matching_open_pos(
    stack: &[(String, usize, usize)],
    name: &str,
    vue3_sfc_mode: bool,
) -> Option<usize> {
    let lower_name = name.to_ascii_lowercase();
    stack.iter().enumerate().rposition(|(index, (tag, _, _))| {
        if vue3_sfc_mode && index == 0 && has_ascii_uppercase(tag) {
            return false;
        }
        tag.to_ascii_lowercase() == lower_name
    })
}

pub(crate) fn has_ascii_uppercase(source: &str) -> bool {
    source.bytes().any(|byte| byte.is_ascii_uppercase())
}

pub(crate) fn malformed_tail_content_end(
    source: &str,
    open: &OpenSfcBlock,
    fallback: usize,
) -> usize {
    let fallback = fallback.min(source.len());
    let tail = &source[open.open_end.min(source.len())..fallback];
    let Some(last_lt) = tail.rfind('<') else {
        return fallback;
    };
    let absolute = open.open_end + last_lt;
    if source[absolute..fallback].contains('>') {
        return fallback;
    }
    absolute
}

pub(crate) fn vue3_collect_sfc_attr_syntax_errors(
    attributes: &[HtmlAttribute],
    source_file: FileId,
    include_duplicates: bool,
    errors: &mut Vec<Vue3SfcParseError>,
) {
    let mut seen = BTreeSet::new();
    for attribute in attributes {
        if include_duplicates && !seen.insert(attribute.name.as_str()) {
            errors.push(vue3_sfc_parse_syntax_error(
                "Duplicate attribute.",
                attribute.name_start,
                source_file,
            ));
        }
        if attribute.name.starts_with('=') {
            errors.push(vue3_sfc_parse_syntax_error(
                "Attribute name cannot start with '='.",
                attribute.name_start,
                source_file,
            ));
        }
        if matches!(attribute.quote, Some(HtmlQuoteKind::Unquoted))
            && attribute.value_content_start == attribute.value_content_end
        {
            let offset = attribute.value_start.unwrap_or(attribute.name_end);
            errors.push(vue3_sfc_parse_syntax_error(
                "Attribute value was expected.",
                offset,
                source_file,
            ));
        }
    }
}

pub(crate) fn attrs_from_html(
    attributes: &[HtmlAttribute],
    decode_entities: bool,
) -> SfcBlockAttrs {
    let mut attrs = SfcBlockAttrs::default();
    for attribute in attributes {
        let value = attribute
            .value
            .as_ref()
            .map(|value| {
                let value = if decode_entities {
                    decode_html_attr_entities(value)
                } else {
                    value.clone()
                };
                SfcAttrValue::String(value)
            })
            .unwrap_or(SfcAttrValue::Bool(true));
        if attrs.raw.contains_key(&attribute.name) {
            attrs.duplicate_attr_starts.push(attribute.name_start);
        }
        attrs.raw.insert(attribute.name.clone(), value.clone());
        attrs
            .ranges
            .insert(attribute.name.clone(), (attribute.start, attribute.end));
        match attribute.name.as_str() {
            "lang" => {
                if let SfcAttrValue::String(value) = value {
                    attrs.lang = Some(value);
                }
            }
            "src" => {
                if let SfcAttrValue::String(value) = value {
                    attrs.src = Some(value);
                }
            }
            "scoped" => {
                attrs.scoped = true;
            }
            "setup" => {
                attrs.setup = true;
            }
            "generic" => {
                if let SfcAttrValue::String(value) = value {
                    attrs.generic = Some(value);
                }
            }
            "module" => {
                attrs.module = Some(match value {
                    SfcAttrValue::String(value) => value,
                    SfcAttrValue::Bool(_) => String::new(),
                });
            }
            _ => {}
        }
    }
    attrs
}

pub(crate) fn is_plain_text_sfc_tag(name: &str) -> bool {
    matches!(name, "script" | "style")
}

pub(crate) fn vue3_sfc_dom_void_tag(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

pub(crate) fn should_vue27_deindent(
    block: &OpenSfcBlock,
    options: &Vue27ParseComponentOptions,
) -> bool {
    if options.deindent == Some(true) {
        return true;
    }
    if options.deindent == Some(false) {
        return false;
    }
    !(block.type_name == "script"
        && block
            .attrs
            .lang
            .as_deref()
            .is_none_or(|lang| matches!(lang, "js" | "jsx" | "ts" | "tsx")))
}

pub(crate) fn deindent(source: &str) -> String {
    if !source
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, '\r' | '\n' | ' ' | '\t'))
    {
        return source.to_string();
    }
    let mut indent_char = None;
    let mut min_indent = usize::MAX;
    let lines = split_preserving_no_cr(source);
    for line in &lines {
        if line.chars().all(char::is_whitespace) {
            continue;
        }
        match indent_char {
            None => {
                let Some(ch) = line.chars().next() else {
                    continue;
                };
                if ch != ' ' && ch != '\t' {
                    return source.to_string();
                }
                indent_char = Some(ch);
                min_indent = min_indent.min(line.chars().take_while(|value| *value == ch).count());
            }
            Some(ch) => {
                min_indent = min_indent.min(line.chars().take_while(|value| *value == ch).count());
            }
        }
    }
    if min_indent == usize::MAX || min_indent == 0 {
        return source.to_string();
    }
    lines
        .iter()
        .map(|line| strip_chars(line, min_indent))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn split_preserving_no_cr(source: &str) -> Vec<String> {
    source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect()
}

pub(crate) fn strip_chars(source: &str, count: usize) -> String {
    let mut cursor = 0usize;
    for _ in 0..count {
        let Some(ch) = source[cursor..].chars().next() else {
            return String::new();
        };
        cursor += ch.len_utf8();
    }
    source[cursor..].to_string()
}

impl Vue27SfcPad {
    pub(crate) fn is_enabled(&self) -> bool {
        !matches!(self, Vue27SfcPad::False)
    }
}

impl Vue3SfcPad {
    pub(crate) fn is_enabled(&self) -> bool {
        !matches!(self, Vue3SfcPad::False)
    }
}

pub(crate) fn vue3_pad_content(source: &str, block: &OpenSfcBlock, pad: &Vue3SfcPad) -> String {
    if matches!(pad, Vue3SfcPad::Space) {
        return source[..block.open_end]
            .chars()
            .map(|ch| if matches!(ch, '\n' | '\r') { ch } else { ' ' })
            .collect();
    }
    let offset = source[..block.open_end].split('\n').count();
    let pad_char = if block.type_name == "script" && block.attrs.lang.is_none() {
        "//\n"
    } else {
        "\n"
    };
    pad_char.repeat(offset.saturating_sub(1))
}

pub(crate) fn vue27_pad_content(source: &str, block: &OpenSfcBlock, pad: &Vue27SfcPad) -> String {
    if matches!(pad, Vue27SfcPad::Space) {
        return source[..block.open_end]
            .chars()
            .map(|ch| if matches!(ch, '\n' | '\r') { ch } else { ' ' })
            .collect();
    }
    let offset = source[..block.open_end].split('\n').count();
    let pad_char = if block.type_name == "script" && block.attrs.lang.is_none() {
        "//\n"
    } else {
        "\n"
    };
    pad_char.repeat(offset.saturating_sub(1))
}

pub(crate) fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}
