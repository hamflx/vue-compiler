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
