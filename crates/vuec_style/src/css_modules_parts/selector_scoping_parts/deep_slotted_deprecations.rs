pub(crate) fn replace_slotted_pseudo_without_scope(
    selector: &str,
    slotted: SelectorMatch,
    inner: &str,
) -> String {
    let Some((_, close)) = slotted.parens else {
        return selector.to_string();
    };
    let prefix = &selector[..slotted.start];
    let inner = strip_leading_universal_selector(inner);
    let inner = if prefix.is_empty() {
        inner.trim_start()
    } else {
        inner
    };
    format!("{prefix}{inner}{}", &selector[close + 1..])
}

pub(crate) fn replace_deep_pseudo_without_scope(
    selector: &str,
    deep: SelectorMatch,
    inner: &str,
) -> String {
    let Some((_, close)) = deep.parens else {
        return selector.to_string();
    };
    let mut suffix = String::new();
    suffix.push_str(inner);
    suffix.push_str(&selector[close + 1..]);
    let suffix = suffix.trim_start();
    let prefix = selector[..deep.start].trim_end();
    if suffix.is_empty() {
        prefix.to_string()
    } else if prefix.is_empty() {
        format!(" {suffix}")
    } else {
        format!("{prefix} {suffix}")
    }
}

pub(crate) fn selector_scope_anchor_before(selector: &str, end: usize) -> bool {
    let prefix = &selector[..end];
    selector_injection_index(prefix).is_some()
}

pub(crate) fn inject_scope_before_container(
    selector: &str,
    container_start: usize,
    scope_id: &str,
) -> String {
    let prefix = &selector[..container_start];
    let trimmed_prefix_end = prefix.trim_end().len();
    let trailing = &prefix[trimmed_prefix_end..];
    let scoped_prefix = inject_scope_attribute(&prefix[..trimmed_prefix_end], scope_id);
    format!("{scoped_prefix}{trailing}{}", &selector[container_start..])
}

pub(crate) fn selector_has_deep(selector: &str) -> bool {
    find_deep_combinator(selector).is_some()
        || find_pseudo_function(selector, &[":deep", "::v-deep"]).is_some()
}

pub(crate) fn selector_has_deep_pseudo(selector: &str) -> bool {
    find_pseudo_function(selector, &[":deep", "::v-deep"]).is_some()
}

pub(crate) fn collect_selector_list_deprecation_warnings(
    selector: &str,
    warnings: &mut Vec<String>,
) {
    for part in split_selector_list(selector) {
        collect_selector_deprecation_warnings(part.trim(), warnings);
    }
}

pub(crate) fn collect_selector_deprecation_warnings(selector: &str, warnings: &mut Vec<String>) {
    let mut state = SelectorScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut index = 0usize;
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    index = consume_selector_escape(selector, index);
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                '[' => bracket_depth += 1,
                ']' if bracket_depth > 0 => bracket_depth -= 1,
                '(' => paren_depth += 1,
                ')' if paren_depth > 0 => paren_depth -= 1,
                _ if bracket_depth == 0 && paren_depth == 0 => {
                    if selector[index..].starts_with(">>>")
                        || selector[index..].starts_with("/deep/")
                    {
                        warnings.push(DEPRECATED_DEEP_COMBINATOR_MESSAGE.to_string());
                        return;
                    }
                    if let Some(deep) =
                        match_selector_pseudo_function(selector, index, &[":deep", "::v-deep"])
                    {
                        if deep.parens.is_none() {
                            let value =
                                matched_selector_name(selector, deep.start, &[":deep", "::v-deep"])
                                    .unwrap_or(":deep");
                            warnings.push(deprecated_deep_pseudo_message(value));
                        }
                        return;
                    }
                    if match_selector_pseudo_function(selector, index, &[":global", "::v-global"])
                        .is_some()
                    {
                        return;
                    }
                    if let Some(slotted) = match_selector_pseudo_function(
                        selector,
                        index,
                        &[":slotted", "::v-slotted"],
                    ) {
                        if let Some((open, close)) = slotted.parens {
                            let inner = first_selector_branch(selector[open + 1..close].trim());
                            collect_selector_deprecation_warnings(inner.trim(), warnings);
                        }
                        return;
                    }
                    if let Some(container) = match_selector_pseudo_function(
                        selector,
                        index,
                        &[":is", ":where", ":not", ":has"],
                    ) {
                        if let Some((open, close)) = container.parens {
                            for branch in split_selector_list(&selector[open + 1..close]) {
                                let branch = branch.trim();
                                if selector_has_deep_pseudo(branch) {
                                    collect_selector_deprecation_warnings(branch, warnings);
                                }
                            }
                        }
                        index = container.end;
                        continue;
                    }
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
}

pub(crate) fn matched_selector_name<'a>(
    selector: &str,
    start: usize,
    names: &'a [&str],
) -> Option<&'a str> {
    names
        .iter()
        .copied()
        .find(|name| selector[start..].starts_with(name))
}

pub(crate) fn rewrite_slotted_selector(selector: &str, scope_id: &str) -> Option<String> {
    let slotted = find_top_level_pseudo_function(selector, &[":slotted", "::v-slotted"])?;
    let (open, close) = slotted.parens?;
    let inner = first_selector_branch(selector[open + 1..close].trim()).trim();
    let mut rewritten = String::new();
    let prefix = &selector[..slotted.start];
    rewritten.push_str(prefix);
    let trim_leading_combinator_space = prefix.is_empty();
    if inner.is_empty() {
        rewritten.push_str(&format!("[{scope_id}-s]"));
    } else {
        let scoped_inner = rewrite_slotted_inner_selector(inner, &format!("{scope_id}-s"));
        if trim_leading_combinator_space {
            rewritten.push_str(scoped_inner.trim_start());
        } else {
            rewritten.push_str(&scoped_inner);
        }
    }
    rewritten.push_str(&selector[close + 1..]);
    Some(rewritten)
}

pub(crate) fn rewrite_slotted_selector_with_prefix_scope(
    selector: &str,
    scope_id: &str,
) -> Option<String> {
    let slotted = find_top_level_pseudo_function(selector, &[":slotted", "::v-slotted"])?;
    let (open, close) = slotted.parens?;
    let inner = first_selector_branch(selector[open + 1..close].trim()).trim();
    let prefix = &selector[..slotted.start];
    let prefix_trimmed = prefix.trim_end();
    let prefix_spacing = &prefix[prefix_trimmed.len()..];
    let scoped_prefix = if prefix_trimmed.is_empty() {
        format!("[{scope_id}]")
    } else {
        inject_scope_attribute(prefix_trimmed, scope_id)
    };
    let slotted_scope = format!("{scope_id}-s");
    let scoped_inner = if inner.is_empty() {
        format!("[{slotted_scope}]")
    } else {
        rewrite_slotted_inner_selector(strip_leading_universal_selector(inner), &slotted_scope)
    };
    let inner = if prefix_trimmed.is_empty() {
        scoped_inner.trim_start()
    } else {
        scoped_inner.as_str()
    };
    Some(format!(
        "{scoped_prefix}{prefix_spacing}{inner}{}",
        &selector[close + 1..]
    ))
}
