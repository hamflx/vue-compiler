use crate::*;

pub(crate) fn inline_preamble_helpers(helpers: &mut Vec<RuntimeHelper>, expr: &str) {
    if expr.contains("_vModel") {
        let preferred = if expr.contains("_Fragment") {
            &[
                RuntimeHelper::Vue3VModelRadio,
                RuntimeHelper::Vue3VModelCheckbox,
                RuntimeHelper::Vue3VModelText,
                RuntimeHelper::Vue3VModelSelect,
                RuntimeHelper::Vue3VModelDynamic,
                RuntimeHelper::Vue3CreateElementVNode,
                RuntimeHelper::Vue3WithDirectives,
                RuntimeHelper::Vue3Unref,
                RuntimeHelper::Vue3IsRef,
                RuntimeHelper::Vue3Fragment,
                RuntimeHelper::Vue3OpenBlock,
                RuntimeHelper::Vue3CreateElementBlock,
            ][..]
        } else {
            &[
                RuntimeHelper::Vue3Unref,
                RuntimeHelper::Vue3IsRef,
                RuntimeHelper::Vue3VModelRadio,
                RuntimeHelper::Vue3VModelCheckbox,
                RuntimeHelper::Vue3VModelText,
                RuntimeHelper::Vue3VModelSelect,
                RuntimeHelper::Vue3VModelDynamic,
                RuntimeHelper::Vue3WithDirectives,
                RuntimeHelper::Vue3OpenBlock,
                RuntimeHelper::Vue3CreateElementBlock,
            ][..]
        };
        reorder_helpers_by_preference(helpers, preferred);
        return;
    }

    if expr.contains("_createElementVNode") {
        let mut preferred = Vec::new();
        if helpers.contains(&RuntimeHelper::Vue3Unref) {
            preferred.push(RuntimeHelper::Vue3Unref);
        }
        if expr.contains("_toDisplayString") {
            preferred.push(RuntimeHelper::Vue3ToDisplayString);
            if expr.contains("_createTextVNode") {
                preferred.push(RuntimeHelper::Vue3CreateTextVNode);
            }
        }
        if expr.contains("_withCtx") {
            preferred.push(RuntimeHelper::Vue3WithCtx);
            preferred.push(RuntimeHelper::Vue3CreateVNode);
            preferred.push(RuntimeHelper::Vue3CreateElementVNode);
        } else {
            if !expr.contains("_toDisplayString") {
                preferred.clear();
            }
            if helpers.contains(&RuntimeHelper::Vue3Unref)
                && !helpers.contains(&RuntimeHelper::Vue3IsRef)
                && expr.contains("_withDirectives")
            {
                preferred.push(RuntimeHelper::Vue3Unref);
            }
            preferred.push(RuntimeHelper::Vue3CreateElementVNode);
            preferred.push(RuntimeHelper::Vue3IsRef);
            if helpers.contains(&RuntimeHelper::Vue3Unref)
                && !preferred.contains(&RuntimeHelper::Vue3Unref)
            {
                preferred.push(RuntimeHelper::Vue3Unref);
            }
            preferred.push(RuntimeHelper::Vue3WithDirectives);
            preferred.push(RuntimeHelper::Vue3CreateVNode);
        }
        preferred.push(RuntimeHelper::Vue3Fragment);
        preferred.push(RuntimeHelper::Vue3OpenBlock);
        preferred.push(RuntimeHelper::Vue3CreateElementBlock);
        reorder_helpers_by_preference(helpers, &preferred);
    } else {
        if expr.contains("\"onUpdate:")
            && helpers.contains(&RuntimeHelper::Vue3Unref)
            && helpers.contains(&RuntimeHelper::Vue3ResolveComponent)
            && helpers.contains(&RuntimeHelper::Vue3IsRef)
        {
            reorder_helpers_by_preference(
                helpers,
                &[
                    RuntimeHelper::Vue3Unref,
                    RuntimeHelper::Vue3ResolveComponent,
                    RuntimeHelper::Vue3IsRef,
                    RuntimeHelper::Vue3OpenBlock,
                    RuntimeHelper::Vue3CreateBlock,
                ],
            );
            return;
        }
        move_helper_before(
            helpers,
            RuntimeHelper::Vue3Unref,
            RuntimeHelper::Vue3OpenBlock,
        );
    }
}

/// Lower a Vue 3 AST document into the shared HIR and the Vue 3 DOM MIR target.
///
/// The lowering records explicit AST -> HIR and HIR -> MIR edges in
/// `LoweringMap`, and registers template expressions into `JsAstStore`.
/// Lowers a Vue 3 AST into shared HIR plus DOM target MIR.
pub fn lower_vue3_ast_to_dom_mir(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
) -> Vue3DomLoweringResult {
    let root_span = ast
        .root_node()
        .map(|node| node.span.clone())
        .unwrap_or_else(|| NodeSpan::missing(MissingSpanReason::LoweringGap));
    let mut state = Vue3DomLoweringState {
        hir: Hir::new(HirNodeKind::Root(HirRoot), root_span.clone()),
        mir: Vue3DomMir::new(
            Vue3DomMirKind::Root(Vue3DomRoot {
                imports: vue3_codegen_root(ast)
                    .map(|root| root.imports.clone())
                    .unwrap_or_default(),
            }),
            root_span,
        ),
        map: LoweringMap::default(),
        js: JsAstStore::new(),
        options: options.clone(),
        source_type: expression_source_type(options),
        do_not_hoist_root: ast
            .root_node()
            .and_then(|root| vue3_single_static_root_child(&root.children, ast)),
        next_hoist_index: 1,
        next_cache_index: 0,
        in_v_once: 0,
        in_static_hoist: 0,
    };
    state.map.record_ast_to_hir(ast.root, state.hir.root);
    state.map.record_hir_to_mir(state.hir.root, state.mir.root);

    if let Some(root) = ast.root_node() {
        lower_vue3_dom_child_sequence(
            &root.children,
            ast,
            state.hir.root,
            state.mir.root,
            &mut state,
        );
    }

    Vue3DomLoweringResult {
        hir: state.hir,
        mir: state.mir,
        map: state.map,
        js: state.js,
    }
}

/// Lower a Vue 3 AST document into the shared HIR and the Vue 3 SSR MIR target.
///
/// This is a structural contract entry for SSR. It records explicit AST -> HIR
/// and HIR -> MIR edges and keeps SSR output in `Vue3SsrMir` instead of
/// deriving it from DOM MIR.
/// Lowers a Vue 3 AST into shared HIR plus SSR target MIR.
pub fn lower_vue3_ast_to_ssr_mir(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
) -> Vue3SsrLoweringResult {
    let root_span = ast
        .root_node()
        .map(|node| node.span.clone())
        .unwrap_or_else(|| NodeSpan::missing(MissingSpanReason::LoweringGap));
    let mut state = Vue3SsrLoweringState {
        hir: Hir::new(HirNodeKind::Root(HirRoot), root_span.clone()),
        mir: Vue3SsrMir::new(
            Vue3SsrMirKind::Root(Vue3SsrRoot {
                imports: vue3_codegen_root(ast)
                    .map(|root| root.imports.clone())
                    .unwrap_or_default(),
            }),
            root_span,
        ),
        map: LoweringMap::default(),
        js: JsAstStore::new(),
        options: options.clone(),
        source_type: expression_source_type(options),
        select_model_stack: Vec::new(),
        flatten_ssr_fragments: 0,
    };
    state.map.record_ast_to_hir(ast.root, state.hir.root);
    state.map.record_hir_to_mir(state.hir.root, state.mir.root);

    if let Some(root) = ast.root_node() {
        lower_vue3_ssr_child_sequence(
            &root.children,
            ast,
            state.hir.root,
            state.mir.root,
            &mut state,
        );
    }

    Vue3SsrLoweringResult {
        hir: state.hir,
        mir: state.mir,
        map: state.map,
        js: state.js,
    }
}

/// Projects a public AST root codegen node into bridge JSON.
pub fn root_codegen_projection(root: &Value) -> Value {
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    match children {
        [] => json!({ "kind": "none" }),
        [_] => root_single_child_codegen_projection(children),
        _ => json!({
            "kind": "fragment",
            "patchFlag": root_fragment_patch_flag(children),
        }),
    }
}

pub(crate) struct Vue3DomLoweringState {
    pub(crate) hir: Hir,
    pub(crate) mir: Vue3DomMir,
    pub(crate) map: LoweringMap,
    pub(crate) js: JsAstStore,
    pub(crate) options: Vue3CompilerOptions,
    pub(crate) source_type: oxc_span::SourceType,
    pub(crate) do_not_hoist_root: Option<NodeId>,
    pub(crate) next_hoist_index: u32,
    pub(crate) next_cache_index: u32,
    pub(crate) in_v_once: u32,
    pub(crate) in_static_hoist: u32,
}

pub(crate) struct Vue3SsrLoweringState {
    pub(crate) hir: Hir,
    pub(crate) mir: Vue3SsrMir,
    pub(crate) map: LoweringMap,
    pub(crate) js: JsAstStore,
    pub(crate) options: Vue3CompilerOptions,
    pub(crate) source_type: oxc_span::SourceType,
    pub(crate) select_model_stack: Vec<JsExprId>,
    pub(crate) flatten_ssr_fragments: u32,
}
