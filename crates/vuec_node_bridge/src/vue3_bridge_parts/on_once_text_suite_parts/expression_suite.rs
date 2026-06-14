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
