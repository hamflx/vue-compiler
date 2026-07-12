fn vue3_dom_root_mut(ast: &mut Vue3Ast) -> Option<&mut Vue3Root> {
    let root = ast.root_node_mut()?;
    match &mut root.kind {
        Vue3AstKind::Root(root) => Some(root),
        _ => None,
    }
}

fn report_transition_invalid_children(ast: &Vue3Ast, ctx: &mut TransformContext) {
    report_transition_invalid_children_for_node(ast, ast.root, ctx);
}

fn report_invalid_native_v_model(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
    ctx: &mut TransformContext,
) {
    for node in &ast.nodes {
        let Vue3AstKind::Element(element) = &node.kind else {
            continue;
        };
        if element.tag_type != Vue3ElementType::Element {
            continue;
        }
        if matches!(
            element.tag.as_str(),
            "input" | "textarea" | "select" | "script" | "style"
        ) {
            continue;
        }
        if options
            .custom_elements
            .iter()
            .any(|custom| custom == &element.tag)
        {
            continue;
        }
        let Some(model) = element.props.iter().find_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "model" => Some(dir),
            _ => None,
        }) else {
            continue;
        };
        if model_binding_error_preempts_invalid_native_model(model, options) {
            continue;
        }
        ctx.report(Diagnostic::vue3_error(
            Vue3ErrorCode::XVModelOnInvalidElement,
            "v-model can only be used on <input>, <textarea> and <select> elements.",
            model.span.or_else(|| node.span.source()),
        ));
    }
}

fn model_binding_error_preempts_invalid_native_model(
    model: &vuec_ast::Vue3Directive,
    options: &Vue3CompilerOptions,
) -> bool {
    let Some(expression) = model.exp.as_ref() else {
        return true;
    };
    let raw = expression.source_string();
    let raw = raw.trim();
    if raw.is_empty() {
        return true;
    }
    if matches!(
        options.binding_metadata.get(raw).map(String::as_str),
        Some("props" | "props-aliased" | "literal-const" | "setup-const")
    ) {
        return true;
    }
    !vuec_vue3_core::model_is_member_expression(raw)
}

fn report_transition_invalid_children_for_node(
    ast: &Vue3Ast,
    node_id: NodeId,
    ctx: &mut TransformContext,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    if let Vue3AstKind::Element(element) = &node.kind {
        if element.tag_type == Vue3ElementType::Component
            && matches!(element.tag.as_str(), "Transition" | "transition")
            && transition_children_are_invalid(ast, &node.children)
        {
            ctx.report(Diagnostic::vue3_error(
                Vue3ErrorCode::XTransitionInvalidChildren,
                "<Transition> expects exactly one child element or component.",
                node.span.source(),
            ));
        }
    }
    for child_id in node.children.clone() {
        report_transition_invalid_children_for_node(ast, child_id, ctx);
    }
}

fn transition_children_are_invalid(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    if children.is_empty() {
        return false;
    }
    transition_child_sequence_is_invalid(ast, &transition_visible_child_ids(ast, children), false)
}

fn transition_child_sequence_is_invalid(
    ast: &Vue3Ast,
    visible_children: &[NodeId],
    empty_is_invalid: bool,
) -> bool {
    if visible_children.is_empty() {
        return empty_is_invalid;
    }
    let mut logical_children = 0usize;
    let mut invalid = false;
    let mut index = 0usize;
    while index < visible_children.len() {
        logical_children += 1;
        let child_id = visible_children[index];
        if transition_child_starts_if_chain(ast, child_id) {
            let (branches, next_index) = collect_transition_if_chain(ast, visible_children, index);
            invalid |= branches
                .iter()
                .any(|branch_id| transition_if_branch_is_invalid(ast, *branch_id));
            index = next_index;
        } else {
            invalid |= transition_single_child_is_invalid(ast, child_id);
            index += 1;
        }
    }
    logical_children != 1 || invalid
}

fn transition_single_child_is_invalid(ast: &Vue3Ast, child_id: NodeId) -> bool {
    let Some(child) = ast.node(child_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &child.kind else {
        return false;
    };
    element_has_directive(element, "for")
}

fn transition_if_branch_is_invalid(ast: &Vue3Ast, branch_id: NodeId) -> bool {
    let Some(branch) = ast.node(branch_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &branch.kind else {
        return false;
    };
    if element_has_directive(element, "for") {
        return true;
    }
    if element.tag == "template" {
        return transition_child_sequence_is_invalid(
            ast,
            &transition_visible_child_ids(ast, &branch.children),
            true,
        );
    }
    false
}

fn collect_transition_if_chain(
    ast: &Vue3Ast,
    visible_children: &[NodeId],
    start: usize,
) -> (Vec<NodeId>, usize) {
    let mut branches = vec![visible_children[start]];
    let mut index = start + 1;
    while index < visible_children.len() {
        let Some(node) = ast.node(visible_children[index]) else {
            index += 1;
            continue;
        };
        let Vue3AstKind::Element(element) = &node.kind else {
            break;
        };
        if element_has_directive(element, "else-if") || element_has_directive(element, "else") {
            branches.push(visible_children[index]);
            index += 1;
        } else {
            break;
        }
    }
    (branches, index)
}

fn transition_child_starts_if_chain(ast: &Vue3Ast, child_id: NodeId) -> bool {
    ast.node(child_id).is_some_and(|child| {
        matches!(
            &child.kind,
            Vue3AstKind::Element(element) if element_has_directive(element, "if")
        )
    })
}

fn transition_visible_child_ids(ast: &Vue3Ast, children: &[NodeId]) -> Vec<NodeId> {
    children
        .iter()
        .copied()
        .filter(|child_id| {
            ast.node(*child_id).is_some_and(|child| match &child.kind {
                Vue3AstKind::Comment(_) => false,
                Vue3AstKind::Text(text) => !text.value.chars().all(is_html_whitespace),
                _ => true,
            })
        })
        .collect()
}

fn element_has_directive(element: &Vue3Element, name: &str) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(directive) if directive.name == name
        )
    })
}

fn is_html_whitespace(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\u{000C}' | '\r' | ' ')
}

fn remove_side_effect_nodes(ast: &mut Vue3Ast, ctx: &mut TransformContext) {
    remove_side_effect_children(ast, ast.root, ctx);
}

fn remove_side_effect_children(ast: &mut Vue3Ast, parent_id: NodeId, ctx: &mut TransformContext) {
    let child_ids = ast
        .node(parent_id)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    let mut retained = Vec::new();
    for child_id in child_ids {
        let remove = ast
            .node(child_id)
            .is_some_and(ast_node_is_side_effect_tag);
        if remove {
            if let Some(span) = ast.node(child_id).and_then(|node| node.span.source()) {
                ctx.report(Diagnostic::vue3_error(
                    Vue3ErrorCode::XIgnoredSideEffectTag,
                    "Tags with side effect (<script> and <style>) are ignored in client component templates.",
                    Some(span),
                ));
            }
        } else {
            remove_side_effect_children(ast, child_id, ctx);
            retained.push(child_id);
        }
    }
    ast.replace_children(parent_id, retained);
}

fn ast_node_is_side_effect_tag(node: &vuec_ast::Node<Vue3AstKind>) -> bool {
    matches!(
        node.kind,
        Vue3AstKind::Element(ref element)
            if element.tag_type == Vue3ElementType::Element && is_side_effect_tag(&element.tag)
    )
}

fn json_node_is_side_effect_tag(node: &Value) -> bool {
    json_u64(node, "type") == Some(1)
        && json_u64(node, "tagType") == Some(0)
        && json_str(node, "tag").is_some_and(is_side_effect_tag)
}

fn is_side_effect_tag(tag: &str) -> bool {
    matches!(tag, "script" | "style")
}
