pub(crate) fn quote_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

pub(crate) fn json_key(key: &str) -> String {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
    {
        key.to_string()
    } else {
        quote_string(key)
    }
}

pub(crate) fn quote_text(value: &str) -> String {
    quote_string(value)
}

pub(crate) fn push_text_and_interpolations(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    file_id: FileId,
    token_start: usize,
    text: &str,
    options: &Vue3CompilerOptions,
) {
    let (open_delimiter, close_delimiter) = options
        .delimiters
        .as_ref()
        .map_or(("{{", "}}"), |items| (items[0].as_str(), items[1].as_str()));
    if open_delimiter.is_empty() || close_delimiter.is_empty() {
        push_text(ast, parent, file_id, token_start, text);
        return;
    }
    let mut cursor = 0usize;
    while let Some(open) = text[cursor..].find(open_delimiter) {
        let open = cursor + open;
        let expression_start = open + open_delimiter.len();
        let Some(close_offset) = text[expression_start..].find(close_delimiter) else {
            push_text(ast, parent, file_id, token_start + cursor, &text[cursor..]);
            return;
        };
        if open > cursor {
            push_text(
                ast,
                parent,
                file_id,
                token_start + cursor,
                &text[cursor..open],
            );
        }
        let close = expression_start + close_offset;
        let expression = decode_html_text_entities(text[expression_start..close].trim());
        let _id = ast.push_child(
            parent,
            Vue3NodeKind::interpolation(expression),
            Some(Span::new(
                file_id,
                token_start + open,
                token_start + close + close_delimiter.len(),
            )),
        );
        cursor = close + close_delimiter.len();
    }
    if cursor < text.len() {
        push_text(ast, parent, file_id, token_start + cursor, &text[cursor..]);
    }
}

pub(crate) fn push_text(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    file_id: FileId,
    start: usize,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let decoded = decode_html_text_entities(text);
    let previous = ast
        .node(parent)
        .and_then(|node| node.children.last().copied());
    if let Some(previous) = previous {
        if let Some(node) = ast.node_mut(previous) {
            if let Vue3AstKind::Text(existing) = &mut node.kind {
                existing.value.push_str(&decoded);
                if let Some(span) = node.span.source_mut() {
                    span.end = vuec_source::BytePos(start + text.len());
                }
                return;
            }
        }
    }
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::text(decoded),
        Some(Span::new(file_id, start, start + text.len())),
    );
}

pub(crate) fn push_raw_text(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    file_id: FileId,
    start: usize,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let previous = ast
        .node(parent)
        .and_then(|node| node.children.last().copied());
    if let Some(previous) = previous {
        if let Some(node) = ast.node_mut(previous) {
            if let Vue3AstKind::Text(existing) = &mut node.kind {
                existing.value.push_str(text);
                if let Some(span) = node.span.source_mut() {
                    span.end = vuec_source::BytePos(start + text.len());
                }
                return;
            }
        }
    }
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::text(text),
        Some(Span::new(file_id, start, start + text.len())),
    );
}
