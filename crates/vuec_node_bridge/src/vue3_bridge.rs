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

pub(crate) fn vue3_core_transform_for_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut state = Vue3ForSuiteState::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_for_suite_transform_children(
            std::mem::take(children),
            &options,
            &mut state,
            &Vue3ModelSuiteScope::default(),
        );
        *children = transformed;
    }
    vue3_for_suite_finalize_root(&mut root);
    root["__vuecErrors"] = json!(state.errors);
    let node = root
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "root": root,
        "node": node,
    })
}

pub(crate) fn vue3_for_suite_transform_children(
    children: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Vec<Value> {
    let mut transformed = Vec::new();
    let mut index = 0usize;
    while index < children.len() {
        let child = children[index].clone();
        if vue3_public_node_type(&child) == Some(1)
            && vue3_text_suite_directive(&child, "if").is_some()
        {
            let if_node =
                vue3_for_suite_transform_if_node(child, &children, index, options, state, scope);
            transformed.push(if_node);
        } else {
            transformed.push(vue3_for_suite_transform_node(child, options, state, scope));
        }
        index += 1;
    }
    transformed
}

pub(crate) fn vue3_for_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) {
        vue3_for_suite_apply_bind_shorthand(&mut node, state);
        if vue3_text_suite_directive(&node, "for").is_some() {
            return vue3_for_suite_transform_for_node(node, options, state, scope);
        }
        vue3_model_suite_process_directive_expressions(&mut node, options, scope);
    }

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed =
            vue3_for_suite_transform_children(std::mem::take(children), options, state, scope);
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(5) {
        vue3_for_suite_process_expression_node(&mut node, "content", options, scope);
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_for_suite_element_codegen(&node, options, state, scope, false);
    }
    node
}

pub(crate) fn vue3_for_suite_transform_if_node(
    mut node: Value,
    siblings: &[Value],
    index: usize,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    vue3_for_suite_apply_bind_shorthand(&mut node, state);
    let Some(dir) = vue3_text_suite_directive(&node, "if").cloned() else {
        return vue3_for_suite_transform_node(node, options, state, scope);
    };
    let projection = vuec_vue3_core::transform_if_projection(&json!({
        "node": node,
        "dir": dir,
        "siblings": siblings,
        "nodeIndex": index,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    for error in projection
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(vue3_model_suite_error_value(error, &dir));
    }

    let mut branch_child = node.clone();
    vue3_text_suite_remove_directive(&mut branch_child, "if");
    let transformed_child = vue3_for_suite_transform_node(branch_child, options, state, scope);
    let condition = projection
        .get("branch")
        .and_then(|branch| branch.get("condition"))
        .filter(|condition| !condition.is_null())
        .map(|condition| vue3_text_suite_materialize_process_projection(condition, condition))
        .or_else(|| dir.get("exp").cloned())
        .unwrap_or(Value::Null);

    let loc = node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value);
    let mut if_node = json!({
        "type": 9,
        "branches": [{
            "type": 10,
            "condition": condition,
            "children": [transformed_child],
            "userKey": Value::Null,
            "isTemplateIf": node.get("tagType").and_then(Value::as_u64) == Some(3),
            "loc": loc,
        }],
        "codegenNode": Value::Null,
        "loc": loc,
    });
    let key_base = projection
        .get("action")
        .and_then(|action| action.get("keyBase"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    if_node["codegenNode"] = vue3_for_suite_if_codegen(&mut if_node, key_base);
    if_node
}

pub(crate) fn vue3_for_suite_transform_for_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let Some(dir) = vue3_text_suite_directive(&node, "for").cloned() else {
        return vue3_for_suite_transform_node(node, options, state, scope);
    };
    let context = vue3_model_suite_transform_context(options, scope);
    let projection = vuec_vue3_core::transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }));
    for error in projection
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(vue3_model_suite_error_value(error, &dir));
    }
    for error in projection
        .get("templateKeyErrors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(vue3_model_suite_error_value(error, &dir));
    }
    let parse_result = projection
        .get("parseResult")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_for_parse_result)
        .unwrap_or_else(|| {
            dir.get("forParseResult")
                .map(vue3_text_suite_materialize_for_parse_result)
                .unwrap_or(Value::Null)
        });

    let mut child_scope = scope.clone();
    vue3_model_suite_add_locals(&mut child_scope, projection.get("locals"));
    child_scope.v_for_depth += 1;
    let original_node = node.clone();
    let fallback_loc = node.get("loc").cloned();
    let children = if projection.get("children").and_then(Value::as_str) == Some("template") {
        node.get_mut("children")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default()
            .into_iter()
            .map(|child| vue3_for_suite_transform_node(child, options, state, &child_scope))
            .collect::<Vec<_>>()
    } else {
        vue3_text_suite_remove_directive(&mut node, "for");
        vec![vue3_for_suite_transform_node(
            node,
            options,
            state,
            &child_scope,
        )]
    };

    let loc = dir
        .get("loc")
        .cloned()
        .or(fallback_loc)
        .unwrap_or_else(vue3_loc_stub_value);
    let mut for_node = json!({
        "type": 11,
        "source": parse_result.get("source").cloned().unwrap_or(Value::Null),
        "valueAlias": parse_result.get("value").cloned().unwrap_or(Value::Null),
        "keyAlias": parse_result.get("key").cloned().unwrap_or(Value::Null),
        "objectIndexAlias": parse_result.get("index").cloned().unwrap_or(Value::Null),
        "parseResult": parse_result,
        "children": children,
        "codegenNode": Value::Null,
        "loc": loc,
    });
    for_node["codegenNode"] =
        vue3_for_suite_for_codegen(&original_node, &mut for_node, options, &child_scope, state);
    for_node
}

pub(crate) fn vue3_for_suite_apply_bind_shorthand(node: &mut Value, state: &mut Vue3ForSuiteState) {
    let projection = vuec_vue3_core::transform_v_bind_shorthand_projection(&json!({
        "node": node,
        "context": { "browser": false },
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

pub(crate) fn vue3_for_suite_process_expression_node(
    node: &mut Value,
    key: &str,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) {
    if !options.prefix_identifiers {
        return;
    }
    let current = node.get(key).cloned().unwrap_or(Value::Null);
    if current.is_null() {
        return;
    }
    let projection = vuec_vue3_core::process_expression_projection(&json!({
        "node": current,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    node[key] = vue3_text_suite_materialize_process_projection(&projection, &current);
}

pub(crate) fn vue3_for_suite_for_codegen(
    original_node: &Value,
    for_node: &mut Value,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
    state: &mut Vue3ForSuiteState,
) -> Value {
    let context = vue3_model_suite_transform_context(options, scope);
    let codegen_projection = vuec_vue3_core::transform_for_projection(&json!({
        "phase": "codegen",
        "node": original_node,
        "forNode": vue3_for_suite_for_node_payload(for_node),
        "context": context,
    }));
    let key_property = vue3_for_suite_key_property(
        codegen_projection.get("keyProperty"),
        original_node,
        options,
        scope,
    );
    let is_stable_fragment = codegen_projection
        .get("isStableFragment")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let exit_projection = vuec_vue3_core::transform_for_projection(&json!({
        "phase": "exitCodegen",
        "node": original_node,
        "forNode": vue3_for_suite_for_node_payload(for_node),
        "isStableFragment": is_stable_fragment,
    }));
    let child_block = vue3_for_suite_child_block(
        &exit_projection,
        original_node,
        for_node,
        key_property.clone(),
        options,
        scope,
        state,
    );
    let render_list = vue3_text_suite_call(
        "RENDER_LIST",
        vec![
            for_node.get("source").cloned().unwrap_or(Value::Null),
            json!({
                "type": 18,
                "params": vue3_for_suite_loop_params(for_node.get("parseResult").unwrap_or(&Value::Null)),
                "returns": child_block,
                "newline": true,
                "isSlot": false,
                "loc": for_node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }),
        ],
    );
    vue3_once_suite_vnode_call(
        "FRAGMENT",
        Value::Null,
        render_list,
        Some(
            codegen_projection
                .get("fragmentFlag")
                .cloned()
                .unwrap_or_else(|| json!(256)),
        ),
        Value::Null,
        true,
        codegen_projection
            .get("disableTracking")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        false,
    )
}

pub(crate) fn vue3_for_suite_child_block(
    projection: &Value,
    original_node: &Value,
    for_node: &mut Value,
    key_property: Option<Value>,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
    state: &mut Vue3ForSuiteState,
) -> Value {
    match projection.get("kind").and_then(Value::as_str) {
        Some("slotOutlet") => {
            let slot = if projection.get("path").and_then(Value::as_str) == Some("templateChild") {
                let index = projection.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                for_node
                    .get("children")
                    .and_then(Value::as_array)
                    .and_then(|children| children.get(index))
                    .cloned()
                    .unwrap_or(Value::Null)
            } else {
                for_node
                    .get("children")
                    .and_then(Value::as_array)
                    .and_then(|children| children.first())
                    .cloned()
                    .unwrap_or(Value::Null)
            };
            let mut codegen = slot.get("codegenNode").cloned().unwrap_or(Value::Null);
            if projection.get("path").and_then(Value::as_str) == Some("templateChild") {
                if let Some(key_property) = key_property {
                    vue3_for_suite_inject_prop(&mut codegen, key_property);
                }
            }
            codegen
        }
        Some("fragmentWrapper") => {
            let props = key_property
                .map(|key| {
                    json!({
                        "type": 15,
                        "properties": [key],
                        "loc": for_node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
                    })
                })
                .unwrap_or(Value::Null);
            vue3_once_suite_vnode_call(
                "FRAGMENT",
                props,
                for_node
                    .get("children")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
                Some(
                    projection
                        .get("patchFlag")
                        .cloned()
                        .unwrap_or_else(|| json!(64)),
                ),
                Value::Null,
                true,
                false,
                false,
            )
        }
        _ => {
            let mut child_block = for_node
                .get("children")
                .and_then(Value::as_array)
                .and_then(|children| children.first())
                .and_then(|child| child.get("codegenNode"))
                .cloned()
                .unwrap_or(Value::Null);
            if original_node.get("tagType").and_then(Value::as_u64) == Some(3) {
                if let Some(key_property) = key_property {
                    vue3_for_suite_inject_prop(&mut child_block, key_property);
                }
            }
            if vue3_public_node_type(&child_block) == Some(13) {
                child_block["isBlock"] = json!(projection
                    .get("childBlockIsBlock")
                    .and_then(Value::as_bool)
                    .unwrap_or(true));
            }
            let _ = (options, scope, state);
            child_block
        }
    }
}

pub(crate) fn vue3_for_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(2)) => {
            let projection = vuec_vue3_core::transform_slot_outlet_projection(&json!({
                "node": node,
                "context": vue3_for_suite_slot_outlet_context(options, scope),
            }));
            let non_name_props = projection
                .get("process")
                .and_then(|process| process.get("nonNameProps"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut slot_state = Vue3SlotOutletSuiteState::default();
            let slot_props = vue3_slot_outlet_suite_props_codegen(
                node,
                &non_name_props,
                options,
                &mut slot_state,
            );
            for error in slot_state.errors {
                state.errors.push(error);
            }
            let slot_name = vue3_slot_outlet_suite_slot_name(node, projection.get("process"));
            vue3_slot_outlet_suite_codegen(
                node,
                slot_name,
                slot_props,
                projection.get("codegen").unwrap_or(&Value::Null),
            )
        }
        (Some(1), Some(0)) => {
            let (props, mut patch_flag, dynamic_props, directives) =
                vue3_for_suite_props_codegen(node, options, state, scope);
            let children = node
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let children = if children.is_empty() {
                Value::Null
            } else if children.len() == 1 && vue3_text_suite_direct_child_value(&children[0]) {
                children[0].clone()
            } else {
                Value::Array(children)
            };
            if patch_flag.is_none()
                && vue3_suite_child_needs_text_patch_flag(&children, options, scope)
            {
                patch_flag = Some(json!(1));
            }
            if patch_flag.is_none() && !directives.is_null() {
                patch_flag = Some(json!(512));
            }
            let mut vnode = vue3_text_suite_vnode_call(
                &vue3_once_suite_quote_string(
                    node.get("tag").and_then(Value::as_str).unwrap_or(""),
                ),
                props,
                children,
                patch_flag,
                is_block,
                false,
                directives,
            );
            vnode["dynamicProps"] = dynamic_props;
            vnode
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_for_suite_props_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> (Value, Option<Value>, Value, Value) {
    let mut properties = Vec::new();
    let mut merge_args = Vec::<Value>::new();
    let mut dynamic_props = Vec::<Value>::new();
    let mut runtime_directives = Vec::<Value>::new();
    let mut prop_summaries = Vec::<Value>::new();
    for prop in node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match vue3_public_node_type(prop) {
            Some(6) => {
                let Some(name) = prop.get("name").and_then(Value::as_str) else {
                    continue;
                };
                prop_summaries.push(json!({
                    "kind": "attribute",
                    "name": name,
                }));
                let value = prop
                    .get("value")
                    .and_then(|value| value.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                properties.push(vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression(name, true),
                    vue3_once_suite_simple_expression(value, true),
                ));
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("bind") => {
                if prop.get("arg").is_none_or(Value::is_null) {
                    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
                        prop_summaries.push(json!({ "kind": "objectBind" }));
                        vue3_for_suite_push_props_object_arg(
                            &mut merge_args,
                            &mut properties,
                            node,
                        );
                        merge_args.push(exp.clone());
                    } else {
                        state
                            .errors
                            .push(vue3_bind_suite_error_value(&json!(34), prop));
                    }
                    continue;
                }
                let projection = vuec_vue3_core::transform_bind_projection(&json!({
                    "dir": prop,
                    "context": vue3_bind_suite_transform_context(false),
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
                    let key =
                        vue3_bind_suite_materialize_projection(projected_prop.get("key"), prop);
                    let value =
                        vue3_bind_suite_materialize_projection(projected_prop.get("value"), prop);
                    if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                        if name != "key" {
                            dynamic_props.push(Value::String(vue3_once_suite_quote_string(&name)));
                        }
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "name": name,
                            "dynamicKey": false,
                            "valueConstant": false,
                        }));
                    } else {
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "dynamicKey": true,
                            "valueConstant": false,
                        }));
                    }
                    properties.push(vue3_once_suite_object_property(key, value));
                }
            }
            Some(7) => {
                let Some(name) = prop.get("name").and_then(Value::as_str) else {
                    continue;
                };
                if !vue3_text_suite_builtin_directive(name) {
                    prop_summaries.push(json!({ "kind": "runtimeDirective" }));
                    runtime_directives.push(Value::Array(vec![Value::String(
                        vue3_text_suite_directive_asset_id(name),
                    )]));
                }
            }
            _ => {}
        }
    }
    let props_projection = vuec_vue3_core::transform_element_props_projection(&json!({
        "props": prop_summaries,
        "hasChildren": node
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| !children.is_empty()),
        "isComponent": false,
        "isDynamicComponent": false,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    let mut props = if merge_args.is_empty() {
        if properties.is_empty() {
            Value::Null
        } else {
            vue3_for_suite_props_object(properties, node)
        }
    } else {
        vue3_for_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
        if merge_args.len() == 1 {
            merge_args.pop().unwrap_or(Value::Null)
        } else {
            vue3_text_suite_call("MERGE_PROPS", merge_args)
        }
    };
    if props_projection
        .get("refForMarker")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        props = vue3_for_suite_prepend_props_expression_prop(
            props,
            vue3_for_suite_ref_for_property(),
            node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        );
    }
    let patch_flag = (!dynamic_props.is_empty()).then(|| json!(8));
    let dynamic_props = if dynamic_props.is_empty() {
        Value::Null
    } else {
        Value::Array(dynamic_props)
    };
    let directives = if runtime_directives.is_empty() {
        Value::Null
    } else {
        json!({
            "type": 17,
            "elements": runtime_directives,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        })
    };
    (props, patch_flag, dynamic_props, directives)
}

pub(crate) fn vue3_for_suite_push_props_object_arg(
    merge_args: &mut Vec<Value>,
    properties: &mut Vec<Value>,
    node: &Value,
) {
    if properties.is_empty() {
        return;
    }
    merge_args.push(vue3_for_suite_props_object(
        std::mem::take(properties),
        node,
    ));
}

pub(crate) fn vue3_for_suite_props_object(properties: Vec<Value>, node: &Value) -> Value {
    json!({
        "type": 15,
        "properties": properties,
        "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_for_suite_ref_for_property() -> Value {
    vue3_once_suite_object_property(
        vue3_once_suite_simple_expression("ref_for", true),
        vue3_once_suite_simple_expression("true", false),
    )
}

pub(crate) fn vue3_for_suite_prepend_props_expression_prop(
    mut props: Value,
    property: Value,
    loc: Value,
) -> Value {
    if props.is_null() || props.is_string() {
        return json!({
            "type": 15,
            "properties": [property],
            "loc": loc,
        });
    }
    if vue3_public_node_type(&props) == Some(15) {
        vue3_for_suite_prepend_object_property(&mut props, property);
        return props;
    }
    if vue3_public_node_type(&props) == Some(14)
        && props.get("callee").and_then(Value::as_str) == Some("MERGE_PROPS")
    {
        if let Some(first) = props
            .get_mut("arguments")
            .and_then(Value::as_array_mut)
            .and_then(|arguments| arguments.first_mut())
        {
            if vue3_public_node_type(first) == Some(15) {
                vue3_for_suite_prepend_object_property(first, property);
                return props;
            }
        }
        if let Some(arguments) = props.get_mut("arguments").and_then(Value::as_array_mut) {
            arguments.insert(
                0,
                json!({
                    "type": 15,
                    "properties": [property],
                    "loc": loc,
                }),
            );
            return props;
        }
    }
    vue3_text_suite_call(
        "MERGE_PROPS",
        vec![
            json!({
                "type": 15,
                "properties": [property],
                "loc": loc,
            }),
            props,
        ],
    )
}

pub(crate) fn vue3_for_suite_prepend_object_property(object: &mut Value, property: Value) {
    let key_name = vue3_model_suite_static_prop_name(property.get("key").unwrap_or(&Value::Null));
    if let Some(properties) = object.get_mut("properties").and_then(Value::as_array_mut) {
        if key_name.is_some_and(|key_name| {
            properties.iter().any(|existing| {
                vue3_model_suite_static_prop_name(existing.get("key").unwrap_or(&Value::Null))
                    .as_deref()
                    == Some(key_name.as_str())
            })
        }) {
            return;
        }
        properties.insert(0, property);
    }
}

pub(crate) fn vue3_for_suite_if_codegen(if_node: &mut Value, key_base: usize) -> Value {
    let branch = if_node
        .get("branches")
        .and_then(Value::as_array)
        .and_then(|branches| branches.first())
        .cloned()
        .unwrap_or(Value::Null);
    let projection = vuec_vue3_core::transform_if_projection(&json!({
        "phase": "branchCodegen",
        "branch": branch,
        "keyIndex": key_base,
    }));
    let mut consequent = branch
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .and_then(|child| child.get("codegenNode"))
        .cloned()
        .unwrap_or(Value::Null);
    if projection.get("kind").and_then(Value::as_str) == Some("for") {
        vue3_for_suite_inject_prop(
            &mut consequent,
            vue3_for_suite_branch_key_property(key_base),
        );
    }
    json!({
        "type": 19,
        "test": branch.get("condition").cloned().unwrap_or(Value::Null),
        "consequent": consequent,
        "alternate": vue3_text_suite_call("CREATE_COMMENT", vec![json!("\"v-if\""), json!("true")]),
        "newline": true,
        "loc": branch.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_for_suite_key_property(
    projection: Option<&Value>,
    original_node: &Value,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) -> Option<Value> {
    let value = projection
        .and_then(|projection| projection.get("value"))
        .filter(|value| !value.is_null())?;
    let materialized = vue3_text_suite_materialize_process_projection(value, value);
    let _ = (original_node, options, scope);
    Some(vue3_once_suite_object_property(
        vue3_once_suite_simple_expression("key", true),
        materialized,
    ))
}

pub(crate) fn vue3_for_suite_branch_key_property(index: usize) -> Value {
    vue3_once_suite_object_property(
        vue3_once_suite_simple_expression("key", true),
        json!({
            "type": 4,
            "content": index.to_string(),
            "isStatic": false,
            "constType": 2,
            "loc": vue3_loc_stub_value(),
        }),
    )
}

pub(crate) fn vue3_for_suite_inject_prop(vnode_call: &mut Value, property: Value) {
    if vue3_public_node_type(vnode_call) != Some(13) {
        return;
    }
    if vnode_call.get("props").is_none_or(Value::is_null) {
        vnode_call["props"] = json!({
            "type": 15,
            "properties": [property],
            "loc": vnode_call.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        });
        return;
    }
    if let Some(properties) = vnode_call
        .get_mut("props")
        .and_then(|props| props.get_mut("properties"))
        .and_then(Value::as_array_mut)
    {
        properties.insert(0, property);
    }
}

pub(crate) fn vue3_for_suite_loop_params(parse_result: &Value) -> Vec<Value> {
    let args = ["value", "key", "index"]
        .into_iter()
        .map(|key| parse_result.get(key).cloned().unwrap_or(Value::Null))
        .collect::<Vec<_>>();
    let Some(last) = args.iter().rposition(|arg| !arg.is_null()) else {
        return Vec::new();
    };
    args.into_iter()
        .take(last + 1)
        .enumerate()
        .map(|(index, arg)| {
            if arg.is_null() {
                vue3_once_suite_simple_expression(&"_".repeat(index + 1), false)
            } else {
                arg
            }
        })
        .collect()
}

pub(crate) fn vue3_for_suite_for_node_payload(for_node: &Value) -> Value {
    json!({
        "source": for_node.get("source").cloned().unwrap_or(Value::Null),
        "children": for_node
            .get("children")
            .and_then(Value::as_array)
            .map(|children| children
                .iter()
                .map(|child| json!({
                    "type": child.get("type").cloned().unwrap_or(Value::Null),
                    "tagType": child.get("tagType").cloned().unwrap_or(Value::Null),
                    "codegenNode": child.get("codegenNode").map(|codegen| json!({
                        "type": codegen.get("type").cloned().unwrap_or(Value::Null),
                        "isBlock": codegen.get("isBlock").cloned().unwrap_or(Value::Bool(false)),
                        "isComponent": codegen.get("isComponent").cloned().unwrap_or(Value::Bool(false)),
                    })).unwrap_or(Value::Null),
                }))
                .collect::<Vec<_>>())
            .unwrap_or_default(),
    })
}

pub(crate) fn vue3_for_suite_slot_outlet_context(
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let mut context = vue3_model_suite_transform_context(options, scope);
    context["scopeId"] = options
        .scope_id
        .clone()
        .map(Value::String)
        .unwrap_or(Value::Null);
    context["slotted"] = json!(options.slotted);
    context
}

pub(crate) fn vue3_for_suite_finalize_root(root: &mut Value) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!([]);
    root["directives"] = json!(vue3_text_suite_collect_directives(root));
    root["helpers"] = json!(vue3_for_suite_helpers(root));
    root["hoists"] = json!([]);
    root["cached"] = json!([]);
    root["temps"] = json!(0);
}

pub(crate) fn vue3_for_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_for_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
    if !vue3_text_suite_collect_directives(root).is_empty() {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_DIRECTIVE");
    }
    [
        "RENDER_LIST",
        "FRAGMENT",
        "OPEN_BLOCK",
        "CREATE_ELEMENT_BLOCK",
        "TO_DISPLAY_STRING",
        "CREATE_ELEMENT_VNODE",
        "RENDER_SLOT",
        "CREATE_COMMENT",
        "RESOLVE_DIRECTIVE",
        "WITH_DIRECTIVES",
    ]
    .into_iter()
    .filter(|helper| used.iter().any(|used| *used == *helper))
    .map(str::to_string)
    .collect()
}

pub(crate) fn vue3_for_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    match vue3_public_node_type(node) {
        Some(5) => vue3_text_suite_add_helper(used, "TO_DISPLAY_STRING"),
        Some(13) => {
            if node.get("tag").and_then(Value::as_str) == Some("FRAGMENT") {
                vue3_text_suite_add_helper(used, "FRAGMENT");
            }
            if node.get("directives").is_some_and(|value| !value.is_null()) {
                vue3_text_suite_add_helper(used, "WITH_DIRECTIVES");
            }
            if node
                .get("isBlock")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "OPEN_BLOCK");
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
            } else {
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_VNODE");
            }
        }
        Some(14) => match node.get("callee").and_then(Value::as_str) {
            Some("RENDER_LIST") => vue3_text_suite_add_helper(used, "RENDER_LIST"),
            Some("RENDER_SLOT") => vue3_text_suite_add_helper(used, "RENDER_SLOT"),
            Some("CREATE_COMMENT") => vue3_text_suite_add_helper(used, "CREATE_COMMENT"),
            _ => {}
        },
        _ => {}
    }
    for key in [
        "children",
        "content",
        "codegenNode",
        "arguments",
        "returns",
        "params",
        "directives",
        "source",
        "valueAlias",
        "keyAlias",
        "objectIndexAlias",
        "parseResult",
        "branches",
        "condition",
        "test",
        "consequent",
        "alternate",
        "value",
        "elements",
        "properties",
        "key",
    ] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_for_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_for_suite_collect_helpers(value, used);
        }
    }
}

#[derive(Default)]
pub(crate) struct Vue3IfSuiteState {
    pub(crate) errors: Vec<Value>,
    pub(crate) cached: usize,
}

pub(crate) fn vue3_core_transform_if_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut state = Vue3IfSuiteState::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        *children = vue3_if_suite_transform_children(
            std::mem::take(children),
            &options,
            &mut state,
            &Vue3ModelSuiteScope::default(),
        );
    }
    vue3_if_suite_finalize_root(&mut root, &state);
    root["__vuecErrors"] = json!(state.errors);
    let node = root
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "root": root,
        "node": node,
    })
}

pub(crate) fn vue3_if_suite_transform_children(
    children: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Vec<Value> {
    let mut transformed = Vec::new();
    let mut index = 0usize;
    let mut key_base = 0usize;
    while index < children.len() {
        let child = children[index].clone();
        if vue3_public_node_type(&child) == Some(1)
            && vue3_text_suite_directive(&child, "if").is_some()
        {
            let (if_node, consumed) =
                vue3_if_suite_transform_if_chain(&children, index, key_base, options, state, scope);
            key_base += if_node
                .get("branches")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or_default();
            transformed.push(if_node);
            index += consumed.max(1);
            continue;
        }
        if vue3_public_node_type(&child) == Some(1)
            && (vue3_text_suite_directive(&child, "else").is_some()
                || vue3_text_suite_directive(&child, "else-if").is_some())
        {
            state.errors.push(json!({
                "code": 30,
                "loc": child.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }));
        }
        transformed.push(vue3_if_suite_transform_node(child, options, state, scope));
        index += 1;
    }
    transformed
}

pub(crate) fn vue3_if_suite_transform_if_chain(
    siblings: &[Value],
    start: usize,
    key_base: usize,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> (Value, usize) {
    let first = siblings.get(start).cloned().unwrap_or(Value::Null);
    let mut branches = Vec::new();
    let first_branch =
        vue3_if_suite_branch_from_node(first.clone(), "if", Vec::new(), options, state, scope);
    branches.push(first_branch);

    let mut scan = start + 1;
    let mut consumed = 1usize;
    while scan < siblings.len() {
        let mut gap = Vec::<Value>::new();
        let mut candidate_index = scan;
        while let Some(candidate) = siblings.get(candidate_index) {
            if !vue3_if_suite_is_comment_or_ascii_whitespace(candidate) {
                break;
            }
            if vue3_public_node_type(candidate) == Some(3) {
                gap.push(candidate.clone());
            }
            candidate_index += 1;
        }
        let Some(candidate) = siblings.get(candidate_index) else {
            break;
        };
        let dir_name = if vue3_public_node_type(candidate) == Some(1)
            && vue3_text_suite_directive(candidate, "else-if").is_some()
        {
            "else-if"
        } else if vue3_public_node_type(candidate) == Some(1)
            && vue3_text_suite_directive(candidate, "else").is_some()
        {
            "else"
        } else {
            break;
        };
        let branch =
            vue3_if_suite_branch_from_node(candidate.clone(), dir_name, gap, options, state, scope);
        if branches
            .last()
            .and_then(|branch| branch.get("condition"))
            .is_some_and(Value::is_null)
        {
            state.errors.push(json!({
                "code": 30,
                "loc": branch.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }));
        }
        let current_key = branch.get("userKey").unwrap_or(&Value::Null);
        if !current_key.is_null()
            && branches.iter().any(|existing| {
                vue3_if_suite_same_user_key(
                    existing.get("userKey").unwrap_or(&Value::Null),
                    current_key,
                )
            })
        {
            state.errors.push(json!({
                "code": 29,
                "loc": current_key
                    .get("loc")
                    .cloned()
                    .unwrap_or_else(vue3_loc_stub_value),
            }));
        }
        branches.push(branch);
        consumed = candidate_index - start + 1;
        scan = candidate_index + 1;
    }

    let loc = first
        .get("loc")
        .cloned()
        .unwrap_or_else(vue3_loc_stub_value);
    let mut if_node = json!({
        "type": 9,
        "branches": branches,
        "codegenNode": Value::Null,
        "loc": loc,
    });
    let codegen = vue3_if_suite_if_codegen(&mut if_node, key_base);
    if_node["codegenNode"] = codegen;
    (if_node, consumed)
}

pub(crate) fn vue3_if_suite_branch_from_node(
    mut node: Value,
    dir_name: &str,
    comments: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    vue3_if_suite_apply_bind_shorthand(&mut node, state);
    let dir = vue3_text_suite_directive(&node, dir_name).cloned();
    let condition = vue3_if_suite_condition(dir.as_ref(), &node, options, state, scope);
    let user_key = vue3_if_suite_user_key(&node).unwrap_or(Value::Null);
    let is_template_if = node.get("tagType").and_then(Value::as_u64) == Some(3);

    vue3_text_suite_remove_directive(&mut node, "if");
    vue3_text_suite_remove_directive(&mut node, "else-if");
    vue3_text_suite_remove_directive(&mut node, "else");

    let mut children = comments;
    if is_template_if && vue3_text_suite_directive(&node, "for").is_none() {
        let template_children = node
            .get_mut("children")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default();
        children.extend(vue3_if_suite_transform_children(
            template_children,
            options,
            state,
            scope,
        ));
    } else {
        children.push(vue3_if_suite_transform_node(
            node.clone(),
            options,
            state,
            scope,
        ));
    }

    json!({
        "type": 10,
        "condition": condition,
        "children": children,
        "userKey": user_key,
        "isTemplateIf": is_template_if,
        "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_if_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) {
        vue3_if_suite_apply_bind_shorthand(&mut node, state);
        vue3_model_suite_process_directive_expressions(&mut node, options, scope);
    }

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        *children =
            vue3_if_suite_transform_children(std::mem::take(children), options, state, scope);
    }

    if vue3_public_node_type(&node) == Some(5) {
        vue3_for_suite_process_expression_node(&mut node, "content", options, scope);
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_if_suite_element_codegen(&node, options, state, scope, false);
    }
    node
}

pub(crate) fn vue3_if_suite_condition(
    dir: Option<&Value>,
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let Some(dir) = dir else {
        return Value::Null;
    };
    if dir.get("name").and_then(Value::as_str) == Some("else") {
        return Value::Null;
    }
    let exp = dir.get("exp").filter(|value| !value.is_null());
    let raw = exp
        .and_then(|exp| exp.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if exp.is_none() || raw.trim().is_empty() {
        state.errors.push(json!({
            "code": 28,
            "loc": dir.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }));
        return json!({
            "type": 4,
            "content": "true",
            "isStatic": false,
            "constType": 0,
            "loc": exp
                .and_then(|exp| exp.get("loc"))
                .cloned()
                .or_else(|| node.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        });
    }
    let current = exp.cloned().unwrap_or(Value::Null);
    if !options.prefix_identifiers {
        return current;
    }
    let projection = vuec_vue3_core::process_expression_projection(&json!({
        "node": current,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    vue3_text_suite_materialize_process_projection(&projection, &current)
}

pub(crate) fn vue3_if_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(2)) => {
            let projection = vuec_vue3_core::transform_slot_outlet_projection(&json!({
                "node": node,
                "context": vue3_for_suite_slot_outlet_context(options, scope),
            }));
            let non_name_props = projection
                .get("process")
                .and_then(|process| process.get("nonNameProps"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut slot_state = Vue3SlotOutletSuiteState::default();
            let slot_props = vue3_slot_outlet_suite_props_codegen(
                node,
                &non_name_props,
                options,
                &mut slot_state,
            );
            for error in slot_state.errors {
                state.errors.push(error);
            }
            let slot_name = vue3_slot_outlet_suite_slot_name(node, projection.get("process"));
            vue3_slot_outlet_suite_codegen(
                node,
                slot_name,
                slot_props,
                projection.get("codegen").unwrap_or(&Value::Null),
            )
        }
        (Some(1), Some(1)) => {
            let (props, patch_flag, dynamic_props, directives, should_use_block) =
                vue3_if_suite_props_codegen(node, options, state, scope);
            let tag = node.get("tag").and_then(Value::as_str).unwrap_or("");
            let mut vnode = vue3_once_suite_vnode_call(
                &vue3_once_suite_component_asset_id(tag),
                props,
                Value::Null,
                patch_flag,
                dynamic_props,
                is_block || should_use_block,
                false,
                true,
            );
            vnode["directives"] = directives;
            vnode
        }
        (Some(1), Some(0)) => {
            let (props, mut patch_flag, dynamic_props, directives, should_use_block) =
                vue3_if_suite_props_codegen(node, options, state, scope);
            let children = node
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let children = if children.is_empty() {
                Value::Null
            } else if children.len() == 1 && vue3_text_suite_direct_child_value(&children[0]) {
                children[0].clone()
            } else {
                Value::Array(children)
            };
            if patch_flag.is_none()
                && vue3_suite_child_needs_text_patch_flag(&children, options, scope)
            {
                patch_flag = Some(json!(1));
            }
            let is_block = is_block || should_use_block;
            if patch_flag.is_none() && !directives.is_null() && !is_block {
                patch_flag = Some(json!(512));
            }
            let mut vnode = vue3_once_suite_vnode_call(
                &vue3_once_suite_quote_string(
                    node.get("tag").and_then(Value::as_str).unwrap_or(""),
                ),
                props,
                children,
                patch_flag,
                dynamic_props,
                is_block,
                false,
                false,
            );
            vnode["directives"] = directives;
            vnode
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_if_suite_props_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> (Value, Option<Value>, Value, Value, bool) {
    let mut properties = Vec::<Value>::new();
    let mut merge_args = Vec::<Value>::new();
    let mut dynamic_props = Vec::<String>::new();
    let mut runtime_directives = Vec::<Value>::new();
    let mut prop_summaries = Vec::<Value>::new();
    let context = vue3_model_suite_transform_context(options, scope);

    for prop in node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match vue3_public_node_type(prop) {
            Some(6) => {
                let Some(name) = prop.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let value = prop
                    .get("value")
                    .and_then(|value| value.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                properties.push(vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression(name, true),
                    vue3_once_suite_simple_expression(value, true),
                ));
                prop_summaries.push(json!({ "kind": "attribute", "name": name, "value": value }));
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("bind") => {
                if prop.get("arg").is_none_or(Value::is_null) {
                    prop_summaries.push(json!({ "kind": "objectBind" }));
                    vue3_if_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
                    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
                        merge_args.push(exp.clone());
                    } else {
                        state
                            .errors
                            .push(vue3_bind_suite_error_value(&json!(34), prop));
                    }
                    continue;
                }
                let projection = vuec_vue3_core::transform_bind_projection(&json!({
                    "dir": prop,
                    "context": vue3_bind_suite_transform_context(false),
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
                    let key =
                        vue3_bind_suite_materialize_projection(projected_prop.get("key"), prop);
                    let value =
                        vue3_bind_suite_materialize_projection(projected_prop.get("value"), prop);
                    let value_constant = vue3_if_suite_value_constant(&value, &context) > 0;
                    if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                        if name != "key" && !value_constant {
                            dynamic_props.push(name.clone());
                        }
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "name": name,
                            "dynamicKey": false,
                            "valueConstant": value_constant,
                            "forceBlock": name == "key",
                        }));
                    } else {
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "dynamicKey": true,
                            "valueConstant": value_constant,
                        }));
                    }
                    properties.push(vue3_once_suite_object_property(key, value));
                }
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("on") => {
                if prop.get("arg").is_none_or(Value::is_null) {
                    prop_summaries.push(json!({ "kind": "objectOn" }));
                    vue3_if_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
                    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
                        let mut args = vec![exp.clone()];
                        if node.get("tagType").and_then(Value::as_u64) == Some(0) {
                            args.push(json!("true"));
                        }
                        merge_args.push(vue3_text_suite_call("TO_HANDLERS", args));
                    } else {
                        state
                            .errors
                            .push(vue3_model_suite_error_value(&json!(35), prop));
                    }
                    continue;
                }
                let projection = vuec_vue3_core::transform_on_projection(&json!({
                    "dir": prop,
                    "node": node,
                    "context": context,
                }));
                for error in projection
                    .get("errors")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    state.errors.push(vue3_model_suite_error_value(error, prop));
                }
                for projected_prop in projection
                    .get("props")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let key = vue3_on_suite_materialize_projection(projected_prop.get("key"), prop);
                    let mut value =
                        vue3_on_suite_materialize_projection(projected_prop.get("value"), prop);
                    let cached = projected_prop
                        .get("cache")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    if cached {
                        value = vue3_if_suite_cache_expression(state, value, false, false, false);
                    }
                    if !projected_prop
                        .get("dynamicKey")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                            if !cached
                                && !projected_prop
                                    .get("valueConstant")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false)
                            {
                                dynamic_props.push(name.clone());
                            }
                            prop_summaries.push(json!({
                                "kind": "directiveProp",
                                "name": name,
                                "dynamicKey": false,
                                "valueConstant": projected_prop
                                    .get("valueConstant")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false),
                                "valueCached": cached,
                            }));
                        }
                    } else {
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "dynamicKey": true,
                            "ignoreDynamicKeyForNormalize": true,
                            "valueConstant": false,
                            "valueCached": cached,
                        }));
                    }
                    properties.push(vue3_once_suite_object_property(key, value));
                }
            }
            Some(7) => {
                let Some(name) = prop.get("name").and_then(Value::as_str) else {
                    continue;
                };
                if !vue3_text_suite_builtin_directive(name) {
                    prop_summaries.push(json!({ "kind": "runtimeDirective" }));
                    runtime_directives.push(Value::Array(vec![Value::String(
                        vue3_text_suite_directive_asset_id(name),
                    )]));
                }
            }
            _ => {}
        }
    }

    let props_projection = vuec_vue3_core::transform_element_props_projection(&json!({
        "props": prop_summaries,
        "hasChildren": node
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| !children.is_empty()),
        "isComponent": node.get("tagType").and_then(Value::as_u64) == Some(1),
        "isDynamicComponent": false,
        "context": context,
    }));

    let mut props = if merge_args.is_empty() {
        if properties.is_empty() {
            Value::Null
        } else {
            vue3_if_suite_props_object(properties, node)
        }
    } else {
        vue3_if_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
        if merge_args.len() == 1 {
            merge_args.pop().unwrap_or(Value::Null)
        } else {
            vue3_text_suite_call("MERGE_PROPS", merge_args)
        }
    };
    vue3_if_suite_apply_props_normalizers(&mut props, &props_projection);
    let patch_flag = props_projection
        .get("patchFlag")
        .and_then(Value::as_u64)
        .filter(|flag| *flag > 0)
        .map(|flag| json!(flag));
    let projected_dynamic_props = props_projection
        .get("dynamicPropNames")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or(dynamic_props);
    let dynamic_props = if projected_dynamic_props.is_empty() {
        Value::Null
    } else {
        Value::String(vue3_model_suite_dynamic_props_string(
            &projected_dynamic_props,
        ))
    };
    let directives = if runtime_directives.is_empty() {
        Value::Null
    } else {
        json!({
            "type": 17,
            "elements": runtime_directives,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        })
    };
    let should_use_block = props_projection
        .get("shouldUseBlock")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    (
        props,
        patch_flag,
        dynamic_props,
        directives,
        should_use_block,
    )
}

pub(crate) fn vue3_if_suite_value_constant(value: &Value, context: &Value) -> u64 {
    if let Some(const_type) = value.get("constType").and_then(Value::as_u64) {
        return const_type;
    }
    vuec_vue3_core::get_constant_type_projection(&json!({
        "node": value,
        "context": context,
    }))
    .get("constantType")
    .and_then(Value::as_u64)
    .unwrap_or(0)
}

pub(crate) fn vue3_suite_child_needs_text_patch_flag(
    child: &Value,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) -> bool {
    if !matches!(vue3_public_node_type(child), Some(5 | 8)) {
        return false;
    }
    let context = vue3_model_suite_transform_context(options, scope);
    vue3_if_suite_value_constant(child, &context) == 0
}

pub(crate) fn vue3_if_suite_apply_props_normalizers(props: &mut Value, projection: &Value) {
    if props.is_null() {
        return;
    }
    let is_call = vue3_public_node_type(props) == Some(14);
    if !is_call {
        if projection
            .get("normalizeClass")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            vue3_if_suite_normalize_object_prop(props, "class", "NORMALIZE_CLASS");
        }
        if projection
            .get("normalizeStyle")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            vue3_if_suite_normalize_object_prop(props, "style", "NORMALIZE_STYLE");
        }
    }
    let guard_reactive_props = projection
        .get("guardReactiveProps")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let normalize_props = projection
        .get("normalizeProps")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if is_call {
        return;
    }
    if guard_reactive_props {
        let current = std::mem::take(props);
        *props = vue3_text_suite_call("GUARD_REACTIVE_PROPS", vec![current]);
    }
    if normalize_props && props.get("callee").and_then(Value::as_str) != Some("NORMALIZE_PROPS") {
        let current = std::mem::take(props);
        *props = vue3_text_suite_call("NORMALIZE_PROPS", vec![current]);
    }
}

pub(crate) fn vue3_if_suite_normalize_object_prop(props: &mut Value, name: &str, helper: &str) {
    match vue3_public_node_type(props) {
        Some(15) => {
            let Some(properties) = props.get_mut("properties").and_then(Value::as_array_mut) else {
                return;
            };
            for property in properties {
                let key_name = property
                    .get("key")
                    .and_then(vue3_model_suite_static_prop_name);
                if key_name.as_deref() != Some(name) {
                    continue;
                }
                let value = property.get("value").cloned().unwrap_or(Value::Null);
                if value.get("callee").and_then(Value::as_str) != Some(helper) {
                    property["value"] = vue3_text_suite_call(helper, vec![value]);
                }
            }
        }
        Some(14) => {
            let Some(arguments) = props.get_mut("arguments").and_then(Value::as_array_mut) else {
                return;
            };
            for argument in arguments {
                vue3_if_suite_normalize_object_prop(argument, name, helper);
            }
        }
        _ => {}
    }
}

pub(crate) fn vue3_if_suite_if_codegen(if_node: &mut Value, key_base: usize) -> Value {
    let branches = if_node
        .get_mut("branches")
        .and_then(Value::as_array_mut)
        .map(Vec::as_mut_slice)
        .unwrap_or(&mut []);
    let mut alternate =
        vue3_text_suite_call("CREATE_COMMENT", vec![json!("\"v-if\""), json!("true")]);
    for (index, branch) in branches.iter_mut().enumerate().rev() {
        let child_codegen = vue3_if_suite_branch_codegen(branch, key_base + index);
        let condition = branch.get("condition").cloned().unwrap_or(Value::Null);
        if condition.is_null() {
            alternate = child_codegen;
        } else {
            alternate = json!({
                "type": 19,
                "test": condition,
                "consequent": child_codegen,
                "alternate": alternate,
                "newline": true,
                "loc": branch.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            });
        }
    }
    alternate
}

pub(crate) fn vue3_if_suite_branch_codegen(branch: &mut Value, key_index: usize) -> Value {
    let projection = vuec_vue3_core::transform_if_projection(&json!({
        "phase": "branchCodegen",
        "branch": branch,
        "keyIndex": key_index,
    }));
    let key_property = vue3_for_suite_branch_key_property(key_index);
    match projection.get("kind").and_then(Value::as_str) {
        Some("for") => {
            if let Some(codegen) = branch
                .get_mut("children")
                .and_then(Value::as_array_mut)
                .and_then(|children| children.first_mut())
                .and_then(|child| child.get_mut("codegenNode"))
            {
                vue3_if_suite_inject_prop(codegen, key_property);
                return codegen.clone();
            }
            Value::Null
        }
        Some("fragment") => {
            let props = json!({
                "type": 15,
                "properties": [key_property],
                "loc": branch.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            });
            vue3_once_suite_vnode_call(
                "FRAGMENT",
                props,
                branch.get("children").cloned().unwrap_or_else(|| json!([])),
                Some(
                    projection
                        .get("patchFlag")
                        .cloned()
                        .unwrap_or_else(|| json!(64)),
                ),
                Value::Null,
                true,
                false,
                false,
            )
        }
        _ => {
            if let Some(codegen) = branch
                .get_mut("children")
                .and_then(Value::as_array_mut)
                .and_then(|children| children.first_mut())
                .and_then(|child| child.get_mut("codegenNode"))
            {
                if vue3_public_node_type(codegen) == Some(13) {
                    codegen["isBlock"] = json!(true);
                }
                vue3_if_suite_inject_prop(codegen, key_property);
                return codegen.clone();
            }
            Value::Null
        }
    }
}

pub(crate) fn vue3_if_suite_inject_prop(target: &mut Value, property: Value) {
    match vue3_public_node_type(target) {
        Some(13) => {
            let props = target.get("props").cloned().unwrap_or(Value::Null);
            target["props"] = vue3_if_suite_inject_prop_into_props(props, property);
        }
        Some(14) if target.get("callee").and_then(Value::as_str) == Some("RENDER_SLOT") => {
            let Some(arguments) = target.get_mut("arguments").and_then(Value::as_array_mut) else {
                return;
            };
            while arguments.len() <= 2 {
                arguments.push(if arguments.len() == 2 {
                    Value::Null
                } else {
                    Value::String("undefined".to_string())
                });
            }
            let props = arguments.get(2).cloned().unwrap_or(Value::Null);
            arguments[2] = vue3_if_suite_inject_prop_into_props(props, property);
        }
        _ => {}
    }
}

pub(crate) fn vue3_if_suite_inject_prop_into_props(props: Value, property: Value) -> Value {
    if props.is_null() || props.is_string() {
        return vue3_if_suite_props_object(vec![property], &Value::Null);
    }
    if vue3_public_node_type(&props) == Some(15) {
        let mut object = props;
        vue3_for_suite_prepend_object_property(&mut object, property);
        return object;
    }
    if vue3_public_node_type(&props) == Some(14) {
        let callee = props.get("callee").and_then(Value::as_str);
        if callee == Some("NORMALIZE_PROPS") {
            let mut call = props;
            let Some(arguments) = call.get_mut("arguments").and_then(Value::as_array_mut) else {
                return call;
            };
            let first = arguments.first().cloned().unwrap_or(Value::Null);
            let injected =
                if first.get("callee").and_then(Value::as_str) == Some("GUARD_REACTIVE_PROPS") {
                    first
                        .get("arguments")
                        .and_then(Value::as_array)
                        .and_then(|arguments| arguments.first())
                        .cloned()
                        .map(|raw| vue3_if_suite_inject_prop_into_props(raw, property))
                        .unwrap_or(first)
                } else {
                    vue3_if_suite_inject_prop_into_props(first, property)
                };
            if arguments.is_empty() {
                arguments.push(injected);
            } else {
                arguments[0] = injected;
            }
            return call;
        }
        if callee == Some("GUARD_REACTIVE_PROPS") {
            return props
                .get("arguments")
                .and_then(Value::as_array)
                .and_then(|arguments| arguments.first())
                .cloned()
                .map(|raw| vue3_if_suite_inject_prop_into_props(raw, property))
                .unwrap_or(props);
        }
        if callee == Some("MERGE_PROPS") {
            let mut call = props;
            if let Some(arguments) = call.get_mut("arguments").and_then(Value::as_array_mut) {
                if let Some(first) = arguments.first_mut() {
                    if vue3_public_node_type(first) == Some(15) {
                        vue3_for_suite_prepend_object_property(first, property);
                    } else {
                        arguments
                            .insert(0, vue3_if_suite_props_object(vec![property], &Value::Null));
                    }
                } else {
                    arguments.push(vue3_if_suite_props_object(vec![property], &Value::Null));
                }
            }
            return call;
        }
        if callee == Some("TO_HANDLERS") {
            return vue3_text_suite_call(
                "MERGE_PROPS",
                vec![
                    vue3_if_suite_props_object(vec![property], &Value::Null),
                    props,
                ],
            );
        }
        let mut call = props;
        if let Some(arguments) = call.get_mut("arguments").and_then(Value::as_array_mut) {
            arguments.insert(0, vue3_if_suite_props_object(vec![property], &Value::Null));
        }
        return call;
    }
    vue3_text_suite_call(
        "MERGE_PROPS",
        vec![
            vue3_if_suite_props_object(vec![property], &Value::Null),
            props,
        ],
    )
}

pub(crate) fn vue3_if_suite_push_props_object_arg(
    merge_args: &mut Vec<Value>,
    properties: &mut Vec<Value>,
    node: &Value,
) {
    if properties.is_empty() {
        return;
    }
    merge_args.push(vue3_if_suite_props_object(std::mem::take(properties), node));
}

pub(crate) fn vue3_if_suite_props_object(properties: Vec<Value>, node: &Value) -> Value {
    json!({
        "type": 15,
        "properties": properties,
        "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_if_suite_apply_bind_shorthand(node: &mut Value, state: &mut Vue3IfSuiteState) {
    let projection = vuec_vue3_core::transform_v_bind_shorthand_projection(&json!({
        "node": node,
        "context": { "browser": false },
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

pub(crate) fn vue3_if_suite_user_key(node: &Value) -> Option<Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| match vue3_public_node_type(prop) {
            Some(6) => prop.get("name").and_then(Value::as_str) == Some("key"),
            Some(7) => {
                prop.get("name").and_then(Value::as_str) == Some("bind")
                    && prop.get("arg").is_some_and(|arg| {
                        vue3_public_node_type(arg) == Some(4)
                            && arg
                                .get("isStatic")
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                            && arg.get("content").and_then(Value::as_str) == Some("key")
                    })
            }
            _ => false,
        })
        .cloned()
}

pub(crate) fn vue3_if_suite_same_user_key(a: &Value, b: &Value) -> bool {
    if a.is_null() || b.is_null() || vue3_public_node_type(a) != vue3_public_node_type(b) {
        return false;
    }
    match vue3_public_node_type(a) {
        Some(6) => {
            a.get("value")
                .and_then(|value| value.get("content"))
                .and_then(Value::as_str)
                == b.get("value")
                    .and_then(|value| value.get("content"))
                    .and_then(Value::as_str)
        }
        Some(7) => {
            let a_exp = a.get("exp").unwrap_or(&Value::Null);
            let b_exp = b.get("exp").unwrap_or(&Value::Null);
            vue3_public_node_type(a_exp) == vue3_public_node_type(b_exp)
                && a_exp.get("isStatic").and_then(Value::as_bool)
                    == b_exp.get("isStatic").and_then(Value::as_bool)
                && a_exp.get("content").and_then(Value::as_str)
                    == b_exp.get("content").and_then(Value::as_str)
        }
        _ => false,
    }
}

pub(crate) fn vue3_if_suite_is_comment_or_ascii_whitespace(node: &Value) -> bool {
    match vue3_public_node_type(node) {
        Some(3) => true,
        Some(2) => node
            .get("content")
            .and_then(Value::as_str)
            .is_some_and(|content| {
                content
                    .bytes()
                    .all(|byte| matches!(byte, b'\t' | b'\n' | b'\x0C' | b'\r' | b' '))
            }),
        Some(12) => node
            .get("content")
            .is_some_and(vue3_if_suite_is_comment_or_ascii_whitespace),
        _ => false,
    }
}

pub(crate) fn vue3_if_suite_cache_expression(
    state: &mut Vue3IfSuiteState,
    value: Value,
    need_pause_tracking: bool,
    in_v_once: bool,
    need_array_spread: bool,
) -> Value {
    let index = state.cached;
    state.cached += 1;
    json!({
        "type": 20,
        "index": index,
        "value": value,
        "needPauseTracking": need_pause_tracking,
        "inVOnce": in_v_once,
        "needArraySpread": need_array_spread,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_if_suite_finalize_root(root: &mut Value, state: &Vue3IfSuiteState) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!(vue3_once_suite_components(root));
    root["directives"] = json!(vue3_if_suite_collect_directives(root));
    root["helpers"] = json!(vue3_if_suite_helpers(root));
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_if_suite_collect_directives(root: &Value) -> Vec<String> {
    let mut directives = Vec::new();
    vue3_if_suite_collect_directives_for_node(root, &mut directives);
    directives
}

pub(crate) fn vue3_if_suite_collect_directives_for_node(
    node: &Value,
    directives: &mut Vec<String>,
) {
    for name in vue3_text_suite_runtime_directive_names(node) {
        if !directives.iter().any(|existing| existing == &name) {
            directives.push(name);
        }
    }
    for key in ["children", "branches", "content"] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_if_suite_collect_directives_for_node(item, directives);
            }
        } else if value.is_object() {
            vue3_if_suite_collect_directives_for_node(value, directives);
        }
    }
}

pub(crate) fn vue3_if_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_if_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
    if root
        .get("components")
        .and_then(Value::as_array)
        .is_some_and(|components| !components.is_empty())
    {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_COMPONENT");
    }
    if !vue3_if_suite_collect_directives(root).is_empty() {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_DIRECTIVE");
    }
    used.into_iter().map(str::to_string).collect()
}

pub(crate) fn vue3_if_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    match vue3_public_node_type(node) {
        Some(9) => {
            if let Some(branches) = node.get("branches").and_then(Value::as_array) {
                if let Some(first) = branches.first() {
                    vue3_if_suite_collect_branch_child_helpers(first, used);
                    vue3_text_suite_add_helper(used, "CREATE_COMMENT");
                }
                for branch in branches.iter().skip(1) {
                    vue3_if_suite_collect_branch_child_helpers(branch, used);
                }
            }
            if let Some(codegen) = node.get("codegenNode") {
                vue3_if_suite_collect_call_helpers(codegen, used);
            }
        }
        Some(19) => {
            vue3_if_suite_collect_helpers(node.get("consequent").unwrap_or(&Value::Null), used);
            vue3_text_suite_add_helper(used, "CREATE_COMMENT");
            vue3_if_suite_collect_helpers(node.get("alternate").unwrap_or(&Value::Null), used);
        }
        Some(13) => {
            if let Some(children) = node.get("children") {
                vue3_if_suite_collect_helpers(children, used);
            }
            if let Some(props) = node.get("props") {
                vue3_if_suite_collect_helpers(props, used);
            }
            if let Some(directives) = node.get("directives") {
                vue3_if_suite_collect_helpers(directives, used);
            }
            if node.get("directives").is_some_and(|value| !value.is_null()) {
                vue3_text_suite_add_helper(used, "WITH_DIRECTIVES");
            }
            if node.get("tag").and_then(Value::as_str) == Some("FRAGMENT") {
                vue3_text_suite_add_helper(used, "FRAGMENT");
            }
            if node
                .get("isBlock")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "OPEN_BLOCK");
                if node
                    .get("isComponent")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    vue3_text_suite_add_helper(used, "CREATE_BLOCK");
                } else {
                    vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
                }
            } else if node
                .get("isComponent")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "CREATE_VNODE");
            } else {
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_VNODE");
            }
        }
        Some(14) => {
            if let Some(callee) = node.get("callee").and_then(Value::as_str) {
                match callee {
                    "CREATE_COMMENT" => vue3_text_suite_add_helper(used, "CREATE_COMMENT"),
                    "MERGE_PROPS" => vue3_text_suite_add_helper(used, "MERGE_PROPS"),
                    "NORMALIZE_PROPS" => vue3_text_suite_add_helper(used, "NORMALIZE_PROPS"),
                    "RENDER_SLOT" => vue3_text_suite_add_helper(used, "RENDER_SLOT"),
                    "TO_HANDLERS" => vue3_text_suite_add_helper(used, "TO_HANDLERS"),
                    _ => {}
                }
            }
            for key in ["arguments", "props", "children"] {
                if let Some(value) = node.get(key) {
                    vue3_if_suite_collect_helpers(value, used);
                }
            }
        }
        Some(5) => vue3_text_suite_add_helper(used, "TO_DISPLAY_STRING"),
        Some(12) => {
            if let Some(codegen) = node.get("codegenNode") {
                vue3_if_suite_collect_helpers(codegen, used);
            }
        }
        _ => {
            if let Some(items) = node.as_array() {
                for item in items {
                    vue3_if_suite_collect_helpers(item, used);
                }
            } else {
                for key in [
                    "children",
                    "content",
                    "codegenNode",
                    "arguments",
                    "returns",
                    "params",
                    "directives",
                    "source",
                    "valueAlias",
                    "keyAlias",
                    "objectIndexAlias",
                    "parseResult",
                    "branches",
                    "condition",
                    "test",
                    "consequent",
                    "alternate",
                    "value",
                    "elements",
                    "properties",
                    "key",
                ] {
                    if let Some(value) = node.get(key) {
                        vue3_if_suite_collect_helpers(value, used);
                    }
                }
            }
        }
    }
}

pub(crate) fn vue3_if_suite_collect_branch_child_helpers(
    branch: &Value,
    used: &mut Vec<&'static str>,
) {
    if let Some(children) = branch.get("children").and_then(Value::as_array) {
        if children.len() != 1 || children.first().and_then(vue3_public_node_type) != Some(1) {
            for child in children {
                vue3_if_suite_collect_helpers(child, used);
            }
            vue3_text_suite_add_helper(used, "FRAGMENT");
            vue3_text_suite_add_helper(used, "OPEN_BLOCK");
            vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
            return;
        }
        if let Some(child) = children.first() {
            vue3_if_suite_collect_helpers(child.get("codegenNode").unwrap_or(&Value::Null), used);
        }
    }
}

pub(crate) fn vue3_if_suite_collect_call_helpers(node: &Value, used: &mut Vec<&'static str>) {
    if vue3_public_node_type(node) == Some(14) {
        if let Some(callee) = node.get("callee").and_then(Value::as_str) {
            match callee {
                "MERGE_PROPS" => vue3_text_suite_add_helper(used, "MERGE_PROPS"),
                "NORMALIZE_PROPS" => vue3_text_suite_add_helper(used, "NORMALIZE_PROPS"),
                "TO_HANDLERS" => vue3_text_suite_add_helper(used, "TO_HANDLERS"),
                _ => {}
            }
        }
    }
    for key in ["arguments", "props", "children", "consequent", "alternate"] {
        if let Some(value) = node.get(key) {
            vue3_if_suite_collect_call_helpers(value, used);
        }
    }
    if let Some(items) = node.as_array() {
        for item in items {
            vue3_if_suite_collect_call_helpers(item, used);
        }
    }
}

#[derive(Default)]
pub(crate) struct Vue3SlotSuiteState {
    pub(crate) errors: Vec<Value>,
    pub(crate) cached: usize,
    pub(crate) text_directive_transforms: Vec<&'static str>,
    pub(crate) skip_slot_scope_tracking: bool,
    pub(crate) transform_element_suite: bool,
    pub(crate) transform_element_bind: bool,
    pub(crate) transform_element_on: bool,
    pub(crate) transform_element_noop_directives: Vec<String>,
    pub(crate) transform_element_self_name: Option<String>,
    pub(crate) transform_element_is_script_setup: Option<bool>,
    pub(crate) transform_element_components: Vec<String>,
    pub(crate) transform_element_helpers: Vec<String>,
}

pub(crate) fn vue3_core_transform_slot_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let transform_text = payload
        .get("options")
        .is_some_and(|options| bool_option(options, "transformText", false));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut state = Vue3SlotSuiteState::default();
    let scope = Vue3ModelSuiteScope::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        *children = vue3_slot_suite_transform_children(
            std::mem::take(children),
            &options,
            transform_text,
            &mut state,
            &scope,
        );
    }
    if transform_text {
        vue3_text_suite_apply_transform_text(&mut root, &options);
    }
    vue3_slot_suite_finalize_root(&mut root, &state);
    root["__vuecErrors"] = json!(state.errors);
    let slots = root
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .filter(|child| vue3_public_node_type(child) == Some(1))
        .and_then(|child| child.get("codegenNode"))
        .and_then(|codegen| codegen.get("children"))
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "root": root,
        "slots": slots,
    })
}

pub(crate) fn vue3_core_transform_element_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let api_options = payload.get("options").unwrap_or(&Value::Null);
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    if bool_option(api_options, "transformStyle", false) {
        vue3_transform_element_suite_apply_style(&mut root);
    }
    let mut state = Vue3SlotSuiteState {
        text_directive_transforms: vue3_transform_element_suite_text_directives(api_options),
        transform_element_suite: true,
        transform_element_bind: bool_option(api_options, "transformBind", false),
        transform_element_on: bool_option(api_options, "transformOn", false),
        transform_element_noop_directives: string_array_option(
            api_options,
            "noopDirectiveTransforms",
        ),
        transform_element_self_name: vue3_transform_element_suite_self_name(&source.filename),
        transform_element_is_script_setup: api_options
            .get("bindingMetadata")
            .and_then(|metadata| metadata.get("__isScriptSetup"))
            .and_then(Value::as_bool),
        ..Default::default()
    };
    let scope = Vue3ModelSuiteScope::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        *children = vue3_slot_suite_transform_children(
            std::mem::take(children),
            &options,
            true,
            &mut state,
            &scope,
        );
    }
    vue3_slot_suite_apply_transform_text(&mut root, &options, &state);
    vue3_transform_element_suite_finalize_root(&mut root, &state);
    root["__vuecErrors"] = json!(state.errors);
    let node = vue3_transform_element_suite_result_node(&root);
    json!({
        "root": root,
        "node": node,
    })
}

pub(crate) fn vue3_core_transform_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut state = Vue3SlotSuiteState::default();
    let scope = Vue3ModelSuiteScope::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        *children = vue3_slot_suite_transform_children(
            std::mem::take(children),
            &options,
            true,
            &mut state,
            &scope,
        );
    }
    vue3_slot_suite_apply_transform_text(&mut root, &options, &state);
    vue3_transform_suite_finalize_root(&mut root, &state);
    root["__vuecErrors"] = json!(state.errors);
    root
}

pub(crate) fn vue3_transform_element_suite_text_directives(options: &Value) -> Vec<&'static str> {
    let mut directives = Vec::new();
    if bool_option(options, "transformBind", false) {
        directives.push("bind");
    }
    if bool_option(options, "transformOn", false) {
        directives.push("on");
    }
    directives
}

pub(crate) fn vue3_transform_element_suite_self_name(filename: &str) -> Option<String> {
    let path = filename.split('?').next().unwrap_or(filename);
    let basename = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .rsplit_once('.')
        .map(|(name, _)| name)
        .unwrap_or(path);
    if basename.is_empty() {
        return None;
    }
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in basename.chars() {
        if matches!(ch, '-' | '_') {
            uppercase_next = true;
            continue;
        }
        if uppercase_next {
            out.extend(ch.to_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    (!out.is_empty()).then_some(out)
}

pub(crate) fn vue3_transform_element_suite_apply_style(node: &mut Value) {
    if vue3_public_node_type(node) == Some(1) {
        let projection = vuec_vue3_dom::transform_style_projection(&json!({ "node": node }));
        let replacements = projection
            .get("replacements")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) {
            for replacement in replacements {
                let Some(index) = replacement.get("index").and_then(Value::as_u64) else {
                    continue;
                };
                let Some(original) = props.get(index as usize).cloned() else {
                    continue;
                };
                if vue3_public_node_type(&original) != Some(6) {
                    continue;
                }
                let loc = original
                    .get("loc")
                    .cloned()
                    .unwrap_or_else(vue3_loc_stub_value);
                props[index as usize] = json!({
                    "type": 7,
                    "name": "bind",
                    "rawName": ":style",
                    "arg": {
                        "type": 4,
                        "content": "style",
                        "isStatic": true,
                        "constType": 3,
                        "loc": loc,
                    },
                    "exp": {
                        "type": 4,
                        "content": replacement
                            .get("expression")
                            .and_then(Value::as_str)
                            .unwrap_or("{}"),
                        "isStatic": false,
                        "constType": 3,
                        "loc": loc,
                    },
                    "modifiers": [],
                    "loc": loc,
                });
            }
        }
    }
    for key in ["children", "branches"] {
        let Some(value) = node.get_mut(key) else {
            continue;
        };
        if let Some(items) = value.as_array_mut() {
            for item in items {
                vue3_transform_element_suite_apply_style(item);
            }
        }
    }
}

pub(crate) fn vue3_transform_element_suite_result_node(root: &Value) -> Value {
    root.get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .and_then(|wrapper| {
            wrapper
                .get("children")
                .and_then(Value::as_array)
                .and_then(|children| children.first())
        })
        .and_then(|node| node.get("codegenNode"))
        .cloned()
        .unwrap_or(Value::Null)
}

pub(crate) fn vue3_slot_suite_transform_children(
    children: Vec<Value>,
    options: &Vue3CompilerOptions,
    transform_text: bool,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Vec<Value> {
    let mut transformed = Vec::new();
    let mut index = 0usize;
    let mut key_base = 0usize;
    while index < children.len() {
        let child = children[index].clone();
        if vue3_public_node_type(&child) == Some(1)
            && !vue3_slot_suite_is_template_slot(&child)
            && vue3_text_suite_directive(&child, "if").is_some()
        {
            let (if_node, consumed) = vue3_slot_suite_transform_if_chain(
                &children,
                index,
                key_base,
                options,
                transform_text,
                state,
                scope,
            );
            key_base += if_node
                .get("branches")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or_default();
            transformed.push(if_node);
            index += consumed.max(1);
            continue;
        }
        if vue3_public_node_type(&child) == Some(1)
            && !vue3_slot_suite_is_template_slot(&child)
            && (vue3_text_suite_directive(&child, "else").is_some()
                || vue3_text_suite_directive(&child, "else-if").is_some())
        {
            state.errors.push(json!({
                "code": 30,
                "loc": child.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }));
        }
        transformed.push(vue3_slot_suite_transform_node(
            child,
            options,
            transform_text,
            state,
            scope,
        ));
        index += 1;
    }
    transformed
}

pub(crate) fn vue3_slot_suite_transform_if_chain(
    siblings: &[Value],
    start: usize,
    key_base: usize,
    options: &Vue3CompilerOptions,
    transform_text: bool,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> (Value, usize) {
    let first = siblings.get(start).cloned().unwrap_or(Value::Null);
    let mut branches = Vec::new();
    branches.push(vue3_slot_suite_branch_from_node(
        first.clone(),
        "if",
        Vec::new(),
        options,
        transform_text,
        state,
        scope,
    ));

    let mut scan = start + 1;
    let mut consumed = 1usize;
    while scan < siblings.len() {
        let mut gap = Vec::<Value>::new();
        let mut candidate_index = scan;
        while let Some(candidate) = siblings.get(candidate_index) {
            if !vue3_if_suite_is_comment_or_ascii_whitespace(candidate) {
                break;
            }
            if vue3_public_node_type(candidate) == Some(3) {
                gap.push(candidate.clone());
            }
            candidate_index += 1;
        }
        let Some(candidate) = siblings.get(candidate_index) else {
            break;
        };
        if vue3_slot_suite_is_template_slot(candidate) {
            break;
        }
        let dir_name = if vue3_public_node_type(candidate) == Some(1)
            && vue3_text_suite_directive(candidate, "else-if").is_some()
        {
            "else-if"
        } else if vue3_public_node_type(candidate) == Some(1)
            && vue3_text_suite_directive(candidate, "else").is_some()
        {
            "else"
        } else {
            break;
        };
        let branch = vue3_slot_suite_branch_from_node(
            candidate.clone(),
            dir_name,
            gap,
            options,
            transform_text,
            state,
            scope,
        );
        if branches
            .last()
            .and_then(|branch| branch.get("condition"))
            .is_some_and(Value::is_null)
        {
            state.errors.push(json!({
                "code": 30,
                "loc": branch.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }));
        }
        let current_key = branch.get("userKey").unwrap_or(&Value::Null);
        if !current_key.is_null()
            && branches.iter().any(|existing| {
                vue3_if_suite_same_user_key(
                    existing.get("userKey").unwrap_or(&Value::Null),
                    current_key,
                )
            })
        {
            state.errors.push(json!({
                "code": 29,
                "loc": current_key
                    .get("loc")
                    .cloned()
                    .unwrap_or_else(vue3_loc_stub_value),
            }));
        }
        branches.push(branch);
        consumed = candidate_index - start + 1;
        scan = candidate_index + 1;
    }

    let loc = first
        .get("loc")
        .cloned()
        .unwrap_or_else(vue3_loc_stub_value);
    let mut if_node = json!({
        "type": 9,
        "branches": branches,
        "codegenNode": Value::Null,
        "loc": loc,
    });
    if_node["codegenNode"] = vue3_if_suite_if_codegen(&mut if_node, key_base);
    if transform_text {
        vue3_slot_suite_apply_transform_text(&mut if_node, options, state);
    }
    (if_node, consumed)
}

pub(crate) fn vue3_slot_suite_branch_from_node(
    mut node: Value,
    dir_name: &str,
    comments: Vec<Value>,
    options: &Vue3CompilerOptions,
    transform_text: bool,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    vue3_slot_suite_apply_bind_shorthand(&mut node, state);
    let dir = vue3_text_suite_directive(&node, dir_name).cloned();
    let condition = vue3_slot_suite_if_condition(dir.as_ref(), &node, options, state, scope);
    let user_key = vue3_if_suite_user_key(&node).unwrap_or(Value::Null);
    let is_template_if = node.get("tagType").and_then(Value::as_u64) == Some(3);

    vue3_text_suite_remove_directive(&mut node, "if");
    vue3_text_suite_remove_directive(&mut node, "else-if");
    vue3_text_suite_remove_directive(&mut node, "else");

    let mut children = comments;
    if is_template_if && vue3_text_suite_directive(&node, "for").is_none() {
        let template_children = node
            .get_mut("children")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default();
        children.extend(vue3_slot_suite_transform_children(
            template_children,
            options,
            transform_text,
            state,
            scope,
        ));
    } else {
        children.push(vue3_slot_suite_transform_node(
            node.clone(),
            options,
            transform_text,
            state,
            scope,
        ));
    }

    json!({
        "type": 10,
        "condition": condition,
        "children": children,
        "userKey": user_key,
        "isTemplateIf": is_template_if,
        "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_slot_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    transform_text: bool,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) {
        vue3_slot_suite_apply_bind_shorthand(&mut node, state);
        if !vue3_slot_suite_is_template_slot(&node)
            && vue3_text_suite_directive(&node, "for").is_some()
        {
            return vue3_slot_suite_transform_for_node(node, options, transform_text, state, scope);
        }
    }

    let mut child_scope = scope.clone();
    if vue3_public_node_type(&node) == Some(1) {
        vue3_slot_suite_track_v_for_slot_scope(&mut node, options, state, &mut child_scope);
        vue3_slot_suite_process_directive_expressions(&mut node, options, state, &child_scope);
        if !state.skip_slot_scope_tracking {
            vue3_slot_suite_track_slot_scope(&node, &mut child_scope);
        }
    }

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        *children = vue3_slot_suite_transform_children(
            std::mem::take(children),
            options,
            transform_text,
            state,
            &child_scope,
        );
    }

    if vue3_public_node_type(&node) == Some(5) {
        vue3_for_suite_process_expression_node(&mut node, "content", options, scope);
    }

    if transform_text && matches!(vue3_public_node_type(&node), Some(1 | 10 | 11)) {
        vue3_slot_suite_apply_transform_text(&mut node, options, state);
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_slot_suite_element_codegen(&node, options, state, scope, false);
    }
    node
}

pub(crate) fn vue3_slot_suite_transform_for_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    transform_text: bool,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let Some(dir) = vue3_text_suite_directive(&node, "for").cloned() else {
        return vue3_slot_suite_transform_node(node, options, transform_text, state, scope);
    };
    let context = vue3_model_suite_transform_context(options, scope);
    let projection = vuec_vue3_core::transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }));
    for error in projection
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(vue3_model_suite_error_value(error, &dir));
    }
    for error in projection
        .get("templateKeyErrors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(vue3_model_suite_error_value(error, &dir));
    }
    let parse_result = projection
        .get("parseResult")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_for_parse_result)
        .unwrap_or_else(|| {
            dir.get("forParseResult")
                .map(vue3_text_suite_materialize_for_parse_result)
                .unwrap_or(Value::Null)
        });

    let mut child_scope = scope.clone();
    vue3_model_suite_add_locals(&mut child_scope, projection.get("locals"));
    child_scope.v_for_depth += 1;
    let original_node = node.clone();
    let fallback_loc = node.get("loc").cloned();
    let children = if projection.get("children").and_then(Value::as_str) == Some("template") {
        node.get_mut("children")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default()
            .into_iter()
            .map(|child| {
                vue3_slot_suite_transform_node(child, options, transform_text, state, &child_scope)
            })
            .collect::<Vec<_>>()
    } else {
        vue3_text_suite_remove_directive(&mut node, "for");
        vec![vue3_slot_suite_transform_node(
            node,
            options,
            transform_text,
            state,
            &child_scope,
        )]
    };

    let loc = dir
        .get("loc")
        .cloned()
        .or(fallback_loc)
        .unwrap_or_else(vue3_loc_stub_value);
    let mut for_node = json!({
        "type": 11,
        "source": parse_result.get("source").cloned().unwrap_or(Value::Null),
        "valueAlias": parse_result.get("value").cloned().unwrap_or(Value::Null),
        "keyAlias": parse_result.get("key").cloned().unwrap_or(Value::Null),
        "objectIndexAlias": parse_result.get("index").cloned().unwrap_or(Value::Null),
        "parseResult": parse_result,
        "children": children,
        "codegenNode": Value::Null,
        "loc": loc,
    });
    let mut for_state = Vue3ForSuiteState::default();
    for_node["codegenNode"] = vue3_for_suite_for_codegen(
        &original_node,
        &mut for_node,
        options,
        &child_scope,
        &mut for_state,
    );
    state.errors.extend(for_state.errors);
    if transform_text {
        vue3_slot_suite_apply_transform_text(&mut for_node, options, state);
    }
    for_node
}

pub(crate) fn vue3_slot_suite_apply_transform_text(
    node: &mut Value,
    options: &Vue3CompilerOptions,
    state: &Vue3SlotSuiteState,
) {
    if state.text_directive_transforms.is_empty() {
        vue3_text_suite_apply_transform_text(node, options);
    } else {
        vue3_text_suite_apply_transform_text_with_directives(
            node,
            options,
            &state.text_directive_transforms,
        );
    }
}

pub(crate) fn vue3_slot_suite_if_condition(
    dir: Option<&Value>,
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let Some(dir) = dir else {
        return Value::Null;
    };
    if dir.get("name").and_then(Value::as_str) == Some("else") {
        return Value::Null;
    }
    let exp = dir.get("exp").filter(|value| !value.is_null());
    let raw = exp
        .and_then(|exp| exp.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if exp.is_none() || raw.trim().is_empty() {
        state.errors.push(json!({
            "code": 28,
            "loc": dir.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }));
        return json!({
            "type": 4,
            "content": "true",
            "isStatic": false,
            "constType": 0,
            "loc": exp
                .and_then(|exp| exp.get("loc"))
                .cloned()
                .or_else(|| node.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        });
    }
    let current = exp.cloned().unwrap_or(Value::Null);
    if !options.prefix_identifiers {
        return current;
    }
    let projection = vuec_vue3_core::process_expression_projection(&json!({
        "node": current,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    vue3_slot_suite_materialize_process_projection(&projection, &current, state)
}

pub(crate) fn vue3_slot_suite_apply_bind_shorthand(
    node: &mut Value,
    state: &mut Vue3SlotSuiteState,
) {
    let projection = vuec_vue3_core::transform_v_bind_shorthand_projection(&json!({
        "node": node,
        "context": { "browser": false },
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

pub(crate) fn vue3_slot_suite_process_directive_expressions(
    node: &mut Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) {
    if !options.prefix_identifiers {
        return;
    }
    let context = vue3_model_suite_transform_context(options, scope);
    let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
        return;
    };
    for prop in props {
        if vue3_public_node_type(prop) != Some(7) {
            continue;
        }
        let name = prop.get("name").and_then(Value::as_str);
        let is_slot = prop.get("name").and_then(Value::as_str) == Some("slot");
        if let Some(current) = prop.get("exp").filter(|value| !value.is_null()).cloned() {
            if vue3_public_node_type(&current) == Some(4) {
                if name == Some("on") && prop.get("arg").is_some_and(|arg| !arg.is_null()) {
                    continue;
                }
                let projection = vuec_vue3_core::process_expression_projection(&json!({
                    "node": current,
                    "context": context,
                    "asParams": is_slot,
                }));
                prop["exp"] =
                    vue3_slot_suite_materialize_process_projection(&projection, &current, state);
            }
        }
        if let Some(current) = prop.get("arg").filter(|value| !value.is_null()).cloned() {
            if vue3_public_node_type(&current) == Some(4)
                && !current
                    .get("isStatic")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                let projection = vuec_vue3_core::process_expression_projection(&json!({
                    "node": current,
                    "context": context,
                }));
                prop["arg"] =
                    vue3_slot_suite_materialize_process_projection(&projection, &current, state);
            }
        }
    }
}

pub(crate) fn vue3_slot_suite_materialize_process_projection(
    projection: &Value,
    current: &Value,
    state: &mut Vue3SlotSuiteState,
) -> Value {
    if projection.get("kind").and_then(Value::as_str) == Some("error") {
        state.errors.push(json!({
            "code": projection.get("code").cloned().unwrap_or(json!(46)),
            "message": projection
                .get("message")
                .cloned()
                .unwrap_or_else(|| json!("Error parsing JavaScript expression: Unexpected token")),
            "loc": projection
                .get("loc")
                .cloned()
                .or_else(|| current.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        }));
        return current.clone();
    }
    vue3_text_suite_materialize_process_projection(projection, current)
}

pub(crate) fn vue3_slot_suite_track_v_for_slot_scope(
    node: &mut Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &mut Vue3ModelSuiteScope,
) {
    if !options.prefix_identifiers || !vue3_slot_suite_is_template_slot(node) {
        return;
    }
    let projection = vuec_vue3_core::track_v_for_slot_scopes_projection(&json!({
        "node": node,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    for error in projection
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(json!({
            "code": error.get("code").cloned().unwrap_or(json!(32)),
            "loc": error
                .get("loc")
                .cloned()
                .unwrap_or_else(|| node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value)),
        }));
    }
    if !projection
        .get("track")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return;
    }
    vue3_model_suite_add_locals(scope, projection.get("locals"));
    if let Some(parse_result) = projection
        .get("parseResult")
        .filter(|value| !value.is_null())
    {
        let materialized = vue3_text_suite_materialize_for_parse_result(parse_result);
        let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
            return;
        };
        if let Some(dir) = props.iter_mut().find(|prop| {
            vue3_public_node_type(prop) == Some(7)
                && prop.get("name").and_then(Value::as_str) == Some("for")
        }) {
            dir["forParseResult"] = materialized;
        }
    }
}

pub(crate) fn vue3_slot_suite_track_slot_scope(node: &Value, scope: &mut Vue3ModelSuiteScope) {
    if vue3_public_node_type(node) != Some(1) {
        return;
    }
    let projection = vuec_vue3_core::track_slot_scopes_projection(&json!({ "node": node }));
    if projection
        .get("track")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        vue3_model_suite_add_locals(scope, projection.get("locals"));
        scope.v_slot_depth += 1;
    }
}

pub(crate) fn vue3_slot_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    if state.transform_element_suite {
        return vue3_transform_element_suite_element_codegen(node, options, state, scope, is_block);
    }
    if let Some(slot) = vue3_text_suite_directive(node, "slot") {
        if node.get("tagType").and_then(Value::as_u64) == Some(0) {
            state.errors.push(json!({
                "code": 40,
                "loc": slot.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }));
        }
    }
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(1)) => {
            vue3_slot_suite_component_codegen(node, options, state, scope, is_block)
        }
        (Some(1), Some(2)) => vue3_slot_suite_slot_outlet_codegen(node, options, state, scope),
        (Some(1), Some(0)) => {
            let mut if_state = Vue3IfSuiteState {
                cached: state.cached,
                ..Default::default()
            };
            let codegen =
                vue3_if_suite_element_codegen(node, options, &mut if_state, scope, is_block);
            state.errors.extend(if_state.errors);
            state.cached = if_state.cached;
            codegen
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_slot_suite_component_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    let mut if_state = Vue3IfSuiteState {
        cached: state.cached,
        ..Default::default()
    };
    let (props, mut patch_flag, dynamic_props, directives, should_use_block) =
        vue3_if_suite_props_codegen(node, options, &mut if_state, scope);
    state.errors.extend(if_state.errors);
    state.cached = if_state.cached;

    let children = node
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let slot_children = if children.is_empty() {
        Value::Null
    } else {
        vue3_slot_suite_build_slots(node, options, state, scope)
    };
    if !slot_children.is_null() {
        let projection = vuec_vue3_core::build_slots_projection(&json!({
            "node": node,
            "context": vue3_model_suite_transform_context(options, scope),
        }));
        if projection
            .get("hasDynamicSlots")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let current = patch_flag.and_then(|flag| flag.as_u64()).unwrap_or(0);
            patch_flag = Some(json!(current | 1024));
        }
    }

    let tag = node.get("tag").and_then(Value::as_str).unwrap_or("");
    let mut vnode = vue3_once_suite_vnode_call(
        &vue3_once_suite_component_asset_id(tag),
        props,
        slot_children,
        patch_flag,
        dynamic_props,
        is_block || should_use_block,
        false,
        true,
    );
    vnode["directives"] = directives;
    vnode
}

pub(crate) fn vue3_transform_element_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    if let Some(slot) = vue3_text_suite_directive(node, "slot") {
        if node.get("tagType").and_then(Value::as_u64) == Some(0) {
            state.errors.push(json!({
                "code": 40,
                "loc": slot.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }));
        }
    }
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(1)) => {
            vue3_transform_element_suite_component_codegen(node, options, state, scope, is_block)
        }
        (Some(1), Some(2)) => vue3_slot_suite_slot_outlet_codegen(node, options, state, scope),
        (Some(1), Some(0)) => vue3_transform_element_suite_plain_element_codegen(
            node, options, state, scope, is_block,
        ),
        _ => Value::Null,
    }
}

pub(crate) fn vue3_transform_element_suite_plain_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    let tag = vue3_once_suite_quote_string(node.get("tag").and_then(Value::as_str).unwrap_or(""));
    let (props, mut patch_flag, dynamic_props, directives, should_use_block) =
        vue3_transform_element_suite_props_codegen(node, options, state, scope, false, false);
    let children = vue3_transform_element_suite_element_children(node);
    if patch_flag.is_none() && vue3_suite_child_needs_text_patch_flag(&children, options, scope) {
        patch_flag = Some(json!(1));
    }
    let mut is_block = is_block || should_use_block;
    if !is_block {
        if let Some(tag) = node.get("tag").and_then(Value::as_str) {
            is_block = matches!(tag, "svg" | "foreignObject" | "math");
        }
    }
    if patch_flag.is_none() && !directives.is_null() && !is_block {
        patch_flag = Some(json!(512));
    }
    let mut vnode = vue3_once_suite_vnode_call(
        &tag,
        props,
        children,
        patch_flag,
        dynamic_props,
        is_block,
        false,
        false,
    );
    vnode["directives"] = directives;
    vnode
}

pub(crate) fn vue3_transform_element_suite_component_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    let context = vue3_transform_element_suite_component_context(options, state, scope);
    let component = vuec_vue3_core::resolve_component_type_projection(&json!({
        "node": node,
        "context": context,
        "ssr": false,
    }));
    let tag = vue3_transform_element_suite_component_tag(&component, node, state);
    let is_dynamic_component =
        tag.get("callee").and_then(Value::as_str) == Some("RESOLVE_DYNAMIC_COMPONENT");
    let (props, mut patch_flag, dynamic_props, directives, should_use_block) =
        vue3_transform_element_suite_props_codegen(
            node,
            options,
            state,
            scope,
            true,
            is_dynamic_component,
        );

    let mut children = vue3_transform_element_suite_element_children(node);
    if vue3_transform_element_suite_should_build_component_slots(&tag, &children) {
        children = vue3_slot_suite_build_slots(node, options, state, scope);
        if !children.is_null() {
            let projection = vuec_vue3_core::build_slots_projection(&json!({
                "node": node,
                "context": vue3_model_suite_transform_context(options, scope),
            }));
            if projection
                .get("hasDynamicSlots")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                let current = patch_flag.as_ref().and_then(Value::as_u64).unwrap_or(0);
                patch_flag = Some(json!(current | 1024));
            }
        }
    } else if let Some(tag_name) = vue3_transform_element_suite_helper_tag_name(&tag) {
        let projection = vuec_vue3_core::transform_element_children_projection(&json!({
            "tag": tag_name,
            "children": node.get("children").cloned().unwrap_or_else(|| json!([])),
        }));
        if projection.get("kind").and_then(Value::as_str) == Some("children") {
            if projection
                .get("shouldUseBlock")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                children = vue3_transform_element_suite_element_children(node);
            }
            if let Some(projected) = projection.get("patchFlag").and_then(Value::as_u64) {
                let current = patch_flag.as_ref().and_then(Value::as_u64).unwrap_or(0);
                patch_flag = Some(json!(current | projected));
            }
        }
    }

    let is_block = is_block
        || should_use_block
        || is_dynamic_component
        || vue3_transform_element_suite_helper_tag_name(&tag)
            .is_some_and(|helper| matches!(helper, "TELEPORT" | "SUSPENSE" | "KEEP_ALIVE"));
    if patch_flag.is_none() && !directives.is_null() && !is_block {
        patch_flag = Some(json!(512));
    }
    let mut vnode = vue3_once_suite_vnode_call(
        "",
        props,
        children,
        patch_flag,
        dynamic_props,
        is_block,
        false,
        true,
    );
    vnode["tag"] = tag;
    vnode["directives"] = directives;
    vnode
}

pub(crate) fn vue3_transform_element_suite_props_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_component: bool,
    is_dynamic_component: bool,
) -> (Value, Option<Value>, Value, Value, bool) {
    let mut properties = Vec::<Value>::new();
    let mut merge_args = Vec::<Value>::new();
    let mut runtime_directives = Vec::<Value>::new();
    let mut prop_summaries = Vec::<Value>::new();
    let context = vue3_model_suite_transform_context(options, scope);

    for prop in node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match vue3_public_node_type(prop) {
            Some(6) => {
                let Some(name) = prop.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let value = prop
                    .get("value")
                    .and_then(|value| value.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if name == "is"
                    && (matches!(
                        node.get("tag").and_then(Value::as_str),
                        Some("component" | "Component")
                    ) || value.starts_with("vue:"))
                {
                    continue;
                }
                properties.push(vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression(name, true),
                    vue3_once_suite_simple_expression(value, true),
                ));
                prop_summaries.push(json!({
                    "kind": "attribute",
                    "name": name,
                    "value": value,
                }));
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("bind") => {
                if prop.get("arg").is_none_or(Value::is_null) {
                    prop_summaries.push(json!({ "kind": "objectBind" }));
                    vue3_if_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
                    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
                        merge_args.push(exp.clone());
                    } else {
                        state
                            .errors
                            .push(vue3_bind_suite_error_value(&json!(34), prop));
                    }
                    continue;
                }

                if vue3_transform_element_suite_is_dynamic_component_is_prop(node, prop) {
                    continue;
                }

                if !state.transform_element_bind {
                    if vue3_transform_element_suite_static_arg(prop).as_deref() == Some("key") {
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "name": "key",
                            "dynamicKey": false,
                            "valueConstant": true,
                            "forceBlock": true,
                        }));
                    }
                    continue;
                }

                let projection = vuec_vue3_core::transform_bind_projection(&json!({
                    "dir": prop,
                    "context": vue3_bind_suite_transform_context(false),
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
                    let key =
                        vue3_bind_suite_materialize_projection(projected_prop.get("key"), prop);
                    let value =
                        vue3_bind_suite_materialize_projection(projected_prop.get("value"), prop);
                    let value_constant = vue3_if_suite_value_constant(&value, &context) > 0;
                    let mut summary = json!({
                        "kind": "directiveProp",
                        "dynamicKey": vue3_model_suite_static_prop_name(&key).is_none(),
                        "valueConstant": value_constant,
                        "valueStatic": value
                            .get("isStatic")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        "valueType": value.get("type").cloned().unwrap_or(Value::Null),
                        "valueStartsWithArray": value
                            .get("content")
                            .and_then(Value::as_str)
                            .is_some_and(|content| content.trim_start().starts_with('[')),
                    });
                    if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                        summary["name"] = json!(name);
                        if name == "key" {
                            summary["forceBlock"] = json!(true);
                        }
                    }
                    if vue3_transform_element_suite_has_modifier(prop, "prop") {
                        summary["propModifier"] = json!(true);
                    }
                    prop_summaries.push(summary);
                    properties.push(vue3_once_suite_object_property(key, value));
                }
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("on") => {
                if prop.get("arg").is_none_or(Value::is_null) {
                    prop_summaries.push(json!({ "kind": "objectOn" }));
                    vue3_if_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
                    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
                        let mut args = vec![exp.clone()];
                        if !is_component {
                            args.push(json!("true"));
                        }
                        merge_args.push(vue3_text_suite_call("TO_HANDLERS", args));
                    } else {
                        state
                            .errors
                            .push(vue3_model_suite_error_value(&json!(35), prop));
                    }
                    continue;
                }

                let force_before_update_block = node
                    .get("children")
                    .and_then(Value::as_array)
                    .is_some_and(|children| !children.is_empty())
                    && vue3_transform_element_suite_static_arg(prop).as_deref()
                        == Some("vue:before-update");
                if !state.transform_element_on {
                    if force_before_update_block {
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "forceBlock": true,
                        }));
                    }
                    continue;
                }

                let projection = vuec_vue3_core::transform_on_projection(&json!({
                    "dir": prop,
                    "node": node,
                    "context": context,
                }));
                for error in projection
                    .get("errors")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    state.errors.push(vue3_model_suite_error_value(error, prop));
                }
                for projected_prop in projection
                    .get("props")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let key = vue3_on_suite_materialize_projection(projected_prop.get("key"), prop);
                    let mut value =
                        vue3_on_suite_materialize_projection(projected_prop.get("value"), prop);
                    let cached = projected_prop
                        .get("cache")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    if cached {
                        value = vue3_transform_element_suite_cache_expression(state, value, false);
                    }
                    let dynamic_key = projected_prop
                        .get("dynamicKey")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let mut summary = json!({
                        "kind": "directiveProp",
                        "dynamicKey": dynamic_key,
                        "ignoreDynamicKeyForNormalize": projected_prop
                            .get("ignoreDynamicKeyForNormalize")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        "valueConstant": projected_prop
                            .get("valueConstant")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        "valueCached": cached,
                        "forceBlock": force_before_update_block,
                    });
                    if !dynamic_key {
                        if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                            summary["name"] = json!(name);
                        }
                    }
                    prop_summaries.push(summary);
                    properties.push(vue3_once_suite_object_property(key, value));
                }
            }
            Some(7) => {
                let Some(name) = prop.get("name").and_then(Value::as_str) else {
                    continue;
                };
                if matches!(name, "once" | "memo") {
                    continue;
                }
                if name == "slot" {
                    continue;
                }
                if state
                    .transform_element_noop_directives
                    .iter()
                    .any(|directive| directive == name)
                {
                    continue;
                }
                if !vue3_text_suite_builtin_directive(name) {
                    prop_summaries.push(json!({ "kind": "runtimeDirective" }));
                    runtime_directives.push(vue3_transform_element_suite_runtime_directive(prop));
                }
            }
            _ => {}
        }
    }

    let props_projection = vuec_vue3_core::transform_element_props_projection(&json!({
        "props": prop_summaries,
        "hasChildren": node
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| !children.is_empty()),
        "isComponent": is_component,
        "isDynamicComponent": is_dynamic_component,
        "context": context,
    }));

    let mut props = if merge_args.is_empty() {
        if properties.is_empty() {
            Value::Null
        } else {
            vue3_if_suite_props_object(
                vue3_transform_element_suite_dedupe_properties(properties),
                node,
            )
        }
    } else {
        vue3_if_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
        if merge_args.len() == 1 {
            merge_args.pop().unwrap_or(Value::Null)
        } else {
            vue3_text_suite_call("MERGE_PROPS", merge_args)
        }
    };

    if props_projection
        .get("refForMarker")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        props = vue3_for_suite_prepend_props_expression_prop(
            props,
            vue3_for_suite_ref_for_property(),
            node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        );
    }
    vue3_transform_element_suite_apply_inline_template_refs(&mut props, &props_projection, node);
    vue3_if_suite_apply_props_normalizers(&mut props, &props_projection);

    let patch_flag = props_projection
        .get("patchFlag")
        .and_then(Value::as_u64)
        .filter(|flag| *flag > 0)
        .map(|flag| json!(flag));
    let dynamic_prop_names = props_projection
        .get("dynamicPropNames")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let dynamic_props = if dynamic_prop_names.is_empty() {
        Value::Null
    } else {
        Value::String(vue3_model_suite_dynamic_props_string(&dynamic_prop_names))
    };
    let directives = if runtime_directives.is_empty() {
        Value::Null
    } else {
        json!({
            "type": 17,
            "elements": runtime_directives,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        })
    };
    let should_use_block = props_projection
        .get("shouldUseBlock")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    (
        props,
        patch_flag,
        dynamic_props,
        directives,
        should_use_block,
    )
}

pub(crate) fn vue3_transform_element_suite_runtime_directive(dir: &Value) -> Value {
    let projection = vuec_vue3_core::build_directive_args_projection(&json!({
        "dir": dir,
        "needRuntime": Value::Null,
    }));
    let mut elements = Vec::new();
    elements.push(vue3_transform_element_suite_directive_runtime(
        projection.get("runtime").unwrap_or(&Value::Null),
        dir,
    ));
    let include_exp = projection
        .get("includeExp")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let include_arg = projection
        .get("includeArg")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if include_exp {
        elements.push(dir.get("exp").cloned().unwrap_or(Value::Null));
    } else if include_arg {
        elements.push(Value::String("undefined".to_string()));
    }
    if include_arg {
        elements.push(dir.get("arg").cloned().unwrap_or(Value::Null));
    }
    let modifiers = projection
        .get("modifiers")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|modifier| modifier.get("name").and_then(Value::as_str))
                .map(|name| {
                    vue3_once_suite_object_property(
                        vue3_once_suite_simple_expression(name, true),
                        vue3_once_suite_simple_expression("true", false),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !modifiers.is_empty() {
        elements.push(vue3_if_suite_props_object(modifiers, dir));
    }
    json!({
        "type": 17,
        "elements": elements,
        "loc": dir.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_transform_element_suite_directive_runtime(
    runtime: &Value,
    dir: &Value,
) -> Value {
    match runtime.get("kind").and_then(Value::as_str) {
        Some("helper") => runtime
            .get("helper")
            .and_then(Value::as_str)
            .or_else(|| runtime.get("helperName").and_then(Value::as_str))
            .map(|helper| Value::String(format!("_{}", vue3_bind_suite_helper_name(helper))))
            .unwrap_or(Value::Null),
        Some("asset") | _ => runtime
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| dir.get("name").and_then(Value::as_str))
            .map(vue3_text_suite_directive_asset_id)
            .map(Value::String)
            .unwrap_or(Value::Null),
    }
}

pub(crate) fn vue3_transform_element_suite_apply_inline_template_refs(
    props: &mut Value,
    projection: &Value,
    node: &Value,
) {
    let refs = projection
        .get("inlineTemplateRefs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if refs.is_empty() || props.is_null() {
        return;
    }
    for reference in refs.into_iter().rev() {
        let Some(content) = reference.get("content").and_then(Value::as_str) else {
            continue;
        };
        vue3_transform_element_suite_apply_inline_template_ref_value(props, content);
        let property = vue3_once_suite_object_property(
            vue3_once_suite_simple_expression("ref_key", true),
            vue3_once_suite_simple_expression(content, true),
        );
        *props = vue3_for_suite_prepend_props_expression_prop(
            std::mem::take(props),
            property,
            node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        );
    }
}

pub(crate) fn vue3_transform_element_suite_apply_inline_template_ref_value(
    props: &mut Value,
    content: &str,
) {
    let Some(properties) = props.get_mut("properties").and_then(Value::as_array_mut) else {
        return;
    };
    for property in properties {
        let Some(key) = property
            .get("key")
            .and_then(vue3_model_suite_static_prop_name)
        else {
            continue;
        };
        if key == "ref" {
            property["value"] = vue3_once_suite_simple_expression(content, false);
        }
    }
}

pub(crate) fn vue3_transform_element_suite_is_dynamic_component_is_prop(
    node: &Value,
    prop: &Value,
) -> bool {
    matches!(
        node.get("tag").and_then(Value::as_str),
        Some("component" | "Component")
    ) && vue3_transform_element_suite_static_arg(prop).as_deref() == Some("is")
}

pub(crate) fn vue3_transform_element_suite_static_arg(prop: &Value) -> Option<String> {
    prop.get("arg")
        .filter(|value| !value.is_null())
        .filter(|arg| {
            arg.get("isStatic")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .and_then(|arg| arg.get("content").and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

pub(crate) fn vue3_transform_element_suite_has_modifier(prop: &Value, name: &str) -> bool {
    prop.get("modifiers")
        .and_then(Value::as_array)
        .is_some_and(|modifiers| {
            modifiers.iter().any(|modifier| {
                modifier
                    .as_str()
                    .or_else(|| modifier.get("content").and_then(Value::as_str))
                    == Some(name)
            })
        })
}

pub(crate) fn vue3_transform_element_suite_dedupe_properties(properties: Vec<Value>) -> Vec<Value> {
    let mut deduped = Vec::<Value>::new();
    for property in properties {
        let key_name = property
            .get("key")
            .and_then(vue3_model_suite_static_prop_name);
        let Some(key_name) = key_name else {
            deduped.push(property);
            continue;
        };
        let Some(existing) = deduped.iter_mut().find(|existing| {
            existing
                .get("key")
                .and_then(vue3_model_suite_static_prop_name)
                .as_deref()
                == Some(key_name.as_str())
        }) else {
            deduped.push(property);
            continue;
        };
        if key_name == "class" || key_name == "style" || key_name.starts_with("on") {
            let next_value = property.get("value").cloned().unwrap_or(Value::Null);
            vue3_transform_element_suite_merge_property_value(existing, next_value);
        } else {
            existing["value"] = property.get("value").cloned().unwrap_or(Value::Null);
        }
    }
    deduped
}

pub(crate) fn vue3_transform_element_suite_merge_property_value(
    property: &mut Value,
    next_value: Value,
) {
    let current = property.get("value").cloned().unwrap_or(Value::Null);
    if vue3_public_node_type(&current) == Some(17) {
        property["value"] = current;
        if let Some(elements) = property
            .get_mut("value")
            .and_then(|value| value.get_mut("elements"))
            .and_then(Value::as_array_mut)
        {
            elements.push(next_value);
        }
        return;
    }
    property["value"] = json!({
        "type": 17,
        "elements": [current, next_value],
        "loc": property.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    });
}

pub(crate) fn vue3_transform_element_suite_cache_expression(
    state: &mut Vue3SlotSuiteState,
    value: Value,
    need_array_spread: bool,
) -> Value {
    let index = state.cached;
    state.cached += 1;
    json!({
        "type": 20,
        "index": index,
        "value": value,
        "needPauseTracking": false,
        "inVOnce": false,
        "needArraySpread": need_array_spread,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_transform_element_suite_component_context(
    options: &Vue3CompilerOptions,
    state: &Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let mut context = vue3_model_suite_transform_context(options, scope);
    context["selfName"] = state
        .transform_element_self_name
        .clone()
        .map(Value::String)
        .unwrap_or(Value::Null);
    if let Some(is_script_setup) = state.transform_element_is_script_setup {
        context["isScriptSetup"] = json!(is_script_setup);
    }
    context["compatIsOnElement"] = json!(false);
    context["builtInComponents"] = json!([]);
    context
}

pub(crate) fn vue3_transform_element_suite_component_tag(
    projection: &Value,
    node: &Value,
    state: &mut Vue3SlotSuiteState,
) -> Value {
    match projection.get("kind").and_then(Value::as_str) {
        Some("dynamic") => {
            vue3_transform_element_suite_register_projection_helpers(projection, state);
            let argument = vue3_slot_suite_projection_node(
                projection.get("argument").unwrap_or(&Value::Null),
                node,
            );
            vue3_text_suite_call("RESOLVE_DYNAMIC_COMPONENT", vec![argument])
        }
        Some("helper") => {
            vue3_transform_element_suite_register_projection_helpers(projection, state);
            projection
                .get("helper")
                .and_then(Value::as_str)
                .or_else(|| projection.get("helperName").and_then(Value::as_str))
                .map(|helper| Value::String(helper.to_string()))
                .unwrap_or(Value::Null)
        }
        Some("expression") => {
            vue3_transform_element_suite_register_projection_helpers(projection, state);
            projection
                .get("content")
                .and_then(Value::as_str)
                .map(|content| Value::String(content.to_string()))
                .unwrap_or(Value::Null)
        }
        Some("asset") => {
            if let Some(component) = projection.get("component").and_then(Value::as_str) {
                vue3_transform_element_suite_push_unique(
                    &mut state.transform_element_components,
                    component.to_string(),
                );
            }
            projection
                .get("assetId")
                .and_then(Value::as_str)
                .map(|asset| Value::String(asset.to_string()))
                .unwrap_or_else(|| {
                    Value::String(vue3_once_suite_component_asset_id(
                        node.get("tag").and_then(Value::as_str).unwrap_or(""),
                    ))
                })
        }
        _ => Value::String(vue3_once_suite_component_asset_id(
            node.get("tag").and_then(Value::as_str).unwrap_or(""),
        )),
    }
}

pub(crate) fn vue3_transform_element_suite_register_projection_helpers(
    projection: &Value,
    state: &mut Vue3SlotSuiteState,
) {
    if projection
        .get("registerHelper")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        if let Some(helper) = projection
            .get("helper")
            .and_then(Value::as_str)
            .or_else(|| projection.get("helperName").and_then(Value::as_str))
        {
            vue3_transform_element_suite_push_unique(
                &mut state.transform_element_helpers,
                helper.to_string(),
            );
        }
    }
    for helper in projection
        .get("helpers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        vue3_transform_element_suite_push_unique(
            &mut state.transform_element_helpers,
            helper.to_string(),
        );
    }
}

pub(crate) fn vue3_transform_element_suite_should_build_component_slots(
    tag: &Value,
    children: &Value,
) -> bool {
    if children.is_null() {
        return false;
    }
    !vue3_transform_element_suite_helper_tag_name(tag)
        .is_some_and(|helper| matches!(helper, "TELEPORT" | "KEEP_ALIVE"))
}

pub(crate) fn vue3_transform_element_suite_helper_tag_name(tag: &Value) -> Option<&str> {
    tag.as_str()
}

pub(crate) fn vue3_transform_element_suite_element_children(node: &Value) -> Value {
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if children.is_empty() {
        Value::Null
    } else if children.len() == 1 && vue3_text_suite_direct_child_value(&children[0]) {
        children[0].clone()
    } else {
        Value::Array(children)
    }
}

pub(crate) fn vue3_transform_element_suite_push_unique(items: &mut Vec<String>, value: String) {
    if !items.iter().any(|item| item == &value) {
        items.push(value);
    }
}

pub(crate) fn vue3_transform_element_suite_finalize_root(
    root: &mut Value,
    state: &Vue3SlotSuiteState,
) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!(state.transform_element_components.clone());
    root["directives"] = json!(vue3_transform_element_suite_collect_directives(root, state));
    root["helpers"] = json!(vue3_transform_element_suite_helpers(root, state));
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_transform_element_suite_collect_directives(
    root: &Value,
    state: &Vue3SlotSuiteState,
) -> Vec<String> {
    vue3_if_suite_collect_directives(root)
        .into_iter()
        .filter(|directive| {
            !state
                .transform_element_noop_directives
                .iter()
                .any(|noop| noop == directive)
        })
        .collect()
}

pub(crate) fn vue3_transform_element_suite_helpers(
    root: &Value,
    state: &Vue3SlotSuiteState,
) -> Vec<String> {
    let mut used = Vec::<&'static str>::new();
    vue3_slot_suite_collect_helpers(root, &mut used);
    vue3_slot_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
    for helper in &state.transform_element_helpers {
        if let Some(helper) = vue3_transform_element_suite_known_helper(helper) {
            vue3_text_suite_add_helper(&mut used, helper);
        }
    }
    if !state.transform_element_components.is_empty() {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_COMPONENT");
    }
    if !vue3_transform_element_suite_collect_directives(root, state).is_empty() {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_DIRECTIVE");
    }
    [
        "TO_DISPLAY_STRING",
        "CREATE_ELEMENT_VNODE",
        "CREATE_TEXT",
        "CREATE_COMMENT",
        "RESOLVE_COMPONENT",
        "RESOLVE_DYNAMIC_COMPONENT",
        "WITH_CTX",
        "RENDER_LIST",
        "CREATE_SLOTS",
        "CREATE_VNODE",
        "OPEN_BLOCK",
        "CREATE_BLOCK",
        "CREATE_ELEMENT_BLOCK",
        "FRAGMENT",
        "RENDER_SLOT",
        "MERGE_PROPS",
        "NORMALIZE_PROPS",
        "NORMALIZE_CLASS",
        "NORMALIZE_STYLE",
        "GUARD_REACTIVE_PROPS",
        "TO_HANDLERS",
        "TO_HANDLER_KEY",
        "SET_BLOCK_TRACKING",
        "RESOLVE_DIRECTIVE",
        "WITH_DIRECTIVES",
        "TELEPORT",
        "SUSPENSE",
        "KEEP_ALIVE",
        "BASE_TRANSITION",
        "UNREF",
    ]
    .into_iter()
    .filter(|helper| used.iter().any(|used| used == helper))
    .map(str::to_string)
    .collect()
}

pub(crate) fn vue3_transform_element_suite_known_helper(helper: &str) -> Option<&'static str> {
    match helper {
        "TO_DISPLAY_STRING" => Some("TO_DISPLAY_STRING"),
        "CREATE_ELEMENT_VNODE" => Some("CREATE_ELEMENT_VNODE"),
        "CREATE_TEXT" => Some("CREATE_TEXT"),
        "CREATE_COMMENT" => Some("CREATE_COMMENT"),
        "RESOLVE_COMPONENT" => Some("RESOLVE_COMPONENT"),
        "RESOLVE_DYNAMIC_COMPONENT" => Some("RESOLVE_DYNAMIC_COMPONENT"),
        "WITH_CTX" => Some("WITH_CTX"),
        "RENDER_LIST" => Some("RENDER_LIST"),
        "CREATE_SLOTS" => Some("CREATE_SLOTS"),
        "CREATE_VNODE" => Some("CREATE_VNODE"),
        "OPEN_BLOCK" => Some("OPEN_BLOCK"),
        "CREATE_BLOCK" => Some("CREATE_BLOCK"),
        "CREATE_ELEMENT_BLOCK" => Some("CREATE_ELEMENT_BLOCK"),
        "FRAGMENT" => Some("FRAGMENT"),
        "RENDER_SLOT" => Some("RENDER_SLOT"),
        "MERGE_PROPS" => Some("MERGE_PROPS"),
        "NORMALIZE_PROPS" => Some("NORMALIZE_PROPS"),
        "NORMALIZE_CLASS" => Some("NORMALIZE_CLASS"),
        "NORMALIZE_STYLE" => Some("NORMALIZE_STYLE"),
        "GUARD_REACTIVE_PROPS" => Some("GUARD_REACTIVE_PROPS"),
        "TO_HANDLERS" => Some("TO_HANDLERS"),
        "TO_HANDLER_KEY" => Some("TO_HANDLER_KEY"),
        "SET_BLOCK_TRACKING" => Some("SET_BLOCK_TRACKING"),
        "RESOLVE_DIRECTIVE" => Some("RESOLVE_DIRECTIVE"),
        "WITH_DIRECTIVES" => Some("WITH_DIRECTIVES"),
        "TELEPORT" => Some("TELEPORT"),
        "SUSPENSE" => Some("SUSPENSE"),
        "KEEP_ALIVE" => Some("KEEP_ALIVE"),
        "BASE_TRANSITION" => Some("BASE_TRANSITION"),
        "UNREF" => Some("UNREF"),
        _ => None,
    }
}

pub(crate) fn vue3_slot_suite_slot_outlet_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let projection = vuec_vue3_core::transform_slot_outlet_projection(&json!({
        "node": node,
        "context": vue3_for_suite_slot_outlet_context(options, scope),
    }));
    let non_name_props = projection
        .get("process")
        .and_then(|process| process.get("nonNameProps"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut slot_state = Vue3SlotOutletSuiteState::default();
    let slot_props =
        vue3_slot_outlet_suite_props_codegen(node, &non_name_props, options, &mut slot_state);
    for error in slot_state.errors {
        state.errors.push(error);
    }
    let slot_name = vue3_slot_outlet_suite_slot_name(node, projection.get("process"));
    vue3_slot_outlet_suite_codegen(
        node,
        slot_name,
        slot_props,
        projection.get("codegen").unwrap_or(&Value::Null),
    )
}

pub(crate) fn vue3_slot_suite_build_slots(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let projection = vuec_vue3_core::build_slots_projection(&json!({
        "node": node,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    for error in projection
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(json!({
            "code": error.get("code").cloned().unwrap_or(json!(0)),
            "loc": error.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }));
    }
    vue3_slot_suite_materialize_slots_projection(&projection, node)
}

pub(crate) fn vue3_slot_suite_materialize_slots_projection(
    projection: &Value,
    node: &Value,
) -> Value {
    let mut properties = Vec::<Value>::new();
    for property in projection
        .get("properties")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        properties.push(vue3_once_suite_object_property(
            vue3_slot_suite_projection_node(property.get("key").unwrap_or(&Value::Null), node),
            vue3_slot_suite_slot_function(property, node),
        ));
    }
    let slot_flag = projection
        .get("slotFlag")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let slot_flag_text = projection
        .get("slotFlagText")
        .and_then(Value::as_str)
        .unwrap_or(match slot_flag {
            2 => "DYNAMIC",
            3 => "FORWARDED",
            _ => "STABLE",
        });
    properties.push(vue3_once_suite_object_property(
        vue3_once_suite_simple_expression("_", true),
        vue3_once_suite_simple_expression(&format!("{slot_flag} /* {slot_flag_text} */"), false),
    ));
    let base = json!({
        "type": 15,
        "properties": properties,
        "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    });
    let dynamic_slots = projection
        .get("dynamicSlots")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if dynamic_slots.is_empty() {
        return base;
    }
    vue3_text_suite_call(
        "CREATE_SLOTS",
        vec![
            base,
            json!({
                "type": 17,
                "elements": dynamic_slots
                    .iter()
                    .map(|slot| vue3_slot_suite_dynamic_slot(slot, node))
                    .collect::<Vec<_>>(),
                "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }),
        ],
    )
}

pub(crate) fn vue3_slot_suite_slot_function(property: &Value, node: &Value) -> Value {
    let returns = vue3_slot_suite_slot_children(property, node);
    json!({
        "type": 18,
        "params": vue3_slot_suite_projection_node(property.get("params").unwrap_or(&Value::Null), node),
        "returns": returns,
        "newline": false,
        "isSlot": true,
        "loc": property
            .get("loc")
            .cloned()
            .or_else(|| node.get("loc").cloned())
            .unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_slot_suite_slot_children(property: &Value, node: &Value) -> Value {
    let mut out = Vec::<Value>::new();
    let unwrap_template = property
        .get("unwrapTemplate")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    for index in property
        .get("indices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .map(|index| index as usize)
    {
        let Some(child) = node
            .get("children")
            .and_then(Value::as_array)
            .and_then(|children| children.get(index))
        else {
            continue;
        };
        if unwrap_template
            && vue3_public_node_type(child) == Some(1)
            && child.get("tag").and_then(Value::as_str) == Some("template")
        {
            out.extend(
                child
                    .get("children")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
            );
        } else {
            out.push(child.clone());
        }
    }
    Value::Array(out)
}

pub(crate) fn vue3_slot_suite_dynamic_slot(projection: &Value, node: &Value) -> Value {
    match projection.get("kind").and_then(Value::as_str) {
        Some("conditional") => json!({
            "type": 19,
            "test": vue3_slot_suite_projection_node(projection.get("test").unwrap_or(&Value::Null), node),
            "consequent": vue3_slot_suite_dynamic_slot(projection.get("consequent").unwrap_or(&Value::Null), node),
            "alternate": vue3_slot_suite_dynamic_slot(projection.get("alternate").unwrap_or(&Value::Null), node),
            "newline": true,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("for") => {
            let params = projection.get("params").unwrap_or(&Value::Null);
            vue3_text_suite_call(
                "RENDER_LIST",
                vec![
                    vue3_slot_suite_projection_node(
                        projection.get("source").unwrap_or(&Value::Null),
                        node,
                    ),
                    json!({
                        "type": 18,
                        "params": vue3_slot_suite_loop_params(params, node),
                        "returns": vue3_slot_suite_dynamic_slot(projection.get("slot").unwrap_or(&Value::Null), node),
                        "newline": true,
                        "isSlot": false,
                        "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
                    }),
                ],
            )
        }
        Some("dynamicSlot") => {
            let mut properties = vec![
                vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression("name", true),
                    vue3_slot_suite_projection_node(
                        projection.get("name").unwrap_or(&Value::Null),
                        node,
                    ),
                ),
                vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression("fn", true),
                    vue3_slot_suite_slot_function(
                        projection.get("slot").unwrap_or(&Value::Null),
                        node,
                    ),
                ),
            ];
            if let Some(key) = projection.get("key").and_then(Value::as_str) {
                properties.push(vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression("key", true),
                    vue3_once_suite_simple_expression(key, true),
                ));
            }
            json!({
                "type": 15,
                "properties": properties,
                "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            })
        }
        Some("simple") | Some("compound") | None => {
            vue3_slot_suite_projection_node(projection, node)
        }
        _ => vue3_slot_suite_projection_node(projection, node),
    }
}

pub(crate) fn vue3_slot_suite_loop_params(params: &Value, node: &Value) -> Vec<Value> {
    let args = ["value", "key", "index"]
        .into_iter()
        .map(|key| {
            params
                .get(key)
                .map(|value| vue3_slot_suite_projection_node(value, node))
                .unwrap_or(Value::Null)
        })
        .collect::<Vec<_>>();
    let Some(last) = args.iter().rposition(|arg| !arg.is_null()) else {
        return Vec::new();
    };
    args.into_iter()
        .take(last + 1)
        .enumerate()
        .map(|(index, arg)| {
            if arg.is_null() {
                vue3_once_suite_simple_expression(&"_".repeat(index + 1), false)
            } else {
                arg
            }
        })
        .collect()
}

pub(crate) fn vue3_slot_suite_projection_node(projection: &Value, node: &Value) -> Value {
    if projection.is_null()
        || projection
            .get("kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "undefined" || kind == "unchanged")
    {
        return Value::Null;
    }
    if projection.is_string() || projection.get("type").is_some() {
        return projection.clone();
    }
    match projection.get("kind").and_then(Value::as_str) {
        Some("simple") => json!({
            "type": 4,
            "content": projection.get("content").and_then(Value::as_str).unwrap_or(""),
            "isStatic": projection.get("isStatic").and_then(Value::as_bool).unwrap_or(false),
            "constType": projection.get("constType").and_then(Value::as_u64).unwrap_or(0),
            "loc": projection
                .get("loc")
                .cloned()
                .or_else(|| node.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("compound") => json!({
            "type": 8,
            "children": projection
                .get("children")
                .and_then(Value::as_array)
                .map(|children| children
                    .iter()
                    .map(|child| vue3_slot_suite_projection_node(child, node))
                    .collect::<Vec<_>>())
                .unwrap_or_default(),
            "loc": projection
                .get("loc")
                .cloned()
                .or_else(|| node.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        }),
        _ => Value::Null,
    }
}

pub(crate) fn vue3_slot_suite_is_template_slot(node: &Value) -> bool {
    vue3_public_node_type(node) == Some(1)
        && node.get("tagType").and_then(Value::as_u64) == Some(3)
        && vue3_text_suite_directive(node, "slot").is_some()
}

#[derive(Default)]
pub(crate) struct Vue3CacheStaticSuiteState {
    pub(crate) errors: Vec<Value>,
    pub(crate) cached: usize,
    pub(crate) hoists: Vec<Value>,
}

pub(crate) fn vue3_core_cache_static_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut slot_state = Vue3SlotSuiteState {
        text_directive_transforms: vec!["bind", "on"],
        skip_slot_scope_tracking: true,
        ..Default::default()
    };
    let scope = Vue3ModelSuiteScope::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        *children = vue3_slot_suite_transform_children(
            std::mem::take(children),
            &options,
            true,
            &mut slot_state,
            &scope,
        );
    }
    vue3_text_suite_apply_transform_text_with_directives(&mut root, &options, &["bind", "on"]);

    let mut state = Vue3CacheStaticSuiteState {
        errors: slot_state.errors,
        cached: slot_state.cached,
        hoists: Vec::new(),
    };
    let projection = vuec_vue3_core::cache_static_projection(&json!({
        "root": root,
        "context": vue3_model_suite_transform_context(&options, &scope),
    }));
    for operation in projection
        .get("operations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        vue3_cache_static_suite_apply_operation(&mut root, operation, &mut state);
    }
    vue3_cache_static_suite_sync_public_codegen_refs(&mut root);
    vue3_cache_static_suite_finalize_root(&mut root, &state);
    root["__vuecErrors"] = json!(state.errors);
    root
}

pub(crate) fn vue3_cache_static_suite_apply_operation(
    root: &mut Value,
    operation: &Value,
    state: &mut Vue3CacheStaticSuiteState,
) {
    match operation.get("kind").and_then(Value::as_str) {
        Some("setPatchFlag") => {
            let Some(target) = vue3_cache_static_suite_path_target_mut(root, operation, "path")
            else {
                return;
            };
            target["patchFlag"] = operation
                .get("patchFlag")
                .cloned()
                .unwrap_or_else(|| json!(-1));
        }
        Some("appendTextCallPatchFlag") => {
            let Some(target) = vue3_cache_static_suite_path_target_mut(root, operation, "path")
            else {
                return;
            };
            let Some(arguments) = target.get_mut("arguments").and_then(Value::as_array_mut) else {
                return;
            };
            if !arguments.is_empty() && arguments.len() < 2 {
                arguments.push(
                    operation
                        .get("patchFlag")
                        .cloned()
                        .unwrap_or_else(|| json!("-1 /* CACHED */")),
                );
            }
        }
        Some("setBlock") => {
            let Some(target) = vue3_cache_static_suite_path_target_mut(root, operation, "path")
            else {
                return;
            };
            target["isBlock"] = operation.get("isBlock").cloned().unwrap_or(json!(false));
        }
        Some("cacheCodegen") => {
            let Some(target) = vue3_cache_static_suite_path_target_mut(root, operation, "path")
            else {
                return;
            };
            let current = target.clone();
            *target = vue3_cache_static_suite_cache_expression(
                state,
                current,
                false,
                false,
                operation
                    .get("needArraySpread")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            );
        }
        Some("cacheChildrenArray") => {
            let children = vue3_cache_static_suite_path_target(root, operation, "childrenPath")
                .cloned()
                .unwrap_or_else(|| json!([]));
            let array = json!({
                "type": 17,
                "elements": children.as_array().cloned().unwrap_or_default(),
                "loc": vue3_cache_static_suite_loc(&children),
            });
            let Some(target) = vue3_cache_static_suite_path_target_mut(root, operation, "path")
            else {
                return;
            };
            *target = vue3_cache_static_suite_cache_expression(
                state,
                array,
                false,
                false,
                operation
                    .get("needArraySpread")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            );
        }
        Some("cacheSlotReturns") => {
            if let Some(owner_path) = operation
                .get("ownerPath")
                .and_then(vue3_cache_static_suite_path)
            {
                if let Some(owner) = vue3_cache_static_suite_node_at_path_mut(root, &owner_path) {
                    vue3_cache_static_suite_sync_component_slot_returns(owner);
                }
            }
            let Some(slot_returns) = vue3_cache_static_suite_slot_returns_mut(
                root,
                operation.get("ownerPath"),
                operation.get("slot"),
            ) else {
                return;
            };
            let current = slot_returns.clone();
            let array = json!({
                "type": 17,
                "elements": current.as_array().cloned().unwrap_or_default(),
                "loc": vue3_cache_static_suite_loc(&current),
            });
            *slot_returns = vue3_cache_static_suite_cache_expression(
                state,
                array,
                false,
                false,
                operation
                    .get("needArraySpread")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            );
        }
        Some("hoistProps") | Some("hoistDynamicProps") => {
            let Some(target) = vue3_cache_static_suite_path_target_mut(root, operation, "path")
            else {
                return;
            };
            let current = target.clone();
            state.hoists.push(current.clone());
            *target = vue3_cache_static_suite_hoisted_expression(state.hoists.len(), &current);
        }
        _ => {}
    }
}

pub(crate) fn vue3_cache_static_suite_path_target_mut<'a>(
    root: &'a mut Value,
    operation: &Value,
    key: &str,
) -> Option<&'a mut Value> {
    let path = vue3_cache_static_suite_path(operation.get(key)?)?;
    vue3_cache_static_suite_node_at_path_mut(root, &path)
}

pub(crate) fn vue3_cache_static_suite_path_target<'a>(
    root: &'a Value,
    operation: &Value,
    key: &str,
) -> Option<&'a Value> {
    let path = vue3_cache_static_suite_path(operation.get(key)?)?;
    vue3_cache_static_suite_node_at_path(root, &path)
}

pub(crate) fn vue3_cache_static_suite_path(value: &Value) -> Option<Vec<String>> {
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(|segment| segment.as_str().map(ToOwned::to_owned))
            .collect(),
    )
}

pub(crate) fn vue3_cache_static_suite_node_at_path_mut<'a>(
    root: &'a mut Value,
    path: &[String],
) -> Option<&'a mut Value> {
    let mut current = root;
    for segment in path {
        if let Ok(index) = segment.parse::<usize>() {
            current = current.as_array_mut()?.get_mut(index)?;
        } else {
            current = current.get_mut(segment)?;
        }
    }
    Some(current)
}

pub(crate) fn vue3_cache_static_suite_node_at_path<'a>(
    root: &'a Value,
    path: &[String],
) -> Option<&'a Value> {
    let mut current = root;
    for segment in path {
        if let Ok(index) = segment.parse::<usize>() {
            current = current.as_array()?.get(index)?;
        } else {
            current = current.get(segment)?;
        }
    }
    Some(current)
}

pub(crate) fn vue3_cache_static_suite_slot_returns_mut<'a>(
    root: &'a mut Value,
    owner_path: Option<&Value>,
    slot: Option<&Value>,
) -> Option<&'a mut Value> {
    let owner_path = vue3_cache_static_suite_path(owner_path?)?;
    let owner = vue3_cache_static_suite_node_at_path_mut(root, &owner_path)?;
    let default_slot = Value::Null;
    let slot = slot.unwrap_or(&default_slot);
    let properties = owner
        .get_mut("codegenNode")?
        .get_mut("children")?
        .get_mut("properties")?
        .as_array_mut()?;
    let property = properties
        .iter_mut()
        .find(|property| vue3_cache_static_suite_slot_matches(property, slot))?;
    property.get_mut("value")?.get_mut("returns")
}

pub(crate) fn vue3_cache_static_suite_slot_matches(property: &Value, slot: &Value) -> bool {
    let Some(key) = property.get("key") else {
        return false;
    };
    match slot.get("kind").and_then(Value::as_str) {
        Some("static") => {
            key.get("content").and_then(Value::as_str)
                == slot.get("name").and_then(Value::as_str).or(Some("default"))
        }
        Some("dynamic") => slot.get("node").is_some_and(|node| key == node),
        _ => false,
    }
}

pub(crate) fn vue3_cache_static_suite_cache_expression(
    state: &mut Vue3CacheStaticSuiteState,
    value: Value,
    need_pause_tracking: bool,
    in_v_once: bool,
    need_array_spread: bool,
) -> Value {
    let index = state.cached;
    state.cached += 1;
    json!({
        "type": 20,
        "index": index,
        "value": value,
        "needPauseTracking": need_pause_tracking,
        "inVOnce": in_v_once,
        "needArraySpread": need_array_spread,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_cache_static_suite_hoisted_expression(index: usize, value: &Value) -> Value {
    json!({
        "type": 4,
        "content": format!("_hoisted_{index}"),
        "isStatic": false,
        "constType": 2,
        "loc": vue3_cache_static_suite_loc(value),
    })
}

pub(crate) fn vue3_cache_static_suite_loc(value: &Value) -> Value {
    value
        .get("loc")
        .cloned()
        .unwrap_or_else(vue3_loc_stub_value)
}

pub(crate) fn vue3_cache_static_suite_finalize_root(
    root: &mut Value,
    state: &Vue3CacheStaticSuiteState,
) {
    vue3_cache_static_suite_set_root_codegen(root);
    root["components"] = json!(vue3_slot_suite_components(root));
    root["directives"] = json!(vue3_if_suite_collect_directives(root));
    root["hoists"] = Value::Array(state.hoists.clone());
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
    root["helpers"] = json!(vue3_cache_static_suite_helpers(root));
}

#[derive(Default)]
pub(crate) struct Vue3CacheStaticHelperTracker {
    pub(crate) helpers: Vec<(&'static str, usize)>,
}

impl Vue3CacheStaticHelperTracker {
    pub(crate) fn add(&mut self, helper: &'static str) {
        if let Some((_, count)) = self
            .helpers
            .iter_mut()
            .find(|(existing, _)| *existing == helper)
        {
            *count += 1;
        } else {
            self.helpers.push((helper, 1));
        }
    }

    pub(crate) fn remove(&mut self, helper: &'static str) {
        let Some(index) = self
            .helpers
            .iter()
            .position(|(existing, _)| *existing == helper)
        else {
            return;
        };
        if self.helpers[index].1 > 1 {
            self.helpers[index].1 -= 1;
        } else {
            self.helpers.remove(index);
        }
    }

    pub(crate) fn into_strings(self) -> Vec<String> {
        self.helpers
            .into_iter()
            .map(|(helper, _)| helper.to_string())
            .collect()
    }
}

pub(crate) struct Vue3CacheStaticHelperContext<'a> {
    pub(crate) hoists: &'a [Value],
}

pub(crate) fn vue3_cache_static_suite_helpers(root: &Value) -> Vec<String> {
    let hoists = root
        .get("hoists")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let context = Vue3CacheStaticHelperContext { hoists };
    let mut tracker = Vue3CacheStaticHelperTracker::default();
    vue3_cache_static_suite_collect_transform_helpers(root, &context, &mut tracker);
    tracker.into_strings()
}

pub(crate) fn vue3_cache_static_suite_collect_transform_helpers(
    node: &Value,
    context: &Vue3CacheStaticHelperContext,
    tracker: &mut Vue3CacheStaticHelperTracker,
) {
    match vue3_public_node_type(node) {
        Some(0) => {
            if let Some(children) = node.get("children").and_then(Value::as_array) {
                for child in children {
                    vue3_cache_static_suite_collect_transform_helpers(child, context, tracker);
                }
            }
            if node
                .get("codegenNode")
                .and_then(|codegen| codegen.get("tag"))
                .and_then(Value::as_str)
                == Some("FRAGMENT")
            {
                if let Some(codegen) = node.get("codegenNode") {
                    vue3_cache_static_suite_collect_vnode_self_helpers(codegen, tracker);
                }
            }
        }
        Some(1) => {
            if let Some(children) = node.get("children").and_then(Value::as_array) {
                for child in children {
                    vue3_cache_static_suite_collect_transform_helpers(child, context, tracker);
                }
            }
            vue3_cache_static_suite_collect_element_exit_helpers(node, context, tracker);
        }
        Some(2) => {}
        Some(3) => tracker.add("CREATE_COMMENT"),
        Some(5) => tracker.add("TO_DISPLAY_STRING"),
        Some(8) => {
            if let Some(children) = node.get("children").and_then(Value::as_array) {
                for child in children {
                    vue3_cache_static_suite_collect_transform_helpers(child, context, tracker);
                }
            }
        }
        Some(9) => vue3_cache_static_suite_collect_if_helpers(node, context, tracker),
        Some(10) => {
            if let Some(children) = node.get("children").and_then(Value::as_array) {
                for child in children {
                    vue3_cache_static_suite_collect_transform_helpers(child, context, tracker);
                }
            }
        }
        Some(11) => vue3_cache_static_suite_collect_for_helpers(node, context, tracker),
        Some(12) => {
            if let Some(content) = node.get("content") {
                vue3_cache_static_suite_collect_transform_helpers(content, context, tracker);
            }
            if let Some(codegen) = node.get("codegenNode") {
                vue3_cache_static_suite_collect_expression_helpers(codegen, context, tracker);
            }
        }
        Some(20) => {
            if node
                .get("needPauseTracking")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                tracker.add("SET_BLOCK_TRACKING");
            }
            if let Some(value) = node.get("value") {
                vue3_cache_static_suite_collect_transform_helpers(value, context, tracker);
            }
        }
        Some(17) => {
            if let Some(elements) = node.get("elements").and_then(Value::as_array) {
                for element in elements {
                    vue3_cache_static_suite_collect_transform_helpers(element, context, tracker);
                }
            }
        }
        Some(18) => {
            if node.get("isSlot").and_then(Value::as_bool).unwrap_or(false) {
                tracker.add("WITH_CTX");
            }
            if let Some(returns) = node.get("returns") {
                vue3_cache_static_suite_collect_transform_helpers(returns, context, tracker);
            }
        }
        _ => {
            if let Some(items) = node.as_array() {
                for item in items {
                    vue3_cache_static_suite_collect_transform_helpers(item, context, tracker);
                }
            }
        }
    }
}

pub(crate) fn vue3_cache_static_suite_collect_element_exit_helpers(
    node: &Value,
    context: &Vue3CacheStaticHelperContext,
    tracker: &mut Vue3CacheStaticHelperTracker,
) {
    let Some(codegen) = node.get("codegenNode") else {
        return;
    };
    if vue3_public_node_type(codegen) != Some(13) {
        vue3_cache_static_suite_collect_expression_helpers(codegen, context, tracker);
        return;
    }
    if codegen
        .get("isComponent")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && codegen
            .get("tag")
            .and_then(Value::as_str)
            .is_some_and(|tag| tag.starts_with("_component_"))
    {
        tracker.add("RESOLVE_COMPONENT");
    }
    if let Some(props) = codegen.get("props") {
        vue3_cache_static_suite_collect_hoisted_expression_helpers(props, context, tracker);
    }
    if let Some(children) = codegen.get("children") {
        vue3_cache_static_suite_collect_slot_build_helpers(children, context, tracker);
    }
    if codegen
        .get("directives")
        .is_some_and(|directives| !directives.is_null())
    {
        tracker.add("RESOLVE_DIRECTIVE");
    }
    vue3_cache_static_suite_collect_vnode_self_helpers(codegen, tracker);
}

pub(crate) fn vue3_cache_static_suite_collect_slot_build_helpers(
    node: &Value,
    context: &Vue3CacheStaticHelperContext,
    tracker: &mut Vue3CacheStaticHelperTracker,
) {
    match vue3_public_node_type(node) {
        Some(14) if node.get("callee").and_then(Value::as_str) == Some("CREATE_SLOTS") => {
            tracker.add("CREATE_SLOTS");
            if let Some(arguments) = node.get("arguments").and_then(Value::as_array) {
                for argument in arguments {
                    vue3_cache_static_suite_collect_slot_build_helpers(argument, context, tracker);
                }
            }
        }
        Some(14) if node.get("callee").and_then(Value::as_str) == Some("RENDER_LIST") => {
            tracker.add("RENDER_LIST");
            if let Some(arguments) = node.get("arguments").and_then(Value::as_array) {
                for argument in arguments {
                    vue3_cache_static_suite_collect_slot_build_helpers(argument, context, tracker);
                }
            }
        }
        Some(15) => {
            if let Some(properties) = node.get("properties").and_then(Value::as_array) {
                for property in properties {
                    if let Some(value) = property.get("value") {
                        vue3_cache_static_suite_collect_slot_build_helpers(value, context, tracker);
                    }
                }
            }
        }
        Some(18) => {
            if node.get("isSlot").and_then(Value::as_bool).unwrap_or(false) {
                tracker.add("WITH_CTX");
            }
            if let Some(returns) = node.get("returns") {
                vue3_cache_static_suite_collect_transform_helpers(returns, context, tracker);
            }
        }
        Some(19) => {
            if let Some(consequent) = node.get("consequent") {
                vue3_cache_static_suite_collect_slot_build_helpers(consequent, context, tracker);
            }
            if let Some(alternate) = node.get("alternate") {
                vue3_cache_static_suite_collect_slot_build_helpers(alternate, context, tracker);
            }
        }
        _ => {}
    }
}

pub(crate) fn vue3_cache_static_suite_collect_if_helpers(
    node: &Value,
    context: &Vue3CacheStaticHelperContext,
    tracker: &mut Vue3CacheStaticHelperTracker,
) {
    if let Some(branches) = node.get("branches").and_then(Value::as_array) {
        for branch in branches {
            vue3_cache_static_suite_collect_transform_helpers(branch, context, tracker);
        }
    }
    if let Some(codegen) = node.get("codegenNode") {
        vue3_cache_static_suite_collect_if_codegen_helpers(codegen, tracker);
    }
}

pub(crate) fn vue3_cache_static_suite_collect_if_codegen_helpers(
    node: &Value,
    tracker: &mut Vue3CacheStaticHelperTracker,
) {
    match vue3_public_node_type(node) {
        Some(19) => {
            if let Some(consequent) = node.get("consequent") {
                vue3_cache_static_suite_collect_if_codegen_helpers(consequent, tracker);
            }
            if node
                .get("alternate")
                .and_then(|alternate| alternate.get("callee"))
                .and_then(Value::as_str)
                == Some("CREATE_COMMENT")
            {
                tracker.add("CREATE_COMMENT");
            } else if let Some(alternate) = node.get("alternate") {
                vue3_cache_static_suite_collect_if_codegen_helpers(alternate, tracker);
            }
        }
        Some(13) => {
            if node.get("tag").and_then(Value::as_str) == Some("FRAGMENT") {
                vue3_cache_static_suite_collect_vnode_self_helpers(node, tracker);
            }
        }
        Some(14) if node.get("callee").and_then(Value::as_str) == Some("CREATE_COMMENT") => {
            tracker.add("CREATE_COMMENT");
        }
        _ => {}
    }
}

pub(crate) fn vue3_cache_static_suite_collect_for_helpers(
    node: &Value,
    context: &Vue3CacheStaticHelperContext,
    tracker: &mut Vue3CacheStaticHelperTracker,
) {
    tracker.add("RENDER_LIST");
    if let Some(codegen) = node.get("codegenNode") {
        vue3_cache_static_suite_collect_vnode_self_helpers(codegen, tracker);
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            vue3_cache_static_suite_collect_transform_helpers(child, context, tracker);
        }
        vue3_cache_static_suite_collect_for_exit_helpers(node, children, tracker);
    }
}

pub(crate) fn vue3_cache_static_suite_collect_for_exit_helpers(
    node: &Value,
    children: &[Value],
    tracker: &mut Vue3CacheStaticHelperTracker,
) {
    if children.len() != 1 || vue3_public_node_type(&children[0]) != Some(1) {
        if children.len() != 1 || vue3_public_node_type(&children[0]) != Some(11) {
            tracker.add("FRAGMENT");
            tracker.add("OPEN_BLOCK");
            tracker.add("CREATE_ELEMENT_BLOCK");
        }
        return;
    }
    let child_codegen = children[0].get("codegenNode").unwrap_or(&Value::Null);
    if vue3_public_node_type(child_codegen) != Some(13) {
        return;
    }
    let final_returns = node
        .get("codegenNode")
        .and_then(|codegen| codegen.get("children"))
        .and_then(|children| children.get("arguments"))
        .and_then(Value::as_array)
        .and_then(|arguments| arguments.get(1))
        .and_then(|function| function.get("returns"))
        .unwrap_or(child_codegen);
    let child_was_block = child_codegen
        .get("isBlock")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let final_is_block = final_returns
        .get("isBlock")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let is_component = child_codegen
        .get("isComponent")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if final_is_block && !child_was_block {
        tracker.remove(if is_component {
            "CREATE_VNODE"
        } else {
            "CREATE_ELEMENT_VNODE"
        });
        tracker.add("OPEN_BLOCK");
        tracker.add(if is_component {
            "CREATE_BLOCK"
        } else {
            "CREATE_ELEMENT_BLOCK"
        });
    } else if !final_is_block && child_was_block {
        tracker.remove("OPEN_BLOCK");
        tracker.remove(if is_component {
            "CREATE_BLOCK"
        } else {
            "CREATE_ELEMENT_BLOCK"
        });
        tracker.add(if is_component {
            "CREATE_VNODE"
        } else {
            "CREATE_ELEMENT_VNODE"
        });
    }
}

pub(crate) fn vue3_cache_static_suite_collect_vnode_self_helpers(
    node: &Value,
    tracker: &mut Vue3CacheStaticHelperTracker,
) {
    if node.get("tag").and_then(Value::as_str) == Some("FRAGMENT") {
        tracker.add("FRAGMENT");
    }
    let is_component = node
        .get("isComponent")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if node
        .get("isBlock")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        tracker.add("OPEN_BLOCK");
        tracker.add(if is_component {
            "CREATE_BLOCK"
        } else {
            "CREATE_ELEMENT_BLOCK"
        });
    } else {
        tracker.add(if is_component {
            "CREATE_VNODE"
        } else {
            "CREATE_ELEMENT_VNODE"
        });
    }
    if node
        .get("directives")
        .is_some_and(|directives| !directives.is_null())
    {
        tracker.add("WITH_DIRECTIVES");
    }
}

pub(crate) fn vue3_cache_static_suite_collect_hoisted_expression_helpers(
    node: &Value,
    context: &Vue3CacheStaticHelperContext,
    tracker: &mut Vue3CacheStaticHelperTracker,
) {
    if let Some(content) = node.get("content").and_then(Value::as_str) {
        if let Some(index) = content
            .strip_prefix("_hoisted_")
            .and_then(|value| value.parse::<usize>().ok())
        {
            if let Some(hoist) = context.hoists.get(index.saturating_sub(1)) {
                vue3_cache_static_suite_collect_expression_helpers(hoist, context, tracker);
                return;
            }
        }
    }
    vue3_cache_static_suite_collect_expression_helpers(node, context, tracker);
}

pub(crate) fn vue3_cache_static_suite_collect_expression_helpers(
    node: &Value,
    context: &Vue3CacheStaticHelperContext,
    tracker: &mut Vue3CacheStaticHelperTracker,
) {
    match vue3_public_node_type(node) {
        Some(5) => tracker.add("TO_DISPLAY_STRING"),
        Some(8) => {
            if let Some(children) = node.get("children").and_then(Value::as_array) {
                for child in children {
                    vue3_cache_static_suite_collect_expression_helpers(child, context, tracker);
                }
            }
        }
        Some(12) => {
            if let Some(content) = node.get("content") {
                vue3_cache_static_suite_collect_expression_helpers(content, context, tracker);
            }
            if let Some(codegen) = node.get("codegenNode") {
                vue3_cache_static_suite_collect_expression_helpers(codegen, context, tracker);
            }
        }
        Some(14) => {
            if let Some(helper) = node
                .get("callee")
                .and_then(Value::as_str)
                .and_then(vue3_cache_static_suite_runtime_helper)
            {
                tracker.add(helper);
            }
            if let Some(arguments) = node.get("arguments").and_then(Value::as_array) {
                for argument in arguments {
                    vue3_cache_static_suite_collect_expression_helpers(argument, context, tracker);
                }
            }
        }
        Some(15) => {
            if let Some(properties) = node.get("properties").and_then(Value::as_array) {
                for property in properties {
                    if let Some(key) = property.get("key") {
                        vue3_cache_static_suite_collect_expression_helpers(key, context, tracker);
                    }
                    if let Some(value) = property.get("value") {
                        vue3_cache_static_suite_collect_expression_helpers(value, context, tracker);
                    }
                }
            }
        }
        Some(17) => {
            if let Some(elements) = node.get("elements").and_then(Value::as_array) {
                for element in elements {
                    vue3_cache_static_suite_collect_expression_helpers(element, context, tracker);
                }
            }
        }
        Some(18) => {
            if node.get("isSlot").and_then(Value::as_bool).unwrap_or(false) {
                tracker.add("WITH_CTX");
            }
            if let Some(returns) = node.get("returns") {
                vue3_cache_static_suite_collect_expression_helpers(returns, context, tracker);
            }
        }
        Some(20) => {
            if node
                .get("needPauseTracking")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                tracker.add("SET_BLOCK_TRACKING");
            }
            if let Some(value) = node.get("value") {
                vue3_cache_static_suite_collect_expression_helpers(value, context, tracker);
            }
        }
        _ => {}
    }
}

pub(crate) fn vue3_cache_static_suite_runtime_helper(name: &str) -> Option<&'static str> {
    match name {
        "CREATE_TEXT" => Some("CREATE_TEXT"),
        "CREATE_COMMENT" => Some("CREATE_COMMENT"),
        "RENDER_LIST" => Some("RENDER_LIST"),
        "CREATE_SLOTS" => Some("CREATE_SLOTS"),
        "RENDER_SLOT" => Some("RENDER_SLOT"),
        "MERGE_PROPS" => Some("MERGE_PROPS"),
        "NORMALIZE_PROPS" => Some("NORMALIZE_PROPS"),
        "NORMALIZE_CLASS" => Some("NORMALIZE_CLASS"),
        "NORMALIZE_STYLE" => Some("NORMALIZE_STYLE"),
        "GUARD_REACTIVE_PROPS" => Some("GUARD_REACTIVE_PROPS"),
        "TO_HANDLERS" => Some("TO_HANDLERS"),
        "TO_HANDLER_KEY" => Some("TO_HANDLER_KEY"),
        _ => None,
    }
}

pub(crate) fn vue3_cache_static_suite_set_root_codegen(root: &mut Value) {
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let visible = children
        .iter()
        .enumerate()
        .filter(|(_, child)| vue3_public_node_type(child) != Some(3))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let has_comments = children
        .iter()
        .any(|child| vue3_public_node_type(child) == Some(3));
    if has_comments && visible.len() == 1 && children.len() > 1 {
        root["codegenNode"] = vue3_once_suite_vnode_call(
            "FRAGMENT",
            Value::Null,
            Value::Array(children),
            Some(json!(2112)),
            Value::Null,
            true,
            false,
            false,
        );
        return;
    }
    vue3_once_suite_set_root_codegen(root);
}

pub(crate) fn vue3_cache_static_suite_sync_public_codegen_refs(node: &mut Value) {
    match vue3_public_node_type(node) {
        Some(0) => {
            if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
                for child in children {
                    vue3_cache_static_suite_sync_public_codegen_refs(child);
                }
            }
        }
        Some(1) => {
            if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
                for child in children {
                    vue3_cache_static_suite_sync_public_codegen_refs(child);
                }
            }
            vue3_cache_static_suite_sync_element_codegen(node);
        }
        Some(9) => {
            if let Some(branches) = node.get_mut("branches").and_then(Value::as_array_mut) {
                for branch in branches {
                    vue3_cache_static_suite_sync_public_codegen_refs(branch);
                }
            }
            vue3_cache_static_suite_sync_if_codegen(node);
        }
        Some(10) => {
            if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
                for child in children {
                    vue3_cache_static_suite_sync_public_codegen_refs(child);
                }
            }
        }
        Some(11) => {
            if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
                for child in children {
                    vue3_cache_static_suite_sync_public_codegen_refs(child);
                }
            }
            vue3_cache_static_suite_sync_for_codegen(node);
        }
        _ => {}
    }
}

pub(crate) fn vue3_cache_static_suite_sync_element_codegen(node: &mut Value) {
    let children = node.get("children").cloned().unwrap_or_else(|| json!([]));
    let tag_type = node.get("tagType").and_then(Value::as_u64);
    if tag_type == Some(0) {
        if let Some(codegen) = node.get_mut("codegenNode") {
            if vue3_public_node_type(codegen) == Some(13)
                && codegen.get("children").and_then(Value::as_array).is_some()
            {
                codegen["children"] = children;
            }
        }
        return;
    }
    if tag_type == Some(1) {
        vue3_cache_static_suite_sync_component_slot_returns(node);
    }
}

pub(crate) fn vue3_cache_static_suite_sync_component_slot_returns(node: &mut Value) {
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(codegen_children) = node
        .get_mut("codegenNode")
        .and_then(|codegen| codegen.get_mut("children"))
    else {
        return;
    };
    vue3_cache_static_suite_sync_slot_object_returns(codegen_children, &children);
    if vue3_public_node_type(codegen_children) == Some(14)
        && codegen_children.get("callee").and_then(Value::as_str) == Some("CREATE_SLOTS")
    {
        if let Some(arguments) = codegen_children
            .get_mut("arguments")
            .and_then(Value::as_array_mut)
        {
            if let Some(base_slots) = arguments.get_mut(0) {
                vue3_cache_static_suite_sync_slot_object_returns(base_slots, &children);
            }
        }
    }
}

pub(crate) fn vue3_cache_static_suite_sync_slot_object_returns(
    slots: &mut Value,
    children: &[Value],
) {
    if vue3_public_node_type(slots) != Some(15) {
        return;
    }
    let Some(properties) = slots.get_mut("properties").and_then(Value::as_array_mut) else {
        return;
    };
    for property in properties {
        let key = property.get("key").cloned().unwrap_or(Value::Null);
        if key.get("content").and_then(Value::as_str) == Some("_") {
            continue;
        }
        let Some(returns) = property
            .get_mut("value")
            .and_then(|value| value.get_mut("returns"))
        else {
            continue;
        };
        if !returns.is_array() {
            continue;
        }
        if key.get("content").and_then(Value::as_str) == Some("default")
            && key
                .get("isStatic")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            *returns = Value::Array(
                children
                    .iter()
                    .filter(|child| !vue3_slot_suite_is_template_slot(child))
                    .cloned()
                    .collect(),
            );
            continue;
        }
        if let Some(template) = children
            .iter()
            .find(|child| vue3_cache_static_suite_template_slot_matches(child, &key))
        {
            *returns = template
                .get("children")
                .cloned()
                .unwrap_or_else(|| json!([]));
        }
    }
}

pub(crate) fn vue3_cache_static_suite_template_slot_matches(template: &Value, key: &Value) -> bool {
    if !vue3_slot_suite_is_template_slot(template) {
        return false;
    }
    let Some(slot) = vue3_text_suite_directive(template, "slot") else {
        return false;
    };
    let arg = slot.get("arg").unwrap_or(&Value::Null);
    if key
        .get("isStatic")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return arg
            .get("isStatic")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && key.get("content").and_then(Value::as_str)
                == arg.get("content").and_then(Value::as_str);
    }
    key == arg
}

pub(crate) fn vue3_cache_static_suite_sync_if_codegen(node: &mut Value) {
    let branches = node
        .get("branches")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if branches.is_empty() {
        return;
    }
    let current = node.get("codegenNode").cloned().unwrap_or(Value::Null);
    node["codegenNode"] = vue3_cache_static_suite_sync_if_codegen_node(&current, &branches, 0);
}

pub(crate) fn vue3_cache_static_suite_sync_if_codegen_node(
    current: &Value,
    branches: &[Value],
    index: usize,
) -> Value {
    let Some(branch) = branches.get(index) else {
        return current.clone();
    };
    if vue3_public_node_type(current) == Some(19) {
        let mut next = current.clone();
        next["consequent"] = vue3_cache_static_suite_branch_codegen(
            next.get("consequent").unwrap_or(&Value::Null),
            branch,
        );
        let alternate = next.get("alternate").cloned().unwrap_or(Value::Null);
        next["alternate"] =
            vue3_cache_static_suite_sync_if_codegen_node(&alternate, branches, index + 1);
        return next;
    }
    vue3_cache_static_suite_branch_codegen(current, branch)
}

pub(crate) fn vue3_cache_static_suite_branch_codegen(existing: &Value, branch: &Value) -> Value {
    let children = branch
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if children.len() == 1 {
        return children[0]
            .get("codegenNode")
            .cloned()
            .unwrap_or_else(|| children[0].clone());
    }
    if vue3_public_node_type(existing) == Some(13)
        && existing.get("tag").and_then(Value::as_str) == Some("FRAGMENT")
    {
        let mut next = existing.clone();
        next["children"] = Value::Array(children);
        return next;
    }
    existing.clone()
}

pub(crate) fn vue3_cache_static_suite_sync_for_codegen(node: &mut Value) {
    let returns = vue3_cache_static_suite_for_returns(node);
    let Some(returns) = returns else {
        return;
    };
    let Some(function) = node
        .get_mut("codegenNode")
        .and_then(|codegen| codegen.get_mut("children"))
        .and_then(|children| children.get_mut("arguments"))
        .and_then(Value::as_array_mut)
        .and_then(|arguments| arguments.get_mut(1))
    else {
        return;
    };
    let existing = function.get("returns").cloned().unwrap_or(Value::Null);
    function["returns"] = vue3_cache_static_suite_merge_for_returns(&existing, returns);
}

pub(crate) fn vue3_cache_static_suite_for_returns(node: &Value) -> Option<Value> {
    let children = node.get("children").and_then(Value::as_array)?;
    if children.len() == 1 {
        return Some(
            children[0]
                .get("codegenNode")
                .cloned()
                .unwrap_or_else(|| children[0].clone()),
        );
    }
    Some(Value::Array(children.clone()))
}

pub(crate) fn vue3_cache_static_suite_merge_for_returns(existing: &Value, updated: Value) -> Value {
    if vue3_public_node_type(existing) != Some(13) || vue3_public_node_type(&updated) != Some(13) {
        return updated;
    }
    let mut merged = updated;
    if let Some(is_block) = existing.get("isBlock").and_then(Value::as_bool) {
        merged["isBlock"] = json!(is_block);
    }
    if let Some(disable_tracking) = existing.get("disableTracking").and_then(Value::as_bool) {
        merged["disableTracking"] = json!(disable_tracking);
    }
    if let Some(key) = existing
        .get("props")
        .and_then(vue3_cache_static_suite_vnode_key_property)
    {
        vue3_cache_static_suite_inject_key_if_missing(&mut merged, key);
    }
    merged
}

pub(crate) fn vue3_cache_static_suite_vnode_key_property(props: &Value) -> Option<Value> {
    props
        .get("properties")
        .and_then(Value::as_array)?
        .iter()
        .find(|property| {
            property
                .get("key")
                .and_then(vue3_model_suite_static_prop_name)
                .as_deref()
                == Some("key")
        })
        .cloned()
}

pub(crate) fn vue3_cache_static_suite_inject_key_if_missing(vnode: &mut Value, key: Value) {
    if vue3_public_node_type(vnode) != Some(13) {
        return;
    }
    if vnode
        .get("props")
        .is_some_and(|props| vue3_cache_static_suite_vnode_key_property(props).is_some())
    {
        return;
    }
    vue3_for_suite_inject_prop(vnode, key);
}

pub(crate) fn vue3_slot_suite_finalize_root(root: &mut Value, state: &Vue3SlotSuiteState) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!(vue3_slot_suite_components(root));
    root["directives"] = json!(vue3_if_suite_collect_directives(root));
    root["helpers"] = json!(vue3_slot_suite_helpers(root));
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_transform_suite_finalize_root(root: &mut Value, state: &Vue3SlotSuiteState) {
    vue3_transform_suite_set_root_codegen(root);
    root["components"] = json!(vue3_slot_suite_components(root));
    root["directives"] = json!(vue3_if_suite_collect_directives(root));
    root["helpers"] = json!(vue3_slot_suite_helpers(root));
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_transform_suite_set_root_codegen(root: &mut Value) {
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    root["codegenNode"] = match children.as_slice() {
        [] => Value::Null,
        [_] => vue3_transform_suite_single_root_codegen(root),
        _ => vue3_once_suite_vnode_call(
            "FRAGMENT",
            Value::Null,
            Value::Array(children.clone()),
            Some(json!(vue3_transform_suite_root_fragment_patch_flag(
                &children
            ))),
            Value::Null,
            true,
            false,
            false,
        ),
    };
}

pub(crate) fn vue3_transform_suite_single_root_codegen(root: &mut Value) -> Value {
    let Some(child) = root
        .get_mut("children")
        .and_then(Value::as_array_mut)
        .and_then(|children| children.first_mut())
    else {
        return Value::Null;
    };
    if vue3_transform_suite_is_single_element_root_child(child)
        && child
            .get("codegenNode")
            .is_some_and(|codegen| !codegen.is_null())
    {
        if child.get("codegenNode").and_then(vue3_public_node_type) == Some(13) {
            child["codegenNode"]["isBlock"] = json!(true);
        }
        return child.get("codegenNode").cloned().unwrap_or(Value::Null);
    }
    child.clone()
}

pub(crate) fn vue3_transform_suite_is_single_element_root_child(child: &Value) -> bool {
    vue3_public_node_type(child) == Some(1)
        && child.get("tagType").and_then(Value::as_u64) != Some(2)
}

pub(crate) fn vue3_transform_suite_root_fragment_patch_flag(children: &[Value]) -> u16 {
    if children
        .iter()
        .filter(|child| vue3_public_node_type(child) != Some(3))
        .count()
        == 1
        && children
            .iter()
            .any(|child| vue3_public_node_type(child) == Some(3))
    {
        64 | 2048
    } else {
        64
    }
}

pub(crate) fn vue3_slot_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_slot_suite_collect_helpers(root, &mut used);
    vue3_slot_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
    if root
        .get("components")
        .and_then(Value::as_array)
        .is_some_and(|components| !components.is_empty())
    {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_COMPONENT");
    }
    if !vue3_if_suite_collect_directives(root).is_empty() {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_DIRECTIVE");
    }
    [
        "TO_DISPLAY_STRING",
        "CREATE_ELEMENT_VNODE",
        "CREATE_TEXT",
        "CREATE_COMMENT",
        "RESOLVE_COMPONENT",
        "WITH_CTX",
        "RENDER_LIST",
        "CREATE_SLOTS",
        "CREATE_VNODE",
        "OPEN_BLOCK",
        "CREATE_BLOCK",
        "CREATE_ELEMENT_BLOCK",
        "FRAGMENT",
        "RENDER_SLOT",
        "MERGE_PROPS",
        "NORMALIZE_PROPS",
        "NORMALIZE_CLASS",
        "NORMALIZE_STYLE",
        "GUARD_REACTIVE_PROPS",
        "TO_HANDLERS",
        "TO_HANDLER_KEY",
        "SET_BLOCK_TRACKING",
        "RESOLVE_DIRECTIVE",
        "WITH_DIRECTIVES",
    ]
    .into_iter()
    .filter(|helper| used.iter().any(|used| used == helper))
    .map(str::to_string)
    .collect()
}

pub(crate) fn vue3_slot_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    match vue3_public_node_type(node) {
        Some(3) => vue3_text_suite_add_helper(used, "CREATE_COMMENT"),
        Some(5) => vue3_text_suite_add_helper(used, "TO_DISPLAY_STRING"),
        Some(12) => {
            if let Some(codegen) = node.get("codegenNode") {
                vue3_slot_suite_collect_helpers(codegen, used);
            }
            if let Some(content) = node.get("content") {
                vue3_slot_suite_collect_helpers(content, used);
            }
        }
        Some(13) => {
            if node.get("tag").and_then(Value::as_str) == Some("FRAGMENT") {
                vue3_text_suite_add_helper(used, "FRAGMENT");
            }
            if node.get("directives").is_some_and(|value| !value.is_null()) {
                vue3_text_suite_add_helper(used, "WITH_DIRECTIVES");
            }
            if node
                .get("isBlock")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "OPEN_BLOCK");
                if node
                    .get("isComponent")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    vue3_text_suite_add_helper(used, "CREATE_BLOCK");
                } else {
                    vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
                }
            } else if node
                .get("isComponent")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "CREATE_VNODE");
            } else {
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_VNODE");
            }
        }
        Some(14) => match node.get("callee").and_then(Value::as_str) {
            Some("CREATE_TEXT") => vue3_text_suite_add_helper(used, "CREATE_TEXT"),
            Some("CREATE_COMMENT") => vue3_text_suite_add_helper(used, "CREATE_COMMENT"),
            Some("RENDER_LIST") => vue3_text_suite_add_helper(used, "RENDER_LIST"),
            Some("CREATE_SLOTS") => vue3_text_suite_add_helper(used, "CREATE_SLOTS"),
            Some("RENDER_SLOT") => vue3_text_suite_add_helper(used, "RENDER_SLOT"),
            Some("MERGE_PROPS") => vue3_text_suite_add_helper(used, "MERGE_PROPS"),
            Some("NORMALIZE_PROPS") => vue3_text_suite_add_helper(used, "NORMALIZE_PROPS"),
            Some("NORMALIZE_CLASS") => vue3_text_suite_add_helper(used, "NORMALIZE_CLASS"),
            Some("NORMALIZE_STYLE") => vue3_text_suite_add_helper(used, "NORMALIZE_STYLE"),
            Some("GUARD_REACTIVE_PROPS") => {
                vue3_text_suite_add_helper(used, "GUARD_REACTIVE_PROPS")
            }
            Some("TO_HANDLERS") => vue3_text_suite_add_helper(used, "TO_HANDLERS"),
            Some("TO_HANDLER_KEY") => vue3_text_suite_add_helper(used, "TO_HANDLER_KEY"),
            _ => {}
        },
        Some(20) => {
            if node
                .get("needPauseTracking")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "SET_BLOCK_TRACKING");
            }
        }
        Some(18) => {
            if node.get("isSlot").and_then(Value::as_bool).unwrap_or(false) {
                vue3_text_suite_add_helper(used, "WITH_CTX");
            }
        }
        _ => {}
    }
    for key in [
        "children",
        "props",
        "content",
        "codegenNode",
        "arguments",
        "returns",
        "params",
        "directives",
        "source",
        "valueAlias",
        "keyAlias",
        "objectIndexAlias",
        "parseResult",
        "branches",
        "condition",
        "test",
        "consequent",
        "alternate",
        "value",
        "elements",
        "properties",
        "key",
        "hoists",
        "cached",
    ] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_slot_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_slot_suite_collect_helpers(value, used);
        }
    }
}

pub(crate) fn vue3_core_transform_model_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut state = Vue3ModelSuiteState::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_model_suite_transform_children(
            std::mem::take(children),
            &options,
            &mut state,
            &Vue3ModelSuiteScope::default(),
        );
        *children = transformed;
    }
    vue3_model_suite_finalize_root(&mut root, &state);
    root["__vuecErrors"] = json!(state.errors);
    root
}

pub(crate) fn vue3_model_suite_transform_children(
    children: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Vec<Value> {
    children
        .into_iter()
        .map(|child| vue3_model_suite_transform_node(child, options, state, scope))
        .collect()
}

pub(crate) fn vue3_model_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) && vue3_text_suite_directive(&node, "for").is_some()
    {
        return vue3_model_suite_transform_for_node(node, options, state, scope);
    }

    if vue3_public_node_type(&node) == Some(1) {
        vue3_model_suite_process_directive_expressions(&mut node, options, scope);
    }

    let once_projection = vue3_once_suite_once_projection(&node, scope.in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let mut child_scope = scope.clone();
    child_scope.in_v_once = child_scope.in_v_once || enters_once;
    vue3_model_suite_track_slot_scope(&node, &mut child_scope);

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_model_suite_transform_children(
            std::mem::take(children),
            options,
            state,
            &child_scope,
        );
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_model_suite_element_codegen(&node, options, state, scope, false);
    }
    if enters_once {
        let codegen = node.get("codegenNode").cloned().unwrap_or(Value::Null);
        node["codegenNode"] = vue3_model_suite_cache_expression(state, codegen, true, true, false);
    }
    node
}

pub(crate) fn vue3_model_suite_transform_for_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let Some(dir) = vue3_text_suite_directive(&node, "for").cloned() else {
        return vue3_model_suite_transform_node(node, options, state, scope);
    };
    let context = vue3_model_suite_transform_context(options, scope);
    let projection = vuec_vue3_core::transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }));
    for error in projection
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(vue3_model_suite_error_value(error, &dir));
    }
    let parse_result = projection
        .get("parseResult")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_for_parse_result)
        .unwrap_or_else(|| {
            dir.get("forParseResult")
                .map(vue3_text_suite_materialize_for_parse_result)
                .unwrap_or(Value::Null)
        });

    let once_projection = vue3_once_suite_once_projection(&node, scope.in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let mut child_scope = scope.clone();
    child_scope.in_v_once = child_scope.in_v_once || enters_once;
    vue3_model_suite_add_locals(&mut child_scope, projection.get("locals"));
    child_scope.v_for_depth += 1;

    let fallback_loc = node.get("loc").cloned();
    let children = if projection.get("children").and_then(Value::as_str) == Some("template") {
        node.get_mut("children")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default()
            .into_iter()
            .map(|child| vue3_model_suite_transform_node(child, options, state, &child_scope))
            .collect::<Vec<_>>()
    } else {
        vue3_text_suite_remove_directive(&mut node, "for");
        vec![vue3_model_suite_transform_node(
            node,
            options,
            state,
            &child_scope,
        )]
    };

    let loc = dir
        .get("loc")
        .cloned()
        .or(fallback_loc)
        .unwrap_or_else(vue3_loc_stub_value);
    let mut for_node = json!({
        "type": 11,
        "source": parse_result.get("source").cloned().unwrap_or(Value::Null),
        "valueAlias": parse_result.get("value").cloned().unwrap_or(Value::Null),
        "keyAlias": parse_result.get("key").cloned().unwrap_or(Value::Null),
        "objectIndexAlias": parse_result.get("index").cloned().unwrap_or(Value::Null),
        "parseResult": parse_result,
        "children": children,
        "codegenNode": Value::Null,
        "loc": loc,
    });
    for_node["codegenNode"] = vue3_text_suite_for_codegen(&for_node);
    if enters_once {
        let codegen = for_node.get("codegenNode").cloned().unwrap_or(Value::Null);
        for_node["codegenNode"] =
            vue3_model_suite_cache_expression(state, codegen, true, true, false);
    }
    for_node
}

pub(crate) fn vue3_model_suite_process_directive_expressions(
    node: &mut Value,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) {
    if !options.prefix_identifiers {
        return;
    }
    let context = vue3_model_suite_transform_context(options, scope);
    let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
        return;
    };
    for prop in props {
        if vue3_public_node_type(prop) != Some(7) {
            continue;
        }
        if let Some(current) = prop.get("exp").filter(|value| !value.is_null()).cloned() {
            if vue3_public_node_type(&current) == Some(4) {
                let projection = vuec_vue3_core::process_expression_projection(&json!({
                    "node": current,
                    "context": context,
                }));
                prop["exp"] = vue3_text_suite_materialize_process_projection(&projection, &current);
            }
        }
        if let Some(current) = prop.get("arg").filter(|value| !value.is_null()).cloned() {
            if vue3_public_node_type(&current) == Some(4)
                && !current
                    .get("isStatic")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                let projection = vuec_vue3_core::process_expression_projection(&json!({
                    "node": current,
                    "context": context,
                }));
                prop["arg"] = vue3_text_suite_materialize_process_projection(&projection, &current);
            }
        }
    }
}

pub(crate) fn vue3_model_suite_track_slot_scope(node: &Value, scope: &mut Vue3ModelSuiteScope) {
    if vue3_public_node_type(node) != Some(1) {
        return;
    }
    let projection = vuec_vue3_core::track_slot_scopes_projection(&json!({ "node": node }));
    if projection
        .get("track")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        vue3_model_suite_add_locals(scope, projection.get("locals"));
    }
}

pub(crate) fn vue3_model_suite_add_locals(scope: &mut Vue3ModelSuiteScope, locals: Option<&Value>) {
    for local in locals
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        *scope.identifiers.entry(local.to_string()).or_insert(0) += 1;
    }
}

pub(crate) fn vue3_model_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    let (props, patch_flag, dynamic_props) =
        vue3_model_suite_props_codegen(node, options, state, scope);
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(1)) => {
            let tag = node.get("tag").and_then(Value::as_str).unwrap_or("");
            vue3_once_suite_vnode_call(
                &vue3_once_suite_component_asset_id(tag),
                props,
                Value::Null,
                patch_flag,
                dynamic_props,
                is_block,
                false,
                true,
            )
        }
        (Some(1), Some(0)) => {
            let children = node
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let children = if children.is_empty() {
                Value::Null
            } else if children.len() == 1 && vue3_text_suite_direct_child_value(&children[0]) {
                children[0].clone()
            } else {
                Value::Array(children)
            };
            vue3_once_suite_vnode_call(
                &vue3_once_suite_quote_string(
                    node.get("tag").and_then(Value::as_str).unwrap_or(""),
                ),
                props,
                children,
                patch_flag,
                dynamic_props,
                is_block,
                false,
                false,
            )
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_model_suite_props_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> (Value, Option<Value>, Value) {
    let mut properties = Vec::new();
    let mut dynamic_props = Vec::<String>::new();
    let mut needs_hydration = false;
    let context = vue3_model_suite_transform_context(options, scope);

    for prop in node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if vue3_public_node_type(prop) != Some(7)
            || prop.get("name").and_then(Value::as_str) != Some("model")
        {
            continue;
        }
        let projection = vuec_vue3_core::transform_model_projection(&json!({
            "dir": prop,
            "node": node,
            "context": context,
        }));
        for error in projection
            .get("errors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            state.errors.push(vue3_model_suite_error_value(error, prop));
        }
        for projected_prop in projection
            .get("props")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let key = vue3_model_suite_materialize_projection(projected_prop.get("key"), prop);
            let mut value =
                vue3_model_suite_materialize_projection(projected_prop.get("value"), prop);
            if projected_prop
                .get("cache")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                value = vue3_model_suite_cache_expression(state, value, false, false, false);
            }
            if projected_prop
                .get("dynamic")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                    dynamic_props.push(name);
                }
            }
            needs_hydration = needs_hydration
                || projected_prop
                    .get("hydrate")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
            properties.push(vue3_once_suite_object_property(key, value));
        }
    }

    let object = if properties.is_empty() {
        Value::Null
    } else {
        json!({
            "type": 15,
            "properties": properties,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        })
    };
    if object.is_null() {
        return (Value::Null, None, Value::Null);
    }
    if vue3_bind_suite_has_dynamic_key(&object) {
        return (
            vue3_text_suite_call("NORMALIZE_PROPS", vec![object]),
            Some(json!(16)),
            Value::Null,
        );
    }

    let patch_flag = (!dynamic_props.is_empty() || needs_hydration).then(|| {
        let mut flag = 0;
        if !dynamic_props.is_empty() {
            flag |= 8;
        }
        if needs_hydration {
            flag |= 32;
        }
        json!(flag)
    });
    let dynamic_props = if dynamic_props.is_empty() {
        Value::Null
    } else {
        Value::String(vue3_model_suite_dynamic_props_string(&dynamic_props))
    };
    (object, patch_flag, dynamic_props)
}

pub(crate) fn vue3_model_suite_materialize_projection(
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
            _ => Value::Null,
        },
        Some("static") | Some("simple") => json!({
            "type": 4,
            "content": projection.get("content").and_then(Value::as_str).unwrap_or(""),
            "isStatic": projection
                .get("isStatic")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| projection.get("kind").and_then(Value::as_str) == Some("static")),
            "constType": projection.get("constType").and_then(Value::as_u64).unwrap_or_else(|| {
                if projection.get("kind").and_then(Value::as_str) == Some("static") { 3 } else { 0 }
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
                        .map(|child| vue3_model_suite_materialize_projection(Some(child), dir))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            json!({
                "type": 8,
                "children": children,
                "loc": projection
                    .get("loc")
                    .cloned()
                    .or_else(|| dir.get("exp").and_then(|exp| exp.get("loc")).cloned())
                    .or_else(|| dir.get("loc").cloned())
                    .unwrap_or_else(vue3_loc_stub_value),
            })
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_model_suite_cache_expression(
    state: &mut Vue3ModelSuiteState,
    value: Value,
    need_pause_tracking: bool,
    in_v_once: bool,
    need_array_spread: bool,
) -> Value {
    let index = state.cached;
    state.cached += 1;
    json!({
        "type": 20,
        "index": index,
        "value": value,
        "needPauseTracking": need_pause_tracking,
        "inVOnce": in_v_once,
        "needArraySpread": need_array_spread,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_model_suite_static_prop_name(key: &Value) -> Option<String> {
    (vue3_public_node_type(key) == Some(4)
        && key
            .get("isStatic")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    .then(|| key.get("content").and_then(Value::as_str).unwrap_or(""))
    .filter(|name| !name.is_empty())
    .map(ToOwned::to_owned)
}

pub(crate) fn vue3_model_suite_dynamic_props_string(props: &[String]) -> String {
    let values = props
        .iter()
        .map(|prop| serde_json::to_string(prop).unwrap_or_else(|_| "\"\"".to_string()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

pub(crate) fn vue3_model_suite_finalize_root(root: &mut Value, state: &Vue3ModelSuiteState) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!(vue3_once_suite_components(root));
    root["helpers"] = json!(vue3_model_suite_helpers(root));
    root["directives"] = json!([]);
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_model_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_model_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
    if root
        .get("components")
        .and_then(Value::as_array)
        .is_some_and(|components| !components.is_empty())
    {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_COMPONENT");
    }
    [
        "NORMALIZE_PROPS",
        "SET_BLOCK_TRACKING",
        "RESOLVE_COMPONENT",
        "CREATE_BLOCK",
        "CREATE_VNODE",
        "CREATE_ELEMENT_VNODE",
        "RENDER_LIST",
        "FRAGMENT",
        "OPEN_BLOCK",
        "CREATE_ELEMENT_BLOCK",
        "IS_REF",
        "TO_HANDLER_KEY",
    ]
    .into_iter()
    .filter(|helper| used.iter().any(|used| used == helper))
    .map(str::to_string)
    .collect()
}

pub(crate) fn vue3_model_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    if node.as_str() == Some("_toHandlerKey(") {
        vue3_text_suite_add_helper(used, "TO_HANDLER_KEY");
        return;
    }
    match vue3_public_node_type(node) {
        Some(13) => {
            if node.get("tag").and_then(Value::as_str) == Some("FRAGMENT") {
                vue3_text_suite_add_helper(used, "FRAGMENT");
            }
            if node
                .get("isBlock")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "OPEN_BLOCK");
                if node
                    .get("isComponent")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    vue3_text_suite_add_helper(used, "CREATE_BLOCK");
                } else {
                    vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
                }
            } else if node
                .get("isComponent")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "CREATE_VNODE");
            } else {
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_VNODE");
            }
        }
        Some(14) => match node.get("callee").and_then(Value::as_str) {
            Some("NORMALIZE_PROPS") => vue3_text_suite_add_helper(used, "NORMALIZE_PROPS"),
            Some("RENDER_LIST") => vue3_text_suite_add_helper(used, "RENDER_LIST"),
            _ => {}
        },
        Some(20) => {
            if node
                .get("needPauseTracking")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "SET_BLOCK_TRACKING");
            }
        }
        _ => {}
    }
    for key in [
        "children",
        "props",
        "content",
        "codegenNode",
        "arguments",
        "returns",
        "params",
        "directives",
        "source",
        "valueAlias",
        "keyAlias",
        "objectIndexAlias",
        "parseResult",
        "branches",
        "condition",
        "test",
        "consequent",
        "alternate",
        "value",
        "elements",
        "properties",
        "key",
    ] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_model_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_model_suite_collect_helpers(value, used);
        }
    }
}

pub(crate) fn vue3_model_suite_error_value(error: &Value, dir: &Value) -> Value {
    let code = error
        .as_u64()
        .or_else(|| error.get("code").and_then(Value::as_u64))
        .unwrap_or(0);
    let loc = match error.get("loc").and_then(Value::as_str) {
        Some("arg") => dir
            .get("arg")
            .and_then(|arg| arg.get("loc"))
            .cloned()
            .or_else(|| dir.get("loc").cloned())
            .unwrap_or_else(vue3_loc_stub_value),
        _ => dir.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    };
    json!({ "code": code, "loc": loc })
}

pub(crate) fn vue3_model_suite_transform_context(
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let mut context = vue3_text_suite_transform_context(options);
    context["cacheHandlers"] = json!(options.cache_handlers);
    context["inVOnce"] = json!(scope.in_v_once);
    context["vForDepth"] = json!(scope.v_for_depth);
    context["vSlotDepth"] = json!(scope.v_slot_depth);
    context["identifiers"] = Value::Object(
        scope
            .identifiers
            .iter()
            .map(|(name, count)| (name.clone(), json!(count)))
            .collect(),
    );
    context
}

pub(crate) fn vue3_core_transform_on_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut state = Vue3ModelSuiteState::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_on_suite_transform_children(
            std::mem::take(children),
            &options,
            &mut state,
            &Vue3ModelSuiteScope::default(),
        );
        *children = transformed;
    }
    vue3_model_suite_finalize_root(&mut root, &state);
    root["__vuecErrors"] = json!(state.errors);

    let node = root
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "root": root,
        "node": node,
    })
}

pub(crate) fn vue3_on_suite_transform_children(
    children: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Vec<Value> {
    children
        .into_iter()
        .map(|child| vue3_on_suite_transform_node(child, options, state, scope))
        .collect()
}

pub(crate) fn vue3_on_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) && vue3_text_suite_directive(&node, "for").is_some()
    {
        return vue3_on_suite_transform_for_node(node, options, state, scope);
    }

    if vue3_public_node_type(&node) == Some(1) {
        vue3_on_suite_process_dynamic_args(&mut node, options, scope);
    }

    let once_projection = vue3_once_suite_once_projection(&node, scope.in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let mut child_scope = scope.clone();
    child_scope.in_v_once = child_scope.in_v_once || enters_once;
    vue3_model_suite_track_slot_scope(&node, &mut child_scope);

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_on_suite_transform_children(
            std::mem::take(children),
            options,
            state,
            &child_scope,
        );
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_on_suite_element_codegen(&node, options, state, scope, false);
    }
    if enters_once {
        let codegen = node.get("codegenNode").cloned().unwrap_or(Value::Null);
        node["codegenNode"] = vue3_model_suite_cache_expression(state, codegen, true, true, false);
    }
    node
}

pub(crate) fn vue3_on_suite_transform_for_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let Some(dir) = vue3_text_suite_directive(&node, "for").cloned() else {
        return vue3_on_suite_transform_node(node, options, state, scope);
    };
    let context = vue3_model_suite_transform_context(options, scope);
    let projection = vuec_vue3_core::transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }));
    for error in projection
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(vue3_model_suite_error_value(error, &dir));
    }
    let parse_result = projection
        .get("parseResult")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_for_parse_result)
        .unwrap_or_else(|| {
            dir.get("forParseResult")
                .map(vue3_text_suite_materialize_for_parse_result)
                .unwrap_or(Value::Null)
        });

    let once_projection = vue3_once_suite_once_projection(&node, scope.in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let mut child_scope = scope.clone();
    child_scope.in_v_once = child_scope.in_v_once || enters_once;
    vue3_model_suite_add_locals(&mut child_scope, projection.get("locals"));
    child_scope.v_for_depth += 1;

    let fallback_loc = node.get("loc").cloned();
    let children = if projection.get("children").and_then(Value::as_str) == Some("template") {
        node.get_mut("children")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default()
            .into_iter()
            .map(|child| vue3_on_suite_transform_node(child, options, state, &child_scope))
            .collect::<Vec<_>>()
    } else {
        vue3_text_suite_remove_directive(&mut node, "for");
        vec![vue3_on_suite_transform_node(
            node,
            options,
            state,
            &child_scope,
        )]
    };

    let loc = dir
        .get("loc")
        .cloned()
        .or(fallback_loc)
        .unwrap_or_else(vue3_loc_stub_value);
    let mut for_node = json!({
        "type": 11,
        "source": parse_result.get("source").cloned().unwrap_or(Value::Null),
        "valueAlias": parse_result.get("value").cloned().unwrap_or(Value::Null),
        "keyAlias": parse_result.get("key").cloned().unwrap_or(Value::Null),
        "objectIndexAlias": parse_result.get("index").cloned().unwrap_or(Value::Null),
        "parseResult": parse_result,
        "children": children,
        "codegenNode": Value::Null,
        "loc": loc,
    });
    for_node["codegenNode"] = vue3_text_suite_for_codegen(&for_node);
    if enters_once {
        let codegen = for_node.get("codegenNode").cloned().unwrap_or(Value::Null);
        for_node["codegenNode"] =
            vue3_model_suite_cache_expression(state, codegen, true, true, false);
    }
    for_node
}

pub(crate) fn vue3_on_suite_process_dynamic_args(
    node: &mut Value,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) {
    if !options.prefix_identifiers {
        return;
    }
    let context = vue3_model_suite_transform_context(options, scope);
    let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
        return;
    };
    for prop in props {
        if vue3_public_node_type(prop) != Some(7)
            || prop.get("name").and_then(Value::as_str) != Some("on")
        {
            continue;
        }
        let Some(current) = prop.get("arg").filter(|value| !value.is_null()).cloned() else {
            continue;
        };
        if vue3_public_node_type(&current) == Some(4)
            && !current
                .get("isStatic")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            let projection = vuec_vue3_core::process_expression_projection(&json!({
                "node": current,
                "context": context,
            }));
            prop["arg"] = vue3_text_suite_materialize_process_projection(&projection, &current);
        }
    }
}

pub(crate) fn vue3_on_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    let (props, patch_flag, dynamic_props) =
        vue3_on_suite_props_codegen(node, options, state, scope);
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(1)) => {
            let tag = node.get("tag").and_then(Value::as_str).unwrap_or("");
            vue3_once_suite_vnode_call(
                &vue3_once_suite_component_asset_id(tag),
                props,
                Value::Null,
                patch_flag,
                dynamic_props,
                is_block,
                false,
                true,
            )
        }
        (Some(1), Some(0)) => {
            let children = node
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let children = if children.is_empty() {
                Value::Null
            } else if children.len() == 1 && vue3_text_suite_direct_child_value(&children[0]) {
                children[0].clone()
            } else {
                Value::Array(children)
            };
            vue3_once_suite_vnode_call(
                &vue3_once_suite_quote_string(
                    node.get("tag").and_then(Value::as_str).unwrap_or(""),
                ),
                props,
                children,
                patch_flag,
                dynamic_props,
                is_block,
                false,
                false,
            )
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_on_suite_props_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> (Value, Option<Value>, Value) {
    let mut properties = Vec::new();
    let mut dynamic_props = Vec::<String>::new();
    let mut has_dynamic_key = false;
    let context = vue3_model_suite_transform_context(options, scope);

    for prop in node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if vue3_public_node_type(prop) != Some(7)
            || prop.get("name").and_then(Value::as_str) != Some("on")
        {
            continue;
        }
        let projection = vuec_vue3_core::transform_on_projection(&json!({
            "dir": prop,
            "node": node,
            "context": context,
        }));
        for error in projection
            .get("errors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            state.errors.push(vue3_model_suite_error_value(error, prop));
        }
        for projected_prop in projection
            .get("props")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let key = vue3_on_suite_materialize_projection(projected_prop.get("key"), prop);
            let mut value = vue3_on_suite_materialize_projection(projected_prop.get("value"), prop);
            let cached = projected_prop
                .get("cache")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if cached {
                value = vue3_model_suite_cache_expression(state, value, false, false, false);
            }
            let dynamic_key = projected_prop
                .get("dynamicKey")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            has_dynamic_key = has_dynamic_key || dynamic_key;
            if !cached && !dynamic_key {
                let value_constant = projected_prop
                    .get("valueConstant")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if !value_constant {
                    if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                        dynamic_props.push(name);
                    }
                }
            }
            properties.push(vue3_once_suite_object_property(key, value));
        }
    }

    let object = if properties.is_empty() {
        Value::Null
    } else {
        json!({
            "type": 15,
            "properties": properties,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        })
    };
    if object.is_null() {
        return (Value::Null, None, Value::Null);
    }

    let patch_flag = if has_dynamic_key {
        Some(json!(16))
    } else if !dynamic_props.is_empty() {
        Some(json!(8))
    } else {
        None
    };
    let dynamic_props = if dynamic_props.is_empty() {
        Value::Null
    } else {
        Value::String(vue3_model_suite_dynamic_props_string(&dynamic_props))
    };
    (object, patch_flag, dynamic_props)
}

pub(crate) fn vue3_on_suite_materialize_projection(
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
                                vue3_on_suite_materialize_projection(Some(child), dir);
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
                if projection.get("kind").and_then(Value::as_str) == Some("static") { 3 } else { 0 }
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
                                vue3_on_suite_materialize_projection(Some(child), dir);
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
                    .or_else(|| dir.get("exp").and_then(|exp| exp.get("loc")).cloned())
                    .or_else(|| dir.get("arg").and_then(|arg| arg.get("loc")).cloned())
                    .or_else(|| dir.get("loc").cloned())
                    .unwrap_or_else(vue3_loc_stub_value),
            })
        }
        _ => Value::Null,
    }
}

#[derive(Default)]
pub(crate) struct Vue3SlotOutletSuiteState {
    pub(crate) errors: Vec<Value>,
}

pub(crate) fn vue3_core_transform_slot_outlet_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut state = Vue3SlotOutletSuiteState::default();
    root = vue3_slot_outlet_suite_transform_node(root, &options, &mut state);
    root["helpers"] = json!(vue3_slot_outlet_suite_helpers(&root));
    root["components"] = json!([]);
    root["directives"] = json!([]);
    root["hoists"] = json!([]);
    root["cached"] = json!([]);
    root["temps"] = json!(0);
    root["__vuecErrors"] = json!(state.errors);
    root
}

pub(crate) fn vue3_slot_outlet_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotOutletSuiteState,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) {
        vue3_slot_outlet_suite_process_directive_expressions(&mut node, options);
    }

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = std::mem::take(children)
            .into_iter()
            .map(|child| vue3_slot_outlet_suite_transform_node(child, options, state))
            .collect::<Vec<_>>();
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(1)
        && node.get("tagType").and_then(Value::as_u64) == Some(2)
    {
        let context = vue3_slot_outlet_suite_transform_context(options);
        let projection = vuec_vue3_core::transform_slot_outlet_projection(&json!({
            "node": node,
            "context": context,
        }));
        vue3_slot_outlet_suite_apply_mutations(&mut node, projection.get("process"));
        let non_name_props = projection
            .get("process")
            .and_then(|process| process.get("nonNameProps"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let slot_props =
            vue3_slot_outlet_suite_props_codegen(&node, &non_name_props, options, state);
        let slot_name = vue3_slot_outlet_suite_slot_name(&node, projection.get("process"));
        let codegen = projection.get("codegen").unwrap_or(&Value::Null);
        node["codegenNode"] = vue3_slot_outlet_suite_codegen(&node, slot_name, slot_props, codegen);
    }
    node
}

pub(crate) fn vue3_slot_outlet_suite_process_directive_expressions(
    node: &mut Value,
    options: &Vue3CompilerOptions,
) {
    if !options.prefix_identifiers {
        return;
    }
    let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
        return;
    };
    let context = vue3_slot_outlet_suite_transform_context(options);
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

pub(crate) fn vue3_slot_outlet_suite_apply_mutations(node: &mut Value, process: Option<&Value>) {
    let mutations = process
        .and_then(|process| process.get("mutations"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
        return;
    };
    for mutation in mutations {
        let Some(index) = mutation.get("index").and_then(Value::as_u64) else {
            continue;
        };
        let Some(prop) = props.get_mut(index as usize) else {
            continue;
        };
        match mutation.get("kind").and_then(Value::as_str) {
            Some("setPropName") => {
                if let Some(name) = mutation.get("name").and_then(Value::as_str) {
                    prop["name"] = json!(name);
                }
            }
            Some("setDirectiveArgContent") => {
                if let Some(content) = mutation.get("content").and_then(Value::as_str) {
                    prop["arg"]["content"] = json!(content);
                }
            }
            Some("setDirectiveExp") => {
                let value = mutation.get("value").cloned().unwrap_or(Value::Null);
                prop["exp"] = vue3_text_suite_materialize_process_projection(&value, &value);
            }
            _ => {}
        }
    }
}

pub(crate) fn vue3_slot_outlet_suite_slot_name(node: &Value, process: Option<&Value>) -> Value {
    let slot_name = process
        .and_then(|process| process.get("slotName"))
        .unwrap_or(&Value::Null);
    match slot_name.get("kind").and_then(Value::as_str) {
        Some("literal") => slot_name.get("value").cloned().unwrap_or(Value::Null),
        Some("node") => {
            let index = slot_name.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let field = slot_name
                .get("field")
                .and_then(Value::as_str)
                .unwrap_or("exp");
            node.get("props")
                .and_then(Value::as_array)
                .and_then(|props| props.get(index))
                .and_then(|prop| prop.get(field))
                .cloned()
                .unwrap_or(Value::Null)
        }
        _ => json!("\"default\""),
    }
}

pub(crate) fn vue3_slot_outlet_suite_props_codegen(
    node: &Value,
    indices: &[Value],
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotOutletSuiteState,
) -> Value {
    let mut properties = Vec::new();
    let props = node
        .get("props")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for index in indices
        .iter()
        .filter_map(Value::as_u64)
        .map(|index| index as usize)
    {
        let Some(prop) = props.get(index) else {
            continue;
        };
        match vue3_public_node_type(prop) {
            Some(6) => {
                let Some(name) = prop.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let value = prop
                    .get("value")
                    .and_then(|value| value.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                properties.push(vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression(name, true),
                    vue3_once_suite_simple_expression(value, true),
                ));
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("bind") => {
                let key = prop.get("arg").cloned().unwrap_or(Value::Null);
                let value = prop
                    .get("exp")
                    .filter(|value| !value.is_null())
                    .cloned()
                    .unwrap_or_else(|| {
                        let content = key
                            .get("content")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        vue3_once_suite_simple_expression(content, false)
                    });
                properties.push(vue3_once_suite_object_property(key, value));
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("on") => {
                let projection = vuec_vue3_core::transform_on_projection(&json!({
                    "dir": prop,
                    "node": node,
                    "context": vue3_slot_outlet_suite_transform_context(options),
                }));
                for error in projection
                    .get("errors")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    state.errors.push(vue3_model_suite_error_value(error, prop));
                }
                for projected_prop in projection
                    .get("props")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let key = vue3_on_suite_materialize_projection(projected_prop.get("key"), prop);
                    let value =
                        vue3_on_suite_materialize_projection(projected_prop.get("value"), prop);
                    properties.push(vue3_once_suite_object_property(key, value));
                }
            }
            Some(7) => {
                state.errors.push(json!({
                    "code": 36,
                    "loc": prop.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
                }));
            }
            _ => {}
        }
    }
    if properties.is_empty() {
        Value::Null
    } else {
        json!({
            "type": 15,
            "properties": properties,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        })
    }
}

pub(crate) fn vue3_slot_outlet_suite_codegen(
    node: &Value,
    slot_name: Value,
    slot_props: Value,
    codegen: &Value,
) -> Value {
    let slots = codegen
        .get("slots")
        .and_then(Value::as_str)
        .unwrap_or("$slots");
    let mut args = vec![
        Value::String(slots.to_string()),
        slot_name,
        Value::String("{}".to_string()),
        Value::String("undefined".to_string()),
        Value::String("true".to_string()),
    ];
    let mut expected_len = codegen
        .get("expectedLen")
        .and_then(Value::as_u64)
        .unwrap_or(2) as usize;
    if !slot_props.is_null() {
        args[2] = slot_props;
        expected_len = expected_len.max(3);
    }
    if node
        .get("children")
        .and_then(Value::as_array)
        .is_some_and(|children| !children.is_empty())
    {
        args[3] = json!({
            "type": 18,
            "params": [],
            "returns": node.get("children").cloned().unwrap_or_else(|| json!([])),
            "newline": false,
            "isSlot": false,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        });
        expected_len = expected_len.max(4);
    }
    args.truncate(expected_len);
    vue3_text_suite_call("RENDER_SLOT", args)
}

pub(crate) fn vue3_slot_outlet_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_slot_outlet_suite_collect_helpers(root, &mut used);
    ["RENDER_SLOT"]
        .into_iter()
        .filter(|helper| used.iter().any(|used| used == helper))
        .map(str::to_string)
        .collect()
}

pub(crate) fn vue3_slot_outlet_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    if vue3_public_node_type(node) == Some(14)
        && node.get("callee").and_then(Value::as_str) == Some("RENDER_SLOT")
    {
        vue3_text_suite_add_helper(used, "RENDER_SLOT");
    }
    for key in [
        "children",
        "content",
        "codegenNode",
        "arguments",
        "returns",
        "params",
        "props",
        "value",
        "key",
        "properties",
    ] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_slot_outlet_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_slot_outlet_suite_collect_helpers(value, used);
        }
    }
}

pub(crate) fn vue3_slot_outlet_suite_transform_context(options: &Vue3CompilerOptions) -> Value {
    let mut context = vue3_text_suite_transform_context(options);
    context["scopeId"] = options
        .scope_id
        .clone()
        .map(Value::String)
        .unwrap_or(Value::Null);
    context["slotted"] = json!(options.slotted);
    context
}

#[derive(Default)]
pub(crate) struct Vue3OnceSuiteState {
    pub(crate) cached: usize,
}

pub(crate) fn vue3_core_transform_once_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut state = Vue3OnceSuiteState::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_once_suite_transform_children(
            std::mem::take(children),
            &options,
            &mut state,
            false,
        );
        *children = transformed;
    }
    vue3_once_suite_finalize_root(&mut root, &state);
    root
}

pub(crate) fn vue3_once_suite_transform_children(
    children: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3OnceSuiteState,
    in_v_once: bool,
) -> Vec<Value> {
    let mut transformed = Vec::new();
    let mut index = 0usize;
    while index < children.len() {
        let child = children[index].clone();
        if vue3_public_node_type(&child) == Some(1)
            && vue3_text_suite_directive(&child, "if").is_some()
        {
            let (if_node, consumed) =
                vue3_once_suite_transform_if_node(&children[index..], options, state, in_v_once);
            transformed.push(if_node);
            index += consumed.max(1);
            continue;
        }
        transformed.push(vue3_once_suite_transform_node(
            child, options, state, in_v_once,
        ));
        index += 1;
    }
    transformed
}

pub(crate) fn vue3_once_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3OnceSuiteState,
    in_v_once: bool,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) && vue3_text_suite_directive(&node, "for").is_some()
    {
        return vue3_once_suite_transform_for_node(node, options, state, in_v_once);
    }

    let once_projection = vue3_once_suite_once_projection(&node, in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let child_in_v_once = in_v_once || enters_once;

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_once_suite_transform_children(
            std::mem::take(children),
            options,
            state,
            child_in_v_once,
        );
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_once_suite_element_codegen(&node, false);
    }

    if enters_once {
        let codegen = node.get("codegenNode").cloned().unwrap_or(Value::Null);
        node["codegenNode"] = vue3_once_suite_cache_expression(state, codegen);
    }
    node
}

pub(crate) fn vue3_once_suite_transform_if_node(
    siblings: &[Value],
    options: &Vue3CompilerOptions,
    state: &mut Vue3OnceSuiteState,
    in_v_once: bool,
) -> (Value, usize) {
    let first = siblings.first().cloned().unwrap_or(Value::Null);
    let once_projection = vue3_once_suite_once_projection(&first, in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let branch_in_v_once = in_v_once || enters_once;
    let mut branches = Vec::new();
    let mut consumed = 0usize;

    for sibling in siblings {
        let dir_name = if consumed == 0 {
            "if"
        } else if vue3_text_suite_directive(sibling, "else-if").is_some() {
            "else-if"
        } else if vue3_text_suite_directive(sibling, "else").is_some() {
            "else"
        } else {
            break;
        };
        let dir = vue3_text_suite_directive(sibling, dir_name);
        if consumed > 0 && dir.is_none() {
            break;
        }
        let mut branch_child = sibling.clone();
        vue3_text_suite_remove_directive(&mut branch_child, "if");
        vue3_text_suite_remove_directive(&mut branch_child, "else-if");
        vue3_text_suite_remove_directive(&mut branch_child, "else");
        let transformed_child =
            vue3_once_suite_transform_node(branch_child, options, state, branch_in_v_once);
        let condition = dir
            .and_then(|dir| dir.get("exp"))
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or(Value::Null);
        branches.push(json!({
            "type": 10,
            "condition": condition,
            "children": [transformed_child],
            "userKey": Value::Null,
            "isTemplateIf": false,
            "loc": sibling.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }));
        consumed += 1;
        if dir_name == "else" {
            break;
        }
    }

    let loc = first
        .get("loc")
        .cloned()
        .unwrap_or_else(vue3_loc_stub_value);
    let mut if_node = json!({
        "type": 9,
        "branches": branches,
        "codegenNode": Value::Null,
        "loc": loc,
    });
    if_node["codegenNode"] = vue3_once_suite_if_codegen(&if_node);
    if enters_once {
        let codegen = if_node.get("codegenNode").cloned().unwrap_or(Value::Null);
        if_node["codegenNode"] = vue3_once_suite_cache_expression(state, codegen);
    }
    (if_node, consumed)
}

pub(crate) fn vue3_once_suite_if_codegen(if_node: &Value) -> Value {
    let branches = if_node
        .get("branches")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut alternate =
        vue3_text_suite_call("CREATE_COMMENT", vec![json!("\"v-if\""), json!("true")]);
    for branch in branches.iter().rev() {
        let child_codegen = branch
            .get("children")
            .and_then(Value::as_array)
            .and_then(|children| children.first())
            .and_then(|child| child.get("codegenNode"))
            .cloned()
            .unwrap_or(Value::Null);
        let condition = branch.get("condition").cloned().unwrap_or(Value::Null);
        if condition.is_null() {
            alternate = child_codegen;
        } else {
            alternate = json!({
                "type": 19,
                "test": condition,
                "consequent": child_codegen,
                "alternate": alternate,
                "newline": true,
                "loc": branch.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            });
        }
    }
    alternate
}

pub(crate) fn vue3_once_suite_transform_for_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3OnceSuiteState,
    in_v_once: bool,
) -> Value {
    let Some(dir) = vue3_text_suite_directive(&node, "for").cloned() else {
        return vue3_once_suite_transform_node(node, options, state, in_v_once);
    };
    let once_projection = vue3_once_suite_once_projection(&node, in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let child_in_v_once = in_v_once || enters_once;
    let context = vue3_text_suite_transform_context(options);
    let projection = vuec_vue3_core::transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }));
    let parse_result = projection
        .get("parseResult")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_for_parse_result)
        .unwrap_or_else(|| {
            dir.get("forParseResult")
                .map(vue3_text_suite_materialize_for_parse_result)
                .unwrap_or(Value::Null)
        });

    vue3_text_suite_remove_directive(&mut node, "for");
    let child = vue3_once_suite_transform_node(node, options, state, child_in_v_once);
    let loc = dir
        .get("loc")
        .cloned()
        .or_else(|| child.get("loc").cloned())
        .unwrap_or_else(vue3_loc_stub_value);
    let mut for_node = json!({
        "type": 11,
        "source": parse_result.get("source").cloned().unwrap_or(Value::Null),
        "valueAlias": parse_result.get("value").cloned().unwrap_or(Value::Null),
        "keyAlias": parse_result.get("key").cloned().unwrap_or(Value::Null),
        "objectIndexAlias": parse_result.get("index").cloned().unwrap_or(Value::Null),
        "parseResult": parse_result,
        "children": [child],
        "codegenNode": Value::Null,
        "loc": loc,
    });
    for_node["codegenNode"] = vue3_text_suite_for_codegen(&for_node);
    if enters_once {
        let codegen = for_node.get("codegenNode").cloned().unwrap_or(Value::Null);
        for_node["codegenNode"] = vue3_once_suite_cache_expression(state, codegen);
    }
    for_node
}

pub(crate) fn vue3_once_suite_once_projection(node: &Value, in_v_once: bool) -> Value {
    vuec_vue3_core::transform_once_projection(&json!({
        "node": node,
        "context": {
            "inVOnce": in_v_once,
            "inSSR": false,
        },
        "seen": false,
    }))
}

pub(crate) fn vue3_once_suite_cache_expression(
    state: &mut Vue3OnceSuiteState,
    value: Value,
) -> Value {
    let index = state.cached;
    state.cached += 1;
    json!({
        "type": 20,
        "index": index,
        "value": value,
        "needPauseTracking": true,
        "inVOnce": true,
        "needArraySpread": false,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_once_suite_element_codegen(node: &Value, is_block: bool) -> Value {
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(2)) => {
            vue3_text_suite_call("RENDER_SLOT", vec![json!("$slots"), json!("\"default\"")])
        }
        (Some(1), Some(1)) => {
            let tag = node.get("tag").and_then(Value::as_str).unwrap_or("");
            let (props, patch_flag, dynamic_props) = vue3_once_suite_props_codegen(node);
            vue3_once_suite_vnode_call(
                &vue3_once_suite_component_asset_id(tag),
                props,
                Value::Null,
                patch_flag,
                dynamic_props,
                is_block,
                false,
                true,
            )
        }
        (Some(1), Some(0)) => {
            let (props, patch_flag, dynamic_props) = vue3_once_suite_props_codegen(node);
            let children = node
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let children = if children.is_empty() {
                Value::Null
            } else if children.len() == 1 && vue3_text_suite_direct_child_value(&children[0]) {
                children[0].clone()
            } else {
                Value::Array(children)
            };
            vue3_once_suite_vnode_call(
                &vue3_once_suite_quote_string(
                    node.get("tag").and_then(Value::as_str).unwrap_or(""),
                ),
                props,
                children,
                patch_flag,
                dynamic_props,
                is_block,
                false,
                false,
            )
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_once_suite_props_codegen(node: &Value) -> (Value, Option<Value>, Value) {
    let mut properties = Vec::new();
    let mut dynamic_props = Vec::new();
    for prop in node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match vue3_public_node_type(prop) {
            Some(6) => {
                let Some(name) = prop.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let value = prop
                    .get("value")
                    .and_then(|value| value.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                properties.push(vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression(name, true),
                    vue3_once_suite_simple_expression(value, true),
                ));
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("bind") => {
                let Some(arg) = prop.get("arg").filter(|value| !value.is_null()) else {
                    continue;
                };
                let Some(name) = arg.get("content").and_then(Value::as_str) else {
                    continue;
                };
                let value = prop
                    .get("exp")
                    .filter(|value| !value.is_null())
                    .cloned()
                    .unwrap_or_else(|| vue3_once_suite_simple_expression(name, false));
                properties.push(vue3_once_suite_object_property(arg.clone(), value));
                dynamic_props.push(Value::String(vue3_once_suite_quote_string(name)));
            }
            _ => {}
        }
    }
    let props = if properties.is_empty() {
        Value::Null
    } else {
        json!({
            "type": 15,
            "properties": properties,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        })
    };
    let patch_flag = (!dynamic_props.is_empty()).then(|| json!(8));
    let dynamic_props = if dynamic_props.is_empty() {
        Value::Null
    } else {
        Value::Array(dynamic_props)
    };
    (props, patch_flag, dynamic_props)
}

pub(crate) fn vue3_once_suite_vnode_call(
    tag: &str,
    props: Value,
    children: Value,
    patch_flag: Option<Value>,
    dynamic_props: Value,
    is_block: bool,
    disable_tracking: bool,
    is_component: bool,
) -> Value {
    json!({
        "type": 13,
        "tag": tag,
        "props": props,
        "children": children,
        "patchFlag": patch_flag.unwrap_or(Value::Null),
        "dynamicProps": dynamic_props,
        "directives": Value::Null,
        "isBlock": is_block,
        "disableTracking": disable_tracking,
        "isComponent": is_component,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_once_suite_object_property(key: Value, value: Value) -> Value {
    json!({
        "type": 16,
        "key": key,
        "value": value,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_once_suite_simple_expression(content: &str, is_static: bool) -> Value {
    json!({
        "type": 4,
        "content": content,
        "isStatic": is_static,
        "constType": if is_static { 3 } else { 0 },
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_once_suite_quote_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

pub(crate) fn vue3_once_suite_component_asset_id(tag: &str) -> String {
    let mut id = String::from("_component_");
    for ch in tag.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            id.push(ch);
        } else {
            id.push('_');
        }
    }
    id
}

pub(crate) fn vue3_once_suite_finalize_root(root: &mut Value, state: &Vue3OnceSuiteState) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!(vue3_once_suite_components(root));
    root["helpers"] = json!(vue3_once_suite_helpers(root));
    root["directives"] = json!([]);
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_once_suite_set_root_codegen(root: &mut Value) {
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let visible = children
        .iter()
        .enumerate()
        .filter(|(_, child)| vue3_public_node_type(child) != Some(3))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    root["codegenNode"] = match visible.as_slice() {
        [] => Value::Null,
        [index] => {
            if let Some(root_child) = root
                .get_mut("children")
                .and_then(Value::as_array_mut)
                .and_then(|items| items.get_mut(*index))
            {
                if vue3_public_node_type(root_child) == Some(1) {
                    if root_child
                        .get("codegenNode")
                        .and_then(vue3_public_node_type)
                        == Some(13)
                    {
                        root_child["codegenNode"]["isBlock"] = json!(true);
                    }
                }
            }
            root.get("children")
                .and_then(Value::as_array)
                .and_then(|items| items.get(*index))
                .and_then(|child| {
                    if matches!(vue3_public_node_type(child), Some(1 | 9 | 11)) {
                        child.get("codegenNode")
                    } else {
                        Some(child)
                    }
                })
                .cloned()
                .unwrap_or(Value::Null)
        }
        _ => vue3_once_suite_vnode_call(
            "FRAGMENT",
            Value::Null,
            Value::Array(children),
            Some(json!(64)),
            Value::Null,
            true,
            false,
            false,
        ),
    };
}

pub(crate) fn vue3_once_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_once_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
    if root
        .get("components")
        .and_then(Value::as_array)
        .is_some_and(|components| !components.is_empty())
    {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_COMPONENT");
    }
    [
        "SET_BLOCK_TRACKING",
        "RESOLVE_COMPONENT",
        "CREATE_VNODE",
        "RENDER_SLOT",
        "CREATE_ELEMENT_VNODE",
        "RENDER_LIST",
        "FRAGMENT",
        "OPEN_BLOCK",
        "CREATE_ELEMENT_BLOCK",
        "CREATE_COMMENT",
    ]
    .into_iter()
    .filter(|helper| used.iter().any(|used| used == helper))
    .map(str::to_string)
    .collect()
}

pub(crate) fn vue3_once_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    match vue3_public_node_type(node) {
        Some(13) => {
            if node.get("tag").and_then(Value::as_str) == Some("FRAGMENT") {
                vue3_text_suite_add_helper(used, "FRAGMENT");
            }
            if node
                .get("isBlock")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "OPEN_BLOCK");
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
            } else if node
                .get("isComponent")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "CREATE_VNODE");
            } else {
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_VNODE");
            }
        }
        Some(14) => match node.get("callee").and_then(Value::as_str) {
            Some("RENDER_SLOT") => vue3_text_suite_add_helper(used, "RENDER_SLOT"),
            Some("RENDER_LIST") => vue3_text_suite_add_helper(used, "RENDER_LIST"),
            Some("CREATE_COMMENT") => vue3_text_suite_add_helper(used, "CREATE_COMMENT"),
            _ => {}
        },
        Some(20) => {
            vue3_text_suite_add_helper(used, "SET_BLOCK_TRACKING");
        }
        _ => {}
    }
    for key in [
        "children",
        "content",
        "codegenNode",
        "arguments",
        "returns",
        "params",
        "directives",
        "source",
        "valueAlias",
        "keyAlias",
        "objectIndexAlias",
        "parseResult",
        "branches",
        "condition",
        "test",
        "consequent",
        "alternate",
        "value",
        "elements",
        "properties",
        "key",
    ] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_once_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_once_suite_collect_helpers(value, used);
        }
    }
}

pub(crate) fn vue3_once_suite_components(root: &Value) -> Vec<String> {
    let mut components = Vec::new();
    vue3_once_suite_collect_components(root, &mut components);
    components
}

pub(crate) fn vue3_slot_suite_components(root: &Value) -> Vec<String> {
    let mut components = Vec::new();
    vue3_slot_suite_collect_components(root, &mut components);
    components
}

pub(crate) fn vue3_slot_suite_collect_components(node: &Value, components: &mut Vec<String>) {
    for key in ["children", "branches"] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_slot_suite_collect_components(item, components);
            }
        }
    }
    if vue3_public_node_type(node) == Some(1)
        && node.get("tagType").and_then(Value::as_u64) == Some(1)
    {
        if let Some(tag) = node.get("tag").and_then(Value::as_str) {
            if !components.iter().any(|component| component == tag) {
                components.push(tag.to_string());
            }
        }
    }
}

pub(crate) fn vue3_once_suite_collect_components(node: &Value, components: &mut Vec<String>) {
    if vue3_public_node_type(node) == Some(1)
        && node.get("tagType").and_then(Value::as_u64) == Some(1)
    {
        if let Some(tag) = node.get("tag").and_then(Value::as_str) {
            if !components.iter().any(|component| component == tag) {
                components.push(tag.to_string());
            }
        }
    }
    for key in ["children", "branches"] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_once_suite_collect_components(item, components);
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct Vue3ExpressionSuiteState {
    pub(crate) errors: Vec<Value>,
}

pub(crate) fn vue3_core_transform_expression_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_expression_suite_options(payload.get("options"));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut state = Vue3ExpressionSuiteState::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = std::mem::take(children)
            .into_iter()
            .map(|child| vue3_expression_suite_transform_node(child, &options, &mut state))
            .collect::<Vec<_>>();
        *children = transformed;
    }
    let mut node = root
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .cloned()
        .unwrap_or(Value::Null);
    if let Some(object) = node.as_object_mut() {
        object.insert("__vuecErrors".into(), json!(state.errors));
    }
    node
}

pub(crate) fn vue3_expression_suite_options(value: Option<&Value>) -> Vue3CompilerOptions {
    let mut options = vue3_options(value);
    let has_prefix_override = value.is_some_and(|value| {
        value.get("prefixIdentifiers").is_some() || value.get("prefix_identifiers").is_some()
    });
    if !has_prefix_override {
        options.prefix_identifiers = true;
    }
    options
}

pub(crate) fn vue3_expression_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ExpressionSuiteState,
) -> Value {
    if matches!(vue3_public_node_type(&node), Some(1 | 5)) {
        let projection = vuec_vue3_core::transform_expression_projection(&json!({
            "node": node.clone(),
            "context": vue3_text_suite_transform_context(options),
        }));
        vue3_expression_suite_apply_operations(&mut node, projection.get("operations"), state);
    }

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = std::mem::take(children)
            .into_iter()
            .map(|child| vue3_expression_suite_transform_node(child, options, state))
            .collect::<Vec<_>>();
        *children = transformed;
    }
    node
}

pub(crate) fn vue3_expression_suite_apply_operations(
    node: &mut Value,
    operations: Option<&Value>,
    state: &mut Vue3ExpressionSuiteState,
) {
    for operation in operations.and_then(Value::as_array).into_iter().flatten() {
        if operation.get("kind").and_then(Value::as_str) != Some("process") {
            continue;
        }
        let path = operation
            .get("path")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let projection = operation.get("projection").unwrap_or(&Value::Null);
        if projection.get("kind").and_then(Value::as_str) == Some("error") {
            state
                .errors
                .push(vue3_expression_suite_error_value(projection));
            continue;
        }
        match path.as_slice() {
            [Value::String(key)] if key == "content" => {
                let current = node.get(key).cloned().unwrap_or(Value::Null);
                node[key] = vue3_text_suite_materialize_process_projection(projection, &current);
            }
            [Value::String(props_key), Value::String(index), Value::String(expr_key)]
                if props_key == "props" =>
            {
                let Ok(index) = index.parse::<usize>() else {
                    continue;
                };
                let Some(prop) = node
                    .get_mut(props_key)
                    .and_then(Value::as_array_mut)
                    .and_then(|props| props.get_mut(index))
                else {
                    continue;
                };
                let current = prop.get(expr_key).cloned().unwrap_or(Value::Null);
                prop[expr_key] =
                    vue3_text_suite_materialize_process_projection(projection, &current);
            }
            _ => {}
        }
    }
}

pub(crate) fn vue3_expression_suite_error_value(error: &Value) -> Value {
    json!({
        "code": error.get("code").and_then(Value::as_u64).unwrap_or(0),
        "loc": error.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        "message": error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Vue compiler error"),
    })
}

pub(crate) fn vue3_core_transform_text_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    root = vue3_text_suite_transform_node(root, &options);
    vue3_text_suite_finalize_root(&mut root);
    root
}

pub(crate) fn vue3_text_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) && vue3_text_suite_directive(&node, "for").is_some()
    {
        return vue3_text_suite_transform_for_node(node, options);
    }

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = std::mem::take(children)
            .into_iter()
            .map(|child| vue3_text_suite_transform_node(child, options))
            .collect::<Vec<_>>();
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(5) {
        vue3_text_suite_process_interpolation(&mut node, options);
    }

    if matches!(vue3_public_node_type(&node), Some(0 | 1 | 10 | 11)) {
        vue3_text_suite_apply_transform_text(&mut node, options);
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_text_suite_element_codegen(&node, false);
    }

    node
}

pub(crate) fn vue3_text_suite_transform_for_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
) -> Value {
    let Some(dir) = vue3_text_suite_directive(&node, "for").cloned() else {
        return node;
    };
    let context = vue3_text_suite_transform_context(options);
    let projection = vuec_vue3_core::transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }));
    let Some(parse_result) = projection
        .get("parseResult")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_for_parse_result)
    else {
        return node;
    };

    let fallback_loc = node.get("loc").cloned();
    let children = if projection.get("children").and_then(Value::as_str) == Some("template") {
        node.get_mut("children")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default()
            .into_iter()
            .map(|child| vue3_text_suite_transform_node(child, options))
            .collect::<Vec<_>>()
    } else {
        vue3_text_suite_remove_directive(&mut node, "for");
        vec![vue3_text_suite_transform_node(node, options)]
    };

    let loc = dir
        .get("loc")
        .cloned()
        .or(fallback_loc)
        .unwrap_or_else(vue3_loc_stub_value);
    let mut for_node = json!({
        "type": 11,
        "source": parse_result.get("source").cloned().unwrap_or(Value::Null),
        "valueAlias": parse_result.get("value").cloned().unwrap_or(Value::Null),
        "keyAlias": parse_result.get("key").cloned().unwrap_or(Value::Null),
        "objectIndexAlias": parse_result.get("index").cloned().unwrap_or(Value::Null),
        "parseResult": parse_result,
        "children": children,
        "codegenNode": Value::Null,
        "loc": loc,
    });
    vue3_text_suite_apply_transform_text(&mut for_node, options);
    for_node["codegenNode"] = vue3_text_suite_for_codegen(&for_node);
    for_node
}

pub(crate) fn vue3_text_suite_materialize_for_parse_result(parse_result: &Value) -> Value {
    let source = parse_result
        .get("source")
        .map(vue3_text_suite_materialize_process_child)
        .unwrap_or(Value::Null);
    let value = parse_result
        .get("value")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_process_child)
        .unwrap_or(Value::Null);
    let key = parse_result
        .get("key")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_process_child)
        .unwrap_or(Value::Null);
    let index = parse_result
        .get("index")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_process_child)
        .unwrap_or(Value::Null);
    json!({
        "source": source,
        "value": value,
        "key": key,
        "index": index,
        "finalized": parse_result
            .get("finalized")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    })
}

pub(crate) fn vue3_text_suite_process_interpolation(
    node: &mut Value,
    options: &Vue3CompilerOptions,
) {
    if !options.prefix_identifiers {
        return;
    }
    let content = node.get("content").cloned().unwrap_or(Value::Null);
    let projection = vuec_vue3_core::process_expression_projection(&json!({
        "node": content,
        "context": vue3_text_suite_transform_context(options),
    }));
    node["content"] = vue3_text_suite_materialize_process_projection(&projection, &content);
}

pub(crate) fn vue3_text_suite_materialize_process_projection(
    projection: &Value,
    current: &Value,
) -> Value {
    match projection.get("kind").and_then(Value::as_str) {
        Some("simple") => json!({
            "type": 4,
            "content": projection.get("content").and_then(Value::as_str).unwrap_or(""),
            "isStatic": projection.get("isStatic").and_then(Value::as_bool).unwrap_or(false),
            "constType": projection.get("constType").and_then(Value::as_u64).unwrap_or(0),
            "loc": projection
                .get("loc")
                .cloned()
                .or_else(|| current.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("compound") => json!({
            "type": 8,
            "children": projection
                .get("children")
                .and_then(Value::as_array)
                .map(|children| children
                    .iter()
                    .map(vue3_text_suite_materialize_process_child)
                    .collect::<Vec<_>>())
                .unwrap_or_default(),
            "loc": projection
                .get("loc")
                .cloned()
                .or_else(|| current.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("setConstType") => {
            let mut next = current.clone();
            next["constType"] = projection.get("constType").cloned().unwrap_or(json!(0));
            next
        }
        _ => current.clone(),
    }
}

pub(crate) fn vue3_text_suite_materialize_process_child(child: &Value) -> Value {
    if child.is_string() || child.get("type").is_some() {
        return child.clone();
    }
    match child.get("kind").and_then(Value::as_str) {
        Some("simple") => json!({
            "type": 4,
            "content": child.get("content").and_then(Value::as_str).unwrap_or(""),
            "isStatic": child.get("isStatic").and_then(Value::as_bool).unwrap_or(false),
            "constType": child.get("constType").and_then(Value::as_u64).unwrap_or(0),
            "loc": child.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("compound") => json!({
            "type": 8,
            "children": child
                .get("children")
                .and_then(Value::as_array)
                .map(|children| children
                    .iter()
                    .map(vue3_text_suite_materialize_process_child)
                    .collect::<Vec<_>>())
                .unwrap_or_default(),
            "loc": child.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }),
        _ => child.clone(),
    }
}

pub(crate) fn vue3_text_suite_apply_transform_text(
    node: &mut Value,
    options: &Vue3CompilerOptions,
) {
    let context = vue3_text_suite_transform_context(options);
    vue3_text_suite_apply_transform_text_with_context(node, context);
}

pub(crate) fn vue3_text_suite_apply_transform_text_with_directives(
    node: &mut Value,
    options: &Vue3CompilerOptions,
    directive_transforms: &[&str],
) {
    let context = vue3_text_suite_transform_context_with_directives(options, directive_transforms);
    vue3_text_suite_apply_transform_text_with_context(node, context);
}

pub(crate) fn vue3_text_suite_apply_transform_text_with_context(node: &mut Value, context: Value) {
    let projection = vuec_vue3_core::transform_text_projection(&json!({
        "node": node,
        "context": context,
    }));
    let operations = projection
        .get("operations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) else {
        return;
    };
    for operation in operations {
        match operation.get("kind").and_then(Value::as_str) {
            Some("mergeText") => {
                let start = operation.get("start").and_then(Value::as_u64).unwrap_or(0) as usize;
                let end = operation
                    .get("end")
                    .and_then(Value::as_u64)
                    .unwrap_or(start as u64) as usize;
                if start >= children.len() || end < start || end >= children.len() {
                    continue;
                }
                let compound = vue3_text_suite_compound(&children[start..=end]);
                children.splice(start..=end, std::iter::once(compound));
            }
            Some("wrapTextCall") => {
                let index = operation.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                if index >= children.len() {
                    continue;
                }
                let child = children[index].clone();
                let include_content = operation
                    .get("includeContent")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let patch_flag = operation.get("patchFlag").cloned().filter(|value| {
                    !value.is_null() && value.as_str().is_none_or(|value| !value.trim().is_empty())
                });
                children[index] = vue3_text_suite_text_call(child, include_content, patch_flag);
            }
            _ => {}
        }
    }
}

pub(crate) fn vue3_text_suite_compound(children: &[Value]) -> Value {
    let mut compound_children = Vec::new();
    for (index, child) in children.iter().enumerate() {
        if index > 0 {
            compound_children.push(json!(" + "));
        }
        compound_children.push(child.clone());
    }
    json!({
        "type": 8,
        "children": compound_children,
        "loc": children
            .first()
            .and_then(|child| child.get("loc"))
            .cloned()
            .unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_text_suite_text_call(
    child: Value,
    include_content: bool,
    patch_flag: Option<Value>,
) -> Value {
    let mut args = Vec::new();
    if include_content {
        args.push(child.clone());
    }
    if let Some(flag) = patch_flag {
        args.push(flag);
    }
    let loc = child
        .get("loc")
        .cloned()
        .unwrap_or_else(vue3_loc_stub_value);
    json!({
        "type": 12,
        "content": child,
        "loc": loc,
        "codegenNode": {
            "type": 14,
            "callee": "CREATE_TEXT",
            "arguments": args,
            "loc": loc,
        },
    })
}

pub(crate) fn vue3_text_suite_finalize_root(root: &mut Value) {
    vue3_text_suite_set_root_codegen(root);
    let directives = vue3_text_suite_collect_directives(root);
    root["directives"] = json!(directives);
    root["helpers"] = json!(vue3_text_suite_helpers(root));
    root["components"] = json!([]);
    root["hoists"] = json!([]);
    root["cached"] = json!([]);
    root["temps"] = json!(0);
}

pub(crate) fn vue3_text_suite_set_root_codegen(root: &mut Value) {
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let visible = children
        .iter()
        .enumerate()
        .filter(|(_, child)| vue3_public_node_type(child) != Some(3))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    root["codegenNode"] = match visible.as_slice() {
        [] => Value::Null,
        [index] => {
            let child = children.get(*index).cloned().unwrap_or(Value::Null);
            match vue3_public_node_type(&child) {
                Some(1) => {
                    if let Some(root_child) = root
                        .get_mut("children")
                        .and_then(Value::as_array_mut)
                        .and_then(|items| items.get_mut(*index))
                    {
                        if let Some(codegen) = root_child.get_mut("codegenNode") {
                            codegen["isBlock"] = json!(true);
                        }
                    }
                    root.get("children")
                        .and_then(Value::as_array)
                        .and_then(|items| items.get(*index))
                        .and_then(|child| child.get("codegenNode"))
                        .cloned()
                        .unwrap_or(Value::Null)
                }
                Some(11) => child.get("codegenNode").cloned().unwrap_or(Value::Null),
                _ => child,
            }
        }
        _ => vue3_text_suite_vnode_call(
            "FRAGMENT",
            Value::Null,
            Value::Array(children),
            Some(json!(64)),
            true,
            false,
            Value::Null,
        ),
    };
}

pub(crate) fn vue3_text_suite_element_codegen(node: &Value, is_block: bool) -> Value {
    if vue3_public_node_type(node) != Some(1)
        || node.get("tagType").and_then(Value::as_u64) != Some(0)
    {
        return Value::Null;
    }
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let children_value = if children.is_empty() {
        Value::Null
    } else if children.len() == 1 && vue3_text_suite_direct_child_value(&children[0]) {
        children[0].clone()
    } else {
        Value::Array(children)
    };
    let directives = vue3_text_suite_runtime_directives(node);
    vue3_text_suite_vnode_call(
        &format!(
            "\"{}\"",
            node.get("tag").and_then(Value::as_str).unwrap_or("")
        ),
        Value::Null,
        children_value,
        None,
        is_block,
        false,
        directives,
    )
}

pub(crate) fn vue3_text_suite_for_codegen(node: &Value) -> Value {
    let parse_result = node.get("parseResult").unwrap_or(&Value::Null);
    let source = parse_result.get("source").cloned().unwrap_or(Value::Null);
    let params = ["value", "key", "index"]
        .into_iter()
        .filter_map(|key| {
            parse_result
                .get(key)
                .filter(|value| !value.is_null())
                .cloned()
        })
        .collect::<Vec<_>>();
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let inner_fragment = vue3_text_suite_vnode_call(
        "FRAGMENT",
        Value::Null,
        Value::Array(children),
        Some(json!(64)),
        true,
        false,
        Value::Null,
    );
    let render_list = vue3_text_suite_call(
        "RENDER_LIST",
        vec![
            source,
            json!({
                "type": 18,
                "params": params,
                "returns": inner_fragment,
                "newline": true,
                "isSlot": false,
                "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }),
        ],
    );
    vue3_text_suite_vnode_call(
        "FRAGMENT",
        Value::Null,
        render_list,
        Some(json!(256)),
        true,
        true,
        Value::Null,
    )
}

pub(crate) fn vue3_text_suite_vnode_call(
    tag: &str,
    props: Value,
    children: Value,
    patch_flag: Option<Value>,
    is_block: bool,
    disable_tracking: bool,
    directives: Value,
) -> Value {
    json!({
        "type": 13,
        "tag": tag,
        "props": props,
        "children": children,
        "patchFlag": patch_flag.unwrap_or(Value::Null),
        "dynamicProps": Value::Null,
        "directives": directives,
        "isBlock": is_block,
        "disableTracking": disable_tracking,
        "isComponent": false,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_text_suite_call(callee: &str, arguments: Vec<Value>) -> Value {
    json!({
        "type": 14,
        "callee": callee,
        "arguments": arguments,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_text_suite_runtime_directives(node: &Value) -> Value {
    let directives = vue3_text_suite_runtime_directive_names(node)
        .into_iter()
        .map(|name| {
            Value::Array(vec![Value::String(vue3_text_suite_directive_asset_id(
                &name,
            ))])
        })
        .collect::<Vec<_>>();
    if directives.is_empty() {
        Value::Null
    } else {
        Value::Array(directives)
    }
}

pub(crate) fn vue3_text_suite_collect_directives(root: &Value) -> Vec<String> {
    let mut directives = Vec::new();
    vue3_text_suite_collect_directives_for_node(root, &mut directives);
    directives
}

pub(crate) fn vue3_text_suite_collect_directives_for_node(
    node: &Value,
    directives: &mut Vec<String>,
) {
    for name in vue3_text_suite_runtime_directive_names(node) {
        if !directives.iter().any(|existing| existing == &name) {
            directives.push(name);
        }
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            vue3_text_suite_collect_directives_for_node(child, directives);
        }
    }
    if let Some(content) = node.get("content").filter(|value| value.is_object()) {
        vue3_text_suite_collect_directives_for_node(content, directives);
    }
}

pub(crate) fn vue3_text_suite_runtime_directive_names(node: &Value) -> Vec<String> {
    node.get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|prop| {
            if vue3_public_node_type(prop) != Some(7) {
                return None;
            }
            let name = prop.get("name").and_then(Value::as_str)?;
            (!vue3_text_suite_builtin_directive(name)).then(|| name.to_string())
        })
        .collect()
}

pub(crate) fn vue3_text_suite_builtin_directive(name: &str) -> bool {
    matches!(
        name,
        "bind"
            | "cloak"
            | "else"
            | "else-if"
            | "for"
            | "html"
            | "if"
            | "memo"
            | "model"
            | "on"
            | "once"
            | "pre"
            | "slot"
            | "text"
    )
}

pub(crate) fn vue3_text_suite_directive_asset_id(name: &str) -> String {
    let mut id = String::from("_directive_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            id.push(ch);
        } else {
            id.push('_');
        }
    }
    id
}

pub(crate) fn vue3_text_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_text_suite_collect_helpers(root, &mut used);
    if !vue3_text_suite_collect_directives(root).is_empty() {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_DIRECTIVE");
    }
    let order = if vue3_text_suite_single_for_root(root) {
        [
            "RENDER_LIST",
            "FRAGMENT",
            "OPEN_BLOCK",
            "CREATE_ELEMENT_BLOCK",
            "CREATE_TEXT",
            "TO_DISPLAY_STRING",
        ]
        .as_slice()
    } else if vue3_text_suite_single_directive_element_root(root) {
        [
            "TO_DISPLAY_STRING",
            "CREATE_TEXT",
            "RESOLVE_DIRECTIVE",
            "OPEN_BLOCK",
            "CREATE_ELEMENT_BLOCK",
            "WITH_DIRECTIVES",
        ]
        .as_slice()
    } else if root
        .get("codegenNode")
        .and_then(|node| node.get("tag"))
        .and_then(Value::as_str)
        == Some("FRAGMENT")
    {
        [
            "CREATE_ELEMENT_VNODE",
            "TO_DISPLAY_STRING",
            "CREATE_TEXT",
            "FRAGMENT",
            "OPEN_BLOCK",
            "CREATE_ELEMENT_BLOCK",
            "RENDER_LIST",
            "WITH_DIRECTIVES",
            "RESOLVE_DIRECTIVE",
        ]
        .as_slice()
    } else {
        [
            "TO_DISPLAY_STRING",
            "CREATE_TEXT",
            "CREATE_ELEMENT_VNODE",
            "OPEN_BLOCK",
            "CREATE_ELEMENT_BLOCK",
            "FRAGMENT",
            "RENDER_LIST",
            "WITH_DIRECTIVES",
            "RESOLVE_DIRECTIVE",
        ]
        .as_slice()
    };
    order
        .iter()
        .copied()
        .filter(|helper| used.iter().any(|used| *used == *helper))
        .map(str::to_string)
        .collect()
}

pub(crate) fn vue3_text_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    match vue3_public_node_type(node) {
        Some(5) => vue3_text_suite_add_helper(used, "TO_DISPLAY_STRING"),
        Some(12) => {
            if let Some(codegen) = node.get("codegenNode") {
                vue3_text_suite_collect_helpers(codegen, used);
            }
            if let Some(content) = node.get("content") {
                vue3_text_suite_collect_helpers(content, used);
            }
        }
        Some(13) => {
            if node.get("tag").and_then(Value::as_str) == Some("FRAGMENT") {
                vue3_text_suite_add_helper(used, "FRAGMENT");
            }
            if node.get("directives").is_some_and(|value| !value.is_null()) {
                vue3_text_suite_add_helper(used, "WITH_DIRECTIVES");
            }
            if node
                .get("isBlock")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "OPEN_BLOCK");
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
            } else {
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_VNODE");
            }
        }
        Some(14) => {
            if let Some(callee) = node.get("callee").and_then(Value::as_str) {
                match callee {
                    "CREATE_TEXT" => vue3_text_suite_add_helper(used, "CREATE_TEXT"),
                    "RENDER_LIST" => vue3_text_suite_add_helper(used, "RENDER_LIST"),
                    _ => {}
                }
            }
        }
        _ => {}
    }
    for key in [
        "children",
        "content",
        "codegenNode",
        "arguments",
        "returns",
        "params",
        "directives",
    ] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_text_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_text_suite_collect_helpers(value, used);
        }
    }
}

pub(crate) fn vue3_text_suite_add_helper(used: &mut Vec<&'static str>, helper: &'static str) {
    if !used.iter().any(|existing| *existing == helper) {
        used.push(helper);
    }
}

pub(crate) fn vue3_text_suite_single_for_root(root: &Value) -> bool {
    root.get("children")
        .and_then(Value::as_array)
        .is_some_and(|children| {
            children.len() == 1 && vue3_public_node_type(&children[0]) == Some(11)
        })
}

pub(crate) fn vue3_text_suite_single_directive_element_root(root: &Value) -> bool {
    root.get("children")
        .and_then(Value::as_array)
        .is_some_and(|children| {
            children.len() == 1
                && vue3_public_node_type(&children[0]) == Some(1)
                && !vue3_text_suite_runtime_directive_names(&children[0]).is_empty()
        })
}

pub(crate) fn vue3_text_suite_direct_child_value(child: &Value) -> bool {
    matches!(vue3_public_node_type(child), Some(2 | 5 | 8))
}

pub(crate) fn vue3_text_suite_directive<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            vue3_public_node_type(prop) == Some(7)
                && prop.get("name").and_then(Value::as_str) == Some(name)
        })
}

pub(crate) fn vue3_text_suite_remove_directive(node: &mut Value, name: &str) {
    if let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) {
        props.retain(|prop| {
            !(vue3_public_node_type(prop) == Some(7)
                && prop.get("name").and_then(Value::as_str) == Some(name))
        });
    }
}

pub(crate) fn vue3_text_suite_transform_context(options: &Vue3CompilerOptions) -> Value {
    json!({
        "compat": true,
        "ssr": false,
        "inSSR": false,
        "prefixIdentifiers": options.prefix_identifiers,
        "inline": options.inline,
        "isTS": options.is_ts,
        "expressionPlugins": options.expression_plugins,
        "directiveTransforms": [],
        "identifiers": {},
        "bindingMetadata": options.binding_metadata,
    })
}

pub(crate) fn vue3_text_suite_transform_context_with_directives(
    options: &Vue3CompilerOptions,
    directive_transforms: &[&str],
) -> Value {
    let mut context = vue3_text_suite_transform_context(options);
    context["directiveTransforms"] = Value::Array(
        directive_transforms
            .iter()
            .map(|name| Value::String((*name).to_string()))
            .collect(),
    );
    context
}

pub(crate) fn vue3_public_node_type(node: &Value) -> Option<u64> {
    node.get("type").and_then(Value::as_u64)
}

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
    if source.ends_with('<') {
        diagnostics.push(vue3_error_value(
            5,
            vue3_source_loc_value(source, source.len(), source.len()),
        ));
    } else if source.ends_with("</") && source.len() <= 2 {
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
                        let code = if source[token.start..token.end]
                            .as_bytes()
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

pub(crate) struct OpenDiagnosticElement {
    pub(crate) name: String,
    pub(crate) start: usize,
    pub(crate) namespace: vuec_ast::HtmlNamespace,
    pub(crate) attributes: Vec<vuec_html::HtmlAttribute>,
    pub(crate) in_v_pre: bool,
}

pub(crate) fn sfc_diagnostic_raw_text_block(
    options: &Vue3CompilerOptions,
    depth: usize,
    tag: &str,
    attributes: &[vuec_html::HtmlAttribute],
) -> bool {
    if !options.sfc_parse_mode || depth != 0 {
        return false;
    }
    tag != "template" || sfc_plain_template_attrs(attributes, options)
}

pub(crate) fn sfc_plain_template_element(
    element: &vuec_ast::Vue3Element,
    options: &Vue3CompilerOptions,
) -> bool {
    if element.tag != "template" {
        return false;
    }
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Attribute(attr)
                if attr.name == "lang"
                    && attr
                        .value
                        .as_deref()
                        .is_some_and(|lang| sfc_plain_template_lang(lang, options))
        )
    })
}

pub(crate) fn sfc_plain_template_attrs(
    attributes: &[vuec_html::HtmlAttribute],
    options: &Vue3CompilerOptions,
) -> bool {
    attributes.iter().any(|attr| {
        attr.name == "lang"
            && attr
                .value
                .as_deref()
                .is_some_and(|lang| sfc_plain_template_lang(lang, options))
    })
}

pub(crate) fn sfc_plain_template_lang(lang: &str, options: &Vue3CompilerOptions) -> bool {
    !lang.is_empty()
        && ((options.sfc_parse_mode && lang != "html")
            || options
                .sfc_plain_template_langs
                .iter()
                .any(|candidate| candidate == lang))
}

pub(crate) fn vue3_diagnostic_tag_namespace(
    options: &Vue3CompilerOptions,
    tag: &str,
    attributes: &[vuec_html::HtmlAttribute],
    parent: Option<&OpenDiagnosticElement>,
) -> vuec_ast::HtmlNamespace {
    if let Some(namespace) = options.namespaces.get(tag).copied() {
        return namespace;
    }
    let mut namespace = parent
        .map(|open| open.namespace)
        .unwrap_or(options.root_namespace);
    if options.dom_namespaces {
        if let Some(parent) = parent {
            if namespace == vuec_ast::HtmlNamespace::MathMl {
                if parent.name == "annotation-xml" {
                    if tag == "svg" {
                        return vuec_ast::HtmlNamespace::Svg;
                    }
                    if diagnostic_attrs_have_value(
                        &parent.attributes,
                        "encoding",
                        &["text/html", "application/xhtml+xml"],
                    ) {
                        namespace = vuec_ast::HtmlNamespace::Html;
                    }
                } else if vue3_mathml_text_integration_point(&parent.name)
                    && tag != "mglyph"
                    && tag != "malignmark"
                {
                    namespace = vuec_ast::HtmlNamespace::Html;
                }
            } else if namespace == vuec_ast::HtmlNamespace::Svg
                && matches!(parent.name.as_str(), "foreignObject" | "desc" | "title")
            {
                namespace = vuec_ast::HtmlNamespace::Html;
            }
        }
        if namespace == vuec_ast::HtmlNamespace::Html {
            if tag == "svg" {
                return vuec_ast::HtmlNamespace::Svg;
            }
            if tag == "math" {
                return vuec_ast::HtmlNamespace::MathMl;
            }
        }
    }
    let _ = attributes;
    namespace
}

pub(crate) fn vue3_mathml_text_integration_point(tag: &str) -> bool {
    matches!(tag, "mi" | "mo" | "mn" | "ms" | "mtext")
}

pub(crate) fn diagnostic_attrs_have_value(
    attributes: &[vuec_html::HtmlAttribute],
    name: &str,
    values: &[&str],
) -> bool {
    attributes.iter().any(|attr| {
        attr.name == name
            && attr
                .value
                .as_deref()
                .is_some_and(|value| values.iter().any(|candidate| *candidate == value))
    })
}

pub(crate) fn pop_diagnostic_stack_until(
    stack: &mut Vec<OpenDiagnosticElement>,
    name: &str,
    v_pre_depth: &mut usize,
) {
    while let Some(open) = stack.pop() {
        if open.in_v_pre && *v_pre_depth > 0 {
            *v_pre_depth -= 1;
        }
        if open.name.eq_ignore_ascii_case(name) {
            break;
        }
    }
}

pub(crate) fn tag_token_is_incomplete(source: &str, start: usize, end: usize) -> bool {
    source
        .get(start..end)
        .is_some_and(|slice| !slice.ends_with('>'))
}

pub(crate) fn tag_token_is_incomplete_at_eof(source: &str, start: usize, end: usize) -> bool {
    end == source.len() && tag_token_is_incomplete(source, start, end)
}

pub(crate) fn collect_missing_end_tag_name_diagnostics(source: &str, diagnostics: &mut Vec<Value>) {
    let mut cursor = 0usize;
    while let Some(offset) = source[cursor..].find("</>") {
        let start = cursor + offset;
        diagnostics.push(vue3_error_value(
            14,
            vue3_source_loc_value(source, start + 2, start + 2),
        ));
        cursor = start + 3;
    }
}

pub(crate) fn collect_start_tag_parse_errors(
    source: &str,
    start: usize,
    end: usize,
    attributes: &[vuec_html::HtmlAttribute],
    diagnostics: &mut Vec<Value>,
) {
    collect_unexpected_equals_before_attribute_name(source, start, end, attributes, diagnostics);
    collect_unexpected_solidus_in_tag(source, start, end, attributes, diagnostics);

    let mut seen_attrs = Vec::<String>::new();
    for attr in attributes {
        if attr.name.starts_with('=') {
            diagnostics.push(vue3_error_value(
                19,
                vue3_source_loc_value(source, attr.name_start, attr.name_start),
            ));
        }

        if seen_attrs.iter().any(|seen| seen == &attr.name) {
            diagnostics.push(vue3_error_value(
                2,
                vue3_source_loc_value(source, attr.name_start, attr.name_start),
            ));
        } else {
            seen_attrs.push(attr.name.clone());
        }

        if let Some(offset) = attr
            .name
            .char_indices()
            .find_map(|(index, ch)| matches!(ch, '"' | '\'' | '<').then_some(index))
        {
            let absolute = attr.name_start + offset;
            diagnostics.push(vue3_error_value(
                17,
                vue3_source_loc_value(source, absolute, absolute),
            ));
        }

        if attr.name.contains('[') && !attr.name.contains(']') {
            diagnostics.push(vue3_error_value(
                27,
                vue3_source_loc_value(source, attr.name_end, attr.name_end),
            ));
        }

        if attr.value.as_deref() == Some("")
            && matches!(attr.quote, Some(vuec_html::HtmlQuoteKind::Unquoted))
            && attr
                .value_start
                .and_then(|value_start| source.as_bytes().get(value_start).copied())
                == Some(b'>')
        {
            let offset = attr.value_start.unwrap_or(attr.end);
            diagnostics.push(vue3_error_value(
                13,
                vue3_source_loc_value(source, offset, offset),
            ));
        }

        if matches!(attr.quote, Some(vuec_html::HtmlQuoteKind::Unquoted)) {
            if let (Some(value_start), Some(value_end)) =
                (attr.value_content_start, attr.value_content_end)
            {
                if let Some(offset) =
                    first_unexpected_unquoted_attribute_value_char(source, value_start, value_end)
                {
                    diagnostics.push(vue3_error_value(
                        18,
                        vue3_source_loc_value(source, offset, offset),
                    ));
                }
            }
        }
    }
}

pub(crate) fn collect_unexpected_equals_before_attribute_name(
    source: &str,
    start: usize,
    end: usize,
    attributes: &[vuec_html::HtmlAttribute],
    diagnostics: &mut Vec<Value>,
) {
    for offset in start..end {
        if source.as_bytes().get(offset) != Some(&b'=') {
            continue;
        }
        if attributes
            .iter()
            .any(|attr| offset >= attr.start && offset < attr.end)
        {
            continue;
        }
        diagnostics.push(vue3_error_value(
            19,
            vue3_source_loc_value(source, offset, offset),
        ));
    }
}

pub(crate) fn collect_unexpected_solidus_in_tag(
    source: &str,
    start: usize,
    end: usize,
    attributes: &[vuec_html::HtmlAttribute],
    diagnostics: &mut Vec<Value>,
) {
    for offset in start..end {
        if source.as_bytes().get(offset) != Some(&b'/') {
            continue;
        }
        if offset == start + 1 {
            continue;
        }
        if attributes.iter().any(|attr| {
            attr.value_content_start
                .zip(attr.value_content_end)
                .is_some_and(|(value_start, value_end)| offset >= value_start && offset < value_end)
        }) {
            continue;
        }
        if source.as_bytes().get(offset + 1) == Some(&b'>') {
            continue;
        }
        diagnostics.push(vue3_error_value(
            22,
            vue3_source_loc_value(source, offset, offset),
        ));
    }
}

pub(crate) fn first_unexpected_unquoted_attribute_value_char(
    source: &str,
    start: usize,
    end: usize,
) -> Option<usize> {
    source
        .get(start..end)?
        .char_indices()
        .find_map(|(index, ch)| matches!(ch, '"' | '\'' | '<' | '=' | '`').then_some(start + index))
}

pub(crate) fn collect_invalid_lt_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
    for node in &ast.nodes {
        let Vue3AstKind::Text(_) = &node.kind else {
            continue;
        };
        if text_has_raw_text_parent(ast, node.id) || text_has_sfc_raw_parent(ast, node.id, options)
        {
            continue;
        }
        let Some(span) = node.span.source() else {
            continue;
        };
        let start = span.start.0.saturating_sub(base_offset);
        let end = span.end.0.saturating_sub(base_offset).min(source.len());
        let Some(slice) = source.get(start..end) else {
            continue;
        };
        let mut cursor = 0usize;
        while let Some(offset) = slice[cursor..].find('<') {
            let local_index = cursor + offset;
            cursor = local_index + 1;
            let global_index = start + local_index;
            match source.as_bytes().get(global_index + 1).copied() {
                Some(b'?') => diagnostics.push(vue3_error_value(
                    21,
                    vue3_source_loc_value(source, global_index + 1, global_index + 1),
                )),
                Some(b'/')
                    if source
                        .as_bytes()
                        .get(global_index + 2)
                        .is_some_and(u8::is_ascii_whitespace) =>
                {
                    diagnostics.push(vue3_error_value(
                        23,
                        vue3_source_loc_value(source, global_index, global_index),
                    ));
                }
                Some(next) if !matches!(next, b'/' | b'!' | b'A'..=b'Z' | b'a'..=b'z') => {
                    diagnostics.push(vue3_error_value(
                        12,
                        vue3_source_loc_value(source, global_index, global_index),
                    ));
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn text_has_raw_text_parent(ast: &Vue3Ast, node_id: vuec_ast::NodeId) -> bool {
    let Some(parent_id) = ast.node(node_id).and_then(|node| node.parent) else {
        return false;
    };
    ast.node(parent_id).is_some_and(|node| {
        matches!(
            &node.kind,
            Vue3AstKind::Element(element)
                if element.ns == vuec_ast::HtmlNamespace::Html
                    && matches!(element.tag.as_str(), "textarea" | "title" | "style" | "script")
        )
    })
}

pub(crate) fn text_has_sfc_raw_parent(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
) -> bool {
    if !options.sfc_parse_mode {
        return false;
    }
    let Some(parent_id) = ast.node(node_id).and_then(|node| node.parent) else {
        return false;
    };
    let Some(parent) = ast.node(parent_id) else {
        return false;
    };
    let Some(root) = ast.node(ast.root) else {
        return false;
    };
    parent.parent == Some(ast.root)
        && root.children.contains(&parent_id)
        && matches!(
            &parent.kind,
            Vue3AstKind::Element(element)
                if element.tag != "template" || sfc_plain_template_element(element, options)
        )
}

pub(crate) fn collect_missing_interpolation_end_diagnostics(
    source: &str,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
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
            HtmlTokenKind::Text(text) if v_pre_depth == 0 => {
                collect_missing_interpolation_end_in_text(source, token.start, &text, diagnostics);
            }
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let starts_v_pre =
                    v_pre_depth == 0 && attributes.iter().any(|attr| attr.name == "v-pre");
                let in_v_pre = v_pre_depth > 0 || starts_v_pre;
                let is_void = vue3_is_void_tag(options, &name);
                let namespace =
                    vue3_diagnostic_tag_namespace(options, &name, &attributes, stack.last());
                let raw_text_kind = vuec_vue3_core::vue3_raw_text_kind(&name, namespace, in_v_pre);
                if !self_closing && !is_void {
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
                if !name.is_empty() {
                    while let Some(open) = stack.pop() {
                        if open.in_v_pre && v_pre_depth > 0 {
                            v_pre_depth -= 1;
                        }
                        if open.name.eq_ignore_ascii_case(&name) {
                            break;
                        }
                    }
                }
            }
            HtmlTokenKind::Cdata(_)
            | HtmlTokenKind::Text(_)
            | HtmlTokenKind::Comment(_)
            | HtmlTokenKind::BogusQuestionTag
            | HtmlTokenKind::Doctype(_)
            | HtmlTokenKind::Eof => {}
        }
        if eof {
            break;
        }
    }
}

pub(crate) fn collect_missing_interpolation_end_in_text(
    source: &str,
    token_start: usize,
    text: &str,
    diagnostics: &mut Vec<Value>,
) {
    let mut cursor = 0usize;
    while let Some(open_offset) = text[cursor..].find("{{") {
        let open = cursor + open_offset;
        let inner_start = open + 2;
        if let Some(close_offset) = text[inner_start..].find("}}") {
            cursor = inner_start + close_offset + 2;
        } else {
            let global_open = token_start + open;
            diagnostics.push(vue3_error_value(
                25,
                vue3_source_loc_value(source, global_open, global_open),
            ));
            break;
        }
    }
}

pub(crate) fn collect_invalid_end_tag_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    _base_offset: usize,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
    let _ = ast;
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
                let starts_v_pre =
                    v_pre_depth == 0 && attributes.iter().any(|attr| attr.name == "v-pre");
                let in_v_pre = v_pre_depth > 0 || starts_v_pre;
                let namespace =
                    vue3_diagnostic_tag_namespace(options, &name, &attributes, stack.last());
                let raw_text_kind = vuec_vue3_core::vue3_raw_text_kind(&name, namespace, in_v_pre);
                if !self_closing
                    && !vue3_is_void_tag(options, &name)
                    && !tag_token_is_incomplete_at_eof(source, token.start, token.end)
                {
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
                    if tag_token_is_incomplete(source, token.start, token.end) {
                        continue;
                    }
                    if source[token.start..token.end]
                        .as_bytes()
                        .get(2)
                        .is_some_and(u8::is_ascii_whitespace)
                    {
                        diagnostics.push(vue3_error_value(
                            23,
                            vue3_source_loc_value(source, token.start, token.start),
                        ));
                    }
                    continue;
                }
                if tag_token_is_incomplete(source, token.start, token.end) {
                    continue;
                }
                if stack
                    .last()
                    .is_some_and(|open| open.name.eq_ignore_ascii_case(&name))
                {
                    if stack.pop().is_some_and(|open| open.in_v_pre) && v_pre_depth > 0 {
                        v_pre_depth -= 1;
                    }
                } else if let Some(matching_index) = stack
                    .iter()
                    .rposition(|open| open.name.eq_ignore_ascii_case(&name))
                {
                    while stack.len() > matching_index + 1 {
                        if let Some(open) = stack.pop() {
                            if open.in_v_pre && v_pre_depth > 0 {
                                v_pre_depth -= 1;
                            }
                            if !open.in_v_pre {
                                diagnostics.push(vue3_error_value(
                                    24,
                                    vue3_source_loc_value(source, open.start, open.start),
                                ));
                            }
                        }
                    }
                    if stack.pop().is_some_and(|open| open.in_v_pre) && v_pre_depth > 0 {
                        v_pre_depth -= 1;
                    }
                } else if !stack
                    .last()
                    .is_some_and(|open| raw_text_tag_ignores_end_tag(&open.name, &name))
                {
                    diagnostics.push(vue3_error_value(
                        23,
                        vue3_source_loc_value(source, token.start, token.start),
                    ));
                }
            }
            HtmlTokenKind::Text(_)
            | HtmlTokenKind::Comment(_)
            | HtmlTokenKind::Cdata(_)
            | HtmlTokenKind::BogusQuestionTag
            | HtmlTokenKind::Doctype(_)
            | HtmlTokenKind::Eof => {}
        }
        if eof {
            break;
        }
    }
    while let Some(open) = stack.pop() {
        if !open.in_v_pre {
            diagnostics.push(vue3_error_value(
                24,
                vue3_source_loc_value(source, open.start, open.start),
            ));
        }
    }
}

pub(crate) fn raw_text_tag_ignores_end_tag(open: &str, close: &str) -> bool {
    matches!(open, "textarea" | "title") && !open.eq_ignore_ascii_case(close)
}

pub(crate) fn vue3_is_void_tag(options: &Vue3CompilerOptions, tag: &str) -> bool {
    options
        .void_tags
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
}

pub(crate) fn collect_missing_directive_name_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    diagnostics: &mut Vec<Value>,
) {
    for node in &ast.nodes {
        let Vue3AstKind::Element(element) = &node.kind else {
            continue;
        };
        for prop in &element.props {
            let Vue3Prop::Attribute(attr) = prop else {
                continue;
            };
            if attr.name == "v-" {
                let loc = attr
                    .span
                    .map(|span| vue3_source_span_value(source, base_offset, span))
                    .unwrap_or_else(vue3_loc_stub_value);
                diagnostics.push(vue3_error_value(26, loc));
            }
        }
    }
}

pub(crate) fn vue3_error_value(code: u8, loc: Value) -> Value {
    json!({
        "code": code,
        "loc": loc,
    })
}

pub(crate) fn vue3_namespace_value(namespace: vuec_ast::HtmlNamespace) -> u8 {
    match namespace {
        vuec_ast::HtmlNamespace::Html => 0,
        vuec_ast::HtmlNamespace::Svg => 1,
        vuec_ast::HtmlNamespace::MathMl => 2,
    }
}

pub(crate) fn vue3_element_type_value(tag_type: vuec_ast::Vue3ElementType) -> u8 {
    match tag_type {
        vuec_ast::Vue3ElementType::Element => 0,
        vuec_ast::Vue3ElementType::Component => 1,
        vuec_ast::Vue3ElementType::SlotOutlet => 2,
        vuec_ast::Vue3ElementType::Template => 3,
    }
}

pub(crate) fn vue3_prop_value(
    source: &str,
    base_offset: usize,
    prop: &Vue3Prop,
    options: &Vue3CompilerOptions,
) -> Value {
    match prop {
        Vue3Prop::Attribute(attr) => vue3_attribute_value(source, base_offset, attr),
        Vue3Prop::Directive(dir) => {
            let exp_mode = match dir.name.as_str() {
                "on" => Vue3ExpressionAstMode::Statements,
                "slot" => Vue3ExpressionAstMode::Params,
                _ => Vue3ExpressionAstMode::Expression,
            };
            let mut value = json!({
                "type": 7,
                "name": dir.name,
                "rawName": dir.raw_name,
                "exp": dir.exp.as_ref().map(|exp| vue3_expression_value_with_mode(source, base_offset, exp, &span_to_node_span(dir.exp_span), false, Vue3ExpressionProjectionMode::Exact, options, exp_mode)),
                "arg": dir.arg.as_ref().map(|arg| vue3_expression_value_with_mode(source, base_offset, arg, &span_to_node_span(dir.arg_span), !dir.is_dynamic_arg, Vue3ExpressionProjectionMode::ExactLocTrimContent, options, Vue3ExpressionAstMode::Expression)),
                "modifiers": dir.modifiers.iter().enumerate().map(|(index, modifier)| {
                    let loc = dir
                        .modifier_spans
                        .get(index)
                        .map(|span| vue3_loc_value(source, base_offset, span))
                        .unwrap_or_else(vue3_loc_stub_value);
                    vue3_simple_expression_value(
                        modifier,
                        !matches!(dir.modifier_spans.get(index), Some(NodeSpan::Missing { .. })),
                        loc,
                    )
                }).collect::<Vec<_>>(),
                "loc": dir.span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
            });
            if dir.name == "for" {
                value["forParseResult"] =
                    vue3_for_parse_result_value(source, base_offset, dir, options);
            }
            value
        }
    }
}

pub(crate) fn vue3_attribute_value(
    source: &str,
    base_offset: usize,
    attr: &vuec_ast::Vue3Attribute,
) -> Value {
    json!({
        "type": 6,
        "name": attr.name,
        "nameLoc": attr.name_span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
        "value": attr.value.as_ref().map(|value| json!({
            "type": 2,
            "content": value,
            "loc": attr.value_span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
        })),
        "loc": attr.span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_inner_loc_value(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    node_id: vuec_ast::NodeId,
) -> Value {
    let Some(node) = ast.node(node_id) else {
        return vue3_loc_stub_value();
    };
    let Some(span) = node.span.source() else {
        return vue3_loc_stub_value();
    };
    let element_start = span.start.0.saturating_sub(base_offset);
    let element_end = span.end.0.saturating_sub(base_offset).min(source.len());
    let open_end = vue3_open_tag_end(source, element_start, element_end).unwrap_or(element_start);
    let inner_end = vue3_close_tag_start(source, open_end, element_end).unwrap_or_else(|| {
        node.children
            .last()
            .and_then(|child_id| ast.node(*child_id))
            .and_then(|child| child.span.source())
            .map(|child_span| {
                child_span
                    .end
                    .0
                    .saturating_sub(base_offset)
                    .min(source.len())
            })
            .unwrap_or(open_end)
    });
    vue3_source_loc_value(source, open_end, inner_end)
}

pub(crate) fn vue3_open_tag_end(source: &str, start: usize, end: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, ch) in source.get(start..end)?.char_indices() {
        match (quote, ch) {
            (Some(active), current) if current == active => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(ch),
            (None, '>') => return Some(start + offset + 1),
            (None, _) => {}
        }
    }
    None
}

pub(crate) fn vue3_close_tag_start(
    source: &str,
    open_end: usize,
    element_end: usize,
) -> Option<usize> {
    let mut cursor = open_end.min(source.len());
    let end = element_end.min(source.len());
    let mut close_start = None;
    while cursor < end {
        let Some(offset) = source.get(cursor..end)?.find("</") else {
            break;
        };
        close_start = Some(cursor + offset);
        cursor += offset + "</".len();
    }
    close_start
}

pub(crate) fn span_to_node_span(span: Option<vuec_source::Span>) -> NodeSpan {
    span.map(NodeSpan::from)
        .unwrap_or_else(|| NodeSpan::missing(vuec_ast::MissingSpanReason::Synthetic))
}

pub(crate) fn vue3_expression_value(
    source_text: &str,
    base_offset: usize,
    expression: &Vue3Expression,
    fallback_span: &NodeSpan,
    is_static: bool,
    options: &Vue3CompilerOptions,
    ast_mode: Vue3ExpressionAstMode,
) -> Value {
    vue3_expression_value_with_mode(
        source_text,
        base_offset,
        expression,
        fallback_span,
        is_static,
        Vue3ExpressionProjectionMode::Trim,
        options,
        ast_mode,
    )
}

#[derive(Clone, Copy)]
pub(crate) enum Vue3ExpressionProjectionMode {
    Trim,
    ExactLocTrimContent,
    Exact,
}

#[derive(Clone, Copy)]
pub(crate) enum Vue3ExpressionAstMode {
    Expression,
    Params,
    Statements,
}

pub(crate) fn vue3_expression_value_with_mode(
    source_text: &str,
    base_offset: usize,
    expression: &Vue3Expression,
    fallback_span: &NodeSpan,
    is_static: bool,
    mode: Vue3ExpressionProjectionMode,
    options: &Vue3CompilerOptions,
    ast_mode: Vue3ExpressionAstMode,
) -> Value {
    let source = expression.source_string();
    let loc = match mode {
        Vue3ExpressionProjectionMode::Trim => {
            vue3_expression_loc(source_text, base_offset, fallback_span, &source)
        }
        Vue3ExpressionProjectionMode::ExactLocTrimContent | Vue3ExpressionProjectionMode::Exact => {
            vue3_loc_value(source_text, base_offset, fallback_span)
        }
    };
    let content = match mode {
        Vue3ExpressionProjectionMode::Exact => source,
        Vue3ExpressionProjectionMode::Trim | Vue3ExpressionProjectionMode::ExactLocTrimContent => {
            source.trim().to_string()
        }
    };
    let mut value = vue3_simple_expression_value(&content, is_static, loc);
    if let Some(ast_value) = vue3_expression_ast_value(&content, is_static, options, ast_mode) {
        value["ast"] = ast_value;
    }
    value
}

pub(crate) fn vue3_simple_expression_value(source: &str, is_static: bool, loc: Value) -> Value {
    json!({
        "type": 4,
        "loc": loc,
        "content": source,
        "isStatic": is_static,
        "constType": if is_static { 3 } else { 0 },
    })
}

pub(crate) fn vue3_expression_ast_value(
    source: &str,
    is_static: bool,
    options: &Vue3CompilerOptions,
    mode: Vue3ExpressionAstMode,
) -> Option<Value> {
    if is_static || !options.prefix_identifiers || source.trim().is_empty() {
        return None;
    }
    let trimmed = source.trim();
    if is_simple_identifier(trimmed) {
        return Some(Value::Null);
    }
    let store = JsAstStore::new();
    let source_type = vue3_expression_source_type(options);
    match mode {
        Vue3ExpressionAstMode::Expression => {
            let expression_source = format!("({trimmed})");
            store
                .parse_expression(&expression_source, source_type)
                .ok()
                .map(|expression| expression_ast_value(&expression))
        }
        Vue3ExpressionAstMode::Params => {
            let expression_source = format!("({trimmed})=>{{}}");
            store
                .parse_expression(&expression_source, source_type)
                .ok()
                .map(|expression| expression_ast_value(&expression))
        }
        Vue3ExpressionAstMode::Statements => {
            let program_source = format!(" {trimmed} ");
            let program = store.parse_program(&program_source, source_type);
            Some(json!({
                "type": "Program",
                "body": program.program.body.iter().map(statement_ast_value).collect::<Vec<_>>(),
            }))
        }
    }
}

pub(crate) fn vue3_for_parse_result_value(
    source: &str,
    base_offset: usize,
    dir: &vuec_ast::Vue3Directive,
    options: &Vue3CompilerOptions,
) -> Value {
    let expression = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let Some((aliases, iterable)) = split_v_for_expression(&expression) else {
        return Value::Null;
    };
    let source_loc = dir
        .exp_span
        .and_then(|span| {
            let local_start = span.start.0.saturating_sub(base_offset);
            let local_end = span.end.0.saturating_sub(base_offset).min(source.len());
            source
                .get(local_start..local_end)
                .and_then(|slice| slice.find(iterable).map(|offset| local_start + offset))
                .map(|start| vue3_source_loc_value(source, start, start + iterable.len()))
        })
        .unwrap_or_else(vue3_loc_stub_value);
    let parts = split_v_for_aliases(aliases);
    json!({
        "source": vue3_simple_expression_with_ast_value(iterable, false, source_loc, options, Vue3ExpressionAstMode::Expression),
        "value": parts.first().map(|value| {
            vue3_simple_expression_with_ast_value(value, false, vue3_loc_stub_value(), options, Vue3ExpressionAstMode::Params)
        }),
        "key": parts.get(1).map(|value| {
            vue3_simple_expression_with_ast_value(value, false, vue3_loc_stub_value(), options, Vue3ExpressionAstMode::Expression)
        }),
        "index": parts.get(2).map(|value| {
            vue3_simple_expression_with_ast_value(value, false, vue3_loc_stub_value(), options, Vue3ExpressionAstMode::Expression)
        }),
        "finalized": false,
    })
}

pub(crate) fn vue3_simple_expression_with_ast_value(
    source: &str,
    is_static: bool,
    loc: Value,
    options: &Vue3CompilerOptions,
    ast_mode: Vue3ExpressionAstMode,
) -> Value {
    let mut value = vue3_simple_expression_value(source, is_static, loc);
    if let Some(ast_value) = vue3_expression_ast_value(source, is_static, options, ast_mode) {
        value["ast"] = ast_value;
    }
    value
}

pub(crate) fn vue3_expression_source_type(options: &Vue3CompilerOptions) -> SourceType {
    if options.is_ts
        || options
            .expression_plugins
            .iter()
            .any(|plugin| plugin == "typescript")
    {
        SourceType::ts()
    } else {
        SourceType::mjs()
    }
}

pub(crate) fn expression_ast_value(expression: &Expression<'_>) -> Value {
    match expression {
        Expression::ArrayExpression(array) => json!({
            "type": "ArrayExpression",
            "elements": array.elements.iter().map(array_element_ast_value).collect::<Vec<_>>(),
        }),
        Expression::ArrowFunctionExpression(function) => json!({
            "type": "ArrowFunctionExpression",
            "params": formal_parameters_ast_values(&function.params),
            "body": function_body_ast_value(&function.body),
        }),
        Expression::AssignmentExpression(assignment) => json!({
            "type": "AssignmentExpression",
            "left": assignment_target_ast_value(&assignment.left),
            "right": expression_ast_value(&assignment.right),
        }),
        Expression::AwaitExpression(await_expression) => json!({
            "type": "AwaitExpression",
            "argument": expression_ast_value(&await_expression.argument),
        }),
        Expression::BinaryExpression(binary) => json!({
            "type": "BinaryExpression",
            "left": expression_ast_value(&binary.left),
            "right": expression_ast_value(&binary.right),
        }),
        Expression::CallExpression(call) => json!({
            "type": "CallExpression",
            "callee": expression_ast_value(&call.callee),
            "arguments": call.arguments.iter().map(argument_ast_value).collect::<Vec<_>>(),
            "optional": call.optional,
        }),
        Expression::ChainExpression(chain) => json!({
            "type": "ChainExpression",
            "expression": chain_element_ast_value(&chain.expression),
        }),
        Expression::ConditionalExpression(conditional) => json!({
            "type": "ConditionalExpression",
            "test": expression_ast_value(&conditional.test),
            "consequent": expression_ast_value(&conditional.consequent),
            "alternate": expression_ast_value(&conditional.alternate),
        }),
        Expression::FunctionExpression(function) => json!({
            "type": "FunctionExpression",
            "params": formal_parameters_ast_values(&function.params),
            "body": function.body.as_ref().map(|body| function_body_ast_value(body)),
        }),
        Expression::Identifier(identifier) => identifier_reference_ast_value(identifier),
        Expression::ImportExpression(import_expression) => json!({
            "type": "ImportExpression",
            "source": expression_ast_value(&import_expression.source),
            "options": import_expression.options.as_ref().map(expression_ast_value),
        }),
        Expression::LogicalExpression(logical) => json!({
            "type": "LogicalExpression",
            "left": expression_ast_value(&logical.left),
            "right": expression_ast_value(&logical.right),
        }),
        Expression::ComputedMemberExpression(member) => computed_member_ast_value(member),
        Expression::StaticMemberExpression(member) => static_member_ast_value(member),
        Expression::PrivateFieldExpression(member) => private_field_ast_value(member),
        Expression::NewExpression(new_expression) => json!({
            "type": "NewExpression",
            "callee": expression_ast_value(&new_expression.callee),
            "arguments": new_expression.arguments.iter().map(argument_ast_value).collect::<Vec<_>>(),
        }),
        Expression::ObjectExpression(object) => json!({
            "type": "ObjectExpression",
            "properties": object.properties.iter().map(object_property_kind_ast_value).collect::<Vec<_>>(),
        }),
        Expression::ParenthesizedExpression(parenthesized) => {
            expression_ast_value(&parenthesized.expression)
        }
        Expression::PrivateInExpression(private_in) => json!({
            "type": "BinaryExpression",
            "right": expression_ast_value(&private_in.right),
        }),
        Expression::SequenceExpression(sequence) => json!({
            "type": "SequenceExpression",
            "expressions": sequence.expressions.iter().map(expression_ast_value).collect::<Vec<_>>(),
        }),
        Expression::TaggedTemplateExpression(tagged) => json!({
            "type": "TaggedTemplateExpression",
            "tag": expression_ast_value(&tagged.tag),
            "quasi": template_literal_ast_value(&tagged.quasi),
        }),
        Expression::TemplateLiteral(template) => template_literal_ast_value(template),
        Expression::ThisExpression(_) => json!({ "type": "ThisExpression" }),
        Expression::UnaryExpression(unary) => json!({
            "type": "UnaryExpression",
            "argument": expression_ast_value(&unary.argument),
        }),
        Expression::UpdateExpression(update) => json!({
            "type": "UpdateExpression",
            "argument": simple_assignment_target_ast_value(&update.argument),
        }),
        Expression::YieldExpression(yield_expression) => json!({
            "type": "YieldExpression",
            "argument": yield_expression.argument.as_ref().map(expression_ast_value),
        }),
        Expression::BooleanLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value,
        }),
        Expression::NullLiteral(_) => json!({
            "type": "Literal",
            "value": Value::Null,
        }),
        Expression::NumericLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value,
        }),
        Expression::StringLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value.as_str(),
        }),
        Expression::BigIntLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value.as_str(),
        }),
        Expression::RegExpLiteral(_) => json!({ "type": "Literal" }),
        Expression::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            ts_expression_ast_value("TSInstantiationExpression", &expression.expression)
        }
        _ => json!({ "type": "Expression" }),
    }
}

pub(crate) fn statement_ast_value(statement: &Statement<'_>) -> Value {
    match statement {
        Statement::BlockStatement(block) => json!({
            "type": "BlockStatement",
            "body": block.body.iter().map(statement_ast_value).collect::<Vec<_>>(),
        }),
        Statement::DoWhileStatement(statement) => json!({
            "type": "DoWhileStatement",
            "body": statement_ast_value(&statement.body),
            "test": expression_ast_value(&statement.test),
        }),
        Statement::ExpressionStatement(statement) => json!({
            "type": "ExpressionStatement",
            "expression": expression_ast_value(&statement.expression),
        }),
        Statement::ForStatement(statement) => json!({
            "type": "ForStatement",
            "test": statement.test.as_ref().map(expression_ast_value),
            "update": statement.update.as_ref().map(expression_ast_value),
            "body": statement_ast_value(&statement.body),
        }),
        Statement::IfStatement(statement) => json!({
            "type": "IfStatement",
            "test": expression_ast_value(&statement.test),
            "consequent": statement_ast_value(&statement.consequent),
            "alternate": statement.alternate.as_ref().map(statement_ast_value),
        }),
        Statement::ReturnStatement(statement) => json!({
            "type": "ReturnStatement",
            "argument": statement.argument.as_ref().map(expression_ast_value),
        }),
        Statement::ThrowStatement(statement) => json!({
            "type": "ThrowStatement",
            "argument": expression_ast_value(&statement.argument),
        }),
        Statement::VariableDeclaration(declaration) => json!({
            "type": "VariableDeclaration",
            "declarations": declaration.declarations.iter().map(|declarator| json!({
                "type": "VariableDeclarator",
                "id": binding_pattern_ast_value(&declarator.id),
                "init": declarator.init.as_ref().map(expression_ast_value),
            })).collect::<Vec<_>>(),
        }),
        Statement::WhileStatement(statement) => json!({
            "type": "WhileStatement",
            "test": expression_ast_value(&statement.test),
            "body": statement_ast_value(&statement.body),
        }),
        _ => json!({ "type": statement_type_name(statement) }),
    }
}

pub(crate) fn statement_type_name(statement: &Statement<'_>) -> &'static str {
    match statement {
        Statement::BlockStatement(_) => "BlockStatement",
        Statement::BreakStatement(_) => "BreakStatement",
        Statement::ContinueStatement(_) => "ContinueStatement",
        Statement::DebuggerStatement(_) => "DebuggerStatement",
        Statement::DoWhileStatement(_) => "DoWhileStatement",
        Statement::EmptyStatement(_) => "EmptyStatement",
        Statement::ExpressionStatement(_) => "ExpressionStatement",
        Statement::ForInStatement(_) => "ForInStatement",
        Statement::ForOfStatement(_) => "ForOfStatement",
        Statement::ForStatement(_) => "ForStatement",
        Statement::IfStatement(_) => "IfStatement",
        Statement::ReturnStatement(_) => "ReturnStatement",
        Statement::SwitchStatement(_) => "SwitchStatement",
        Statement::ThrowStatement(_) => "ThrowStatement",
        Statement::TryStatement(_) => "TryStatement",
        Statement::VariableDeclaration(_) => "VariableDeclaration",
        Statement::WhileStatement(_) => "WhileStatement",
        _ => "Statement",
    }
}

pub(crate) fn identifier_reference_ast_value(
    identifier: &oxc_ast::ast::IdentifierReference<'_>,
) -> Value {
    json!({
        "type": "Identifier",
        "name": identifier.name.as_str(),
    })
}

pub(crate) fn identifier_name_ast_value(identifier: &oxc_ast::ast::IdentifierName<'_>) -> Value {
    json!({
        "type": "Identifier",
        "name": identifier.name.as_str(),
    })
}

pub(crate) fn private_identifier_ast_value(
    identifier: &oxc_ast::ast::PrivateIdentifier<'_>,
) -> Value {
    json!({
        "type": "PrivateName",
        "name": identifier.name.as_str(),
    })
}

pub(crate) fn computed_member_ast_value(
    member: &oxc_ast::ast::ComputedMemberExpression<'_>,
) -> Value {
    json!({
        "type": "MemberExpression",
        "object": expression_ast_value(&member.object),
        "property": expression_ast_value(&member.expression),
        "computed": true,
        "optional": member.optional,
    })
}

pub(crate) fn static_member_ast_value(member: &oxc_ast::ast::StaticMemberExpression<'_>) -> Value {
    json!({
        "type": "MemberExpression",
        "object": expression_ast_value(&member.object),
        "property": identifier_name_ast_value(&member.property),
        "computed": false,
        "optional": member.optional,
    })
}

pub(crate) fn private_field_ast_value(member: &oxc_ast::ast::PrivateFieldExpression<'_>) -> Value {
    json!({
        "type": "MemberExpression",
        "object": expression_ast_value(&member.object),
        "property": private_identifier_ast_value(&member.field),
        "computed": false,
        "optional": member.optional,
    })
}

pub(crate) fn template_literal_ast_value(template: &oxc_ast::ast::TemplateLiteral<'_>) -> Value {
    json!({
        "type": "TemplateLiteral",
        "expressions": template.expressions.iter().map(expression_ast_value).collect::<Vec<_>>(),
    })
}

pub(crate) fn ts_expression_ast_value(kind: &str, expression: &Expression<'_>) -> Value {
    json!({
        "type": kind,
        "expression": expression_ast_value(expression),
    })
}

pub(crate) fn array_element_ast_value(element: &ArrayExpressionElement<'_>) -> Value {
    match element {
        ArrayExpressionElement::SpreadElement(spread) => json!({
            "type": "SpreadElement",
            "argument": expression_ast_value(&spread.argument),
        }),
        ArrayExpressionElement::Elision(_) => Value::Null,
        _ => element
            .as_expression()
            .map(expression_ast_value)
            .unwrap_or_else(|| json!({ "type": "Expression" })),
    }
}

pub(crate) fn argument_ast_value(argument: &Argument<'_>) -> Value {
    match argument {
        Argument::SpreadElement(spread) => json!({
            "type": "SpreadElement",
            "argument": expression_ast_value(&spread.argument),
        }),
        _ => argument
            .as_expression()
            .map(expression_ast_value)
            .unwrap_or_else(|| json!({ "type": "Expression" })),
    }
}

pub(crate) fn object_property_kind_ast_value(property: &ObjectPropertyKind<'_>) -> Value {
    match property {
        ObjectPropertyKind::ObjectProperty(property) => json!({
            "type": "ObjectProperty",
            "key": property_key_ast_value(&property.key),
            "value": expression_ast_value(&property.value),
            "computed": property.computed,
            "shorthand": property.shorthand,
        }),
        ObjectPropertyKind::SpreadProperty(spread) => json!({
            "type": "SpreadElement",
            "argument": expression_ast_value(&spread.argument),
        }),
    }
}

pub(crate) fn property_key_ast_value(key: &PropertyKey<'_>) -> Value {
    match key {
        PropertyKey::StaticIdentifier(identifier) => identifier_name_ast_value(identifier),
        PropertyKey::PrivateIdentifier(identifier) => private_identifier_ast_value(identifier),
        _ => key
            .as_expression()
            .map(expression_ast_value)
            .unwrap_or_else(|| json!({ "type": "Identifier", "name": "" })),
    }
}

pub(crate) fn chain_element_ast_value(element: &ChainElement<'_>) -> Value {
    match element {
        ChainElement::CallExpression(call) => json!({
            "type": "CallExpression",
            "callee": expression_ast_value(&call.callee),
            "arguments": call.arguments.iter().map(argument_ast_value).collect::<Vec<_>>(),
            "optional": call.optional,
        }),
        ChainElement::ComputedMemberExpression(member) => computed_member_ast_value(member),
        ChainElement::StaticMemberExpression(member) => static_member_ast_value(member),
        ChainElement::PrivateFieldExpression(member) => private_field_ast_value(member),
        ChainElement::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
    }
}

pub(crate) fn assignment_target_ast_value(target: &AssignmentTarget<'_>) -> Value {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            identifier_reference_ast_value(identifier)
        }
        AssignmentTarget::ComputedMemberExpression(member) => computed_member_ast_value(member),
        AssignmentTarget::StaticMemberExpression(member) => static_member_ast_value(member),
        AssignmentTarget::PrivateFieldExpression(member) => private_field_ast_value(member),
        AssignmentTarget::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        AssignmentTarget::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        AssignmentTarget::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        AssignmentTarget::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
        AssignmentTarget::ArrayAssignmentTarget(target) => json!({
            "type": "ArrayPattern",
            "elements": target.elements.iter().map(|element| {
                element
                    .as_ref()
                    .map(assignment_target_maybe_default_ast_value)
                    .unwrap_or(Value::Null)
            }).collect::<Vec<_>>(),
            "rest": target.rest.as_ref().map(|rest| json!({
                "type": "RestElement",
                "argument": assignment_target_ast_value(&rest.target),
            })),
        }),
        AssignmentTarget::ObjectAssignmentTarget(target) => json!({
            "type": "ObjectPattern",
            "properties": target.properties.iter().map(assignment_target_property_ast_value).collect::<Vec<_>>(),
            "rest": target.rest.as_ref().map(|rest| json!({
                "type": "RestElement",
                "argument": assignment_target_ast_value(&rest.target),
            })),
        }),
    }
}

pub(crate) fn simple_assignment_target_ast_value(target: &SimpleAssignmentTarget<'_>) -> Value {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            identifier_reference_ast_value(identifier)
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            computed_member_ast_value(member)
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => static_member_ast_value(member),
        SimpleAssignmentTarget::PrivateFieldExpression(member) => private_field_ast_value(member),
        SimpleAssignmentTarget::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        SimpleAssignmentTarget::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        SimpleAssignmentTarget::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        SimpleAssignmentTarget::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
    }
}

pub(crate) fn assignment_target_maybe_default_ast_value(
    target: &oxc_ast::ast::AssignmentTargetMaybeDefault<'_>,
) -> Value {
    match target {
        oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(target) => json!({
            "type": "AssignmentPattern",
            "left": assignment_target_ast_value(&target.binding),
            "right": expression_ast_value(&target.init),
        }),
        oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetIdentifier(identifier) => {
            identifier_reference_ast_value(identifier)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::ComputedMemberExpression(member) => {
            computed_member_ast_value(member)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::StaticMemberExpression(member) => {
            static_member_ast_value(member)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::PrivateFieldExpression(member) => {
            private_field_ast_value(member)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::ArrayAssignmentTarget(target) => json!({
            "type": "ArrayPattern",
            "elements": target.elements.iter().map(|element| {
                element
                    .as_ref()
                    .map(assignment_target_maybe_default_ast_value)
                    .unwrap_or(Value::Null)
            }).collect::<Vec<_>>(),
        }),
        oxc_ast::ast::AssignmentTargetMaybeDefault::ObjectAssignmentTarget(target) => json!({
            "type": "ObjectPattern",
            "properties": target.properties.iter().map(assignment_target_property_ast_value).collect::<Vec<_>>(),
        }),
    }
}

pub(crate) fn assignment_target_property_ast_value(
    property: &oxc_ast::ast::AssignmentTargetProperty<'_>,
) -> Value {
    match property {
        oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(property) => {
            let mut value = json!({
                "type": "ObjectProperty",
                "key": identifier_reference_ast_value(&property.binding),
                "value": identifier_reference_ast_value(&property.binding),
                "computed": false,
                "shorthand": true,
            });
            if let Some(init) = &property.init {
                value["value"] = json!({
                    "type": "AssignmentPattern",
                    "left": identifier_reference_ast_value(&property.binding),
                    "right": expression_ast_value(init),
                });
            }
            value
        }
        oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(property) => {
            json!({
                "type": "ObjectProperty",
                "key": property_key_ast_value(&property.name),
                "value": assignment_target_maybe_default_ast_value(&property.binding),
                "computed": property.computed,
                "shorthand": false,
            })
        }
    }
}

pub(crate) fn formal_parameters_ast_values(
    parameters: &oxc_ast::ast::FormalParameters<'_>,
) -> Vec<Value> {
    let mut params = parameters
        .items
        .iter()
        .map(formal_parameter_ast_value)
        .collect::<Vec<_>>();
    if let Some(rest) = &parameters.rest {
        params.push(json!({
            "type": "RestElement",
            "argument": binding_pattern_ast_value(&rest.rest.argument),
        }));
    }
    params
}

pub(crate) fn formal_parameter_ast_value(parameter: &FormalParameter<'_>) -> Value {
    let pattern = binding_pattern_ast_value(&parameter.pattern);
    match &parameter.initializer {
        Some(initializer) => json!({
            "type": "AssignmentPattern",
            "left": pattern,
            "right": expression_ast_value(initializer),
        }),
        None => pattern,
    }
}

pub(crate) fn function_body_ast_value(body: &oxc_ast::ast::FunctionBody<'_>) -> Value {
    json!({
        "type": "BlockStatement",
        "body": body.statements.iter().map(statement_ast_value).collect::<Vec<_>>(),
    })
}

pub(crate) fn binding_pattern_ast_value(pattern: &BindingPattern<'_>) -> Value {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => json!({
            "type": "Identifier",
            "name": identifier.name.as_str(),
        }),
        BindingPattern::ObjectPattern(pattern) => {
            let mut properties = pattern
                .properties
                .iter()
                .map(binding_property_ast_value)
                .collect::<Vec<_>>();
            if let Some(rest) = &pattern.rest {
                properties.push(json!({
                    "type": "RestElement",
                    "argument": binding_pattern_ast_value(&rest.argument),
                }));
            }
            json!({
                "type": "ObjectPattern",
                "properties": properties,
            })
        }
        BindingPattern::ArrayPattern(pattern) => {
            let mut elements = pattern
                .elements
                .iter()
                .map(|element| {
                    element
                        .as_ref()
                        .map(binding_pattern_ast_value)
                        .unwrap_or(Value::Null)
                })
                .collect::<Vec<_>>();
            if let Some(rest) = &pattern.rest {
                elements.push(json!({
                    "type": "RestElement",
                    "argument": binding_pattern_ast_value(&rest.argument),
                }));
            }
            json!({
                "type": "ArrayPattern",
                "elements": elements,
            })
        }
        BindingPattern::AssignmentPattern(pattern) => json!({
            "type": "AssignmentPattern",
            "left": binding_pattern_ast_value(&pattern.left),
            "right": expression_ast_value(&pattern.right),
        }),
    }
}

pub(crate) fn binding_property_ast_value(property: &oxc_ast::ast::BindingProperty<'_>) -> Value {
    json!({
        "type": "ObjectProperty",
        "key": property_key_ast_value(&property.key),
        "value": binding_pattern_ast_value(&property.value),
        "computed": property.computed,
        "shorthand": property.shorthand,
    })
}

pub(crate) fn split_v_for_expression(source: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 => {
                let rest = &source[index..];
                if rest.starts_with(" in ") {
                    return Some((source[..index].trim(), source[index + 4..].trim()));
                }
                if rest.starts_with(" of ") {
                    return Some((source[..index].trim(), source[index + 4..].trim()));
                }
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn split_v_for_aliases(source: &str) -> Vec<String> {
    let aliases = source
        .trim()
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or_else(|| source.trim());
    split_top_level_csv(aliases)
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn split_top_level_csv(source: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let item = source[start..index].trim();
                if !item.is_empty() {
                    items.push(item);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        items.push(tail);
    }
    items
}

pub(crate) fn vue3_expression_loc(
    source: &str,
    base_offset: usize,
    fallback_span: &NodeSpan,
    expression: &str,
) -> Value {
    let Some(span) = fallback_span.source() else {
        return vue3_loc_stub_value();
    };
    let local_span_start = span.start.0.saturating_sub(base_offset);
    let local_span_end = span.end.0.saturating_sub(base_offset).min(source.len());
    let node_source = source
        .get(local_span_start..local_span_end)
        .unwrap_or_default();
    if let Some((inner_start, inner_end)) =
        default_interpolation_inner_trimmed_span(source, local_span_start, local_span_end)
    {
        return vue3_source_loc_value(source, inner_start, inner_end);
    }
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        let inner_start = if node_source.starts_with("{{") {
            local_span_start + "{{".len()
        } else {
            local_span_start
        };
        return vue3_source_loc_value(source, inner_start, inner_start);
    }
    if let Some(local_start) = node_source.find(trimmed) {
        let start = local_span_start + local_start;
        return vue3_source_loc_value(source, start, start + trimmed.len());
    }
    vue3_loc_value(source, base_offset, fallback_span)
}

pub(crate) fn default_interpolation_inner_trimmed_span(
    source: &str,
    start: usize,
    end: usize,
) -> Option<(usize, usize)> {
    let slice = source.get(start..end)?;
    if !slice.starts_with("{{") || !slice.ends_with("}}") {
        return None;
    }
    let mut inner_start = start + "{{".len();
    let mut inner_end = end.saturating_sub("}}".len());
    while inner_start < inner_end
        && source
            .get(inner_start..inner_end)
            .and_then(|value| value.chars().next())
            .is_some_and(char::is_whitespace)
    {
        let ch = source[inner_start..inner_end].chars().next()?;
        inner_start += ch.len_utf8();
    }
    while inner_end > inner_start
        && source
            .get(inner_start..inner_end)
            .and_then(|value| value.chars().next_back())
            .is_some_and(char::is_whitespace)
    {
        let ch = source[inner_start..inner_end].chars().next_back()?;
        inner_end -= ch.len_utf8();
    }
    Some((inner_start, inner_end))
}

pub(crate) fn vue3_loc_value(source: &str, base_offset: usize, span: &NodeSpan) -> Value {
    let Some(span) = span.source() else {
        return vue3_loc_stub_value();
    };
    vue3_source_span_value(source, base_offset, span)
}

pub(crate) fn vue3_text_loc_value(source: &str, base_offset: usize, span: &NodeSpan) -> Value {
    let Some(source_span) = span.source() else {
        return vue3_loc_stub_value();
    };
    let start = source_span.start.0.saturating_sub(base_offset);
    let end = source_span.end.0.saturating_sub(base_offset);
    if end == source.len()
        && source_span.end.0 >= source_span.start.0
        && source
            .get(start..end)
            .is_some_and(|slice| slice == "/" && source.ends_with('/'))
        && source[..start].rfind('<').is_some_and(|tag_start| {
            source
                .get(tag_start..)
                .is_some_and(|slice| slice.starts_with('<') && !slice.contains('>'))
        })
    {
        return vue3_source_signed_start_loc_value(source, -1, end);
    }
    vue3_source_span_value(source, base_offset, source_span)
}

pub(crate) fn vue3_source_span_value(
    source: &str,
    base_offset: usize,
    span: vuec_source::Span,
) -> Value {
    let start = span.start.0.saturating_sub(base_offset);
    let end = span.end.0.saturating_sub(base_offset);
    vue3_source_loc_value(source, start, end)
}

pub(crate) fn vue3_source_signed_start_loc_value(source: &str, start: isize, end: usize) -> Value {
    let local_start = if start < 0 && end <= source.len() {
        end.saturating_sub(1)
    } else {
        start.max(0) as usize
    };
    let local_end = end.min(source.len()).max(local_start);
    json!({
        "start": vue3_signed_position(source, start),
        "end": vue3_position(source, end),
        "source": source.get(local_start..local_end).unwrap_or_default(),
    })
}

pub(crate) fn vue3_source_loc_value(source: &str, start: usize, end: usize) -> Value {
    let local_start = start.min(source.len());
    let local_end = end.min(source.len()).max(local_start);
    let start_pos = vue3_position(source, start);
    let end_pos = vue3_position(source, end);
    json!({
        "start": start_pos,
        "end": end_pos,
        "source": source.get(local_start..local_end).unwrap_or_default(),
    })
}

pub(crate) fn vue3_position(source: &str, offset: usize) -> Value {
    let mut line = 1usize;
    let mut column = 1usize;
    let mut index = 0usize;
    let mut utf16_offset = 0usize;
    for ch in source.chars() {
        if index >= offset {
            break;
        }
        index += ch.len_utf8();
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += ch.len_utf16();
        }
        utf16_offset += ch.len_utf16();
    }
    if offset > index {
        let extra = offset - index;
        column += extra;
        utf16_offset += extra;
    }
    json!({
        "offset": utf16_offset,
        "line": line,
        "column": column,
    })
}

pub(crate) fn vue3_signed_position(source: &str, offset: isize) -> Value {
    if offset >= 0 {
        return vue3_position(source, offset as usize);
    }
    json!({
        "offset": offset,
        "line": 1,
        "column": 1isize + offset,
    })
}

pub(crate) fn vue3_loc_stub_value() -> Value {
    json!({
        "start": { "offset": 0, "line": 1, "column": 1 },
        "end": { "offset": 0, "line": 1, "column": 1 },
        "source": "",
    })
}
