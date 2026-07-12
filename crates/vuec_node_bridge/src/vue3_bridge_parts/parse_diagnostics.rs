pub(crate) struct OpenDiagnosticElement {
    pub(crate) name: String,
    pub(crate) start: usize,
    pub(crate) namespace: vuec_ast::HtmlNamespace,
    pub(crate) attributes: Vec<vuec_html::HtmlAttribute>,
    pub(crate) in_v_pre: bool,
}

pub(crate) fn sfc_diagnostic_raw_text_block(
    options: &Vue3CompilerOptions,
    depth: usize,
    tag: &str,
    attributes: &[vuec_html::HtmlAttribute],
) -> bool {
    if !options.sfc_parse_mode || depth != 0 {
        return false;
    }
    tag != "template" || sfc_plain_template_attrs(attributes, options)
}

pub(crate) fn sfc_plain_template_element(
    element: &vuec_ast::Vue3Element,
    options: &Vue3CompilerOptions,
) -> bool {
    if element.tag != "template" {
        return false;
    }
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Attribute(attr)
                if attr.name == "lang"
                    && attr
                        .value
                        .as_deref()
                        .is_some_and(|lang| sfc_plain_template_lang(lang, options))
        )
    })
}

pub(crate) fn sfc_plain_template_attrs(
    attributes: &[vuec_html::HtmlAttribute],
    options: &Vue3CompilerOptions,
) -> bool {
    attributes.iter().any(|attr| {
        attr.name == "lang"
            && attr
                .value
                .as_deref()
                .is_some_and(|lang| sfc_plain_template_lang(lang, options))
    })
}

pub(crate) fn sfc_plain_template_lang(lang: &str, options: &Vue3CompilerOptions) -> bool {
    !lang.is_empty()
        && ((options.sfc_parse_mode && lang != "html")
            || options
                .sfc_plain_template_langs
                .iter()
                .any(|candidate| candidate == lang))
}

pub(crate) fn vue3_diagnostic_tag_namespace(
    options: &Vue3CompilerOptions,
    tag: &str,
    attributes: &[vuec_html::HtmlAttribute],
    parent: Option<&OpenDiagnosticElement>,
) -> vuec_ast::HtmlNamespace {
    if let Some(namespace) = options.namespaces.get(tag).copied() {
        return namespace;
    }
    let mut namespace = parent
        .map(|open| open.namespace)
        .unwrap_or(options.root_namespace);
    if options.dom_namespaces {
        if let Some(parent) = parent {
            if namespace == vuec_ast::HtmlNamespace::MathMl {
                if parent.name == "annotation-xml" {
                    if tag == "svg" {
                        return vuec_ast::HtmlNamespace::Svg;
                    }
                    if diagnostic_attrs_have_value(
                        &parent.attributes,
                        "encoding",
                        &["text/html", "application/xhtml+xml"],
                    ) {
                        namespace = vuec_ast::HtmlNamespace::Html;
                    }
                } else if vue3_mathml_text_integration_point(&parent.name)
                    && tag != "mglyph"
                    && tag != "malignmark"
                {
                    namespace = vuec_ast::HtmlNamespace::Html;
                }
            } else if namespace == vuec_ast::HtmlNamespace::Svg
                && matches!(parent.name.as_str(), "foreignObject" | "desc" | "title")
            {
                namespace = vuec_ast::HtmlNamespace::Html;
            }
        }
        if namespace == vuec_ast::HtmlNamespace::Html {
            if tag == "svg" {
                return vuec_ast::HtmlNamespace::Svg;
            }
            if tag == "math" {
                return vuec_ast::HtmlNamespace::MathMl;
            }
        }
    }
    let _ = attributes;
    namespace
}

pub(crate) fn vue3_mathml_text_integration_point(tag: &str) -> bool {
    matches!(tag, "mi" | "mo" | "mn" | "ms" | "mtext")
}

pub(crate) fn diagnostic_attrs_have_value(
    attributes: &[vuec_html::HtmlAttribute],
    name: &str,
    values: &[&str],
) -> bool {
    attributes.iter().any(|attr| {
        attr.name == name
            && attr
                .value
                .as_deref()
                .is_some_and(|value| values.contains(&value))
    })
}

pub(crate) fn pop_diagnostic_stack_until(
    stack: &mut Vec<OpenDiagnosticElement>,
    name: &str,
    v_pre_depth: &mut usize,
) {
    while let Some(open) = stack.pop() {
        if open.in_v_pre && *v_pre_depth > 0 {
            *v_pre_depth -= 1;
        }
        if open.name.eq_ignore_ascii_case(name) {
            break;
        }
    }
}

pub(crate) fn tag_token_is_incomplete(source: &str, start: usize, end: usize) -> bool {
    source
        .get(start..end)
        .is_some_and(|slice| !slice.ends_with('>'))
}

pub(crate) fn tag_token_is_incomplete_at_eof(source: &str, start: usize, end: usize) -> bool {
    end == source.len() && tag_token_is_incomplete(source, start, end)
}

pub(crate) fn collect_missing_end_tag_name_diagnostics(source: &str, diagnostics: &mut Vec<Value>) {
    let mut cursor = 0usize;
    while let Some(offset) = source[cursor..].find("</>") {
        let start = cursor + offset;
        diagnostics.push(vue3_error_value(
            14,
            vue3_source_loc_value(source, start + 2, start + 2),
        ));
        cursor = start + 3;
    }
}

pub(crate) fn collect_start_tag_parse_errors(
    source: &str,
    start: usize,
    end: usize,
    attributes: &[vuec_html::HtmlAttribute],
    diagnostics: &mut Vec<Value>,
) {
    collect_unexpected_equals_before_attribute_name(source, start, end, attributes, diagnostics);
    collect_unexpected_solidus_in_tag(source, start, end, attributes, diagnostics);

    let mut seen_attrs = Vec::<String>::new();
    for attr in attributes {
        if attr.name.starts_with('=') {
            diagnostics.push(vue3_error_value(
                19,
                vue3_source_loc_value(source, attr.name_start, attr.name_start),
            ));
        }

        if seen_attrs.iter().any(|seen| seen == &attr.name) {
            diagnostics.push(vue3_error_value(
                2,
                vue3_source_loc_value(source, attr.name_start, attr.name_start),
            ));
        } else {
            seen_attrs.push(attr.name.clone());
        }

        if let Some(offset) = attr
            .name
            .char_indices()
            .find_map(|(index, ch)| matches!(ch, '"' | '\'' | '<').then_some(index))
        {
            let absolute = attr.name_start + offset;
            diagnostics.push(vue3_error_value(
                17,
                vue3_source_loc_value(source, absolute, absolute),
            ));
        }

        if attr.name.contains('[') && !attr.name.contains(']') {
            diagnostics.push(vue3_error_value(
                27,
                vue3_source_loc_value(source, attr.name_end, attr.name_end),
            ));
        }

        if attr.value.as_deref() == Some("")
            && matches!(attr.quote, Some(vuec_html::HtmlQuoteKind::Unquoted))
            && attr
                .value_start
                .and_then(|value_start| source.as_bytes().get(value_start).copied())
                == Some(b'>')
        {
            let offset = attr.value_start.unwrap_or(attr.end);
            diagnostics.push(vue3_error_value(
                13,
                vue3_source_loc_value(source, offset, offset),
            ));
        }

        if matches!(attr.quote, Some(vuec_html::HtmlQuoteKind::Unquoted)) {
            if let (Some(value_start), Some(value_end)) =
                (attr.value_content_start, attr.value_content_end)
            {
                if let Some(offset) =
                    first_unexpected_unquoted_attribute_value_char(source, value_start, value_end)
                {
                    diagnostics.push(vue3_error_value(
                        18,
                        vue3_source_loc_value(source, offset, offset),
                    ));
                }
            }
        }
    }
}

pub(crate) fn collect_unexpected_equals_before_attribute_name(
    source: &str,
    start: usize,
    end: usize,
    attributes: &[vuec_html::HtmlAttribute],
    diagnostics: &mut Vec<Value>,
) {
    for offset in start..end {
        if source.as_bytes().get(offset) != Some(&b'=') {
            continue;
        }
        if attributes
            .iter()
            .any(|attr| offset >= attr.start && offset < attr.end)
        {
            continue;
        }
        diagnostics.push(vue3_error_value(
            19,
            vue3_source_loc_value(source, offset, offset),
        ));
    }
}

pub(crate) fn collect_unexpected_solidus_in_tag(
    source: &str,
    start: usize,
    end: usize,
    attributes: &[vuec_html::HtmlAttribute],
    diagnostics: &mut Vec<Value>,
) {
    for offset in start..end {
        if source.as_bytes().get(offset) != Some(&b'/') {
            continue;
        }
        if offset == start + 1 {
            continue;
        }
        if attributes.iter().any(|attr| {
            attr.value_content_start
                .zip(attr.value_content_end)
                .is_some_and(|(value_start, value_end)| offset >= value_start && offset < value_end)
        }) {
            continue;
        }
        if source.as_bytes().get(offset + 1) == Some(&b'>') {
            continue;
        }
        diagnostics.push(vue3_error_value(
            22,
            vue3_source_loc_value(source, offset, offset),
        ));
    }
}

pub(crate) fn first_unexpected_unquoted_attribute_value_char(
    source: &str,
    start: usize,
    end: usize,
) -> Option<usize> {
    source
        .get(start..end)?
        .char_indices()
        .find_map(|(index, ch)| matches!(ch, '"' | '\'' | '<' | '=' | '`').then_some(start + index))
}

pub(crate) fn collect_invalid_lt_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
    for node in &ast.nodes {
        let Vue3AstKind::Text(_) = &node.kind else {
            continue;
        };
        if text_has_raw_text_parent(ast, node.id) || text_has_sfc_raw_parent(ast, node.id, options)
        {
            continue;
        }
        let Some(span) = node.span.source() else {
            continue;
        };
        let start = span.start.0.saturating_sub(base_offset);
        let end = span.end.0.saturating_sub(base_offset).min(source.len());
        let Some(slice) = source.get(start..end) else {
            continue;
        };
        let mut cursor = 0usize;
        while let Some(offset) = slice[cursor..].find('<') {
            let local_index = cursor + offset;
            cursor = local_index + 1;
            let global_index = start + local_index;
            match source.as_bytes().get(global_index + 1).copied() {
                Some(b'?') => diagnostics.push(vue3_error_value(
                    21,
                    vue3_source_loc_value(source, global_index + 1, global_index + 1),
                )),
                Some(b'/')
                    if source
                        .as_bytes()
                        .get(global_index + 2)
                        .is_some_and(u8::is_ascii_whitespace) =>
                {
                    diagnostics.push(vue3_error_value(
                        23,
                        vue3_source_loc_value(source, global_index, global_index),
                    ));
                }
                Some(next) if !matches!(next, b'/' | b'!' | b'A'..=b'Z' | b'a'..=b'z') => {
                    diagnostics.push(vue3_error_value(
                        12,
                        vue3_source_loc_value(source, global_index, global_index),
                    ));
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn text_has_raw_text_parent(ast: &Vue3Ast, node_id: vuec_ast::NodeId) -> bool {
    let Some(parent_id) = ast.node(node_id).and_then(|node| node.parent) else {
        return false;
    };
    ast.node(parent_id).is_some_and(|node| {
        matches!(
            &node.kind,
            Vue3AstKind::Element(element)
                if element.ns == vuec_ast::HtmlNamespace::Html
                    && matches!(element.tag.as_str(), "textarea" | "title" | "style" | "script")
        )
    })
}

pub(crate) fn text_has_sfc_raw_parent(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
) -> bool {
    if !options.sfc_parse_mode {
        return false;
    }
    let Some(parent_id) = ast.node(node_id).and_then(|node| node.parent) else {
        return false;
    };
    let Some(parent) = ast.node(parent_id) else {
        return false;
    };
    let Some(root) = ast.node(ast.root) else {
        return false;
    };
    parent.parent == Some(ast.root)
        && root.children.contains(&parent_id)
        && matches!(
            &parent.kind,
            Vue3AstKind::Element(element)
                if element.tag != "template" || sfc_plain_template_element(element, options)
        )
}

pub(crate) fn collect_missing_interpolation_end_diagnostics(
    source: &str,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
    let mut stack = Vec::<OpenDiagnosticElement>::new();
    let mut v_pre_depth = 0usize;
    let mut tokenizer = HtmlTokenizer::new(source);
    loop {
        if v_pre_depth > 0 {
            tokenizer.set_interpolation_delimiters("", "");
        } else if let Some([open, close]) = &options.delimiters {
            tokenizer.set_interpolation_delimiters(open, close);
        } else {
            tokenizer.set_interpolation_delimiters("{{", "}}");
        }
        let token = tokenizer.next_token();
        let eof = matches!(token.kind, HtmlTokenKind::Eof);
        match token.kind {
            HtmlTokenKind::Text(text) if v_pre_depth == 0 => {
                collect_missing_interpolation_end_in_text(source, token.start, &text, diagnostics);
            }
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let starts_v_pre =
                    v_pre_depth == 0 && attributes.iter().any(|attr| attr.name == "v-pre");
                let in_v_pre = v_pre_depth > 0 || starts_v_pre;
                let is_void = vue3_is_void_tag(options, &name);
                let namespace =
                    vue3_diagnostic_tag_namespace(options, &name, &attributes, stack.last());
                let raw_text_kind = vuec_vue3_core::vue3_raw_text_kind(&name, namespace, in_v_pre);
                if !self_closing && !is_void {
                    let raw_tag = name.clone();
                    let sfc_raw_text =
                        sfc_diagnostic_raw_text_block(options, stack.len(), &raw_tag, &attributes);
                    stack.push(OpenDiagnosticElement {
                        name,
                        start: token.start,
                        namespace,
                        attributes,
                        in_v_pre,
                    });
                    if in_v_pre {
                        v_pre_depth += 1;
                    }
                    if raw_text_kind.is_some() || sfc_raw_text {
                        if let Some((_text_end, end_tag_end)) =
                            vuec_vue3_core::find_matching_raw_text_end(source, token.end, &raw_tag)
                        {
                            tokenizer.set_cursor(end_tag_end);
                            if let Some(open) = stack.pop() {
                                if open.in_v_pre && v_pre_depth > 0 {
                                    v_pre_depth -= 1;
                                }
                            }
                        }
                    }
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if !name.is_empty() {
                    while let Some(open) = stack.pop() {
                        if open.in_v_pre && v_pre_depth > 0 {
                            v_pre_depth -= 1;
                        }
                        if open.name.eq_ignore_ascii_case(&name) {
                            break;
                        }
                    }
                }
            }
            HtmlTokenKind::Cdata(_)
            | HtmlTokenKind::Text(_)
            | HtmlTokenKind::Comment(_)
            | HtmlTokenKind::BogusQuestionTag
            | HtmlTokenKind::Doctype(_)
            | HtmlTokenKind::Eof => {}
        }
        if eof {
            break;
        }
    }
}

pub(crate) fn collect_missing_interpolation_end_in_text(
    source: &str,
    token_start: usize,
    text: &str,
    diagnostics: &mut Vec<Value>,
) {
    let mut cursor = 0usize;
    while let Some(open_offset) = text[cursor..].find("{{") {
        let open = cursor + open_offset;
        let inner_start = open + 2;
        if let Some(close_offset) = text[inner_start..].find("}}") {
            cursor = inner_start + close_offset + 2;
        } else {
            let global_open = token_start + open;
            diagnostics.push(vue3_error_value(
                25,
                vue3_source_loc_value(source, global_open, global_open),
            ));
            break;
        }
    }
}

pub(crate) fn collect_invalid_end_tag_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    _base_offset: usize,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
    let _ = ast;
    let mut stack = Vec::<OpenDiagnosticElement>::new();
    let mut v_pre_depth = 0usize;
    let mut tokenizer = HtmlTokenizer::new(source);
    loop {
        if v_pre_depth > 0 {
            tokenizer.set_interpolation_delimiters("", "");
        } else if let Some([open, close]) = &options.delimiters {
            tokenizer.set_interpolation_delimiters(open, close);
        } else {
            tokenizer.set_interpolation_delimiters("{{", "}}");
        }
        let token = tokenizer.next_token();
        let eof = matches!(token.kind, HtmlTokenKind::Eof);
        match token.kind {
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let starts_v_pre =
                    v_pre_depth == 0 && attributes.iter().any(|attr| attr.name == "v-pre");
                let in_v_pre = v_pre_depth > 0 || starts_v_pre;
                let namespace =
                    vue3_diagnostic_tag_namespace(options, &name, &attributes, stack.last());
                let raw_text_kind = vuec_vue3_core::vue3_raw_text_kind(&name, namespace, in_v_pre);
                if !self_closing
                    && !vue3_is_void_tag(options, &name)
                    && !tag_token_is_incomplete_at_eof(source, token.start, token.end)
                {
                    let raw_tag = name.clone();
                    let sfc_raw_text =
                        sfc_diagnostic_raw_text_block(options, stack.len(), &raw_tag, &attributes);
                    stack.push(OpenDiagnosticElement {
                        name,
                        start: token.start,
                        namespace,
                        attributes,
                        in_v_pre,
                    });
                    if in_v_pre {
                        v_pre_depth += 1;
                    }
                    if raw_text_kind.is_some() || sfc_raw_text {
                        if let Some((_text_end, end_tag_end)) =
                            vuec_vue3_core::find_matching_raw_text_end(source, token.end, &raw_tag)
                        {
                            tokenizer.set_cursor(end_tag_end);
                            if let Some(open) = stack.pop() {
                                if open.in_v_pre && v_pre_depth > 0 {
                                    v_pre_depth -= 1;
                                }
                            }
                        }
                    }
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if name.is_empty() {
                    if tag_token_is_incomplete(source, token.start, token.end) {
                        continue;
                    }
                    if source.as_bytes()[token.start..token.end]
                        .get(2)
                        .is_some_and(u8::is_ascii_whitespace)
                    {
                        diagnostics.push(vue3_error_value(
                            23,
                            vue3_source_loc_value(source, token.start, token.start),
                        ));
                    }
                    continue;
                }
                if tag_token_is_incomplete(source, token.start, token.end) {
                    continue;
                }
                if stack
                    .last()
                    .is_some_and(|open| open.name.eq_ignore_ascii_case(&name))
                {
                    if stack.pop().is_some_and(|open| open.in_v_pre) && v_pre_depth > 0 {
                        v_pre_depth -= 1;
                    }
                } else if let Some(matching_index) = stack
                    .iter()
                    .rposition(|open| open.name.eq_ignore_ascii_case(&name))
                {
                    while stack.len() > matching_index + 1 {
                        if let Some(open) = stack.pop() {
                            if open.in_v_pre && v_pre_depth > 0 {
                                v_pre_depth -= 1;
                            }
                            if !open.in_v_pre {
                                diagnostics.push(vue3_error_value(
                                    24,
                                    vue3_source_loc_value(source, open.start, open.start),
                                ));
                            }
                        }
                    }
                    if stack.pop().is_some_and(|open| open.in_v_pre) && v_pre_depth > 0 {
                        v_pre_depth -= 1;
                    }
                } else if !stack
                    .last()
                    .is_some_and(|open| raw_text_tag_ignores_end_tag(&open.name, &name))
                {
                    diagnostics.push(vue3_error_value(
                        23,
                        vue3_source_loc_value(source, token.start, token.start),
                    ));
                }
            }
            HtmlTokenKind::Text(_)
            | HtmlTokenKind::Comment(_)
            | HtmlTokenKind::Cdata(_)
            | HtmlTokenKind::BogusQuestionTag
            | HtmlTokenKind::Doctype(_)
            | HtmlTokenKind::Eof => {}
        }
        if eof {
            break;
        }
    }
    while let Some(open) = stack.pop() {
        if !open.in_v_pre {
            diagnostics.push(vue3_error_value(
                24,
                vue3_source_loc_value(source, open.start, open.start),
            ));
        }
    }
}

pub(crate) fn raw_text_tag_ignores_end_tag(open: &str, close: &str) -> bool {
    matches!(open, "textarea" | "title") && !open.eq_ignore_ascii_case(close)
}

pub(crate) fn vue3_is_void_tag(options: &Vue3CompilerOptions, tag: &str) -> bool {
    options
        .void_tags
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
}

pub(crate) fn collect_missing_directive_name_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    diagnostics: &mut Vec<Value>,
) {
    for node in &ast.nodes {
        let Vue3AstKind::Element(element) = &node.kind else {
            continue;
        };
        for prop in &element.props {
            let Vue3Prop::Attribute(attr) = prop else {
                continue;
            };
            if attr.name == "v-" {
                let loc = attr
                    .span
                    .map(|span| vue3_source_span_value(source, base_offset, span))
                    .unwrap_or_else(vue3_loc_stub_value);
                diagnostics.push(vue3_error_value(26, loc));
            }
        }
    }
}

pub(crate) fn vue3_error_value(code: u8, loc: Value) -> Value {
    json!({
        "code": code,
        "loc": loc,
    })
}

pub(crate) fn vue3_namespace_value(namespace: vuec_ast::HtmlNamespace) -> u8 {
    match namespace {
        vuec_ast::HtmlNamespace::Html => 0,
        vuec_ast::HtmlNamespace::Svg => 1,
        vuec_ast::HtmlNamespace::MathMl => 2,
    }
}

pub(crate) fn vue3_element_type_value(tag_type: vuec_ast::Vue3ElementType) -> u8 {
    match tag_type {
        vuec_ast::Vue3ElementType::Element => 0,
        vuec_ast::Vue3ElementType::Component => 1,
        vuec_ast::Vue3ElementType::SlotOutlet => 2,
        vuec_ast::Vue3ElementType::Template => 3,
    }
}

pub(crate) fn vue3_prop_value(
    source: &str,
    base_offset: usize,
    prop: &Vue3Prop,
    options: &Vue3CompilerOptions,
) -> Value {
    match prop {
        Vue3Prop::Attribute(attr) => vue3_attribute_value(source, base_offset, attr),
        Vue3Prop::Directive(dir) => {
            let exp_mode = match dir.name.as_str() {
                "on" => Vue3ExpressionAstMode::Statements,
                "slot" => Vue3ExpressionAstMode::Params,
                _ => Vue3ExpressionAstMode::Expression,
            };
            let mut value = json!({
                "type": 7,
                "name": dir.name,
                "rawName": dir.raw_name,
                "exp": dir.exp.as_ref().map(|exp| vue3_expression_value_with_mode(source, base_offset, exp, &span_to_node_span(dir.exp_span), false, Vue3ExpressionProjectionMode::Exact, options, exp_mode)),
                "arg": dir.arg.as_ref().map(|arg| vue3_expression_value_with_mode(source, base_offset, arg, &span_to_node_span(dir.arg_span), !dir.is_dynamic_arg, Vue3ExpressionProjectionMode::ExactLocTrimContent, options, Vue3ExpressionAstMode::Expression)),
                "modifiers": dir.modifiers.iter().enumerate().map(|(index, modifier)| {
                    let loc = dir
                        .modifier_spans
                        .get(index)
                        .map(|span| vue3_loc_value(source, base_offset, span))
                        .unwrap_or_else(vue3_loc_stub_value);
                    vue3_simple_expression_value(
                        modifier,
                        !matches!(dir.modifier_spans.get(index), Some(NodeSpan::Missing { .. })),
                        loc,
                    )
                }).collect::<Vec<_>>(),
                "loc": dir.span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
            });
            if dir.name == "for" {
                value["forParseResult"] =
                    vue3_for_parse_result_value(source, base_offset, dir, options);
            }
            value
        }
    }
}

pub(crate) fn vue3_attribute_value(
    source: &str,
    base_offset: usize,
    attr: &vuec_ast::Vue3Attribute,
) -> Value {
    json!({
        "type": 6,
        "name": attr.name,
        "nameLoc": attr.name_span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
        "value": attr.value.as_ref().map(|value| json!({
            "type": 2,
            "content": value,
            "loc": attr.value_span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
        })),
        "loc": attr.span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_inner_loc_value(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    node_id: vuec_ast::NodeId,
) -> Value {
    let Some(node) = ast.node(node_id) else {
        return vue3_loc_stub_value();
    };
    let Some(span) = node.span.source() else {
        return vue3_loc_stub_value();
    };
    let element_start = span.start.0.saturating_sub(base_offset);
    let element_end = span.end.0.saturating_sub(base_offset).min(source.len());
    let open_end = vue3_open_tag_end(source, element_start, element_end).unwrap_or(element_start);
    let inner_end = vue3_close_tag_start(source, open_end, element_end).unwrap_or_else(|| {
        node.children
            .last()
            .and_then(|child_id| ast.node(*child_id))
            .and_then(|child| child.span.source())
            .map(|child_span| {
                child_span
                    .end
                    .0
                    .saturating_sub(base_offset)
                    .min(source.len())
            })
            .unwrap_or(open_end)
    });
    vue3_source_loc_value(source, open_end, inner_end)
}

pub(crate) fn vue3_open_tag_end(source: &str, start: usize, end: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, ch) in source.get(start..end)?.char_indices() {
        match (quote, ch) {
            (Some(active), current) if current == active => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(ch),
            (None, '>') => return Some(start + offset + 1),
            (None, _) => {}
        }
    }
    None
}

pub(crate) fn vue3_close_tag_start(
    source: &str,
    open_end: usize,
    element_end: usize,
) -> Option<usize> {
    let mut cursor = open_end.min(source.len());
    let end = element_end.min(source.len());
    let mut close_start = None;
    while cursor < end {
        let Some(offset) = source.get(cursor..end)?.find("</") else {
            break;
        };
        close_start = Some(cursor + offset);
        cursor += offset + "</".len();
    }
    close_start
}

pub(crate) fn span_to_node_span(span: Option<vuec_source::Span>) -> NodeSpan {
    span.map(NodeSpan::from)
        .unwrap_or_else(|| NodeSpan::missing(vuec_ast::MissingSpanReason::Synthetic))
}

pub(crate) fn vue3_expression_value(
    source_text: &str,
    base_offset: usize,
    expression: &Vue3Expression,
    fallback_span: &NodeSpan,
    is_static: bool,
    options: &Vue3CompilerOptions,
    ast_mode: Vue3ExpressionAstMode,
) -> Value {
    vue3_expression_value_with_mode(
        source_text,
        base_offset,
        expression,
        fallback_span,
        is_static,
        Vue3ExpressionProjectionMode::Trim,
        options,
        ast_mode,
    )
}

#[derive(Clone, Copy)]
pub(crate) enum Vue3ExpressionProjectionMode {
    Trim,
    ExactLocTrimContent,
    Exact,
}

#[derive(Clone, Copy)]
pub(crate) enum Vue3ExpressionAstMode {
    Expression,
    Params,
    Statements,
}

pub(crate) fn vue3_expression_value_with_mode(
    source_text: &str,
    base_offset: usize,
    expression: &Vue3Expression,
    fallback_span: &NodeSpan,
    is_static: bool,
    mode: Vue3ExpressionProjectionMode,
    options: &Vue3CompilerOptions,
    ast_mode: Vue3ExpressionAstMode,
) -> Value {
    let source = expression.source_string();
    let loc = match mode {
        Vue3ExpressionProjectionMode::Trim => {
            vue3_expression_loc(source_text, base_offset, fallback_span, &source)
        }
        Vue3ExpressionProjectionMode::ExactLocTrimContent | Vue3ExpressionProjectionMode::Exact => {
            vue3_loc_value(source_text, base_offset, fallback_span)
        }
    };
    let content = match mode {
        Vue3ExpressionProjectionMode::Exact => source,
        Vue3ExpressionProjectionMode::Trim | Vue3ExpressionProjectionMode::ExactLocTrimContent => {
            source.trim().to_string()
        }
    };
    let mut value = vue3_simple_expression_value(&content, is_static, loc);
    if let Some(ast_value) = vue3_expression_ast_value(&content, is_static, options, ast_mode) {
        value["ast"] = ast_value;
    }
    value
}

pub(crate) fn vue3_simple_expression_value(source: &str, is_static: bool, loc: Value) -> Value {
    json!({
        "type": 4,
        "loc": loc,
        "content": source,
        "isStatic": is_static,
        "constType": if is_static { 3 } else { 0 },
    })
}

pub(crate) fn vue3_expression_ast_value(
    source: &str,
    is_static: bool,
    options: &Vue3CompilerOptions,
    mode: Vue3ExpressionAstMode,
) -> Option<Value> {
    if is_static || !options.prefix_identifiers || source.trim().is_empty() {
        return None;
    }
    let trimmed = source.trim();
    if is_simple_identifier(trimmed) {
        return Some(Value::Null);
    }
    let store = JsAstStore::new();
    let source_type = vue3_expression_source_type(options);
    match mode {
        Vue3ExpressionAstMode::Expression => {
            let expression_source = format!("({trimmed})");
            store
                .parse_expression(&expression_source, source_type)
                .ok()
                .map(|expression| expression_ast_value(&expression))
        }
        Vue3ExpressionAstMode::Params => {
            let expression_source = format!("({trimmed})=>{{}}");
            store
                .parse_expression(&expression_source, source_type)
                .ok()
                .map(|expression| expression_ast_value(&expression))
        }
        Vue3ExpressionAstMode::Statements => {
            let program_source = format!(" {trimmed} ");
            let program = store.parse_program(&program_source, source_type);
            Some(json!({
                "type": "Program",
                "body": program.program.body.iter().map(statement_ast_value).collect::<Vec<_>>(),
            }))
        }
    }
}
