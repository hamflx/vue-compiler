use crate::*;

pub(crate) fn vue3_base_compile_value(
    source: TemplateSource,
    options: Vue3CompilerOptions,
) -> Value {
    let mut ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut ctx = vuec_pass::TransformContext::default();
    Vue3Dialect::transform(&mut ast, &mut ctx, &options);
    let result = Vue3Dialect::finish_compile(ast.clone(), source.clone(), options.clone(), ctx);
    let ast_value = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        true,
    );
    json!({
        "ast": ast_value,
        "code": result.code,
        "preamble": result.preamble,
        "map": result.map,
        "diagnostics": vue3_compile_diagnostics_value(
            &result.diagnostics,
            &source.source,
            source.base_offset,
        ),
    })
}

pub(crate) fn vue3_compile_value(
    result: vuec_vue3_core::CodegenResult,
    source: &TemplateSource,
) -> Value {
    json!({
        "code": result.code,
        "map": result.map,
        "ast_summary": result.ast_summary,
        "diagnostics": vue3_compile_diagnostics_value(
            &result.diagnostics,
            &source.source,
            source.base_offset,
        ),
        "preamble": result.preamble,
    })
}

#[derive(Default)]
pub(crate) struct Vue3BindSuiteState {
    pub(crate) errors: Vec<Value>,
}

pub(crate) fn vue3_core_transform_bind_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let browser = payload
        .get("options")
        .is_some_and(|options| bool_option(options, "__vuecBrowser", false));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut state = Vue3BindSuiteState::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = std::mem::take(children)
            .into_iter()
            .map(|child| vue3_bind_suite_transform_node(child, &options, browser, &mut state))
            .collect::<Vec<_>>();
        *children = transformed;
    }
    let mut node = root
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .cloned()
        .unwrap_or(Value::Null);
    node["__vuecErrors"] = json!(state.errors);
    node
}

pub(crate) fn vue3_bind_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    browser: bool,
    state: &mut Vue3BindSuiteState,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) {
        vue3_bind_suite_apply_shorthand(&mut node, browser, state);
        vue3_bind_suite_process_directive_expressions(&mut node, options);
    }

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = std::mem::take(children)
            .into_iter()
            .map(|child| vue3_bind_suite_transform_node(child, options, browser, state))
            .collect::<Vec<_>>();
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_bind_suite_codegen(&node, browser, state);
    }
    node
}

pub(crate) fn vue3_bind_suite_apply_shorthand(
    node: &mut Value,
    browser: bool,
    state: &mut Vue3BindSuiteState,
) {
    let projection = vuec_vue3_core::transform_v_bind_shorthand_projection(&json!({
        "node": node,
        "context": { "browser": browser },
    }));
    let operations = projection
        .get("operations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
        return;
    };
    for operation in operations {
        let Some(index) = operation.get("index").and_then(Value::as_u64) else {
            continue;
        };
        let Some(prop) = props.get_mut(index as usize) else {
            continue;
        };
        for error in operation
            .get("errors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            state.errors.push(vue3_bind_suite_error_value(error, prop));
        }
        if operation.get("kind").and_then(Value::as_str) == Some("setExp") {
            let exp = operation.get("exp").cloned().unwrap_or(Value::Null);
            prop["exp"] = vue3_text_suite_materialize_process_projection(&exp, &exp);
        }
    }
}

pub(crate) fn vue3_bind_suite_process_directive_expressions(
    node: &mut Value,
    options: &Vue3CompilerOptions,
) {
    if !options.prefix_identifiers {
        return;
    }
    let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
        return;
    };
    let context = vue3_text_suite_transform_context(options);
    for prop in props {
        if vue3_public_node_type(prop) != Some(7) {
            continue;
        }
        for key in ["exp", "arg"] {
            let Some(current) = prop.get(key).filter(|value| !value.is_null()).cloned() else {
                continue;
            };
            if vue3_public_node_type(&current) != Some(4) {
                continue;
            }
            let projection = vuec_vue3_core::process_expression_projection(&json!({
                "node": current,
                "context": context,
            }));
            prop[key] = vue3_text_suite_materialize_process_projection(&projection, &current);
        }
    }
}

pub(crate) fn vue3_bind_suite_codegen(
    node: &Value,
    browser: bool,
    state: &mut Vue3BindSuiteState,
) -> Value {
    let mut properties = Vec::new();
    let props = node
        .get("props")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for prop in props {
        if vue3_public_node_type(prop) != Some(7)
            || prop.get("name").and_then(Value::as_str) != Some("bind")
            || prop.get("arg").is_none_or(Value::is_null)
        {
            continue;
        }
        let projection = vuec_vue3_core::transform_bind_projection(&json!({
            "dir": prop,
            "context": vue3_bind_suite_transform_context(browser),
        }));
        for error in projection
            .get("errors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            state.errors.push(vue3_bind_suite_error_value(error, prop));
        }
        for projected_prop in projection
            .get("props")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            properties.push(vue3_once_suite_object_property(
                vue3_bind_suite_materialize_projection(projected_prop.get("key"), prop),
                vue3_bind_suite_materialize_projection(projected_prop.get("value"), prop),
            ));
        }
    }

    let props_value = if properties.is_empty() {
        Value::Null
    } else {
        let object = json!({
            "type": 15,
            "properties": properties,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        });
        if vue3_bind_suite_has_dynamic_key(&object) {
            vue3_text_suite_call("NORMALIZE_PROPS", vec![object])
        } else {
            object
        }
    };

    vue3_text_suite_vnode_call(
        &format!(
            "\"{}\"",
            node.get("tag").and_then(Value::as_str).unwrap_or("")
        ),
        props_value,
        Value::Null,
        None,
        false,
        false,
        Value::Null,
    )
}

pub(crate) fn vue3_bind_suite_materialize_projection(
    projection: Option<&Value>,
    dir: &Value,
) -> Value {
    let Some(projection) = projection else {
        return Value::Null;
    };
    if projection.is_string() || projection.get("type").is_some() {
        return projection.clone();
    }
    match projection.get("kind").and_then(Value::as_str) {
        Some("undefined") => Value::Null,
        Some("node") => match projection.get("path").and_then(Value::as_str) {
            Some("dir.arg") => dir.get("arg").cloned().unwrap_or(Value::Null),
            Some("dir.exp") => dir.get("exp").cloned().unwrap_or(Value::Null),
            Some("dir.arg.children") => dir
                .get("arg")
                .and_then(|arg| arg.get("children"))
                .cloned()
                .unwrap_or_else(|| json!([])),
            _ => Value::Null,
        },
        Some("children") => projection
            .get("children")
            .and_then(Value::as_array)
            .map(|children| {
                Value::Array(
                    children
                        .iter()
                        .flat_map(|child| {
                            let materialized =
                                vue3_bind_suite_materialize_projection(Some(child), dir);
                            match materialized {
                                Value::Array(items) => items,
                                value => vec![value],
                            }
                        })
                        .collect(),
                )
            })
            .unwrap_or_else(|| json!([])),
        Some("helperString") => {
            let helper = projection
                .get("helper")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Value::String(format!("_{}(", vue3_bind_suite_helper_name(helper)))
        }
        Some("static") | Some("simple") => json!({
            "type": 4,
            "content": projection.get("content").and_then(Value::as_str).unwrap_or(""),
            "isStatic": projection
                .get("isStatic")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| projection.get("kind").and_then(Value::as_str) == Some("static")),
            "constType": projection.get("constType").and_then(Value::as_u64).unwrap_or_else(|| {
                let is_static = projection
                    .get("isStatic")
                    .and_then(Value::as_bool)
                    .unwrap_or_else(|| projection.get("kind").and_then(Value::as_str) == Some("static"));
                if is_static { 3 } else { 0 }
            }),
            "loc": projection
                .get("loc")
                .cloned()
                .or_else(|| dir.get("exp").and_then(|exp| exp.get("loc")).cloned())
                .or_else(|| dir.get("arg").and_then(|arg| arg.get("loc")).cloned())
                .or_else(|| dir.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("compound") => {
            let children = projection
                .get("children")
                .and_then(Value::as_array)
                .map(|children| {
                    children
                        .iter()
                        .flat_map(|child| {
                            let materialized =
                                vue3_bind_suite_materialize_projection(Some(child), dir);
                            match materialized {
                                Value::Array(items) => items,
                                value => vec![value],
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            json!({
                "type": 8,
                "children": children,
                "loc": projection
                    .get("loc")
                    .cloned()
                    .or_else(|| dir.get("arg").and_then(|arg| arg.get("loc")).cloned())
                    .or_else(|| dir.get("loc").cloned())
                    .unwrap_or_else(vue3_loc_stub_value),
            })
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_bind_suite_has_dynamic_key(node: &Value) -> bool {
    if vue3_public_node_type(node) == Some(15) {
        return node
            .get("properties")
            .and_then(Value::as_array)
            .is_some_and(|properties| {
                properties.iter().any(|property| {
                    vue3_bind_suite_property_key_is_dynamic(
                        property.get("key").unwrap_or(&Value::Null),
                    )
                })
            });
    }
    if vue3_public_node_type(node) == Some(16) {
        return vue3_bind_suite_property_key_is_dynamic(node.get("key").unwrap_or(&Value::Null));
    }
    vue3_bind_suite_property_key_is_dynamic(node)
}

pub(crate) fn vue3_bind_suite_property_key_is_dynamic(key: &Value) -> bool {
    if vue3_public_node_type(key) == Some(8) {
        return true;
    }
    vue3_public_node_type(key) == Some(4)
        && !key
            .get("isStatic")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

pub(crate) fn vue3_bind_suite_error_value(error: &Value, dir: &Value) -> Value {
    let code = error
        .as_u64()
        .or_else(|| error.get("code").and_then(Value::as_u64))
        .unwrap_or(0);
    let loc = if error.get("loc").and_then(Value::as_str) == Some("arg") {
        dir.get("arg")
            .and_then(|arg| arg.get("loc"))
            .cloned()
            .or_else(|| dir.get("loc").cloned())
            .unwrap_or_else(vue3_loc_stub_value)
    } else {
        dir.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value)
    };
    json!({ "code": code, "loc": loc })
}

pub(crate) fn vue3_bind_suite_transform_context(browser: bool) -> Value {
    json!({
        "inSSR": false,
        "browser": browser,
    })
}

pub(crate) fn vue3_bind_suite_helper_name(helper: &str) -> &str {
    match helper {
        "CAMELIZE" => "camelize",
        "NORMALIZE_PROPS" => "normalizeProps",
        "TO_HANDLER_KEY" => "toHandlerKey",
        _ => helper,
    }
}

#[derive(Clone, Default)]
pub(crate) struct Vue3ModelSuiteScope {
    pub(crate) identifiers: BTreeMap<String, usize>,
    pub(crate) in_v_once: bool,
    pub(crate) v_for_depth: usize,
    pub(crate) v_slot_depth: usize,
}

#[derive(Default)]
pub(crate) struct Vue3ModelSuiteState {
    pub(crate) errors: Vec<Value>,
    pub(crate) cached: usize,
}

#[derive(Default)]
pub(crate) struct Vue3ForSuiteState {
    pub(crate) errors: Vec<Value>,
}
