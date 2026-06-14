pub(crate) fn rewrite_deep_container_selector_for_rule(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> Option<SelectorRewriteResult> {
    let names = [":is", ":where", ":not", ":has"];
    let container = find_top_level_pseudo_function(selector, &names)?;
    let (open, close) = container.parens?;
    let inner = &selector[open + 1..close];
    if !selector_has_deep(inner) {
        return None;
    }

    let name = matched_selector_name(selector, container.start, &names)?;
    let prefix = &selector[..container.start];
    let suffix = &selector[close + 1..];
    let has_scope_anchor = selector_scope_anchor_before(selector, container.start);
    let has_trailing_nodes = !suffix.trim().is_empty();
    let branches = split_selector_list(inner)
        .into_iter()
        .map(str::trim)
        .collect::<Vec<_>>();
    let has_deep = branches.iter().any(|branch| selector_has_deep(branch));
    let has_normal = branches.iter().any(|branch| !selector_has_deep(branch));
    let first_branch_has_deep = branches
        .first()
        .is_some_and(|branch| selector_has_deep(branch));
    let can_split = matches!(name, ":is" | ":where" | ":has");
    let should_split =
        can_split && has_deep && has_normal && !has_scope_anchor && has_trailing_nodes;

    if name == ":not" && has_deep && has_normal && !has_scope_anchor && has_trailing_nodes {
        return None;
    }

    if rule_has_direct_nested_rule
        && has_deep
        && !first_branch_has_deep
        && (!has_trailing_nodes || has_scope_anchor)
    {
        let rewritten_inner = rewrite_direct_nested_first_normal_deep_container_inner_branches(
            inner,
            scope_id,
            rule_has_nested_block,
            rule_has_direct_nested_rule,
        );
        let mut rewritten = format!("{prefix}{name}({rewritten_inner}){suffix}");
        if has_scope_anchor {
            rewritten = inject_scope_before_container(&rewritten, container.start, scope_id);
        } else {
            rewritten = inject_scope_after_container_pseudo(&rewritten, name, scope_id);
        }
        return Some(SelectorRewriteResult {
            selector: rewritten,
            deep_passthrough: true,
        });
    }

    if should_split {
        let mut deep_passthrough = false;
        let mut selector = String::new();
        let first_normal_direct_nested =
            rule_has_direct_nested_rule && has_deep && !first_branch_has_deep;
        let mut seen_deep = false;
        for (index, part) in split_selector_list(inner).into_iter().enumerate() {
            let branch = part.trim();
            let branch_has_deep = selector_has_deep(branch);
            let branch_is_first_deep = first_normal_direct_nested && !seen_deep && branch_has_deep;
            let branch_before_first_deep =
                first_normal_direct_nested && !seen_deep && !branch_has_deep;
            if branch_has_deep {
                seen_deep = true;
            }
            let rewritten = if branch_before_first_deep {
                let branch = rewrite_direct_nested_first_normal_split_branch(branch, name, suffix);
                format!("{prefix}{name}({branch}){suffix}")
            } else if branch_has_deep {
                let result = rewrite_single_selector_branch(
                    branch,
                    scope_id,
                    rule_has_nested_block,
                    rule_has_direct_nested_rule,
                );
                deep_passthrough = true;
                let mut rewritten = format!("{prefix}{name}({}){suffix}", result.selector);
                if rule_has_direct_nested_rule {
                    rewritten = inject_scope_after_container_pseudo(&rewritten, name, scope_id);
                }
                rewritten
            } else if matches!(name, ":is" | ":where") && selector_suffix_is_pseudo_only(suffix) {
                let result = if rule_has_direct_nested_rule {
                    rewrite_direct_nested_deep_container_branch(
                        branch,
                        scope_id,
                        rule_has_nested_block,
                        rule_has_direct_nested_rule,
                    )
                } else {
                    rewrite_single_selector_branch(
                        branch,
                        scope_id,
                        rule_has_nested_block,
                        rule_has_direct_nested_rule,
                    )
                };
                format!("{prefix}{name}({}){suffix}", result.selector)
            } else {
                let branch_selector = format!("{prefix}{name}({branch}){suffix}");
                inject_scope_attribute(&branch_selector, scope_id)
            };
            if index > 0 {
                selector.push(',');
                if matches!(name, ":is" | ":where") {
                    selector.push(' ');
                } else if name == ":has" && !selector_suffix_is_pseudo_only(suffix) {
                    selector.push(' ');
                } else if name == ":has"
                    && selector_suffix_is_pseudo_only(suffix)
                    && branch_has_deep
                {
                    selector.push(' ');
                } else if branch_is_first_deep {
                    selector.push(' ');
                } else {
                    let preserve_leading = if matches!(name, ":is" | ":where")
                        && selector_suffix_is_pseudo_only(suffix)
                        && !branch_has_deep
                    {
                        !branch.is_empty() && !rewritten.starts_with('[')
                    } else {
                        selector_list_branch_preserves_leading_whitespace(branch, &rewritten)
                    };
                    if preserve_leading {
                        selector.push_str(selector_leading_whitespace(part));
                    }
                }
            }
            selector.push_str(&rewritten);
        }
        return Some(SelectorRewriteResult {
            selector,
            deep_passthrough,
        });
    }

    if has_scope_anchor && !rule_has_direct_nested_rule {
        let rewritten_inner = rewrite_scope_anchored_deep_container_inner_branches(inner);
        let rewritten = format!("{prefix}{name}({rewritten_inner}){suffix}");
        return Some(SelectorRewriteResult {
            selector: inject_scope_before_container(&rewritten, container.start, scope_id),
            deep_passthrough: true,
        });
    }

    let (rewritten_inner, deep_passthrough) = rewrite_scoped_deep_container_inner_branches(
        inner,
        scope_id,
        rule_has_nested_block,
        rule_has_direct_nested_rule,
    );
    let mut rewritten = format!("{prefix}{name}({rewritten_inner}){suffix}");
    if has_scope_anchor {
        rewritten = inject_scope_before_container(&rewritten, container.start, scope_id);
    } else if rule_has_direct_nested_rule && deep_passthrough {
        rewritten = inject_scope_after_container_pseudo(&rewritten, name, scope_id);
    }
    Some(SelectorRewriteResult {
        selector: rewritten,
        deep_passthrough,
    })
}

pub(crate) fn rewrite_scoped_deep_container_inner_branches(
    inner: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> (String, bool) {
    let mut rewritten = String::new();
    let mut deep_passthrough = false;
    for (index, part) in split_selector_list(inner).into_iter().enumerate() {
        let trimmed = part.trim();
        if selector_has_deep(trimmed) {
            deep_passthrough = true;
        }
        let result = if rule_has_direct_nested_rule {
            rewrite_direct_nested_deep_container_branch(
                trimmed,
                scope_id,
                rule_has_nested_block,
                rule_has_direct_nested_rule,
            )
        } else {
            rewrite_single_selector_branch(
                trimmed,
                scope_id,
                rule_has_nested_block,
                rule_has_direct_nested_rule,
            )
        };
        if index > 0 {
            rewritten.push(',');
            if selector_list_branch_preserves_leading_whitespace(trimmed, &result.selector) {
                rewritten.push_str(selector_leading_whitespace(part));
            }
        }
        rewritten.push_str(&result.selector);
    }
    (rewritten, deep_passthrough)
}

pub(crate) fn rewrite_direct_nested_first_normal_split_branch(
    branch: &str,
    name: &str,
    suffix: &str,
) -> String {
    if matches!(name, ":is" | ":where") && selector_suffix_is_pseudo_only(suffix) {
        rewrite_direct_nested_parent_selector_branch(branch)
    } else {
        branch.to_string()
    }
}

pub(crate) fn rewrite_direct_nested_first_normal_deep_container_inner_branches(
    inner: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> String {
    let mut rewritten = String::new();
    let mut seen_deep = false;
    for (index, part) in split_selector_list(inner).into_iter().enumerate() {
        let trimmed = part.trim();
        let branch_has_deep = selector_has_deep(trimmed);
        if branch_has_deep {
            seen_deep = true;
        }
        let branch = if seen_deep {
            rewrite_direct_nested_deep_container_branch(
                trimmed,
                scope_id,
                rule_has_nested_block,
                rule_has_direct_nested_rule,
            )
            .selector
        } else {
            rewrite_direct_nested_parent_selector_branch(trimmed)
        };
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

pub(crate) fn rewrite_direct_nested_deep_container_branch(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> SelectorRewriteResult {
    if let Some(slotted) = rewrite_slotted_selector_with_prefix_scope(selector, scope_id) {
        return SelectorRewriteResult {
            selector: slotted,
            deep_passthrough: false,
        };
    }
    rewrite_single_selector_branch(
        selector,
        scope_id,
        rule_has_nested_block,
        rule_has_direct_nested_rule,
    )
}

pub(crate) fn deep_container_direct_nested_wraps_parent_declarations(selector: &str) -> bool {
    let names = [":is", ":where", ":not", ":has"];
    let Some(container) = find_top_level_pseudo_function(selector, &names) else {
        return false;
    };
    let Some((open, close)) = container.parens else {
        return false;
    };
    let inner = &selector[open + 1..close];
    if !selector_has_deep(inner) {
        return false;
    }
    split_selector_list(inner)
        .first()
        .is_some_and(|branch| !selector_has_deep(branch.trim()))
}

pub(crate) fn rewrite_scope_anchored_deep_container_inner_branches(inner: &str) -> String {
    let mut rewritten = String::new();
    for (index, part) in split_selector_list(inner).into_iter().enumerate() {
        let trimmed = part.trim();
        let branch = rewrite_scope_anchored_deep_container_branch(trimmed);
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

pub(crate) fn rewrite_scope_anchored_deep_container_branch(selector: &str) -> String {
    let names = [
        ":global",
        "::v-global",
        ":slotted",
        "::v-slotted",
        ":deep",
        "::v-deep",
    ];
    let Some(special) = find_top_level_pseudo_function(selector, &names) else {
        return selector.to_string();
    };
    let Some((open, close)) = special.parens else {
        return selector.to_string();
    };
    let name = matched_selector_name(selector, special.start, &names).unwrap_or_default();
    let inner = first_selector_branch(selector[open + 1..close].trim()).trim();
    if matches!(name, ":global" | "::v-global") {
        return inner.to_string();
    }
    if matches!(name, ":slotted" | "::v-slotted") {
        return replace_slotted_pseudo_without_scope(selector, special, inner);
    }
    replace_deep_pseudo_without_scope(selector, special, inner)
}

pub(crate) fn selector_suffix_is_pseudo_only(suffix: &str) -> bool {
    let suffix = suffix.trim();
    if suffix.is_empty() {
        return false;
    }
    let mut index = 0usize;
    while index < suffix.len() {
        let Some(ch) = suffix[index..].chars().next() else {
            break;
        };
        if ch != ':' {
            return false;
        }
        index += ch.len_utf8();
        if suffix[index..].starts_with(':') {
            index += ':'.len_utf8();
        }
        let name_start = index;
        while index < suffix.len() {
            let Some(ch) = suffix[index..].chars().next() else {
                break;
            };
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                index += ch.len_utf8();
            } else {
                break;
            }
        }
        if index == name_start {
            return false;
        }
        let open = skip_selector_whitespace(suffix, index);
        if suffix[open..].starts_with('(') {
            let Some(close) = find_matching_selector_paren(suffix, open) else {
                return false;
            };
            index = close + 1;
        }
    }
    index == suffix.len()
}
