pub(crate) fn rewrite_scoped_container_injection_target_with(
    selector: &str,
    scope_id: &str,
    rewrite_branches: fn(&str, &str) -> String,
) -> Option<String> {
    let selector = strip_leading_universal_selector(selector.trim());
    let target = scoped_container_injection_target(selector)?;
    let (open, close) = target.parens?;
    let name = matched_selector_name(selector, target.start, &[":is", ":where"])?;
    let rewritten_inner = rewrite_branches(&selector[open + 1..close], scope_id);

    Some(format!(
        "{}{name}({rewritten_inner}){}",
        &selector[..target.start],
        &selector[close + 1..]
    ))
}

pub(crate) fn rewrite_single_selector(selector: &str, scope_id: &str) -> String {
    rewrite_single_selector_for_rule(selector, scope_id, false, false).selector
}

pub(crate) fn rewrite_single_selector_for_rule(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> SelectorRewriteResult {
    rewrite_single_selector_with_options(
        selector,
        scope_id,
        rule_has_nested_block,
        rule_has_direct_nested_rule,
        false,
    )
}

pub(crate) fn rewrite_single_selector_branch(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> SelectorRewriteResult {
    rewrite_single_selector_with_options(
        selector,
        scope_id,
        rule_has_nested_block,
        rule_has_direct_nested_rule,
        true,
    )
}

pub(crate) fn rewrite_single_selector_with_options(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
    in_container_branch: bool,
) -> SelectorRewriteResult {
    let normalized_selector = normalize_selector_comments(selector);
    let selector = normalized_selector.trim();
    if selector.is_empty() {
        return SelectorRewriteResult {
            selector: selector.to_string(),
            deep_passthrough: false,
        };
    }
    if let Some(global) = find_top_level_pseudo_function(selector, &[":global", "::v-global"]) {
        if let Some((open, close)) = global.parens {
            return SelectorRewriteResult {
                selector: first_selector_branch(selector[open + 1..close].trim())
                    .trim()
                    .to_string(),
                deep_passthrough: false,
            };
        }
    }
    if let Some(deep) = find_deep_combinator(selector) {
        return SelectorRewriteResult {
            selector: rewrite_deep_selector(
                &selector[..deep.start],
                &selector[deep.end..],
                scope_id,
            ),
            deep_passthrough: false,
        };
    }
    if let Some(rewritten) = rewrite_deep_container_selector_for_rule(
        selector,
        scope_id,
        rule_has_nested_block,
        rule_has_direct_nested_rule,
    ) {
        return rewritten;
    }
    if let Some(deep) = find_top_level_pseudo_function(selector, &[":deep", "::v-deep"]) {
        if let Some((open, close)) = deep.parens {
            let mut rhs = first_selector_branch(selector[open + 1..close].trim())
                .trim()
                .to_string();
            rhs.push_str(&selector[close + 1..]);
            return SelectorRewriteResult {
                selector: rewrite_deep_selector(&selector[..deep.start], &rhs, scope_id),
                deep_passthrough: !in_container_branch,
            };
        }
        return SelectorRewriteResult {
            selector: rewrite_deep_selector(
                &selector[..deep.start],
                &selector[deep.end..],
                scope_id,
            ),
            deep_passthrough: !in_container_branch,
        };
    }
    if let Some(rewritten) = rewrite_slotted_selector(selector, scope_id) {
        return SelectorRewriteResult {
            selector: rewritten,
            deep_passthrough: false,
        };
    }
    if let Some(rewritten) = rewrite_scoped_container_injection_target(selector, scope_id) {
        return SelectorRewriteResult {
            selector: rewritten,
            deep_passthrough: false,
        };
    }
    SelectorRewriteResult {
        selector: inject_scope_attribute(selector, scope_id),
        deep_passthrough: false,
    }
}

pub(crate) fn rewrite_scoped_container_injection_target(
    selector: &str,
    scope_id: &str,
) -> Option<String> {
    rewrite_scoped_container_injection_target_with(selector, scope_id, rewrite_selector_branches)
}

pub(crate) fn scoped_container_injection_target(selector: &str) -> Option<SelectorMatch> {
    let mut state = SelectorScannerState::Normal;
    let mut target = None;
    let mut has_target = false;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    let end = consume_selector_token(selector, index);
                    target = None;
                    has_target = true;
                    index = end;
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                '&' if !has_target => {
                    has_target = true;
                    target = None;
                }
                '&' => {}
                '[' => {
                    let end = find_matching_selector_bracket(selector, index)
                        .unwrap_or(selector.len().saturating_sub(1));
                    target = None;
                    has_target = true;
                    index = end + 1;
                    continue;
                }
                ':' => {
                    if let Some(pseudo) =
                        match_selector_pseudo_function(selector, index, &[":is", ":where"])
                    {
                        if !has_target {
                            target = Some(pseudo);
                            has_target = true;
                        }
                        index = pseudo.end;
                        continue;
                    }
                    index = skip_selector_pseudo(selector, index);
                    continue;
                }
                '>' | '+' | '~' | ',' => {}
                '*' if !has_target => {
                    has_target = true;
                    target = None;
                }
                '*' => {}
                _ if ch.is_whitespace() => {}
                _ if is_selector_ident_start(ch) || ch == '.' || ch == '#' => {
                    let end = consume_selector_token(selector, index);
                    target = None;
                    has_target = true;
                    index = end;
                    continue;
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    target
}
