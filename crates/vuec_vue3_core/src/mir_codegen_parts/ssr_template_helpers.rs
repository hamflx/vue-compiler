pub(crate) fn vue3_ssr_helper_order() -> &'static [RuntimeHelper] {
    &[
        RuntimeHelper::Vue3SsrRenderClass,
        RuntimeHelper::Vue3SsrRenderStyle,
        RuntimeHelper::Vue3SsrInterpolate,
        RuntimeHelper::Vue3SsrRenderAttr,
        RuntimeHelper::Vue3SsrRenderDynamicAttr,
        RuntimeHelper::Vue3SsrIncludeBooleanAttr,
        RuntimeHelper::Vue3SsrLooseContain,
        RuntimeHelper::Vue3SsrLooseEqual,
        RuntimeHelper::Vue3SsrRenderDynamicModel,
        RuntimeHelper::Vue3SsrRenderAttrs,
        RuntimeHelper::Vue3SsrGetDynamicModelProps,
        RuntimeHelper::Vue3SsrGetDirectiveProps,
        RuntimeHelper::Vue3SsrRenderVNode,
        RuntimeHelper::Vue3SsrRenderComponent,
        RuntimeHelper::Vue3SsrRenderSlot,
        RuntimeHelper::Vue3SsrRenderList,
        RuntimeHelper::Vue3SsrRenderTeleport,
        RuntimeHelper::Vue3SsrRenderSuspense,
    ]
}

pub(crate) enum SsrTemplatePart {
    Static(String),
    Expr(String),
}

pub(crate) type ParsedSsrOpenTag = (String, Vec<(String, Option<String>)>);

pub(crate) fn render_ssr_template_literal(parts: &[SsrTemplatePart]) -> String {
    let parts = merge_adjacent_ssr_template_static_parts(parts);
    let mut output = String::from("`");
    let multiline_exprs = parts.len() > 3;
    for part in &parts {
        match part {
            SsrTemplatePart::Static(value) => {
                output.push_str(&escape_template_literal_static(value));
            }
            SsrTemplatePart::Expr(value) => {
                if multiline_exprs {
                    output.push_str("${\n");
                    output.push_str(&indent_lines(value, 2));
                    output.push_str("\n}");
                } else {
                    output.push_str("${");
                    output.push_str(value);
                    output.push('}');
                }
            }
        }
    }
    output.push('`');
    output
}

pub(crate) fn merge_adjacent_ssr_template_static_parts(
    parts: &[SsrTemplatePart],
) -> Vec<SsrTemplatePart> {
    let mut merged = Vec::new();
    for part in parts {
        match (merged.last_mut(), part) {
            (Some(SsrTemplatePart::Static(previous)), SsrTemplatePart::Static(value)) => {
                previous.push_str(value);
            }
            (_, SsrTemplatePart::Static(value)) => {
                merged.push(SsrTemplatePart::Static(value.clone()));
            }
            (_, SsrTemplatePart::Expr(value)) => {
                merged.push(SsrTemplatePart::Expr(value.clone()));
            }
        }
    }
    merged
}

pub(crate) fn append_static_to_ssr_template_literal(mut literal: String, value: &str) -> String {
    if literal.pop() == Some('`') {
        literal.push_str(&escape_template_literal_static(value));
        literal.push('`');
        literal
    } else {
        literal
    }
}

pub(crate) fn escape_template_literal_static(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('$', "\\$")
}

pub(crate) fn parse_ssr_open_tag_start(value: &str) -> Option<ParsedSsrOpenTag> {
    let rest = value.strip_prefix('<')?;
    let tag_end = rest
        .char_indices()
        .find_map(|(index, ch)| (is_vue3_html_whitespace(ch) || ch == '>').then_some(index))
        .unwrap_or(rest.len());
    let tag = rest.get(..tag_end)?.to_string();
    if tag.is_empty() || tag.starts_with('/') || tag.starts_with('!') {
        return None;
    }
    let mut attrs = Vec::new();
    let mut input = rest.get(tag_end..).unwrap_or("").trim_start();
    while !input.is_empty() {
        let name_end = input
            .char_indices()
            .find_map(|(index, ch)| (is_vue3_html_whitespace(ch) || ch == '=').then_some(index))
            .unwrap_or(input.len());
        let name = input.get(..name_end)?.to_string();
        if name.is_empty() {
            break;
        }
        input = input.get(name_end..).unwrap_or("").trim_start();
        if let Some(after_eq) = input.strip_prefix('=') {
            input = after_eq.trim_start();
            if let Some(after_quote) = input.strip_prefix('"') {
                if let Some(end_quote) = after_quote.find('"') {
                    let value = after_quote.get(..end_quote)?.to_string();
                    attrs.push((name, Some(value)));
                    input = after_quote.get(end_quote + 1..).unwrap_or("").trim_start();
                    continue;
                }
            }
            attrs.push((name, Some(String::new())));
            break;
        }
        attrs.push((name, None));
    }
    Some((tag, attrs))
}
