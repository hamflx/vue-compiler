pub(crate) fn render_slot_outlet(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let name = element
        .props
        .iter()
        .find_map(|prop| match prop {
            Vue3Prop::Attribute(attr) if attr.name == "name" => attr.value.as_deref(),
            _ => None,
        })
        .map(quote_string)
        .or_else(|| {
            directive_by_name(element, "bind").and_then(|dir| {
                if dir.is_dynamic_arg {
                    return None;
                }
                let arg = dir.arg.as_ref()?.source_string();
                (arg == "name").then(|| {
                    rewrite_expression_with_scope(
                        &dir.exp
                            .as_ref()
                            .map(Vue3Expression::source_string)
                            .unwrap_or_default(),
                        options,
                        scope,
                    )
                })
            })
        })
        .unwrap_or_else(|| quote_string("default"));
    let slots = if options.prefix_identifiers || options.mode == "module" {
        "_ctx.$slots"
    } else {
        "$slots"
    };
    let props = render_slot_outlet_props(element, options, scope, memo_index);
    if props.is_empty() {
        format!("_renderSlot({}, {})", slots, name)
    } else {
        format!("_renderSlot({}, {}, {})", slots, name, props)
    }
}

pub(crate) fn render_element_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    parent_mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .collect::<Vec<_>>();
    if options.hoist_static
        && once_children_mode(parent_mode)
        && child_nodes.len() == 1
        && child_nodes
            .first()
            .is_some_and(|child| is_static_element_tree_for_cache(ast, child))
    {
        let rendered = render_node_expr_scoped(
            ast,
            child_nodes[0].id,
            options,
            NodeRenderMode::Cached,
            scope,
            memo_index,
        );
        return render_array(&[render_cached_single_child(rendered, memo_index.alloc())]);
    }
    if !scope.disable_stringify_static_chunks
        && options.hoist_static
        && options.stringify_static
        && root_like_render_mode(parent_mode)
        && !should_cache_children(ast, &child_nodes)
        && should_stringify_static_children(&child_nodes)
    {
        if let Some(static_call) = render_static_vnode_cache(ast, &child_nodes, options, scope) {
            return render_cached_children_array(vec![static_call], memo_index.alloc(), false);
        }
    }
    if options.hoist_static
        && root_like_render_mode(parent_mode)
        && should_cache_children(ast, &child_nodes)
        && !child_nodes
            .iter()
            .any(|child| static_tree_contains_comment(ast, child))
    {
        if options.stringify_static && !scope.disable_stringify_static_chunks {
            if let Some(static_call) = render_static_vnode_cache(ast, &child_nodes, options, scope)
            {
                return render_cached_children_array(vec![static_call], memo_index.alloc(), false);
            }
            if let Some(rendered) = render_static_vnode_chunked_children(
                ast,
                &child_nodes,
                options,
                scope,
                NodeRenderMode::Cached,
                memo_index,
            ) {
                return render_cached_children_array(
                    rendered,
                    memo_index.alloc(),
                    !options.stringify_static,
                );
            }
        }
        let rendered = child_nodes
            .iter()
            .map(|child| {
                render_node_expr_scoped(
                    ast,
                    child.id,
                    options,
                    NodeRenderMode::Cached,
                    scope,
                    memo_index,
                )
            })
            .collect::<Vec<_>>();
        if !rendered.is_empty() {
            return render_cached_children_array(
                rendered,
                memo_index.alloc(),
                !options.stringify_static,
            );
        }
    }
    if !scope.disable_stringify_static_chunks
        && options.hoist_static
        && options.stringify_static
        && (root_like_render_mode(parent_mode) || once_children_mode(parent_mode))
    {
        if let Some(rendered) = render_static_vnode_chunked_children(
            ast,
            &child_nodes,
            options,
            scope,
            NodeRenderMode::Child,
            memo_index,
        ) {
            let cache_static_chunks = once_children_mode(parent_mode)
                || rendered
                    .iter()
                    .any(|item| item.contains("_setBlockTracking(-1, true)"));
            let rendered = if cache_static_chunks {
                rendered
                    .into_iter()
                    .map(|item| {
                        if item.contains("_createStaticVNode(") {
                            render_cached_single_child(item, memo_index.alloc())
                        } else {
                            item
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                rendered
            };
            return render_array(&rendered);
        }
    }
    if !scope.disable_stringify_static_chunks
        && options.hoist_static
        && options.stringify_static
        && once_children_mode(parent_mode)
        && !child_nodes.iter().all(|child| {
            matches!(
                child.kind,
                Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
            )
        })
    {
        let rendered = render_child_sequence_or_static_cache(
            ast,
            children,
            options,
            NodeRenderMode::Child,
            scope,
            memo_index,
            true,
        );
        if !rendered.is_empty() {
            return render_array(&rendered);
        }
    }
    if child_nodes.iter().all(|child| {
        matches!(
            child.kind,
            Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
        )
    }) {
        if once_children_mode(parent_mode) {
            return render_array(&[render_text_vnode(ast, children, options, scope)]);
        }
        return render_text_sequence_expr(ast, children, options, scope);
    }
    let rendered = render_child_sequence(
        ast,
        children,
        options,
        NodeRenderMode::Child,
        scope,
        memo_index,
    );
    if rendered.is_empty() {
        String::new()
    } else if rendered.len() == 1
        && child_nodes.first().is_some_and(|child| is_text_like(child))
        && !root_like_render_mode(parent_mode)
    {
        rendered.into_iter().next().unwrap()
    } else {
        render_array(&rendered)
    }
}

pub(crate) fn render_child_sequence_or_static_cache(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
    cache_static_chunks: bool,
) -> Vec<String> {
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .collect::<Vec<_>>();
    if options.hoist_static && options.stringify_static {
        if let Some(rendered) = render_static_vnode_chunked_children(
            ast,
            &child_nodes,
            options,
            scope,
            mode,
            memo_index,
        ) {
            return rendered
                .into_iter()
                .map(|item| {
                    if cache_static_chunks && item.contains("_createStaticVNode(") {
                        render_cached_single_child(item, memo_index.alloc())
                    } else {
                        item
                    }
                })
                .collect();
        }
    }
    render_child_sequence(ast, children, options, mode, scope, memo_index)
}

pub(crate) fn render_cached_children_array(
    rendered: Vec<String>,
    cache_index: usize,
    compact_single_vnode: bool,
) -> String {
    if compact_single_vnode
        && rendered
            .first()
            .is_some_and(|item| !item.contains("_createStaticVNode("))
    {
        if let [single] = rendered.as_slice() {
            return format!("[...(_cache[{cache_index}] || (_cache[{cache_index}] = [{single}]))]");
        }
    }
    if rendered
        .iter()
        .any(|item| item.contains("_createStaticVNode("))
    {
        return format!(
            "[...(_cache[{cache_index}] || (_cache[{cache_index}] = {}))]",
            render_array(&rendered)
        );
    }
    format!(
        "[...(_cache[{cache_index}] || (_cache[{cache_index}] = {}))]",
        render_array(&rendered)
    )
}

pub(crate) fn render_cached_single_child(rendered: String, cache_index: usize) -> String {
    format!("_cache[{cache_index}] || (_cache[{cache_index}] = {rendered})")
}

pub(crate) fn render_child_sequence(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> Vec<String> {
    let mut rendered = Vec::new();
    let mut index = 0usize;
    while index < children.len() {
        if !scope.disable_stringify_static_chunks
            && options.hoist_static
            && options.stringify_static
        {
            let remaining_nodes = children[index..]
                .iter()
                .filter_map(|child_id| ast.node(*child_id))
                .collect::<Vec<_>>();
            if let Some(chunks) = render_static_vnode_chunked_children(
                ast,
                &remaining_nodes,
                options,
                scope,
                mode,
                memo_index,
            ) {
                for item in chunks {
                    rendered.push(if item.contains("_createStaticVNode(") {
                        render_cached_single_child(item, memo_index.alloc())
                    } else {
                        item
                    });
                }
                break;
            }
        }
        let child_id = children[index];
        let Some(child) = ast.node(child_id) else {
            index += 1;
            continue;
        };
        if options.hoist_static
            && mode == NodeRenderMode::RootChild
            && is_static_element_tree_for_cache(ast, child)
            && !static_tree_contains_comment(ast, child)
        {
            rendered.push(render_static_element_cache(
                ast, child.id, options, scope, memo_index,
            ));
            index += 1;
            continue;
        }
        if is_text_like(child) {
            let start = index;
            index += 1;
            while index < children.len()
                && ast
                    .node(children[index])
                    .is_some_and(|candidate| is_text_like(candidate))
            {
                index += 1;
            }
            rendered.push(render_text_vnode(
                ast,
                &children[start..index],
                options,
                scope,
            ));
            continue;
        }
        if let Vue3AstKind::Element(element) = &child.kind {
            if directive_by_name(element, "if").is_some() {
                let mut branch_ids = vec![child_id];
                let mut branch_comment_ids: Vec<Vec<vuec_ast::NodeId>> = vec![Vec::new()];
                let mut pending_comment_ids = Vec::new();
                index += 1;
                while index < children.len() {
                    let Some(candidate) = ast.node(children[index]) else {
                        index += 1;
                        continue;
                    };
                    if matches!(candidate.kind, Vue3AstKind::Comment(_)) {
                        pending_comment_ids.push(children[index]);
                        index += 1;
                        continue;
                    }
                    if let Vue3AstKind::Element(candidate_element) = &candidate.kind {
                        if is_else_branch(candidate_element) {
                            branch_ids.push(children[index]);
                            branch_comment_ids.push(std::mem::take(&mut pending_comment_ids));
                            index += 1;
                            continue;
                        }
                    }
                    break;
                }
                rendered.push(render_maybe_once_if_chain(
                    ast,
                    &branch_ids,
                    &branch_comment_ids,
                    options,
                    mode,
                    scope,
                    memo_index,
                ));
                continue;
            }
            if is_else_branch(element) {
                index += 1;
                continue;
            }
        }
        rendered.push(render_node_expr_scoped(
            ast, child_id, options, mode, scope, memo_index,
        ));
        index += 1;
    }
    rendered
}

pub(crate) fn is_static_element_tree_for_cache(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
) -> bool {
    let Vue3AstKind::Element(element) = &node.kind else {
        return false;
    };
    if element.tag == "slot"
        || element.tag_type != Vue3ElementType::Element
        || !element
            .props
            .iter()
            .all(vue3_prop_is_vnode_cacheable_static)
    {
        return false;
    }
    node.children.iter().all(|child_id| {
        ast.node(*child_id).is_some_and(|child| match &child.kind {
            Vue3AstKind::Text(_) | Vue3AstKind::Comment(_) => true,
            Vue3AstKind::Element(_) => is_static_element_tree_for_cache(ast, child),
            _ => false,
        })
    })
}

pub(crate) fn static_tree_contains_comment(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
) -> bool {
    matches!(node.kind, Vue3AstKind::Comment(_))
        || node.children.iter().any(|child_id| {
            ast.node(*child_id)
                .is_some_and(|child| static_tree_contains_comment(ast, child))
        })
}

pub(crate) fn select_children_include_unstringifiable_option_value(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
) -> bool {
    element.tag == "select"
        && element.ns == vuec_ast::HtmlNamespace::Html
        && ast.node(node_id).is_some_and(|node| {
            node.children.iter().any(|child_id| {
                ast.node(*child_id)
                    .is_some_and(|child| option_has_unstringifiable_value_binding(child))
            })
        })
}

pub(crate) fn p_children_include_invalid_html_descendant(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
) -> bool {
    element.tag.eq_ignore_ascii_case("p")
        && element.ns == vuec_ast::HtmlNamespace::Html
        && ast.node(node_id).is_some_and(|node| {
            node.children
                .iter()
                .any(|child_id| static_html_contains_invalid_p_descendant(ast, *child_id))
        })
}

pub(crate) fn option_has_unstringifiable_value_binding(
    node: &vuec_ast::Node<Vue3NodeKind>,
) -> bool {
    let Vue3AstKind::Element(element) = &node.kind else {
        return false;
    };
    element.tag == "option"
        && element.ns == vuec_ast::HtmlNamespace::Html
        && element.props.iter().any(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "bind" => dir
                .arg
                .as_ref()
                .is_some_and(|arg| arg.source_string() == "value"),
            _ => false,
        })
}

pub(crate) fn render_static_element_cache(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let rendered = render_node_expr_scoped(
        ast,
        node_id,
        options,
        NodeRenderMode::Cached,
        scope,
        memo_index,
    );
    render_cached_single_child(rendered, memo_index.alloc())
}

pub(crate) fn is_text_like(node: &vuec_ast::Node<Vue3NodeKind>) -> bool {
    matches!(
        node.kind,
        Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
    )
}

pub(crate) fn render_text_sequence_expr(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter_map(|child| match &child.kind {
            Vue3AstKind::Text(text) => Some(quote_text(&text.value)),
            Vue3AstKind::Interpolation(interpolation) => Some(format!(
                "_toDisplayString({})",
                rewrite_expression_with_scope(
                    &interpolation.expression.source_string(),
                    options,
                    scope
                )
            )),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

pub(crate) fn render_text_vnode(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let expression = render_text_sequence_expr(ast, children, options, scope);
    let has_interpolation = children.iter().any(|child_id| {
        ast.node(*child_id)
            .is_some_and(|child| matches!(child.kind, Vue3AstKind::Interpolation(_)))
    });
    if has_interpolation {
        format!("_createTextVNode({}, 1 /* TEXT */)", expression)
    } else {
        format!("_createTextVNode({})", expression)
    }
}

pub(crate) fn render_if_chain(
    ast: &Vue3Ast,
    branch_ids: &[vuec_ast::NodeId],
    branch_comment_ids: &[Vec<vuec_ast::NodeId>],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    fn render_branch(
        ast: &Vue3Ast,
        branch_ids: &[vuec_ast::NodeId],
        branch_comment_ids: &[Vec<vuec_ast::NodeId>],
        index: usize,
        options: &Vue3CompilerOptions,
        mode: NodeRenderMode,
        scope: &RenderScope,
        memo_index: &mut MemoIndex,
    ) -> String {
        let Some(branch_id) = branch_ids.get(index).copied() else {
            return "_createCommentVNode(\"v-if\", true)".into();
        };
        let Some(node) = ast.node(branch_id) else {
            return "_createCommentVNode(\"v-if\", true)".into();
        };
        let Some(element) = (match &node.kind {
            Vue3AstKind::Element(element) => Some(element),
            _ => None,
        }) else {
            return render_node_expr_scoped(ast, branch_id, options, mode, scope, memo_index);
        };
        let branch_expr = render_if_branch_expr(
            ast,
            branch_id,
            element,
            branch_comment_ids
                .get(index)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            options,
            mode,
            scope,
            index,
            memo_index,
        );
        let condition = if index == 0 {
            directive_by_name(element, "if")
        } else {
            directive_by_name(element, "else-if")
        };
        if let Some(condition) = condition.and_then(|dir| dir.exp.as_ref()) {
            let condition = render_condition(
                &rewrite_expression_with_scope(&condition.source_string(), options, scope),
                options,
            );
            let next_is_else_if = branch_ids
                .get(index + 1)
                .and_then(|branch_id| ast.node(*branch_id))
                .and_then(|node| match &node.kind {
                    Vue3AstKind::Element(element) => Some(element),
                    _ => None,
                })
                .is_some_and(|element| directive_by_name(element, "else-if").is_some());
            let alternate = render_branch(
                ast,
                branch_ids,
                branch_comment_ids,
                index + 1,
                options,
                mode,
                scope,
                memo_index,
            );
            format!(
                "{condition}\n  ? {}\n  : {}",
                indent_after_first_line(&branch_expr, 4),
                indent_after_first_line(&alternate, if next_is_else_if { 2 } else { 4 })
            )
        } else {
            branch_expr
        }
    }
    render_branch(
        ast,
        branch_ids,
        branch_comment_ids,
        0,
        options,
        mode,
        scope,
        memo_index,
    )
}

pub(crate) fn render_maybe_once_if_chain(
    ast: &Vue3Ast,
    branch_ids: &[vuec_ast::NodeId],
    branch_comment_ids: &[Vec<vuec_ast::NodeId>],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    if scope.in_v_once {
        return render_if_chain(
            ast,
            branch_ids,
            branch_comment_ids,
            options,
            mode,
            scope,
            memo_index,
        );
    }
    let Some(first_element) = branch_ids
        .first()
        .and_then(|branch_id| ast.node(*branch_id))
        .and_then(|node| match &node.kind {
            Vue3AstKind::Element(element) => Some(element),
            _ => None,
        })
    else {
        return render_if_chain(
            ast,
            branch_ids,
            branch_comment_ids,
            options,
            mode,
            scope,
            memo_index,
        );
    };
    if directive_by_name(first_element, "once").is_none() {
        return render_if_chain(
            ast,
            branch_ids,
            branch_comment_ids,
            options,
            mode,
            scope,
            memo_index,
        );
    }
    let (once_index, scoped) = if directive_by_name(first_element, "memo").is_some() {
        let memo_slot = memo_index.alloc();
        let once_slot = memo_index.alloc();
        (
            once_slot,
            scope
                .with_v_once()
                .with_memo_index_override(branch_ids[0], memo_slot),
        )
    } else {
        (memo_index.alloc(), scope.with_v_once())
    };
    let rendered = render_if_chain(
        ast,
        branch_ids,
        branch_comment_ids,
        options,
        mode,
        &scoped,
        memo_index,
    );
    render_with_v_once(rendered, once_index)
}

pub(crate) fn render_if_branch_expr(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    leading_comment_ids: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    _mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: usize,
    memo_index: &mut MemoIndex,
) -> String {
    if element.tag == "template" {
        let children = ast
            .node(node_id)
            .map(|node| {
                render_fragment_children(
                    ast,
                    &prefixed_child_ids(leading_comment_ids, &node.children),
                    options,
                    scope,
                    memo_index,
                )
            })
            .unwrap_or_else(|| "[]".into());
        return format!(
            "(_openBlock(), _createElementBlock(_Fragment, {{ key: {branch_key} }}, {children}, 64 /* STABLE_FRAGMENT */))"
        );
    }
    if !leading_comment_ids.is_empty() {
        let mut rendered = leading_comment_ids
            .iter()
            .map(|comment_id| {
                render_node_expr_scoped(
                    ast,
                    *comment_id,
                    options,
                    NodeRenderMode::Child,
                    scope,
                    memo_index,
                )
            })
            .collect::<Vec<_>>();
        rendered.push(render_maybe_memo_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::Child,
            scope,
            None,
            memo_index,
        ));
        let children = render_array(&rendered);
        return format!(
            "(_openBlock(), _createElementBlock(_Fragment, {{ key: {branch_key} }}, {children}, 2112 /* STABLE_FRAGMENT, DEV_ROOT_FRAGMENT */))"
        );
    }
    if directive_by_name(element, "once").is_some() {
        return render_maybe_memo_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::OnceBlockRoot,
            scope,
            Some(branch_key),
            memo_index,
        );
    }
    render_maybe_memo_element(
        ast,
        node_id,
        element,
        options,
        NodeRenderMode::Root,
        scope,
        Some(branch_key),
        memo_index,
    )
}

pub(crate) fn prefixed_child_ids(
    prefix: &[vuec_ast::NodeId],
    children: &[vuec_ast::NodeId],
) -> Vec<vuec_ast::NodeId> {
    prefix.iter().chain(children.iter()).copied().collect()
}

pub(crate) fn render_fragment_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let rendered = render_child_sequence(
        ast,
        children,
        options,
        NodeRenderMode::Child,
        scope,
        memo_index,
    );
    render_array(&rendered)
}

pub(crate) fn render_for_node(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    directive: &Vue3Directive,
    options: &Vue3CompilerOptions,
    _mode: NodeRenderMode,
    scope: &RenderScope,
    _memo_index: &mut MemoIndex,
) -> String {
    let Some(expression) = directive.exp.as_ref().map(Vue3Expression::source_string) else {
        return render_once_plain_fallback(ast, node_id, element, options, scope, _memo_index);
    };
    let parsed = parse_v_for_expression(&expression);
    let Some((source, aliases)) = parsed else {
        return render_once_plain_fallback(ast, node_id, element, options, scope, _memo_index);
    };
    let source = rewrite_expression_with_scope(&source, options, scope);
    let scoped = scope.with_locals(normalize_v_for_aliases(&aliases));
    let should_wrap_once = directive_by_name(element, "once").is_some() && !scope.in_v_once;
    let once_index = (should_wrap_once && directive_by_name(element, "memo").is_none())
        .then(|| _memo_index.alloc());
    let scoped = if directive_by_name(element, "once").is_some() && !scope.in_v_once {
        scoped.with_v_once()
    } else {
        scoped
    };
    let params = aliases.join(", ");
    let Some(memo) = directive_by_name(element, "memo") else {
        let body = render_v_for_body(ast, node_id, element, options, &scoped, _memo_index);
        let fragment_flag = v_for_fragment_patch_flag(element);
        let body = indent_after_first_line(&body, 2);
        let rendered = format!(
            "(_openBlock(true), _createElementBlock(_Fragment, null, _renderList({source}, ({params}) => {{\n  return {body}\n}}), {fragment_flag}))"
        );
        return once_index.map_or(rendered.clone(), |index| {
            render_with_v_once(rendered, index)
        });
    };
    let cache_index = _memo_index.alloc();
    // Vue's transformMemo reserves a cache slot for v-for memo wrappers even
    // though the emitted render-list memo path only references cache_index.
    _memo_index.reserve();
    let once_index = should_wrap_once.then(|| _memo_index.alloc());
    let body = render_v_for_body(ast, node_id, element, options, &scoped, _memo_index);
    let params = format!("{params}, __, ___, _cached");
    let memo_expression = memo
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let memo_expression = rewrite_expression_with_scope(&memo_expression, options, &scoped);
    let key = v_for_key_expression(element, options, &scoped);
    let guard = key.map_or_else(
        || format!("_cached && _cached.el && _isMemoSame(_cached, _memo)"),
        |key| {
            format!("_cached && _cached.el && _cached.key === {key} && _isMemoSame(_cached, _memo)")
        },
    );
    let body = indent_after_first_line(&body, 2);
    let rendered = format!(
        "(_openBlock(true), _createElementBlock(_Fragment, null, _renderList({source}, ({params}) => {{\n  const _memo = ({memo_expression})\n  if ({guard}) return _cached\n  const _item = {body}\n  _item.memo = _memo\n  return _item\n}}, _cache, {cache_index}), 128 /* KEYED_FRAGMENT */))"
    );
    once_index.map_or(rendered.clone(), |index| {
        render_with_v_once(rendered, index)
    })
}

pub(crate) fn render_once_plain_fallback(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    if directive_by_name(element, "once").is_some() && !scope.in_v_once {
        let once_index = memo_index.alloc();
        let scope = scope.with_v_once();
        let rendered = render_plain_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::OnceRoot,
            &scope,
            None,
            memo_index,
        );
        render_with_v_once(rendered, once_index)
    } else {
        render_plain_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::Root,
            scope,
            None,
            memo_index,
        )
    }
}

pub(crate) fn render_v_for_body(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    if element.tag == "template" {
        let Some(node) = ast.node(node_id) else {
            return "null".into();
        };
        let visible = visible_children(ast, &node.children);
        if visible.len() == 1 {
            if let Some(child) = visible.first() {
                if let Vue3AstKind::Element(child_element) = &child.kind {
                    let key = v_for_key_expression(element, options, scope);
                    let body = render_plain_element(
                        ast,
                        child.id,
                        child_element,
                        options,
                        NodeRenderMode::Root,
                        scope,
                        None,
                        memo_index,
                    );
                    return inject_key_into_vnode_call(&body, key.as_deref());
                }
            }
        }
        let key = v_for_key_expression(element, options, scope);
        let children = render_fragment_children(ast, &node.children, options, scope, memo_index);
        let props = key
            .map(|key| format!("{{ key: {key} }}"))
            .unwrap_or_else(|| "null".into());
        return format!(
            "(_openBlock(), _createElementBlock(_Fragment, {props}, {children}, 64 /* STABLE_FRAGMENT */))"
        );
    }
    render_plain_element(
        ast,
        node_id,
        element,
        options,
        NodeRenderMode::Root,
        scope,
        None,
        memo_index,
    )
}

pub(crate) fn v_for_fragment_patch_flag(element: &Vue3Element) -> &'static str {
    if v_for_key_expression(
        element,
        &Vue3CompilerOptions::default(),
        &RenderScope::default(),
    )
    .is_some()
    {
        "128 /* KEYED_FRAGMENT */"
    } else {
        "256 /* UNKEYED_FRAGMENT */"
    }
}

pub(crate) fn v_for_key_expression(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "key") =>
        {
            dir.exp
                .as_ref()
                .map(Vue3Expression::source_string)
                .filter(|value| !value.trim().is_empty())
                .map(|value| rewrite_expression_with_scope(&value, options, scope))
        }
        Vue3Prop::Attribute(attr) if attr.name == "key" => {
            attr.value.as_ref().map(|value| quote_string(value))
        }
        _ => None,
    })
}

pub(crate) fn inject_key_into_vnode_call(body: &str, key: Option<&str>) -> String {
    let Some(key) = key else {
        return body.to_string();
    };
    if body.contains(" key: ") || body.contains("{ key:") {
        return body.to_string();
    }
    let Some(start) = body.find("_createElementBlock(") else {
        return body.to_string();
    };
    let args_start = start + "_createElementBlock(".len();
    let Some(first_comma) = find_top_level_comma(body, args_start) else {
        return body.to_string();
    };
    let Some(close) = find_matching_call_close(body, args_start) else {
        return body.to_string();
    };
    if body[first_comma + 1..close].trim().is_empty() {
        let mut output = body.to_string();
        output.insert_str(first_comma, &format!(", {{ key: {key} }}"));
        return output;
    }
    let second_arg_start = first_comma + 1;
    let second_arg_end = find_top_level_comma(body, second_arg_start).unwrap_or(close);
    let second_arg = body[second_arg_start..second_arg_end].trim();
    if second_arg == "null" {
        let mut output = body.to_string();
        output.replace_range(
            second_arg_start..second_arg_end,
            &format!(" {{ key: {key} }}"),
        );
        return output;
    }
    body.to_string()
}

pub(crate) fn find_top_level_comma(value: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let chars = value.char_indices().skip_while(|(index, _)| *index < start);
    for (index, ch) in chars {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            ',' if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

pub(crate) fn find_matching_call_close(value: &str, start: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut quote = None;
    let chars = value.char_indices().skip_while(|(index, _)| *index < start);
    for (index, ch) in chars {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn render_array(items: &[String]) -> String {
    if items.is_empty() {
        "[]".into()
    } else {
        format!(
            "[\n{}\n]",
            items
                .iter()
                .map(|item| indent_lines(item, 2))
                .collect::<Vec<_>>()
                .join(",\n")
        )
    }
}

pub(crate) fn render_string_array(items: &[String]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|item| quote_string(item))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub(crate) fn vue3_dom_event_option_postfix(modifiers: &[String]) -> String {
    modifiers
        .iter()
        .map(|modifier| capitalize(modifier))
        .collect()
}

pub(crate) fn indent_lines(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    value
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn indent_after_first_line(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    let mut lines = value.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut output = first.to_string();
    for line in lines {
        output.push('\n');
        output.push_str(&prefix);
        output.push_str(line);
    }
    output
}

pub(crate) fn dedent_after_first_line(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    let mut lines = value.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut output = first.to_string();
    for line in lines {
        output.push('\n');
        output.push_str(line.strip_prefix(&prefix).unwrap_or(line));
    }
    output
}

pub(crate) fn render_condition(condition: &str, options: &Vue3CompilerOptions) -> String {
    if uses_prefixed_identifiers(options) {
        format!("({condition})")
    } else {
        condition.to_string()
    }
}

pub(crate) fn render_vue3_ssr_slot_condition(condition: String) -> String {
    if condition.starts_with("_ctx.") {
        format!("({condition})")
    } else {
        condition
    }
}

pub(crate) fn should_cache_children(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
) -> bool {
    !children.is_empty()
        && children
            .iter()
            .all(|child| is_static_element_tree_for_cache(ast, child))
}

pub(crate) fn should_stringify_static_children(children: &[&vuec_ast::Node<Vue3NodeKind>]) -> bool {
    !children.is_empty()
        && children
            .iter()
            .all(|child| is_stringifiable_static_node_for_cache(child))
}

pub(crate) fn is_stringifiable_static_node_for_cache(node: &vuec_ast::Node<Vue3NodeKind>) -> bool {
    match &node.kind {
        Vue3AstKind::Element(element) => {
            element.tag != "slot"
                && element
                    .props
                    .iter()
                    .all(|prop| vue3_prop_is_static_cacheable_for_ns(prop, element.ns))
        }
        Vue3AstKind::Text(_) => true,
        Vue3AstKind::Interpolation(interpolation) => {
            static_const_eval_source(&interpolation.expression.source_string()).is_some()
        }
        _ => false,
    }
}

pub(crate) const STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT: usize = 5;
pub(crate) const STRINGIFY_STATIC_NODE_COUNT: usize = 20;
