pub(crate) fn vue3_start_tag_is_incomplete(source: &str, start: usize, end: usize) -> bool {
    source
        .get(start..end)
        .is_some_and(|slice| !slice.ends_with('>'))
}

pub(crate) fn vue3_empty_end_tag_should_be_text(source: &str, start: usize, end: usize) -> bool {
    let Some(slice) = source.get(start..end) else {
        return false;
    };
    if slice.ends_with('>') {
        return false;
    }
    slice
        .strip_prefix("</")
        .is_some_and(|after_slash| after_slash.trim().is_empty())
}

pub(crate) fn stack_is_root_only(stack: &[vuec_ast::NodeId], root: vuec_ast::NodeId) -> bool {
    stack.len() == 1 && stack.first().copied() == Some(root)
}

pub(crate) fn push_incomplete_start_tag_recovery_text(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    source: &TemplateSource,
    token_start: usize,
    token_end: usize,
) {
    let Some(slice) = source.source.get(token_start..token_end) else {
        return;
    };
    let Some(local_start) = incomplete_start_tag_recovery_text_start(slice) else {
        return;
    };
    let text = &slice[local_start..];
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::text(decode_html_text_entities(text)),
        Some(Span::new(
            source.file_id,
            source.base_offset + token_start + local_start,
            source.base_offset + token_start + local_start + text.len(),
        )),
    );
}

pub(crate) fn incomplete_start_tag_recovery_text_start(slice: &str) -> Option<usize> {
    slice.rfind('/').filter(|index| {
        slice
            .get(index + 1..)
            .is_some_and(|tail| tail.chars().all(char::is_whitespace))
    })
}

/// Returns the Vue 3 raw-text parsing mode for a tag and namespace.
pub fn vue3_raw_text_kind(
    tag: &str,
    namespace: vuec_ast::HtmlNamespace,
    in_v_pre: bool,
) -> Option<HtmlTextMode> {
    match raw_text_mode_for_tag(tag, html_namespace_to_html(namespace), in_v_pre) {
        HtmlTextMode::Data => None,
        mode => Some(mode),
    }
}

pub(crate) fn vue3_is_sfc_plain_template(
    tag: &str,
    parent: vuec_ast::NodeId,
    root: vuec_ast::NodeId,
    attributes: &[vuec_html::HtmlAttribute],
    options: &Vue3CompilerOptions,
) -> bool {
    if parent != root || tag != "template" || options.sfc_plain_template_langs.is_empty() {
        return false;
    }
    let Some(lang) = attributes
        .iter()
        .find(|attr| attr.name == "lang")
        .and_then(|attr| attr.value.as_deref())
    else {
        return false;
    };
    vue3_sfc_plain_template_lang(lang, options)
}

pub(crate) fn vue3_is_sfc_custom_block(
    tag: &str,
    parent: vuec_ast::NodeId,
    root: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
) -> bool {
    options.sfc_parse_mode && parent == root && tag != "template"
}

pub(crate) fn current_parent_raw_text_ignores_end_tag(
    ast: &Vue3Ast,
    parent: vuec_ast::NodeId,
    name: &str,
) -> bool {
    let Some(node) = ast.node(parent) else {
        return false;
    };
    matches!(
        &node.kind,
        Vue3AstKind::Element(element)
            if matches!(element.tag.as_str(), "textarea" | "title")
                && !element.tag.eq_ignore_ascii_case(name)
    )
}

pub(crate) fn stack_has_matching_element(
    ast: &Vue3Ast,
    stack: &[vuec_ast::NodeId],
    name: &str,
) -> bool {
    stack.iter().copied().skip(1).any(|node_id| {
        ast.node(node_id).is_some_and(|node| {
            matches!(
                &node.kind,
                Vue3AstKind::Element(element) if element.tag.eq_ignore_ascii_case(name)
            )
        })
    })
}

pub(crate) fn extend_open_element_spans_to(
    ast: &mut Vue3Ast,
    stack: &[vuec_ast::NodeId],
    end: usize,
) {
    for node_id in stack.iter().copied().skip(1) {
        let Some(node) = ast.node_mut(node_id) else {
            continue;
        };
        if !matches!(node.kind, Vue3AstKind::Element(_)) {
            continue;
        }
        if let Some(span) = node.span.source_mut() {
            if span.end.0 < end {
                span.end = vuec_source::BytePos(end);
            }
        }
    }
}
