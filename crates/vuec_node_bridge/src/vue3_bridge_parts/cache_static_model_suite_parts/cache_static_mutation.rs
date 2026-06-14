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
