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
