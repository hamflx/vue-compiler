pub(crate) fn vue3_ssr_compile_value(
    result: vuec_vue3_ssr::SsrCompileResult,
    source: &TemplateSource,
) -> Value {
    json!({
        "code": result.code,
        "map": result.map,
        "ast_helpers": result.ast_helpers,
        "ast_summary": result.ast_summary,
        "diagnostics": vue3_compile_diagnostics_value(
            &result.diagnostics,
            &source.source,
            source.base_offset,
        ),
        "preamble": result.preamble,
    })
}

pub(crate) fn vue3_sfc_compile_template_value(
    payload: &Value,
    filename: &str,
    compile_source: &str,
    public_source: &str,
    sfc_options: &SfcTemplateCompileOptions,
) -> Value {
    let bridge_options = payload
        .get("bridgeOptions")
        .or_else(|| payload.get("options"))
        .unwrap_or(&Value::Null);
    let source = template_source_from_transformed_sfc_ast_payload(payload, filename.to_string())
        .or_else(|| template_source_from_ast_payload(payload, filename.to_string()))
        .unwrap_or_else(|| TemplateSource {
            filename: filename.to_string(),
            source: compile_source.to_string(),
            file_id: FileId(0),
            base_offset: 0,
        });
    let mut core = vue3_options(Some(bridge_options));
    core.prefix_identifiers = true;
    core.mode = "module".into();
    core.hoist_static = sfc_options.hoist_static;
    core.cache_handlers = true;
    core.scope_id = sfc_options.scope_id.clone();
    core.slotted = sfc_options.slotted;
    core.source_map = true;
    core.ssr = sfc_options.ssr;
    if core.source_map_source.is_none() {
        if let Some(ast_source) = payload
            .get("ast")
            .and_then(|ast| ast.get("source"))
            .and_then(Value::as_str)
        {
            core.source_map_source = Some(ast_source.to_string());
            core.source_map_base_offset = 0;
        }
    }
    apply_bridge_dom_parser_defaults(&mut core, Some(bridge_options));

    if sfc_options.ssr {
        let result = vuec_vue3_ssr::compile(
            source.clone(),
            SsrCompilerOptions {
                core,
                scope_id: sfc_options.scope_id.clone(),
                slotted: sfc_options.slotted,
                slotted_is_explicit: true,
                mode_is_explicit: true,
                transform_asset_urls: sfc_options.transform_asset_urls,
                asset_url_options: sfc_options.asset_url_options.clone(),
            },
        );
        let errors =
            vue3_compile_diagnostics_value(&result.diagnostics, &source.source, source.base_offset);
        return json!({
            "code": result.code,
            "map": result.map,
            "errors": errors,
            "bindings": [],
            "ast_summary": result.ast_summary,
            "ast": {},
            "preamble": result.preamble,
            "source": public_source,
            "tips": [],
        });
    }

    let result = vuec_vue3_dom::compile(
        source.clone(),
        DomCompilerOptions {
            core,
            transform_asset_urls: sfc_options.transform_asset_urls,
            asset_url_options: sfc_options.asset_url_options.clone(),
            ..DomCompilerOptions::default()
        },
    );
    let errors =
        vue3_compile_diagnostics_value(&result.diagnostics, &source.source, source.base_offset);
    json!({
        "code": result.code,
        "map": result.map,
        "errors": errors,
        "bindings": [],
        "ast_summary": result.ast_summary,
        "ast": {},
        "preamble": result.preamble,
        "source": public_source,
        "tips": [],
    })
}

pub(crate) fn vue3_compile_diagnostics_value(
    diagnostics: &[vuec_diagnostics::Diagnostic],
    source: &str,
    base_offset: usize,
) -> Vec<Value> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "code": diagnostic.code.parse::<u32>().ok().unwrap_or(0),
                "message": diagnostic.message,
                "loc": diagnostic.span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
            })
        })
        .collect()
}

pub(crate) fn vue3_parse_value(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    include_sfc_inner_loc: bool,
    options: &Vue3CompilerOptions,
    include_codegen: bool,
) -> Value {
    let imports = vue3_root_imports_value(ast);
    json!({
        "type": 0,
        "source": source,
        "children": vue3_root_children(ast, source, base_offset, include_sfc_inner_loc, options, include_codegen),
        "helpers": [],
        "components": [],
        "directives": [],
        "hoists": [],
        "imports": imports,
        "cached": [],
        "temps": 0,
        "codegenNode": Value::Null,
        "loc": ast.root_node().map(|node| vue3_loc_value(source, base_offset, &node.span)).unwrap_or_else(vue3_loc_stub_value),
        "__vuecDiagnostics": vue3_parse_diagnostics(ast, source, base_offset, options),
    })
}

pub(crate) fn vue3_root_imports_value(ast: &Vue3Ast) -> Vec<Value> {
    ast.root_node()
        .and_then(|node| match &node.kind {
            Vue3AstKind::Root(root) => Some(&root.imports),
            _ => None,
        })
        .into_iter()
        .flatten()
        .map(vue3_import_item_value)
        .collect()
}

pub(crate) fn vue3_import_item_value(import: &Vue3ImportItem) -> Value {
    json!({
        "exp": {
            "type": 4,
            "content": import.name,
            "isStatic": false,
            "constType": 3,
            "loc": vue3_loc_stub_value(),
        },
        "path": import.path,
    })
}

pub(crate) fn vue3_root_children(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    include_sfc_inner_loc: bool,
    options: &Vue3CompilerOptions,
    include_codegen: bool,
) -> Vec<Value> {
    ast.node(ast.root)
        .map(|root| {
            root.children
                .iter()
                .filter_map(|child_id| ast.node(*child_id))
                .map(|node| {
                    vue3_node_summary(
                        ast,
                        source,
                        base_offset,
                        node.id,
                        include_sfc_inner_loc,
                        options,
                        include_codegen,
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn vue3_node_summary(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    node_id: vuec_ast::NodeId,
    include_sfc_inner_loc: bool,
    options: &Vue3CompilerOptions,
    include_codegen: bool,
) -> Value {
    let Some(node) = ast.node(node_id) else {
        return Value::Null;
    };
    match &node.kind {
        Vue3AstKind::Root(root) => json!({
            "type": 0,
            "source": source,
            "children": node.children.iter().filter_map(|child_id| ast.node(*child_id)).map(|child| vue3_node_summary(ast, source, base_offset, child.id, include_sfc_inner_loc, options, include_codegen)).collect::<Vec<_>>(),
            "helpers": [],
            "components": [],
            "directives": [],
            "hoists": [],
            "imports": root.imports.iter().map(vue3_import_item_value).collect::<Vec<_>>(),
            "cached": [],
            "temps": 0,
            "codegenNode": Value::Null,
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
        Vue3AstKind::Element(element) => {
            let mut value = json!({
                "type": 1,
                "tag": element.tag,
                "ns": vue3_namespace_value(element.ns),
                "tagType": vue3_element_type_value(element.tag_type),
                "props": element.props.iter().map(|prop| vue3_prop_value(source, base_offset, prop, options)).collect::<Vec<_>>(),
                "children": node.children.iter().filter_map(|child_id| ast.node(*child_id)).map(|child| vue3_node_summary(ast, source, base_offset, child.id, include_sfc_inner_loc, options, include_codegen)).collect::<Vec<_>>(),
                "loc": vue3_loc_value(source, base_offset, &node.span),
                "codegenNode": Value::Null,
                "isSelfClosing": if element.self_closing { json!(true) } else { json!(null) },
            });
            if include_codegen {
                value["codegenNode"] =
                    vue3_element_codegen_value(ast, node_id, source, base_offset, element, options);
            }
            if include_sfc_inner_loc {
                value["innerLoc"] = vue3_inner_loc_value(ast, source, base_offset, node_id);
            }
            value
        }
        Vue3AstKind::Text(text) => json!({
            "type": 2,
            "content": text.value,
            "loc": vue3_text_loc_value(source, base_offset, &node.span),
        }),
        Vue3AstKind::Interpolation(interpolation) => json!({
            "type": 5,
            "content": vue3_expression_value(source, base_offset, &interpolation.expression, &node.span, false, options, Vue3ExpressionAstMode::Expression),
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
        Vue3AstKind::Comment(comment) => json!({
            "type": 3,
            "content": comment.value,
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
        _ => json!({
            "type": 7,
            "name": "unsupported",
            "exp": null,
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
    }
}

pub(crate) fn vue3_parse_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    options: &Vue3CompilerOptions,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    collect_html_parse_error_diagnostics(source, options, &mut diagnostics);
    collect_invalid_lt_diagnostics(ast, source, base_offset, options, &mut diagnostics);
    collect_missing_interpolation_end_diagnostics(source, options, &mut diagnostics);
    collect_invalid_end_tag_diagnostics(ast, source, base_offset, options, &mut diagnostics);
    collect_missing_directive_name_diagnostics(ast, source, base_offset, &mut diagnostics);
    diagnostics
}

pub(crate) fn vue3_element_codegen_value(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    source: &str,
    base_offset: usize,
    element: &vuec_ast::Vue3Element,
    options: &Vue3CompilerOptions,
) -> Value {
    if element.tag_type != vuec_ast::Vue3ElementType::Element {
        return Value::Null;
    }
    let is_root = ast.node(node_id).and_then(|node| node.parent) == Some(ast.root);
    let patch_flag =
        vuec_vue3_core::vue3_element_codegen_patch_flag(ast, node_id, options, is_root);
    json!({
        "type": 13,
        "tag": format!("\"{}\"", element.tag),
        "props": Value::Null,
        "children": Value::Null,
        "patchFlag": patch_flag,
        "dynamicProps": Value::Null,
        "directives": Value::Null,
        "isBlock": is_root,
        "disableTracking": false,
        "isComponent": false,
        "loc": ast.node(node_id).map(|node| vue3_loc_value(source, base_offset, &node.span)).unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn collect_html_parse_error_diagnostics(
    source: &str,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
    if source.ends_with('<') || (source.ends_with("</") && source.len() <= 2) {
        diagnostics.push(vue3_error_value(
            5,
            vue3_source_loc_value(source, source.len(), source.len()),
        ));
    }
    collect_missing_end_tag_name_diagnostics(source, diagnostics);

    let mut stack = Vec::<OpenDiagnosticElement>::new();
    let mut v_pre_depth = 0usize;
    let mut tokenizer = HtmlTokenizer::new(source);
    loop {
        if v_pre_depth > 0 {
            tokenizer.set_interpolation_delimiters("", "");
        } else if let Some([open, close]) = &options.delimiters {
            tokenizer.set_interpolation_delimiters(open, close);
        } else {
            tokenizer.set_interpolation_delimiters("{{", "}}");
        }
        let token = tokenizer.next_token();
        let eof = matches!(token.kind, HtmlTokenKind::Eof);
        match token.kind {
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let incomplete = tag_token_is_incomplete(source, token.start, token.end);
                collect_start_tag_parse_errors(
                    source,
                    token.start,
                    token.end,
                    &attributes,
                    diagnostics,
                );
                if incomplete && token.end == source.len() {
                    diagnostics.push(vue3_error_value(
                        9,
                        vue3_source_loc_value(source, source.len(), source.len()),
                    ));
                } else if !self_closing && !vue3_is_void_tag(options, &name) {
                    let starts_v_pre =
                        v_pre_depth == 0 && attributes.iter().any(|attr| attr.name == "v-pre");
                    let in_v_pre = v_pre_depth > 0 || starts_v_pre;
                    let namespace =
                        vue3_diagnostic_tag_namespace(options, &name, &attributes, stack.last());
                    let raw_text_kind =
                        vuec_vue3_core::vue3_raw_text_kind(&name, namespace, in_v_pre);
                    let raw_tag = name.clone();
                    let sfc_raw_text =
                        sfc_diagnostic_raw_text_block(options, stack.len(), &raw_tag, &attributes);
                    stack.push(OpenDiagnosticElement {
                        name,
                        start: token.start,
                        namespace,
                        attributes,
                        in_v_pre,
                    });
                    if in_v_pre {
                        v_pre_depth += 1;
                    }
                    if raw_text_kind.is_some() || sfc_raw_text {
                        if let Some((_text_end, end_tag_end)) =
                            vuec_vue3_core::find_matching_raw_text_end(source, token.end, &raw_tag)
                        {
                            tokenizer.set_cursor(end_tag_end);
                            if let Some(open) = stack.pop() {
                                if open.in_v_pre && v_pre_depth > 0 {
                                    v_pre_depth -= 1;
                                }
                            }
                        }
                    }
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if name.is_empty() {
                    if token.end == source.len()
                        && tag_token_is_incomplete(source, token.start, token.end)
                    {
                        let code = if source.as_bytes()[token.start..token.end]
                            .get(2)
                            .is_some_and(u8::is_ascii_whitespace)
                        {
                            9
                        } else {
                            5
                        };
                        diagnostics.push(vue3_error_value(
                            code,
                            vue3_source_loc_value(source, source.len(), source.len()),
                        ));
                    } else {
                        pop_diagnostic_stack_until(&mut stack, &name, &mut v_pre_depth);
                    }
                } else if tag_token_is_incomplete(source, token.start, token.end) {
                    diagnostics.push(vue3_error_value(
                        9,
                        vue3_source_loc_value(source, source.len(), source.len()),
                    ));
                } else {
                    pop_diagnostic_stack_until(&mut stack, &name, &mut v_pre_depth);
                }
            }
            HtmlTokenKind::Comment(_) => {
                if source[token.start..].starts_with("<!--")
                    && token.end == source.len()
                    && !source[token.start..token.end].ends_with("-->")
                {
                    diagnostics.push(vue3_error_value(
                        7,
                        vue3_source_loc_value(source, source.len(), source.len()),
                    ));
                }
            }
            HtmlTokenKind::Cdata(_) => {
                if stack
                    .last()
                    .is_none_or(|open| open.namespace == vuec_ast::HtmlNamespace::Html)
                {
                    diagnostics.push(vue3_error_value(
                        1,
                        vue3_source_loc_value(source, token.start, token.start),
                    ));
                }
                if source[token.start..].starts_with("<![CDATA[")
                    && token.end == source.len()
                    && !source[token.start..token.end].ends_with("]]>")
                {
                    diagnostics.push(vue3_error_value(
                        6,
                        vue3_source_loc_value(source, source.len(), source.len()),
                    ));
                }
            }
            HtmlTokenKind::BogusQuestionTag => {
                diagnostics.push(vue3_error_value(
                    21,
                    vue3_source_loc_value(source, token.start + 1, token.start + 1),
                ));
            }
            HtmlTokenKind::Text(_) | HtmlTokenKind::Doctype(_) | HtmlTokenKind::Eof => {}
        }
        if eof {
            break;
        }
    }
}
