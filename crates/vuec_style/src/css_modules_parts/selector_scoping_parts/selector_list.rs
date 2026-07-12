pub(crate) fn rewrite_selector_list(selector: &str, scope_id: &str) -> String {
    rewrite_selector_list_for_rule(selector, scope_id, false, false).selector
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SelectorRewriteResult {
    pub(crate) selector: String,
    pub(crate) deep_passthrough: bool,
}

pub(crate) fn rewrite_selector_list_for_rule(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> SelectorRewriteResult {
    let mut deep_passthrough = false;
    let parts = split_selector_list(selector);
    let mut rewritten = String::new();
    for (index, part) in parts.into_iter().enumerate() {
        let trimmed = part.trim();
        let result = rewrite_single_selector_for_rule(
            trimmed,
            scope_id,
            rule_has_nested_block,
            rule_has_direct_nested_rule,
        );
        deep_passthrough |= result.deep_passthrough;

        if index > 0 {
            rewritten.push(',');
            if selector_list_branch_preserves_leading_whitespace(trimmed, &result.selector) {
                rewritten.push_str(selector_leading_whitespace(part));
            }
        }
        rewritten.push_str(&result.selector);
    }
    let selector = if selector.ends_with(' ') {
        format!("{rewritten} ")
    } else {
        rewritten
    };
    SelectorRewriteResult {
        selector,
        deep_passthrough,
    }
}

pub(crate) fn selector_list_branch_preserves_leading_whitespace(
    original: &str,
    rewritten: &str,
) -> bool {
    if original.is_empty() || rewritten.starts_with('[') || original.starts_with('*') {
        return false;
    }
    if original.starts_with(">>>") || original.starts_with("/deep/") {
        return false;
    }
    match_selector_pseudo_function(
        original,
        0,
        &[
            ":global",
            "::v-global",
            ":slotted",
            "::v-slotted",
            ":deep",
            "::v-deep",
        ],
    )
    .is_none()
}

pub(crate) fn selector_leading_whitespace(selector: &str) -> &str {
    let end = selector
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_whitespace()).then_some(index))
        .unwrap_or(selector.len());
    &selector[..end]
}

pub(crate) fn rewrite_selector_branches(selector: &str, scope_id: &str) -> String {
    let parts = split_selector_list(selector);
    let mut rewritten = String::new();
    for (index, part) in parts.into_iter().enumerate() {
        let trimmed = part.trim();
        let branch = rewrite_single_selector(trimmed, scope_id);
        if index > 0 {
            rewritten.push(',');
            if selector_list_branch_preserves_leading_whitespace(trimmed, &branch) {
                rewritten.push_str(selector_leading_whitespace(part));
            }
        }
        rewritten.push_str(&branch);
    }
    rewritten
}

pub(crate) fn rewrite_direct_nested_parent_selector(selector: &str) -> String {
    if !direct_nested_parent_selector_needs_rewrite(selector) {
        return selector.to_string();
    }
    let parts = split_selector_list(selector);
    let mut rewritten = String::new();
    for (index, part) in parts.into_iter().enumerate() {
        let trimmed = part.trim();
        let branch = rewrite_direct_nested_parent_selector_branch(trimmed);
        if index > 0 {
            rewritten.push(',');
            if selector_list_branch_preserves_leading_whitespace(trimmed, &branch) {
                rewritten.push_str(selector_leading_whitespace(part));
            }
        }
        rewritten.push_str(&branch);
    }
    rewritten
}

pub(crate) fn direct_nested_parent_selector_needs_rewrite(selector: &str) -> bool {
    split_selector_list(selector)
        .into_iter()
        .any(|part| direct_nested_parent_selector_branch_needs_rewrite(part.trim()))
}

pub(crate) fn direct_nested_parent_selector_branch_needs_rewrite(selector: &str) -> bool {
    let normalized_selector = normalize_selector_comments(selector);
    let selector = normalized_selector.trim();
    if selector.is_empty() {
        return false;
    }
    let stripped = strip_leading_universal_selector(selector);
    if stripped != selector {
        return true;
    }
    if rewrite_scope_anchored_deep_container_branch(selector) != selector {
        return true;
    }
    direct_nested_parent_container_selector_needs_rewrite(selector)
}

pub(crate) fn rewrite_direct_nested_parent_selector_branch(selector: &str) -> String {
    let normalized_selector = normalize_selector_comments(selector);
    let selector = strip_leading_universal_selector(normalized_selector.trim());
    let selector = rewrite_scope_anchored_deep_container_branch(selector);
    rewrite_direct_nested_parent_container_selector(&selector).unwrap_or(selector)
}

pub(crate) fn direct_nested_parent_container_selector_needs_rewrite(selector: &str) -> bool {
    let Some(target) = scoped_container_injection_target(selector) else {
        return false;
    };
    let Some((open, close)) = target.parens else {
        return false;
    };
    let Some(name) = matched_selector_name(selector, target.start, &[":is", ":where"]) else {
        return false;
    };
    let suffix = &selector[close + 1..];
    if !suffix.trim().is_empty() && !selector_suffix_is_pseudo_only(suffix) {
        return false;
    }
    matches!(name, ":is" | ":where")
        && direct_nested_parent_selector_needs_rewrite(&selector[open + 1..close])
}

pub(crate) fn rewrite_direct_nested_parent_container_selector(selector: &str) -> Option<String> {
    let target = scoped_container_injection_target(selector)?;
    let (open, close) = target.parens?;
    let name = matched_selector_name(selector, target.start, &[":is", ":where"])?;
    let suffix = &selector[close + 1..];
    if !suffix.trim().is_empty() && !selector_suffix_is_pseudo_only(suffix) {
        return None;
    }
    let rewritten_inner = rewrite_direct_nested_parent_selector(&selector[open + 1..close]);
    Some(format!(
        "{}{name}({rewritten_inner}){suffix}",
        &selector[..target.start]
    ))
}

pub(crate) fn rewrite_slotted_inner_selector(selector: &str, scope_id: &str) -> String {
    rewrite_scoped_container_injection_target_with(selector, scope_id, rewrite_selector_branches)
        .unwrap_or_else(|| inject_scope_attribute(selector, scope_id))
}
