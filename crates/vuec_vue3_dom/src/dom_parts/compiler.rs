/// Parses a Vue 3 template with DOM parser defaults and DOM normalization.
pub fn parse(source: TemplateSource, options: &DomCompilerOptions) -> Vue3Ast {
    let mut ast = Vue3Dialect::base_parse(source, &options.core);
    normalize_dom_ast(&mut ast, options);
    ast
}

/// Compiles a Vue 3 template for DOM render output.
pub fn compile(source: TemplateSource, options: DomCompilerOptions) -> CodegenResult {
    let ast = parse(source.clone(), &options);
    compile_parsed_ast(source, options, ast)
}

fn compile_parsed_ast(
    source: TemplateSource,
    options: DomCompilerOptions,
    mut ast: Vue3Ast,
) -> CodegenResult {
    let mut ctx = TransformContext::default();
    remove_side_effect_nodes(&mut ast, &mut ctx);
    report_transition_invalid_children(&ast, &mut ctx);
    transform_transition_children(&mut ast, &mut ctx);
    report_invalid_native_v_model(&ast, &options.core, &mut ctx);
    let mut asset_imports = Vec::<Vue3ImportItem>::new();
    for node_index in 0..ast.nodes.len() {
        if let Vue3AstKind::Element(element) = &mut ast.nodes[node_index].kind {
            let tag = element.tag.clone();
            if options.transform_asset_urls {
                transform_asset_url_props(
                    &tag,
                    &mut element.props,
                    &options.asset_url_options,
                    options.core.mode == "module",
                    &mut asset_imports,
                );
            }
        }
    }
    if !asset_imports.is_empty() {
        if let Some(root) = vue3_dom_root_mut(&mut ast) {
            root.imports = asset_imports;
        }
    }
    let dom_summary = dom_directive_summary(&ast);
    Vue3Dialect::transform(&mut ast, &mut ctx, &options.core);
    let mut result = Vue3Dialect::finish_compile(ast, source, options.core, ctx);
    result.ast_summary = if dom_summary.is_empty() {
        format!("dom:{}", result.ast_summary)
    } else {
        format!("dom:{};{}", result.ast_summary, dom_summary.join("|"))
    };
    result
}

fn dom_directive_summary(ast: &Vue3Ast) -> Vec<String> {
    ast.nodes
        .iter()
        .filter_map(|node| {
            let Vue3AstKind::Element(element) = &node.kind else {
                return None;
            };
            let summaries = element
                .template_attributes()
                .iter()
                .filter_map(|attr| {
                    parse_directive(attr).map(|directive| match directive.name.as_str() {
                        "html" => "v-html".to_string(),
                        "text" => "v-text".to_string(),
                        "show" => "v-show".to_string(),
                        "model" => {
                            format!("v-model:{}", model_runtime_helper(&element.tag, &directive))
                        }
                        "on" => format!("v-on:{}", directive.modifiers.join(".")),
                        "bind" => format!("v-bind:{}", directive.modifiers.join(".")),
                        _ => format!("v-{}", directive.name),
                    })
                })
                .collect::<Vec<_>>();
            (!summaries.is_empty()).then(|| summaries.join(","))
        })
        .collect()
}

fn transform_transition_children(ast: &mut Vue3Ast, ctx: &mut TransformContext) {
    let transition_ids = ast
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            Vue3AstKind::Element(element)
                if element.tag_type == Vue3ElementType::Component
                    && matches!(element.tag.as_str(), "Transition" | "transition") =>
            {
                Some(node.id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    for node_id in transition_ids {
        let had_ignored_comments = ast.node(node_id).is_some_and(|node| {
            node.children.iter().any(|child_id| {
                ast.node(*child_id)
                    .is_some_and(|child| matches!(child.kind, Vue3AstKind::Comment(_)))
            })
        });
        let visible_children = ast
            .node(node_id)
            .map(|node| transition_visible_child_ids(ast, &node.children))
            .unwrap_or_default();
        if let Some(node) = ast.node_mut(node_id) {
            node.children = visible_children.clone();
        }
        if had_ignored_comments {
            ctx.add_helper(RuntimeHelper::Vue3CreateCommentVNode);
        }
        if transition_single_child_has_v_show(ast, &visible_children) {
            if let Some(node) = ast.node_mut(node_id) {
                if let Vue3AstKind::Element(element) = &mut node.kind {
                    if !element.props.iter().any(|prop| {
                        matches!(prop, Vue3Prop::Attribute(attr) if attr.name == "persisted")
                    }) {
                        element.props.push(Vue3Prop::from(TemplateAttribute {
                            name: "persisted".into(),
                            value: None,
                        }));
                    }
                }
            }
        }
    }
}

fn transition_single_child_has_v_show(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    let [child_id] = children else {
        return false;
    };
    let Some(child) = ast.node(*child_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &child.kind else {
        return false;
    };
    element_has_directive(element, "show")
}

/// Applies DOM-specific AST normalization in-place.
pub fn normalize_dom_ast(ast: &mut Vue3Ast, options: &DomCompilerOptions) {
    for node in &mut ast.nodes {
        match &mut node.kind {
            Vue3AstKind::Text(text) if options.decode_entities => {
                text.value = decode_basic_entities(&text.value);
            }
            Vue3AstKind::Element(element) => {
                if options
                    .is_custom_element
                    .iter()
                    .any(|custom| custom == &element.tag)
                {
                    let mut attributes = element.template_attributes();
                    attributes.push(TemplateAttribute {
                        name: "data-vuec-custom-element".into(),
                        value: None,
                    });
                    element.props = attributes.into_iter().map(Vue3Prop::from).collect();
                }
            }
            _ => {}
        }
    }
}
