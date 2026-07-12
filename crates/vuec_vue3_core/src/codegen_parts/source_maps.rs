/// Builds a render source map for generated code and a Vue 3 AST.
pub fn source_map_for_render(
    code: &str,
    ast: &Vue3Ast,
    source: &TemplateSource,
    options: &Vue3CompilerOptions,
) -> Option<SourceMapArtifact> {
    let root = ast.node(ast.root)?;
    let source_name = if source.filename.is_empty() {
        "template.vue.html".to_string()
    } else {
        source.filename.clone()
    };
    let mut names = Vec::new();
    let mut segments = Vec::new();
    let source_map_source = options
        .source_map_source
        .as_deref()
        .unwrap_or(&source.source);
    let source_map_base_offset = if options.source_map_source.is_some() {
        options.source_map_base_offset
    } else {
        source.base_offset
    };
    collect_source_map_segments(
        code,
        ast,
        &root.children,
        source_map_base_offset,
        source_map_source,
        options,
        &mut names,
        &mut segments,
    );
    if segments.is_empty() {
        return None;
    }
    segments.sort_by_key(|segment| {
        (
            segment.generated_line,
            segment.generated_column,
            segment.original_line,
            segment.original_column,
            segment.name_index.unwrap_or(usize::MAX),
        )
    });
    segments.dedup_by_key(|segment| {
        (
            segment.generated_line,
            segment.generated_column,
            segment.original_line,
            segment.original_column,
            segment.name_index,
        )
    });
    Some(SourceMapArtifact::from_segments(
        None,
        source_name,
        source_map_source.to_string(),
        names,
        segments,
    ))
}

pub(crate) fn collect_source_map_segments(
    code: &str,
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    let mut cursor = 0usize;
    for child_id in children {
        collect_node_source_map(
            code,
            ast,
            *child_id,
            base_offset,
            source,
            options,
            names,
            segments,
            &mut cursor,
        );
    }
}

pub(crate) fn collect_node_source_map(
    code: &str,
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    match &node.kind {
        Vue3AstKind::Element(element) => {
            add_vnode_mapping(code, node, base_offset, source, segments, cursor);
            add_element_prop_mappings(code, element, base_offset, source, options, names, segments);
            for child_id in &node.children {
                collect_node_source_map(
                    code,
                    ast,
                    *child_id,
                    base_offset,
                    source,
                    options,
                    names,
                    segments,
                    cursor,
                );
            }
        }
        Vue3AstKind::Interpolation(_) => {
            add_interpolation_mapping(
                code,
                node,
                base_offset,
                source,
                options,
                names,
                segments,
                cursor,
            );
        }
        _ => {}
    }
}

pub(crate) fn add_vnode_mapping(
    code: &str,
    node: &vuec_ast::Node<Vue3NodeKind>,
    base_offset: usize,
    source: &str,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Some(span) = node.span.source() else {
        return;
    };
    let local_start = span.start.0.saturating_sub(base_offset);
    let local_end = span.end.0.saturating_sub(base_offset);
    let Some(start) = loc_for_offset(source, local_start) else {
        return;
    };
    let Some(end) = loc_for_offset(source, local_end) else {
        return;
    };
    let tag = match &node.kind {
        Vue3AstKind::Element(element) => &element.tag,
        _ => return,
    };
    let block_needle = format!("_createElementBlock(\"{tag}\"");
    let vnode_needle = format!("_createElementVNode(\"{tag}\"");
    let block_offset = find_code_offset(code, &block_needle, *cursor);
    let vnode_offset = find_code_offset(code, &vnode_needle, *cursor);
    let helper_offset = match (block_offset, vnode_offset) {
        (Some(block), Some(vnode)) => block.min(vnode),
        (Some(block), None) => block,
        (None, Some(vnode)) => vnode,
        (None, None) => return,
    };
    if let Some((line, column)) = loc_for_offset(code, helper_offset) {
        segments.push(SourceMapSegment {
            generated_line: line,
            generated_column: column,
            original_line: start.0,
            original_column: start.1,
            name_index: None,
        });
        let tag_needle = format!("\"{tag}\"");
        if let Some(tag_offset) = find_code_offset(code, &tag_needle, helper_offset) {
            if let Some((end_line, end_column)) = loc_for_offset(code, tag_offset) {
                segments.push(SourceMapSegment {
                    generated_line: end_line,
                    generated_column: end_column,
                    original_line: end.0,
                    original_column: end.1,
                    name_index: None,
                });
                *cursor = tag_offset + tag_needle.len();
            }
        } else {
            *cursor = helper_offset + block_needle.len().min(vnode_needle.len());
        }
    }
}

pub(crate) fn add_interpolation_mapping(
    code: &str,
    node: &vuec_ast::Node<Vue3NodeKind>,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Vue3AstKind::Interpolation(_) = &node.kind else {
        return;
    };
    let Some(span) = node.span.source() else {
        return;
    };
    let Some((original_expression, original_start)) =
        original_interpolation_expression(source, span, base_offset, options)
    else {
        return;
    };
    *cursor = add_expression_token_mappings(
        code,
        source,
        original_expression,
        original_start,
        *cursor,
        uses_prefixed_identifiers(options),
        names,
        segments,
    );
}

pub(crate) fn original_interpolation_expression<'a>(
    source: &'a str,
    span: Span,
    base_offset: usize,
    options: &Vue3CompilerOptions,
) -> Option<(&'a str, usize)> {
    let (local_start, local_end) = local_source_span_range(source, span, base_offset)?;
    let node_source = source.get(local_start..local_end)?;
    let (open_delimiter, close_delimiter) = options
        .delimiters
        .as_ref()
        .map_or(("{{", "}}"), |items| (items[0].as_str(), items[1].as_str()));
    if open_delimiter.is_empty() || close_delimiter.is_empty() {
        return None;
    }
    let open_start = node_source.find(open_delimiter)?;
    let expression_start = local_start + open_start + open_delimiter.len();
    let expression_end = expression_start
        + source
            .get(expression_start..local_end)?
            .find(close_delimiter)?;
    trimmed_source_range(source, expression_start, expression_end)
}

pub(crate) fn original_expression_from_span(
    source: &str,
    span: Span,
    base_offset: usize,
) -> Option<(&str, usize)> {
    let (local_start, local_end) = local_source_span_range(source, span, base_offset)?;
    trimmed_source_range(source, local_start, local_end)
}

pub(crate) fn local_source_span_range(
    source: &str,
    span: Span,
    base_offset: usize,
) -> Option<(usize, usize)> {
    let start = span.start.0.checked_sub(base_offset)?;
    let end = span.end.0.checked_sub(base_offset)?;
    if start > end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return None;
    }
    Some((start, end))
}

pub(crate) fn trimmed_source_range(
    source: &str,
    start: usize,
    end: usize,
) -> Option<(&str, usize)> {
    let start = trim_start_offset(source, start, end);
    let end = trim_end_offset(source, start, end);
    Some((source.get(start..end)?, start))
}

pub(crate) fn add_element_prop_mappings(
    code: &str,
    element: &Vue3Element,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    for prop in &element.props {
        match prop {
            Vue3Prop::Attribute(attr) => {
                if let Some(span) = attr.name_span {
                    add_direct_mapping(
                        code,
                        source,
                        &attr.name,
                        span.start.0.saturating_sub(base_offset),
                        0,
                        None,
                        segments,
                    );
                }
                if let (Some(value), Some(span)) = (&attr.value, attr.value_span) {
                    add_direct_mapping(
                        code,
                        source,
                        &quote_string(value),
                        span.start.0.saturating_sub(base_offset),
                        0,
                        None,
                        segments,
                    );
                }
            }
            Vue3Prop::Directive(dir) => {
                if dir.name == "bind" {
                    if dir
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.source_string() == "class")
                    {
                        if let Some(arg_span) = dir.arg_span {
                            add_direct_mapping(
                                code,
                                source,
                                "class:",
                                arg_span.start.0.saturating_sub(base_offset),
                                0,
                                None,
                                segments,
                            );
                        }
                    }
                    add_directive_expression_token_mappings(
                        code,
                        source,
                        dir,
                        base_offset,
                        options,
                        names,
                        segments,
                    );
                }
                if dir.name == "on" && dir.arg.is_some() {
                    if let (Some(exp), Some(span)) = (&dir.exp, dir.exp_span) {
                        let expression = exp.source_string();
                        let fallback_start = span.start.0.saturating_sub(base_offset);
                        let (original_expression, original_start) =
                            original_expression_from_span(source, span, base_offset)
                                .unwrap_or((expression.trim(), fallback_start));
                        add_event_handler_token_mappings(
                            code,
                            source,
                            original_expression,
                            original_start,
                            0,
                            uses_prefixed_identifiers(options),
                            names,
                            segments,
                        );
                    }
                }
                if matches!(dir.name.as_str(), "if" | "else-if" | "for") {
                    if let (Some(exp), Some(span)) = (&dir.exp, dir.exp_span) {
                        let expression = exp.source_string();
                        let fallback_start = span.start.0.saturating_sub(base_offset);
                        let (original_expression, original_start) =
                            original_expression_from_span(source, span, base_offset)
                                .unwrap_or((expression.trim(), fallback_start));
                        add_expression_token_mappings(
                            code,
                            source,
                            original_expression,
                            original_start,
                            0,
                            uses_prefixed_identifiers(options),
                            names,
                            segments,
                        );
                    }
                }
            }
        }
    }
}

pub(crate) fn add_directive_expression_token_mappings(
    code: &str,
    source: &str,
    dir: &Vue3Directive,
    base_offset: usize,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    let (Some(exp), Some(span)) = (&dir.exp, dir.exp_span) else {
        return;
    };
    let expression = exp.source_string();
    let fallback_start = span.start.0.saturating_sub(base_offset);
    let (original_expression, original_start) =
        original_expression_from_span(source, span, base_offset)
            .unwrap_or((expression.trim(), fallback_start));
    add_expression_token_mappings(
        code,
        source,
        original_expression,
        original_start,
        0,
        uses_prefixed_identifiers(options),
        names,
        segments,
    );
}

pub(crate) fn add_direct_mapping(
    code: &str,
    source: &str,
    generated_needle: &str,
    original_offset: usize,
    generated_from: usize,
    name: Option<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    let Some(generated_offset) = find_code_offset(code, generated_needle, generated_from) else {
        return;
    };
    let Some((generated_line, generated_column)) = loc_for_offset(code, generated_offset) else {
        return;
    };
    let Some((original_line, original_column)) = loc_for_offset(source, original_offset) else {
        return;
    };
    let name_index = name.map(|_| 0);
    segments.push(SourceMapSegment {
        generated_line,
        generated_column,
        original_line,
        original_column,
        name_index,
    });
}

pub(crate) fn add_expression_token_mappings(
    code: &str,
    source: &str,
    expression: &str,
    original_expression_start: usize,
    generated_from: usize,
    precise_members: bool,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) -> usize {
    add_expression_token_mappings_with_options(
        code,
        source,
        expression,
        original_expression_start,
        generated_from,
        precise_members,
        false,
        names,
        segments,
    )
}

pub(crate) fn add_event_handler_token_mappings(
    code: &str,
    source: &str,
    expression: &str,
    original_expression_start: usize,
    generated_from: usize,
    precise_members: bool,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) -> usize {
    add_expression_token_mappings_with_options(
        code,
        source,
        expression,
        original_expression_start,
        generated_from,
        precise_members,
        true,
        names,
        segments,
    )
}

pub(crate) fn add_expression_token_mappings_with_options(
    code: &str,
    source: &str,
    expression: &str,
    original_expression_start: usize,
    generated_from: usize,
    precise_members: bool,
    include_globals: bool,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) -> usize {
    let tokens = expression_source_map_tokens(expression, include_globals);
    let mut token_cursors = BTreeMap::new();
    let mut generated_end = generated_from;
    let mut single_token_end = None;
    for (original_relative, token) in tokens.iter().copied() {
        let token_cursor = token_cursors.get(token).copied().unwrap_or(generated_from);
        let generated_needles = if uses_ctx_prefix_for_generated(code, token) {
            vec![format!("_ctx.{token}"), token.to_string()]
        } else {
            vec![token.to_string(), format!("_ctx.{token}")]
        };
        let generated_match = generated_needles.iter().find_map(|needle| {
            find_code_offset(code, needle, token_cursor)
                .map(|offset| (offset, needle.len()))
        });
        let Some((generated_offset, generated_len)) = generated_match else {
            continue;
        };
        let token_end = generated_offset + generated_len;
        token_cursors.insert(token, token_end);
        generated_end = generated_end.max(token_end);
        single_token_end = Some(token_end);
        let original_offset = if precise_members
            || !is_member_tail_token(expression, original_relative)
        {
            original_expression_start + original_relative
        } else {
            original_expression_start
        };
        let Some((generated_line, generated_column)) = loc_for_offset(code, generated_offset)
        else {
            continue;
        };
        let Some((original_line, original_column)) = loc_for_offset(source, original_offset) else {
            continue;
        };
        let name_index = Some(name_index(names, token));
        segments.push(SourceMapSegment {
            generated_line,
            generated_column,
            original_line,
            original_column,
            name_index,
        });
    }
    if tokens.len() == 1 {
        if let Some(generated_end) = single_token_end {
            add_expression_end_mapping(
                code,
                source,
                generated_end,
                original_expression_start + expression.len(),
                segments,
            );
        }
    }
    generated_end
}

pub(crate) fn add_expression_end_mapping(
    code: &str,
    source: &str,
    generated_offset: usize,
    original_offset: usize,
    segments: &mut Vec<SourceMapSegment>,
) {
    let Some((generated_line, generated_column)) = loc_for_offset(code, generated_offset) else {
        return;
    };
    let Some((original_line, original_column)) = loc_for_offset(source, original_offset) else {
        return;
    };
    segments.push(SourceMapSegment {
        generated_line,
        generated_column,
        original_line,
        original_column,
        name_index: None,
    });
}

pub(crate) fn uses_ctx_prefix_for_generated(code: &str, token: &str) -> bool {
    code.contains(&format!("_ctx.{token}"))
}

pub(crate) fn is_member_tail_token(expression: &str, token_start: usize) -> bool {
    token_start > 0 && expression[..token_start].ends_with('.')
}

pub(crate) fn expression_source_map_tokens(
    expression: &str,
    include_globals: bool,
) -> Vec<(usize, &str)> {
    let mut tokens = Vec::new();
    for (index, ch) in expression.char_indices() {
        if !is_identifier_start(ch) {
            continue;
        }
        if index > 0
            && expression[..index]
                .chars()
                .last()
                .is_some_and(is_identifier_continue)
        {
            continue;
        }
        let end = expression[index + ch.len_utf8()..]
            .char_indices()
            .find_map(|(offset, current)| {
                (!is_identifier_continue(current)).then_some(index + ch.len_utf8() + offset)
            })
            .unwrap_or(expression.len());
        let token = &expression[index..end];
        if !is_keyword(token) && (include_globals || !is_global_or_literal(token)) {
            tokens.push((index, token));
        }
    }
    if tokens.is_empty() && !expression.is_empty() {
        tokens.push((0, expression));
    }
    tokens
}

pub(crate) fn name_index(names: &mut Vec<String>, name: &str) -> usize {
    if let Some(index) = names.iter().position(|existing| existing == name) {
        index
    } else {
        names.push(name.to_string());
        names.len() - 1
    }
}

pub(crate) fn loc_for_offset(source: &str, offset: usize) -> Option<(u32, u32)> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let mut line = 0u32;
    let mut line_start = 0usize;
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < offset {
        match bytes[index] {
            b'\r' => {
                if index + 1 < offset && bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                line += 1;
                line_start = index + 1;
            }
            b'\n' => {
                line += 1;
                line_start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    let column = source[line_start..offset].encode_utf16().count() as u32;
    Some((line, column))
}

pub(crate) fn find_code_offset(code: &str, needle: &str, from: usize) -> Option<usize> {
    code.get(from..)?.find(needle).map(|offset| from + offset)
}
