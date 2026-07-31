use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CssBlockContext {
    Root,
    Container,
    Deep,
    Keyframes,
}

pub(crate) fn css_block_body_has_trailing_whitespace(body: &str) -> bool {
    body.chars().next_back().is_some_and(char::is_whitespace)
}

pub(crate) fn rewrite_css_items(
    source: &str,
    scope_id: &str,
    keyframes: &BTreeMap<String, String>,
    context: CssBlockContext,
    budget: &mut ScopedStyleBudget,
) -> Result<String, StylePreprocessError> {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        let whitespace_start = cursor;
        cursor = skip_css_whitespace(source, cursor);
        if cursor > whitespace_start {
            if matches!(context, CssBlockContext::Deep)
                && output.is_empty()
                && css_next_item_is_block(source, cursor)
            {
                // Leading whitespace in deep passthrough bodies is parser trivia;
                // preserve only whitespace introduced by special selector rewrites.
            } else if matches!(context, CssBlockContext::Deep)
                && output.ends_with(';')
                && css_next_item_is_at_rule_block(source, cursor)
            {
                if !output.ends_with('\n') {
                    budget.append_render_char(&mut output, '\n')?;
                }
            } else {
                append_scoped_normalized_whitespace(
                    &mut output,
                    &source[whitespace_start..cursor],
                    budget,
                )?;
            }
        }
        if cursor >= source.len() {
            break;
        }
        if source[cursor..].starts_with("/*") {
            let Some(end_offset) = source[cursor + 2..].find("*/") else {
                budget.append_render_str(&mut output, &source[cursor..])?;
                break;
            };
            let end = cursor + 2 + end_offset + 2;
            budget.append_render_str(&mut output, &source[cursor..end])?;
            cursor = end;
            continue;
        }

        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(source, cursor) else {
            if context == CssBlockContext::Deep {
                let rewritten =
                    rewrite_scoped_declaration_body(&source[cursor..], keyframes, budget)?;
                budget.append_render_str(&mut output, &rewritten)?;
            } else {
                budget.append_render_str(&mut output, &source[cursor..])?;
            }
            break;
        };
        let raw_prelude = &source[cursor..delimiter];
        let prelude_end = raw_prelude.trim_end().len();
        let prelude = raw_prelude[..prelude_end].trim();
        let brace_spacing = &raw_prelude[prelude_end..];
        if delimiter_ch == ';' {
            if context == CssBlockContext::Deep {
                budget.claim_render_copy(prelude.len())?;
                let rewritten = rewrite_declaration_segment(prelude, keyframes);
                budget.append_render_str(&mut output, &rewritten)?;
            } else {
                budget.append_render_str(&mut output, prelude)?;
            }
            budget.append_render_char(&mut output, ';')?;
            cursor = delimiter + 1;
            continue;
        }

        let Some(close) = find_matching_brace(source, delimiter) else {
            budget.append_render_str(&mut output, &source[cursor..])?;
            break;
        };
        let body = &source[delimiter + 1..close];
        if prelude.starts_with('@') {
            let rewritten_prelude = rewrite_at_rule_prelude(prelude, keyframes);
            budget.append_render_str(&mut output, &rewritten_prelude)?;
            budget.append_render_str(&mut output, brace_spacing)?;
            budget.append_render_char(&mut output, '{')?;
            let next_context = if is_keyframes_at_rule(prelude) {
                CssBlockContext::Keyframes
            } else if matches!(context, CssBlockContext::Deep) {
                CssBlockContext::Deep
            } else {
                CssBlockContext::Container
            };
            let rewritten_body =
                rewrite_css_items(body, scope_id, keyframes, next_context, budget)?;
            if css_block_contains_style_rules(&rewritten_body)
                || css_block_contains_at_rule_with_style_rules(&rewritten_body)
            {
                budget.append_render_char(&mut output, '\n')?;
                if next_context == CssBlockContext::Deep {
                    budget.append_render_str(&mut output, rewritten_body.trim_end())?;
                } else {
                    budget.append_render_str(&mut output, rewritten_body.trim())?;
                }
                if css_block_body_has_trailing_whitespace(body) {
                    budget.append_render_char(&mut output, '\n')?;
                }
            } else {
                budget.append_render_str(&mut output, &rewritten_body)?;
            }
            budget.append_render_char(&mut output, '}')?;
        } else {
            let has_nested_block =
                !matches!(context, CssBlockContext::Keyframes) && css_block_has_nested_block(body);
            let has_direct_nested_rule = has_nested_block && css_block_has_direct_nested_rule(body);
            let selector_rewrite =
                if matches!(context, CssBlockContext::Root | CssBlockContext::Container) {
                    rewrite_selector_list_for_rule(
                        prelude,
                        scope_id,
                        has_nested_block,
                        has_direct_nested_rule,
                    )
                } else {
                    SelectorRewriteResult {
                        selector: prelude.to_string(),
                        deep_passthrough: false,
                    }
                };
            let selector = if context == CssBlockContext::Keyframes {
                prelude.to_string()
            } else if context == CssBlockContext::Deep {
                rewrite_deep_passthrough_selector(prelude)
            } else if selector_rewrite.deep_passthrough {
                selector_rewrite.selector
            } else if has_direct_nested_rule && find_deep_combinator(prelude).is_some() {
                rewrite_deep_combinator_selector_without_scope(prelude)
                    .unwrap_or(selector_rewrite.selector)
            } else if has_direct_nested_rule {
                rewrite_direct_nested_parent_selector(prelude)
            } else {
                selector_rewrite.selector
            };
            budget.append_render_str(&mut output, &selector)?;
            budget.append_render_str(&mut output, brace_spacing)?;
            budget.append_render_char(&mut output, '{')?;
            if context == CssBlockContext::Keyframes {
                let rewritten = rewrite_css_items(
                    body,
                    scope_id,
                    keyframes,
                    CssBlockContext::Keyframes,
                    budget,
                )?;
                budget.append_render_str(&mut output, &rewritten)?;
            } else if context == CssBlockContext::Deep
                || (selector_rewrite.deep_passthrough && has_nested_block)
            {
                let rewritten_body = if selector_rewrite.deep_passthrough
                    && has_direct_nested_rule
                    && deep_container_direct_nested_wraps_parent_declarations(prelude)
                {
                    rewrite_deep_passthrough_wrapped_nested_body(body, scope_id, keyframes, budget)?
                } else {
                    rewrite_deep_passthrough_body(body, scope_id, keyframes, budget)?
                };
                if css_block_starts_with_block(&rewritten_body) {
                    if css_block_starts_with_commented_block(&rewritten_body) {
                        budget.append_render_char(&mut output, ' ')?;
                    } else {
                        budget.append_render_char(&mut output, '\n')?;
                    }
                    budget.append_render_str(&mut output, rewritten_body.trim())?;
                    budget.append_render_char(&mut output, '\n')?;
                } else {
                    budget.append_render_str(&mut output, &rewritten_body)?;
                }
            } else if has_nested_block {
                let rewritten = rewrite_nested_scoped_rule_body(
                    body,
                    scope_id,
                    keyframes,
                    has_direct_nested_rule,
                    budget,
                )?;
                budget.append_render_str(&mut output, &rewritten)?;
            } else {
                let rewritten = rewrite_scoped_declaration_body(body, keyframes, budget)?;
                budget.append_render_str(&mut output, &rewritten)?;
            }
            budget.append_render_char(&mut output, '}')?;
        }
        cursor = close + 1;
    }
    Ok(output)
}

fn append_scoped_normalized_whitespace(
    output: &mut String,
    whitespace: &str,
    budget: &mut ScopedStyleBudget,
) -> Result<(), StylePreprocessError> {
    if whitespace.contains('\n') || whitespace.contains('\r') {
        budget.append_render_char(output, '\n')
    } else {
        budget.append_render_str(output, whitespace)
    }
}

pub(crate) fn collect_scoped_selector_deprecation_warnings(
    source: &str,
    context: CssBlockContext,
    warnings: &mut Vec<Diagnostic>,
    budget: &mut ScopedStyleBudget,
) -> Result<(), StylePreprocessError> {
    let mut cursor = 0usize;
    while cursor < source.len() {
        cursor = skip_css_whitespace(source, cursor);
        if cursor >= source.len() {
            break;
        }
        if source[cursor..].starts_with("/*") {
            let Some(end_offset) = source[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + end_offset + 2;
            continue;
        }

        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(source, cursor) else {
            break;
        };
        if delimiter_ch == ';' {
            cursor = delimiter + 1;
            continue;
        }

        let prelude = source[cursor..delimiter].trim();
        let Some(close) = find_matching_brace(source, delimiter) else {
            break;
        };
        let body = &source[delimiter + 1..close];
        if prelude.starts_with('@') {
            if !is_keyframes_at_rule(prelude) {
                collect_scoped_selector_deprecation_warnings(
                    body,
                    CssBlockContext::Container,
                    warnings,
                    budget,
                )?;
            }
        } else if !matches!(context, CssBlockContext::Keyframes)
            && !css_prelude_is_block_declaration(prelude)
        {
            collect_selector_list_deprecation_warnings(prelude, warnings, budget)?;
            collect_scoped_selector_deprecation_warnings(body, context, warnings, budget)?;
        }
        cursor = close + 1;
    }
    Ok(())
}

pub(crate) fn rewrite_deep_passthrough_body(
    body: &str,
    scope_id: &str,
    keyframes: &BTreeMap<String, String>,
    budget: &mut ScopedStyleBudget,
) -> Result<String, StylePreprocessError> {
    let rewritten = rewrite_css_items(body, scope_id, keyframes, CssBlockContext::Deep, budget)?;
    budget.claim_render_copy(rewritten.len())?;
    Ok(normalize_deep_passthrough_parent_anchor_blocks(&rewritten))
}

pub(crate) fn rewrite_deep_passthrough_wrapped_nested_body(
    body: &str,
    scope_id: &str,
    keyframes: &BTreeMap<String, String>,
    budget: &mut ScopedStyleBudget,
) -> Result<String, StylePreprocessError> {
    let mut declarations = String::new();
    let mut nested_blocks = Vec::new();
    let mut cursor = 0usize;
    while cursor < body.len() {
        let whitespace_start = cursor;
        cursor = skip_css_whitespace(body, cursor);
        if cursor > whitespace_start {
            append_scoped_normalized_whitespace(
                &mut declarations,
                &body[whitespace_start..cursor],
                budget,
            )?;
        }
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end_offset) = body[cursor + 2..].find("*/") else {
                budget.append_render_str(&mut declarations, &body[cursor..])?;
                break;
            };
            let end = cursor + 2 + end_offset + 2;
            budget.append_render_str(&mut declarations, &body[cursor..end])?;
            cursor = end;
            continue;
        }

        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(body, cursor) else {
            budget.append_render_str(&mut declarations, &body[cursor..])?;
            break;
        };
        let raw_prelude = &body[cursor..delimiter];
        let prelude_end = raw_prelude.trim_end().len();
        let prelude = raw_prelude[..prelude_end].trim();
        let brace_spacing = &raw_prelude[prelude_end..];
        if delimiter_ch == ';' {
            budget.append_render_str(&mut declarations, prelude)?;
            budget.append_render_char(&mut declarations, ';')?;
            cursor = delimiter + 1;
            continue;
        }

        let Some(close) = find_matching_brace(body, delimiter) else {
            budget.append_render_str(&mut declarations, &body[cursor..])?;
            break;
        };
        if css_prelude_is_block_declaration(prelude) {
            let end = css_block_declaration_end(body, close);
            budget.append_render_str(&mut declarations, &body[cursor..end])?;
            cursor = end;
            continue;
        }

        let nested_body = &body[delimiter + 1..close];
        let mut block = String::new();
        if prelude.starts_with('@') {
            let rewritten_prelude = rewrite_at_rule_prelude(prelude, keyframes);
            let next_context = if is_keyframes_at_rule(prelude) {
                CssBlockContext::Keyframes
            } else {
                CssBlockContext::Deep
            };
            let nested_rewritten =
                rewrite_css_items(nested_body, scope_id, keyframes, next_context, budget)?;
            budget.append_render_str(&mut block, &rewritten_prelude)?;
            budget.append_render_str(&mut block, brace_spacing)?;
            budget.append_render_char(&mut block, '{')?;
            if css_block_contains_style_rules(&nested_rewritten)
                || css_block_contains_at_rule_with_style_rules(&nested_rewritten)
            {
                budget.append_render_char(&mut block, '\n')?;
                budget.append_render_str(&mut block, nested_rewritten.trim())?;
                budget.append_render_char(&mut block, '\n')?;
            } else {
                budget.append_render_str(&mut block, &nested_rewritten)?;
            }
            budget.append_render_char(&mut block, '}')?;
        } else {
            let selector = rewrite_deep_passthrough_selector(prelude);
            budget.append_render_str(&mut block, &selector)?;
            budget.append_render_str(&mut block, brace_spacing)?;
            budget.append_render_char(&mut block, '{')?;
            let nested_rewritten = rewrite_css_items(
                nested_body,
                scope_id,
                keyframes,
                CssBlockContext::Deep,
                budget,
            )?;
            budget.append_render_str(&mut block, &nested_rewritten)?;
            budget.append_render_char(&mut block, '}')?;
        }
        budget.claim_render_copy(block.len())?;
        nested_blocks.push(normalize_style_output(&block));
        cursor = close + 1;
    }

    let mut output = String::new();
    let declarations = rewrite_scoped_declaration_body(&declarations, keyframes, budget)?;
    let declarations = normalize_nested_scoped_declarations_with_budget(&declarations, budget)?;
    if !declarations.trim().is_empty() {
        budget.append_render_str(&mut output, "\n& {")?;
        budget.append_render_str(&mut output, &declarations)?;
        budget.append_render_str(&mut output, "\n}")?;
    }
    for block in nested_blocks {
        budget.append_render_char(&mut output, '\n')?;
        budget.append_render_str(&mut output, &block)?;
    }
    if !output.is_empty() {
        budget.append_render_char(&mut output, '\n')?;
    }
    Ok(output)
}

pub(crate) fn normalize_deep_passthrough_parent_anchor_blocks(source: &str) -> String {
    normalize_deep_passthrough_parent_anchor_blocks_inner(source).0
}

fn normalize_deep_passthrough_parent_anchor_blocks_inner(source: &str) -> (String, usize) {
    let mut output = String::new();
    let mut state = CssScannerState::Normal;
    let mut depth = 0usize;
    let mut index = 0usize;
    let mut cached_delimiter = None;
    let mut delimiter_scans = 0usize;
    while index < source.len() {
        if matches!(state, CssScannerState::Normal) && source[index..].starts_with("/*") {
            let Some(end_offset) = source[index + 2..].find("*/") else {
                output.push_str(&source[index..]);
                break;
            };
            let end = index + 2 + end_offset + 2;
            output.push_str(&source[index..end]);
            index = end;
            continue;
        }

        let ch = source[index..].chars().next().expect("valid char boundary");
        match state {
            CssScannerState::Normal => match ch {
                '\'' => state = CssScannerState::SingleQuote,
                '"' => state = CssScannerState::DoubleQuote,
                '{' => depth += 1,
                '}' => depth = depth.saturating_sub(1),
                '(' | ')' | '[' | ']' => cached_delimiter = None,
                '&' if depth == 0 => {
                    let delimiter = match cached_delimiter {
                        Some(Some((delimiter, delimiter_ch))) if delimiter >= index => {
                            Some((delimiter, delimiter_ch))
                        }
                        Some(None) => None,
                        _ => {
                            delimiter_scans += 1;
                            let delimiter = find_next_css_delimiter(source, index);
                            cached_delimiter = Some(delimiter);
                            delimiter
                        }
                    };
                    if matches!(delimiter, Some((_, '{'))) {
                        trim_trailing_horizontal_whitespace(&mut output);
                        if !output.is_empty() && !output.ends_with('\n') {
                            output.push('\n');
                        }
                    }
                }
                _ => {}
            },
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    output.push(ch);
                    index += ch.len_utf8();
                    if index < source.len() {
                        let escaped = source[index..].chars().next().expect("valid char boundary");
                        output.push(escaped);
                        index += escaped.len_utf8();
                    }
                    continue;
                }
                if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                if ch == '\\' {
                    output.push(ch);
                    index += ch.len_utf8();
                    if index < source.len() {
                        let escaped = source[index..].chars().next().expect("valid char boundary");
                        output.push(escaped);
                        index += escaped.len_utf8();
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {}
        }
        output.push(ch);
        index += ch.len_utf8();
        if matches!(cached_delimiter, Some(Some((delimiter, _))) if delimiter < index) {
            cached_delimiter = None;
        }
    }
    (output, delimiter_scans)
}

#[cfg(test)]
pub(crate) fn deep_passthrough_parent_anchor_delimiter_scans(source: &str) -> usize {
    normalize_deep_passthrough_parent_anchor_blocks_inner(source).1
}

pub(crate) fn trim_trailing_horizontal_whitespace(output: &mut String) {
    while output.ends_with([' ', '\t', '\r']) {
        output.pop();
    }
}

pub(crate) fn rewrite_deep_passthrough_selector(selector: &str) -> String {
    rewrite_scope_anchored_deep_container_branch(selector)
}

pub(crate) fn css_next_item_is_block(source: &str, cursor: usize) -> bool {
    find_next_css_delimiter(source, cursor).is_some_and(|(_, delimiter_ch)| delimiter_ch == '{')
}

pub(crate) fn css_next_item_is_at_rule_block(source: &str, cursor: usize) -> bool {
    let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(source, cursor) else {
        return false;
    };
    delimiter_ch == '{' && source[cursor..delimiter].trim().starts_with('@')
}

pub(crate) fn css_block_starts_with_block(body: &str) -> bool {
    let mut cursor = skip_css_whitespace(body, 0);
    if cursor >= body.len() {
        return false;
    }
    if body[cursor..].starts_with("/*") {
        let Some(end_offset) = body[cursor + 2..].find("*/") else {
            return false;
        };
        cursor += 2 + end_offset + 2;
        cursor = skip_css_whitespace(body, cursor);
    }
    if cursor >= body.len() {
        return false;
    }
    let Some((_delimiter, delimiter_ch)) = find_next_css_delimiter(body, cursor) else {
        return false;
    };
    delimiter_ch == '{'
}

pub(crate) fn css_block_starts_with_commented_block(body: &str) -> bool {
    let mut cursor = skip_css_whitespace(body, 0);
    if cursor >= body.len() || !body[cursor..].starts_with("/*") {
        return false;
    }
    let Some(end_offset) = body[cursor + 2..].find("*/") else {
        return false;
    };
    cursor += 2 + end_offset + 2;
    cursor = skip_css_whitespace(body, cursor);
    if cursor >= body.len() {
        return false;
    }
    matches!(find_next_css_delimiter(body, cursor), Some((_, '{')))
}

pub(crate) fn css_block_has_nested_block(body: &str) -> bool {
    let mut cursor = 0usize;
    while cursor < body.len() {
        cursor = skip_css_whitespace(body, cursor);
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end_offset) = body[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + end_offset + 2;
            continue;
        }
        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(body, cursor) else {
            break;
        };
        if delimiter_ch == ';' {
            cursor = delimiter + 1;
            continue;
        }
        let prelude = body[cursor..delimiter].trim();
        let Some(close) = find_matching_brace(body, delimiter) else {
            break;
        };
        if !css_prelude_is_block_declaration(prelude) {
            return true;
        }
        cursor = css_block_declaration_end(body, close);
    }
    false
}

pub(crate) fn css_block_has_direct_nested_rule(body: &str) -> bool {
    let mut cursor = 0usize;
    while cursor < body.len() {
        cursor = skip_css_whitespace(body, cursor);
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end_offset) = body[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + end_offset + 2;
            continue;
        }
        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(body, cursor) else {
            break;
        };
        if delimiter_ch == ';' {
            cursor = delimiter + 1;
            continue;
        }
        let prelude = body[cursor..delimiter].trim();
        if css_prelude_is_block_declaration(prelude) {
            let Some(close) = find_matching_brace(body, delimiter) else {
                break;
            };
            cursor = css_block_declaration_end(body, close);
            continue;
        }
        if !prelude.starts_with('@') {
            return true;
        }
        let Some(close) = find_matching_brace(body, delimiter) else {
            break;
        };
        cursor = close + 1;
    }
    false
}

pub(crate) fn rewrite_nested_scoped_rule_body(
    body: &str,
    scope_id: &str,
    keyframes: &BTreeMap<String, String>,
    wrap_declarations: bool,
    budget: &mut ScopedStyleBudget,
) -> Result<String, StylePreprocessError> {
    let mut declarations = String::new();
    let mut nested_blocks = Vec::new();
    let mut ordered_output = String::new();
    let mut trim_wrapped_declaration_semicolon = false;
    let mut cursor = 0usize;
    while cursor < body.len() {
        let whitespace_start = cursor;
        cursor = skip_css_whitespace(body, cursor);
        if cursor > whitespace_start {
            if wrap_declarations {
                budget.append_render_str(&mut declarations, &body[whitespace_start..cursor])?;
            } else {
                append_scoped_normalized_whitespace(
                    &mut declarations,
                    &body[whitespace_start..cursor],
                    budget,
                )?;
            }
        }
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end_offset) = body[cursor + 2..].find("*/") else {
                budget.append_render_str(&mut declarations, &body[cursor..])?;
                break;
            };
            let end = cursor + 2 + end_offset + 2;
            budget.append_render_str(&mut declarations, &body[cursor..end])?;
            cursor = end;
            continue;
        }

        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(body, cursor) else {
            budget.append_render_str(&mut declarations, &body[cursor..])?;
            break;
        };
        let raw_prelude = &body[cursor..delimiter];
        let prelude_end = raw_prelude.trim_end().len();
        let prelude = raw_prelude[..prelude_end].trim();
        let brace_spacing = &raw_prelude[prelude_end..];
        if delimiter_ch == ';' {
            budget.append_render_str(&mut declarations, prelude)?;
            budget.append_render_char(&mut declarations, ';')?;
            cursor = delimiter + 1;
            continue;
        }

        let Some(close) = find_matching_brace(body, delimiter) else {
            budget.append_render_str(&mut declarations, &body[cursor..])?;
            break;
        };
        if css_prelude_is_block_declaration(prelude) {
            let end = css_block_declaration_end(body, close);
            budget.append_render_str(&mut declarations, &body[cursor..end])?;
            cursor = end;
            continue;
        }
        let nested_body = &body[delimiter + 1..close];
        if !wrap_declarations {
            flush_scoped_nested_declarations(
                &mut ordered_output,
                &mut declarations,
                keyframes,
                true,
                budget,
            )?;
        } else if nested_blocks.is_empty() {
            trim_wrapped_declaration_semicolon =
                nested_block_trims_previous_declaration_semicolon(prelude, nested_body);
        }
        if prelude.starts_with('@') {
            let rewritten_prelude = rewrite_at_rule_prelude(prelude, keyframes);
            let next_context = if is_keyframes_at_rule(prelude) {
                CssBlockContext::Keyframes
            } else {
                CssBlockContext::Container
            };
            let nested_rewritten =
                if wrap_declarations && next_context == CssBlockContext::Container {
                    rewrite_nested_scoped_rule_body(nested_body, scope_id, keyframes, true, budget)?
                } else {
                    rewrite_css_items(nested_body, scope_id, keyframes, next_context, budget)?
                };
            let mut block = String::new();
            budget.append_render_str(&mut block, &rewritten_prelude)?;
            budget.append_render_str(&mut block, brace_spacing)?;
            budget.append_render_char(&mut block, '{')?;
            if css_block_contains_style_rules(&nested_rewritten)
                || css_block_contains_at_rule_with_style_rules(&nested_rewritten)
            {
                budget.append_render_char(&mut block, '\n')?;
                budget.append_render_str(&mut block, nested_rewritten.trim())?;
                budget.append_render_char(&mut block, '\n')?;
            } else {
                budget.append_render_str(&mut block, &nested_rewritten)?;
            }
            budget.append_render_char(&mut block, '}')?;
            budget.claim_render_copy(block.len())?;
            let block = normalize_style_output(&block);
            if wrap_declarations {
                nested_blocks.push(block);
            } else {
                if !ordered_output.is_empty() && !ordered_output.ends_with('\n') {
                    budget.append_render_char(&mut ordered_output, '\n')?;
                }
                budget.append_render_str(&mut ordered_output, &block)?;
            }
        } else {
            let has_nested_block = css_block_has_nested_block(nested_body);
            let has_direct_nested_rule =
                has_nested_block && css_block_has_direct_nested_rule(nested_body);
            let mut block = String::new();
            if has_direct_nested_rule {
                budget.append_render_str(&mut block, prelude)?;
            } else {
                let selector = rewrite_selector_list(prelude, scope_id);
                budget.append_render_str(&mut block, &selector)?;
            }
            budget.append_render_str(&mut block, brace_spacing)?;
            budget.append_render_char(&mut block, '{')?;
            if has_nested_block {
                let rewritten = rewrite_nested_scoped_rule_body(
                    nested_body,
                    scope_id,
                    keyframes,
                    has_direct_nested_rule,
                    budget,
                )?;
                budget.append_render_str(&mut block, &rewritten)?;
            } else {
                let rewritten = rewrite_scoped_declaration_body(nested_body, keyframes, budget)?;
                budget.append_render_str(&mut block, &rewritten)?;
            }
            budget.append_render_char(&mut block, '}')?;
            budget.claim_render_copy(block.len())?;
            let block = normalize_style_output(&block);
            if wrap_declarations {
                nested_blocks.push(block);
            } else {
                if !ordered_output.is_empty() && !ordered_output.ends_with('\n') {
                    budget.append_render_char(&mut ordered_output, '\n')?;
                }
                budget.append_render_str(&mut ordered_output, &block)?;
            }
        }
        cursor = close + 1;
    }

    let mut output = String::new();
    if wrap_declarations {
        if let Some(declarations) = format_scoped_nested_declarations(
            &declarations,
            keyframes,
            trim_wrapped_declaration_semicolon,
            budget,
        )? {
            budget.append_render_str(&mut output, "\n&[")?;
            budget.append_render_str(&mut output, scope_id)?;
            budget.append_render_str(&mut output, "] {")?;
            budget.append_render_str(&mut output, &declarations)?;
            budget.append_render_str(&mut output, "\n}")?;
        }
        for block in nested_blocks {
            budget.append_render_char(&mut output, '\n')?;
            budget.append_render_str(&mut output, &block)?;
        }
    } else {
        flush_scoped_nested_declarations(
            &mut ordered_output,
            &mut declarations,
            keyframes,
            false,
            budget,
        )?;
        budget.append_render_str(&mut output, &ordered_output)?;
    }
    if !output.is_empty() {
        budget.append_render_char(&mut output, '\n')?;
    }
    Ok(output)
}

pub(crate) fn flush_scoped_nested_declarations(
    output: &mut String,
    declarations: &mut String,
    keyframes: &BTreeMap<String, String>,
    separate_before_next_block: bool,
    budget: &mut ScopedStyleBudget,
) -> Result<(), StylePreprocessError> {
    if declarations.is_empty() {
        return Ok(());
    }
    let rewritten = rewrite_scoped_declaration_body(declarations, keyframes, budget)?;
    budget.append_render_str(output, rewritten.trim_end())?;
    if separate_before_next_block && !output.ends_with('\n') {
        budget.append_render_char(output, '\n')?;
    }
    declarations.clear();
    Ok(())
}

pub(crate) fn rewrite_scoped_declaration_body(
    body: &str,
    keyframes: &BTreeMap<String, String>,
    budget: &mut ScopedStyleBudget,
) -> Result<String, StylePreprocessError> {
    budget.claim_render_copy(body.len())?;
    Ok(rewrite_animation_declarations(body, keyframes))
}

pub(crate) fn format_scoped_nested_declarations(
    declarations: &str,
    keyframes: &BTreeMap<String, String>,
    trim_last_semicolon: bool,
    budget: &mut ScopedStyleBudget,
) -> Result<Option<String>, StylePreprocessError> {
    let mut declarations = rewrite_scoped_declaration_body(declarations, keyframes, budget)?;
    if trim_last_semicolon {
        budget.claim_render_copy(declarations.len().saturating_sub(1))?;
        declarations = remove_last_top_level_semicolon(&declarations);
    }
    if declarations.trim().is_empty() {
        return Ok(None);
    }
    if declarations.contains('\n') || declarations.contains('\r') {
        let trimmed = declarations.trim_end();
        budget.claim_render_copy(trimmed.len())?;
        Ok(Some(trimmed.to_string()))
    } else {
        normalize_nested_scoped_declarations_with_budget(&declarations, budget).map(Some)
    }
}

pub(crate) fn nested_block_trims_previous_declaration_semicolon(prelude: &str, body: &str) -> bool {
    if prelude.starts_with('@') {
        if is_keyframes_at_rule(prelude) {
            return false;
        }
        return first_nested_style_rule_has_terminal_semicolon(body).is_some_and(|has| !has);
    }
    top_level_declarations_have_terminal_semicolon(body).is_some_and(|has| !has)
}

pub(crate) fn first_nested_style_rule_has_terminal_semicolon(body: &str) -> Option<bool> {
    let mut cursor = 0usize;
    while cursor < body.len() {
        cursor = skip_css_whitespace(body, cursor);
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end_offset) = body[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + end_offset + 2;
            continue;
        }
        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(body, cursor) else {
            break;
        };
        if delimiter_ch == ';' {
            cursor = delimiter + 1;
            continue;
        }
        let prelude = body[cursor..delimiter].trim();
        let Some(close) = find_matching_brace(body, delimiter) else {
            break;
        };
        if css_prelude_is_block_declaration(prelude) {
            cursor = css_block_declaration_end(body, close);
            continue;
        }
        if prelude.starts_with('@') {
            if let Some(has_semicolon) =
                first_nested_style_rule_has_terminal_semicolon(&body[delimiter + 1..close])
            {
                return Some(has_semicolon);
            }
        } else {
            return top_level_declarations_have_terminal_semicolon(&body[delimiter + 1..close]);
        }
        cursor = close + 1;
    }
    None
}

pub(crate) fn top_level_declarations_have_terminal_semicolon(body: &str) -> Option<bool> {
    let mut cursor = 0usize;
    let mut terminal_semicolon = None;
    while cursor < body.len() {
        let whitespace_start = cursor;
        cursor = skip_css_whitespace(body, cursor);
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end_offset) = body[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + end_offset + 2;
            continue;
        }
        if cursor > whitespace_start && terminal_semicolon.is_none() {
            terminal_semicolon = None;
        }
        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(body, cursor) else {
            if !body[cursor..].trim().is_empty() {
                terminal_semicolon = Some(false);
            }
            break;
        };
        if delimiter_ch == ';' {
            terminal_semicolon = Some(true);
            cursor = delimiter + 1;
            continue;
        }
        let prelude = body[cursor..delimiter].trim();
        let Some(close) = find_matching_brace(body, delimiter) else {
            break;
        };
        if css_prelude_is_block_declaration(prelude) {
            let end = css_block_declaration_end(body, close);
            terminal_semicolon = Some(end > close + 1);
            cursor = end;
            continue;
        }
        break;
    }
    terminal_semicolon
}

pub(crate) fn remove_last_top_level_semicolon(source: &str) -> String {
    let Some(index) = last_top_level_semicolon(source) else {
        return source.to_string();
    };
    let mut output = String::with_capacity(source.len().saturating_sub(1));
    output.push_str(&source[..index]);
    output.push_str(&source[index + 1..]);
    output
}

pub(crate) fn last_top_level_semicolon(source: &str) -> Option<usize> {
    let mut state = CssScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut last = None;
    let mut index = 0usize;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match state {
            CssScannerState::Normal => {
                if source[index..].starts_with("/*") {
                    state = CssScannerState::BlockComment;
                    index += 2;
                    continue;
                }
                match ch {
                    '\'' => state = CssScannerState::SingleQuote,
                    '"' => state = CssScannerState::DoubleQuote,
                    '(' => paren_depth += 1,
                    ')' if paren_depth > 0 => paren_depth -= 1,
                    '[' => bracket_depth += 1,
                    ']' if bracket_depth > 0 => bracket_depth -= 1,
                    ';' if paren_depth == 0 && bracket_depth == 0 => last = Some(index),
                    _ => {}
                }
            }
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next()?.len_utf8();
                    }
                    continue;
                }
                if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next()?.len_utf8();
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {
                if source[index..].starts_with("*/") {
                    state = CssScannerState::Normal;
                    index += 2;
                    continue;
                }
            }
        }
        index += ch.len_utf8();
    }
    last
}

fn normalize_nested_scoped_declarations_with_budget(
    declarations: &str,
    budget: &mut ScopedStyleBudget,
) -> Result<String, StylePreprocessError> {
    let trimmed = declarations.trim();
    budget.claim_render_copy(trimmed.len())?;
    let collapsed = collapse_css_whitespace_outside_strings(trimmed);
    if collapsed.is_empty() {
        return Ok(String::new());
    }
    let output_bytes = collapsed
        .len()
        .checked_add(1)
        .ok_or_else(|| StylePreprocessError::scoped_limit("scoped style output size overflowed"))?;
    budget.claim_render_copy(output_bytes)?;
    Ok(format!(" {collapsed}"))
}

pub(crate) fn collapse_css_whitespace_outside_strings(source: &str) -> String {
    let mut output = String::new();
    let mut state = CssScannerState::Normal;
    let mut pending_space = false;
    let mut index = 0usize;
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        match state {
            CssScannerState::Normal => {
                if source[index..].starts_with("/*") {
                    if pending_space && !output.is_empty() {
                        output.push(' ');
                    }
                    pending_space = false;
                    if let Some(end_offset) = source[index + 2..].find("*/") {
                        let end = index + 2 + end_offset + 2;
                        output.push_str(&source[index..end]);
                        index = end;
                        continue;
                    }
                }
                match ch {
                    '\'' => {
                        if pending_space && !output.is_empty() {
                            output.push(' ');
                        }
                        pending_space = false;
                        output.push(ch);
                        state = CssScannerState::SingleQuote;
                    }
                    '"' => {
                        if pending_space && !output.is_empty() {
                            output.push(' ');
                        }
                        pending_space = false;
                        output.push(ch);
                        state = CssScannerState::DoubleQuote;
                    }
                    _ if ch.is_whitespace() => pending_space = true,
                    _ => {
                        if pending_space && !output.is_empty() {
                            output.push(' ');
                        }
                        pending_space = false;
                        output.push(ch);
                    }
                }
            }
            CssScannerState::SingleQuote => {
                output.push(ch);
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        if let Some(next) = source[index..].chars().next() {
                            output.push(next);
                            index += next.len_utf8();
                            continue;
                        }
                    }
                } else if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                output.push(ch);
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        if let Some(next) = source[index..].chars().next() {
                            output.push(next);
                            index += next.len_utf8();
                            continue;
                        }
                    }
                } else if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {}
        }
        index += ch.len_utf8();
    }
    output
}

pub(crate) fn css_block_contains_style_rules(body: &str) -> bool {
    let mut cursor = 0usize;
    while cursor < body.len() {
        cursor = skip_css_whitespace(body, cursor);
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end_offset) = body[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + end_offset + 2;
            continue;
        }
        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(body, cursor) else {
            break;
        };
        if delimiter_ch == ';' {
            cursor = delimiter + 1;
            continue;
        }
        let prelude = body[cursor..delimiter].trim();
        if !prelude.starts_with('@') && !css_prelude_is_block_declaration(prelude) {
            return true;
        }
        let Some(close) = find_matching_brace(body, delimiter) else {
            break;
        };
        if prelude.starts_with('@') && css_block_contains_style_rules(&body[delimiter + 1..close]) {
            return true;
        }
        cursor = css_block_declaration_end(body, close);
    }
    false
}

pub(crate) fn css_block_contains_at_rule_with_style_rules(body: &str) -> bool {
    let mut cursor = 0usize;
    while cursor < body.len() {
        cursor = skip_css_whitespace(body, cursor);
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end_offset) = body[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + end_offset + 2;
            continue;
        }
        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(body, cursor) else {
            break;
        };
        if delimiter_ch == ';' {
            cursor = delimiter + 1;
            continue;
        }
        let prelude = body[cursor..delimiter].trim();
        let Some(close) = find_matching_brace(body, delimiter) else {
            break;
        };
        if prelude.starts_with('@') && css_block_contains_style_rules(&body[delimiter + 1..close]) {
            return true;
        }
        cursor = css_block_declaration_end(body, close);
    }
    false
}

pub(crate) fn css_prelude_is_block_declaration(prelude: &str) -> bool {
    let prelude = prelude.trim();
    let Some(name) = prelude.strip_suffix(':') else {
        return false;
    };
    is_style_property_name(name.trim())
}

pub(crate) fn css_block_declaration_end(body: &str, close: usize) -> usize {
    let after_close = close + 1;
    if after_close < body.len() && body[after_close..].starts_with(';') {
        after_close + 1
    } else {
        after_close
    }
}
