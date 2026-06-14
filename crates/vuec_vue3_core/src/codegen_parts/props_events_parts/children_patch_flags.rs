pub(crate) fn child_sequence_needs_text_vnode(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
) -> bool {
    let visible = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| !matches!(child.kind, Vue3AstKind::Comment(_)))
        .collect::<Vec<_>>();
    visible.len() > 1 && !visible.iter().all(|child| is_text_like(child))
}

pub(crate) fn children_literal_const_only(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
) -> bool {
    let mut has_interpolation = false;
    for child_id in children {
        let Some(child) = ast.node(*child_id) else {
            continue;
        };
        match &child.kind {
            Vue3AstKind::Interpolation(interpolation) => {
                has_interpolation = true;
                let expression = interpolation.expression.source_string();
                if options
                    .binding_metadata
                    .get(expression.trim())
                    .map(String::as_str)
                    != Some("literal-const")
                {
                    return false;
                }
            }
            Vue3AstKind::Text(text) if text.value.trim().is_empty() => {}
            Vue3AstKind::Comment(_) => {}
            _ => return false,
        }
    }
    has_interpolation
}

pub(crate) fn render_patch_flag_kind(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
) -> Option<i32> {
    let children = ast
        .node(node_id)
        .map(|node| node.children.as_slice())
        .unwrap_or(&[]);
    if mode == NodeRenderMode::Cached {
        Some(-1)
    } else if has_dynamic_arg_binding(element) {
        Some(16)
    } else {
        let mut flag = 0;
        if element.tag_type == Vue3ElementType::Element && has_class_binding(element) {
            flag |= 2;
        }
        if element.tag_type == Vue3ElementType::Element && has_style_binding(element) {
            flag |= 4;
        }
        if has_dynamic_non_key_props(element, options, scope)
            && !(mode == NodeRenderMode::Cached && static_cached_bindings_are_constant(element))
        {
            flag |= 8;
        }
        if !once_children_mode(mode)
            && element.tag != "template"
            && child_sequence_is_direct_dynamic_text(ast, children, options)
        {
            flag |= 1;
        }
        if flag == 0
            && (has_vnode_hook(element)
                || has_runtime_directive(element)
                || has_native_v_model(element))
        {
            flag |= 512;
        }
        (flag != 0).then_some(flag)
    }
}

pub(crate) fn render_patch_flag_text(flag: Option<i32>) -> String {
    match flag {
        Some(flag) => format!(", {}", public_patch_flag_text(flag)),
        None => String::new(),
    }
}

/// Computes the public codegen patch flag for a Vue 3 element node.
pub fn vue3_element_codegen_patch_flag(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    is_root: bool,
) -> Option<i32> {
    let Some(node) = ast.node(node_id) else {
        return None;
    };
    let Vue3AstKind::Element(element) = &node.kind else {
        return None;
    };
    let mode = if is_root {
        NodeRenderMode::Root
    } else {
        NodeRenderMode::Child
    };
    let scope = RenderScope::default();
    render_patch_flag_kind(ast, node_id, element, options, mode, &scope)
}
