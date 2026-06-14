pub(crate) fn render_root_expr(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    if options.hoist_static && options.stringify_static {
        if let Some(root_static_call) =
            render_root_static_vnode_cache(ast, children, options, scope)
        {
            return render_cached_single_child(root_static_call, memo_index.alloc());
        }
    }
    let visible = visible_child_ids(ast, children);
    match visible.as_slice() {
        [] => "null".into(),
        [single]
            if children == [*single]
                && root_single_visible_child_uses_direct_codegen(ast, *single) =>
        {
            render_node_expr_scoped(
                ast,
                *single,
                options,
                NodeRenderMode::Root,
                scope,
                memo_index,
            )
        }
        _ => {
            let rendered = render_root_child_sequence(ast, children, options, scope, memo_index);
            format!(
                "(_openBlock(), _createElementBlock(_Fragment, null, {}, {}))",
                render_array(&rendered),
                public_patch_flag_text(root_fragment_patch_flag_ast(ast, children) as i32)
            )
        }
    }
}

pub(crate) fn render_root_child_sequence(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> Vec<String> {
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .collect::<Vec<_>>();
    if !scope.disable_stringify_static_chunks && options.hoist_static && options.stringify_static {
        if let Some(rendered) = render_static_vnode_chunked_children(
            ast,
            &child_nodes,
            options,
            scope,
            NodeRenderMode::RootChild,
            memo_index,
        ) {
            return rendered
                .into_iter()
                .map(|item| {
                    if item.contains("_createStaticVNode(") {
                        render_cached_single_child(item, memo_index.alloc())
                    } else {
                        item
                    }
                })
                .collect();
        }
    }
    render_child_sequence(
        ast,
        children,
        options,
        NodeRenderMode::RootChild,
        scope,
        memo_index,
    )
}

pub(crate) fn root_single_visible_child_uses_direct_codegen(
    ast: &Vue3Ast,
    child_id: vuec_ast::NodeId,
) -> bool {
    ast.node(child_id).is_some_and(|node| match &node.kind {
        Vue3AstKind::Interpolation(_) => true,
        Vue3AstKind::Element(element) => {
            element.tag_type != Vue3ElementType::SlotOutlet
                && directive_by_name(element, "if").is_none()
                && directive_by_name(element, "for").is_none()
        }
        _ => false,
    })
}

pub(crate) fn root_fragment_patch_flag_ast(ast: &Vue3Ast, children: &[vuec_ast::NodeId]) -> u16 {
    let visible = visible_child_ids(ast, children).len();
    if visible == 1
        && children.iter().any(|child_id| {
            ast.node(*child_id)
                .is_some_and(|child| matches!(child.kind, Vue3AstKind::Comment(_)))
        })
    {
        64 | 2048
    } else {
        64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NodeRenderMode {
    Root,
    OnceRoot,
    OnceBlockRoot,
    RootChild,
    Child,
    Cached,
}

pub(crate) fn root_like_render_mode(mode: NodeRenderMode) -> bool {
    matches!(
        mode,
        NodeRenderMode::Root | NodeRenderMode::OnceRoot | NodeRenderMode::OnceBlockRoot
    )
}

pub(crate) fn block_render_mode(mode: NodeRenderMode) -> bool {
    matches!(mode, NodeRenderMode::Root | NodeRenderMode::OnceBlockRoot)
}

pub(crate) fn once_children_mode(mode: NodeRenderMode) -> bool {
    matches!(
        mode,
        NodeRenderMode::OnceRoot | NodeRenderMode::OnceBlockRoot
    )
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RenderScope {
    pub(crate) locals: Vec<String>,
    pub(crate) in_v_once: bool,
    pub(crate) memo_index_overrides: BTreeMap<vuec_ast::NodeId, usize>,
    pub(crate) static_hoists: StaticHoists,
    pub(crate) disable_stringify_static_chunks: bool,
}

impl RenderScope {
    pub(crate) fn with_locals(&self, locals: Vec<String>) -> Self {
        let mut next = self.clone();
        for local in locals {
            if !next.locals.iter().any(|existing| existing == &local) {
                next.locals.push(local);
            }
        }
        next
    }

    pub(crate) fn with_v_once(&self) -> Self {
        let mut next = self.clone();
        next.in_v_once = true;
        next
    }

    pub(crate) fn with_memo_index_override(&self, node_id: vuec_ast::NodeId, index: usize) -> Self {
        let mut next = self.clone();
        next.memo_index_overrides.insert(node_id, index);
        next
    }

    pub(crate) fn with_static_hoists(&self, hoists: StaticHoists) -> Self {
        let mut next = self.clone();
        next.static_hoists = hoists;
        next
    }

    pub(crate) fn without_stringify_static_chunks(&self) -> Self {
        let mut next = self.clone();
        next.disable_stringify_static_chunks = true;
        next
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct StaticHoists {
    pub(crate) declarations: Vec<StaticHoistDeclaration>,
    pub(crate) props_by_node: BTreeMap<vuec_ast::NodeId, usize>,
    pub(crate) bindings_by_node_prop: BTreeMap<(vuec_ast::NodeId, usize), usize>,
}

impl StaticHoists {
    pub(crate) fn push_binding(
        &mut self,
        node_id: vuec_ast::NodeId,
        prop_index: usize,
        expression: String,
        reuse_existing: bool,
    ) -> usize {
        if reuse_existing {
            if let Some(index) = self
                .declarations
                .iter()
                .position(|declaration| {
                    matches!(
                        declaration,
                        StaticHoistDeclaration::BindingExpression { expression: existing }
                            if existing == &expression
                    )
                })
                .map(|index| index + 1)
            {
                self.bindings_by_node_prop
                    .insert((node_id, prop_index), index);
                return index;
            }
        }
        self.declarations
            .push(StaticHoistDeclaration::BindingExpression { expression });
        let index = self.declarations.len();
        self.bindings_by_node_prop
            .insert((node_id, prop_index), index);
        index
    }

    pub(crate) fn push_props_object(&mut self, node_id: vuec_ast::NodeId) -> usize {
        self.declarations
            .push(StaticHoistDeclaration::PropsObject { node_id });
        let index = self.declarations.len();
        self.props_by_node.insert(node_id, index);
        index
    }

    pub(crate) fn binding_index(
        &self,
        node_id: vuec_ast::NodeId,
        prop_index: usize,
    ) -> Option<usize> {
        self.bindings_by_node_prop
            .get(&(node_id, prop_index))
            .copied()
    }
}

#[derive(Clone, Debug)]
pub(crate) enum StaticHoistDeclaration {
    PropsObject { node_id: vuec_ast::NodeId },
    BindingExpression { expression: String },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MemoIndex {
    pub(crate) next: usize,
}

impl MemoIndex {
    pub(crate) fn alloc(&mut self) -> usize {
        let index = self.next;
        self.next += 1;
        index
    }

    pub(crate) fn reserve(&mut self) {
        self.next += 1;
    }
}

pub(crate) fn render_node_expr_scoped(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let Some(node) = ast.node(node_id) else {
        return "null".into();
    };
    match &node.kind {
        Vue3AstKind::Root(_) => {
            let rendered = render_child_sequence(
                ast,
                &node.children,
                options,
                NodeRenderMode::Root,
                scope,
                memo_index,
            );
            format!("[{}]", rendered.join(", "))
        }
        Vue3AstKind::Text(text) => quote_text(&text.value),
        Vue3AstKind::Interpolation(interpolation) => {
            format!(
                "_toDisplayString({})",
                rewrite_expression_with_scope(
                    &interpolation.expression.source_string(),
                    options,
                    scope
                )
            )
        }
        Vue3AstKind::Comment(comment) => {
            format!("_createCommentVNode({})", quote_string(&comment.value))
        }
        Vue3AstKind::Element(element) => {
            if let Some(for_dir) = directive_by_name(element, "for") {
                return render_for_node(
                    ast, node_id, element, for_dir, options, mode, scope, memo_index,
                );
            }
            if directive_by_name(element, "if").is_some() {
                return render_maybe_once_if_chain(
                    ast,
                    &[node_id],
                    &[Vec::new()],
                    options,
                    mode,
                    scope,
                    memo_index,
                );
            }
            if is_else_branch(element) {
                return "null".into();
            }
            render_maybe_memo_element(
                ast, node_id, element, options, mode, scope, None, memo_index,
            )
        }
        _ => "null".into(),
    }
}

pub(crate) fn render_with_v_once(rendered: String, index: usize) -> String {
    format!(
        "_cache[{index}] || (\n  _setBlockTracking(-1, true),\n  (_cache[{index}] = {}).cacheIndex = {index},\n  _setBlockTracking(1),\n  _cache[{index}]\n)",
        indent_after_first_line(&rendered, 2)
    )
}

pub(crate) fn render_plain_element(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: Option<usize>,
    memo_index: &mut MemoIndex,
) -> String {
    let tag = &element.tag;
    if tag == "slot" {
        return render_slot_outlet(element, options, scope, memo_index);
    }
    if element.tag_type == Vue3ElementType::Component {
        return render_component_element(
            ast, node_id, element, options, mode, scope, branch_key, memo_index,
        );
    }
    let helper = if block_render_mode(mode) {
        "_createElementBlock"
    } else {
        "_createElementVNode"
    };
    let element_scope = if directive_by_name(element, "once").is_some() || scope.in_v_once {
        scope.with_v_once()
    } else {
        scope.clone()
    };
    let element_scope =
        if select_children_include_unstringifiable_option_value(ast, node_id, element)
            || p_children_include_invalid_html_descendant(ast, node_id, element)
        {
            element_scope.without_stringify_static_chunks()
        } else {
            element_scope
        };
    let props = if branch_key.is_none() {
        element_scope
            .static_hoists
            .props_by_node
            .get(&node_id)
            .map(|index| format!("_hoisted_{index}"))
            .unwrap_or_else(|| {
                render_props(
                    node_id,
                    element,
                    options,
                    &element_scope,
                    branch_key,
                    memo_index,
                )
            })
    } else {
        render_props(
            node_id,
            element,
            options,
            &element_scope,
            branch_key,
            memo_index,
        )
    };
    let static_content = render_static_content_directive_child(element, options);
    let children = if let Some(content) = static_content.as_ref() {
        Some(content.clone())
    } else if exact_content_directive(element).is_some() {
        None
    } else {
        ast.node(node_id)
            .map(|node| {
                render_element_children(
                    ast,
                    &node.children,
                    options,
                    mode,
                    &element_scope,
                    memo_index,
                )
            })
            .filter(|children| !children.is_empty())
    };
    let patch_flag = render_patch_flag_text(render_patch_flag_kind(
        ast,
        node_id,
        element,
        options,
        mode,
        &element_scope,
    ));
    let attrs = if props.is_empty() { None } else { Some(props) };
    let args = render_call_args(
        quote_string(tag),
        attrs.as_deref(),
        children.as_deref(),
        patch_flag.as_str(),
        dynamic_props_arg(element, options, &element_scope).as_str(),
    );
    let rendered = if block_render_mode(mode) {
        format!("(_openBlock(), {}({}))", helper, args)
    } else {
        format!("{}({})", helper, args)
    };
    render_with_runtime_directives(rendered, element, options, scope)
}

pub(crate) fn render_maybe_memo_element(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: Option<usize>,
    memo_index: &mut MemoIndex,
) -> String {
    let Some(memo) = directive_by_name(element, "memo") else {
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
                branch_key,
                memo_index,
            );
            return render_with_v_once(rendered, once_index);
        }
        let scope = if scope.in_v_once {
            scope.with_v_once()
        } else {
            scope.clone()
        };
        return render_plain_element(
            ast, node_id, element, options, mode, &scope, branch_key, memo_index,
        );
    };
    let cache_index = scope
        .memo_index_overrides
        .get(&node_id)
        .copied()
        .unwrap_or_else(|| memo_index.alloc());
    let once_index = (directive_by_name(element, "once").is_some() && !scope.in_v_once)
        .then(|| memo_index.alloc());
    let memo_mode = if element.tag_type == Vue3ElementType::Component {
        if once_index.is_some() && matches!(mode, NodeRenderMode::Root) {
            NodeRenderMode::OnceRoot
        } else {
            mode
        }
    } else if once_index.is_some() && matches!(mode, NodeRenderMode::Root) {
        NodeRenderMode::OnceBlockRoot
    } else {
        NodeRenderMode::Root
    };
    let scope = if once_index.is_some() || scope.in_v_once {
        scope.with_v_once()
    } else {
        scope.clone()
    };
    let rendered = render_plain_element(
        ast, node_id, element, options, memo_mode, &scope, branch_key, memo_index,
    );
    let rendered = render_with_memo(memo, rendered, options, &scope, cache_index);
    if let Some(index) = once_index {
        render_with_v_once(rendered, index)
    } else {
        rendered
    }
}

pub(crate) fn render_with_memo(
    memo: &Vue3Directive,
    rendered: String,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    index: usize,
) -> String {
    let expression = memo
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let expression = rewrite_expression_with_scope(&expression, options, scope);
    format!("_withMemo({expression}, () => {rendered}, _cache, {index})")
}

pub(crate) fn render_call_args(
    tag: String,
    props: Option<&str>,
    children: Option<&str>,
    patch_flag: &str,
    dynamic_props: &str,
) -> String {
    let mut args = vec![tag];
    if let Some(props) = props {
        args.push(props.to_string());
    } else if children.is_some() || !patch_flag.is_empty() || !dynamic_props.is_empty() {
        args.push("null".into());
    }
    if let Some(children) = children {
        args.push(children.to_string());
    } else if !patch_flag.is_empty() || !dynamic_props.is_empty() {
        args.push("null".into());
    }
    if !patch_flag.is_empty() {
        args.push(patch_flag.trim_start_matches(", ").to_string());
    }
    if !dynamic_props.is_empty() {
        args.push(dynamic_props.trim_start_matches(", ").to_string());
    }
    args.join(", ")
}

pub(crate) fn render_component_element(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: Option<usize>,
    memo_index: &mut MemoIndex,
) -> String {
    let tag = render_component_tag_expr(element, options, scope);
    let props = render_props(node_id, element, options, scope, branch_key, memo_index);
    let attrs = if props.is_empty() { None } else { Some(props) };
    let children = render_component_slots(ast, node_id, options, scope, memo_index);
    let patch_flag = render_patch_flag_text(component_patch_flag_kind(
        ast, node_id, element, options, scope,
    ));
    let helper = if mode == NodeRenderMode::Root {
        "_createBlock"
    } else {
        "_createVNode"
    };
    let args = render_call_args(
        tag,
        attrs.as_deref(),
        children.as_deref(),
        patch_flag.as_str(),
        dynamic_props_arg(element, options, scope).as_str(),
    );
    let rendered = if mode == NodeRenderMode::Root {
        format!("(_openBlock(), {}({}))", helper, args)
    } else {
        format!("{}({})", helper, args)
    };
    render_with_runtime_directives(rendered, element, options, scope)
}

pub(crate) fn render_component_tag_expr(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    if let Some(expression) = vue3_dynamic_component_is_expression(element) {
        return format!(
            "_resolveDynamicComponent({})",
            rewrite_expression_with_scope(&expression.source_string(), options, scope)
        );
    }
    if let Some(helper) = vue3_core_component_runtime_helper(&element.tag) {
        return helper_reference(helper);
    }
    if let Some(expression) = render_direct_setup_or_props_component_tag(&element.tag, options) {
        return expression;
    }
    if let Some(expression) =
        render_namespaced_setup_or_props_component_tag(&element.tag, options, scope)
    {
        return expression;
    }
    component_asset_id(&element.tag)
}

pub(crate) fn render_direct_setup_or_props_component_tag(
    tag: &str,
    options: &Vue3CompilerOptions,
) -> Option<String> {
    let name = setup_reference_name_for_tag(tag, options)?;
    match options.binding_metadata.get(&name).map(String::as_str) {
        Some("setup-const" | "setup-reactive-const" | "literal-const") if options.inline => {
            Some(name.to_string())
        }
        Some("setup-let" | "setup-ref" | "setup-maybe-ref") if options.inline => {
            Some(format!("_unref({name})"))
        }
        Some("props") if options.inline => {
            Some(format!("_unref(__props[{}])", quote_string(&name)))
        }
        Some(kind) if kind.starts_with("setup") || kind == "literal-const" => {
            Some(format!("$setup[{}]", quote_string(&name)))
        }
        Some("props") => Some(format!("_unref($props[{}])", quote_string(&name))),
        _ => None,
    }
}

pub(crate) fn render_namespaced_setup_or_props_component_tag(
    tag: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    let (namespace, member) = tag.split_once('.')?;
    if namespace.is_empty() || member.is_empty() {
        return None;
    }
    match options.binding_metadata.get(namespace).map(String::as_str) {
        Some("setup-ref" | "setup-maybe-ref" | "setup-let" | "props" | "props-aliased")
            if options.inline =>
        {
            render_setup_or_props_component_namespace(namespace, options, scope)
                .map(|namespace| format!("{namespace}.{member}"))
        }
        Some(kind) if kind.starts_with("setup") || kind == "literal-const" => {
            let namespace = rewrite_identifier_with_scope(namespace, options, scope);
            Some(format!("{namespace}.{member}"))
        }
        _ => None,
    }
}

pub(crate) fn render_setup_or_props_component_namespace(
    namespace: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    if !uses_prefixed_identifiers(options) || scope.locals.iter().any(|local| local == namespace) {
        return Some(namespace.to_string());
    }
    match options.binding_metadata.get(namespace).map(String::as_str) {
        Some("setup-ref" | "setup-maybe-ref" | "setup-let") if options.inline => {
            Some(format!("_unref({namespace})"))
        }
        Some("props") if options.inline => {
            Some(format!("_unref(__props[{}])", quote_string(namespace)))
        }
        Some("props-aliased") if options.inline => {
            let source = options
                .props_aliases
                .get(namespace)
                .map_or(namespace, String::as_str);
            Some(format!(
                "_unref({})",
                render_props_access("__props", source)
            ))
        }
        _ => None,
    }
}

pub(crate) fn render_with_runtime_directives(
    vnode: String,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let directives = element
        .props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "model" => {
                render_model_runtime_directive_arg(element, dir, options, scope)
            }
            Vue3Prop::Directive(dir) if vue3_directive_needs_runtime_asset(&dir.name) => {
                Some(render_runtime_directive_arg(dir, options, scope))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if directives.is_empty() {
        vnode
    } else {
        format!("_withDirectives({vnode}, {})", render_array(&directives))
    }
}

pub(crate) fn render_runtime_directive_arg(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let runtime = if dir.name == "show" {
        "_vShow".to_string()
    } else if let Some(runtime) = render_setup_runtime_directive(&dir.name, options) {
        runtime
    } else {
        directive_asset_id(&dir.name)
    };
    let mut args = vec![runtime];
    if let Some(exp) = dir.exp.as_ref() {
        args.push(rewrite_expression_with_scope(
            &exp.source_string(),
            options,
            scope,
        ));
    } else if dir.arg.is_some() || !dir.modifiers.is_empty() {
        args.push("void 0".into());
    }
    if let Some(arg) = dir.arg.as_ref() {
        let arg = if dir.is_dynamic_arg {
            rewrite_expression_with_scope(&arg.source_string(), options, scope)
        } else {
            quote_string(&arg.source_string())
        };
        args.push(arg);
    } else if !dir.modifiers.is_empty() {
        args.push("void 0".into());
    }
    if !dir.modifiers.is_empty() {
        let modifiers = dir
            .modifiers
            .iter()
            .map(|modifier| format!("{}: true", json_key(modifier)))
            .collect::<Vec<_>>();
        args.push(render_object(&modifiers));
    }
    format!("[{}]", args.join(", "))
}

pub(crate) fn render_setup_runtime_directive(
    name: &str,
    options: &Vue3CompilerOptions,
) -> Option<String> {
    let binding = format!("v-{name}");
    let name = setup_reference_name(&binding, options)?;
    match options.binding_metadata.get(&name).map(String::as_str) {
        Some("setup-const" | "setup-reactive-const" | "literal-const") if options.inline => {
            Some(name.to_string())
        }
        Some("setup-let" | "setup-ref" | "setup-maybe-ref") if options.inline => {
            Some(format!("_unref({name})"))
        }
        Some("props") if options.inline => {
            Some(format!("_unref(__props[{}])", quote_string(&name)))
        }
        Some(kind) if kind.starts_with("setup") || kind == "literal-const" => {
            Some(format!("$setup[{}]", quote_string(&name)))
        }
        Some("props") => Some(format!("_unref($props[{}])", quote_string(&name))),
        _ => None,
    }
}

pub(crate) fn render_model_runtime_directive_arg(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    if dir.arg.is_some() {
        return None;
    }
    let helper = helper_reference(vue3_dom_model_runtime_helper(vue3_dom_model_kind(element)?));
    let mut expression = dir
        .exp
        .as_ref()
        .map(|exp| {
            rewrite_expression_with_scope_preserve_outer(&exp.source_string(), options, scope)
        })
        .unwrap_or_else(|| "undefined".into());
    if expression.contains('\n') {
        expression = dedent_after_first_line(&expression, 4);
    }
    let mut args = vec![helper, expression];
    if !dir.modifiers.is_empty() {
        args.push("void 0".into());
        let modifiers = dir
            .modifiers
            .iter()
            .map(|modifier| format!("{}: true", json_key(modifier)))
            .collect::<Vec<_>>();
        args.push(render_object(&modifiers));
    }
    Some(format!("[{}]", args.join(", ")))
}
