#[derive(Clone, Copy, Debug)]
pub(crate) enum Vue3StringifyVirtualChild {
    Original(usize),
    StaticCall,
}

pub(crate) fn vue3_stringify_parent_is_cached(parent: &Value) -> bool {
    json_node_type(parent) == Some(1)
        && parent.get("codegenNode").is_some_and(|codegen| {
            json_node_type(codegen) == Some(13)
                && codegen
                    .get("children")
                    .is_some_and(|children| json_node_type(children) == Some(20))
        })
}

pub(crate) fn vue3_stringify_cached_node(node: &Value) -> Option<&Value> {
    let cacheable = (json_node_type(node) == Some(1) && json_u64(node, "tagType") == Some(0))
        || json_node_type(node) == Some(12);
    if !cacheable {
        return None;
    }
    let codegen = node.get("codegenNode")?;
    (json_node_type(codegen) == Some(20)).then_some(codegen)
}

pub(crate) fn vue3_stringify_flush_public_chunk(
    current_index: usize,
    is_parent_cached: bool,
    current_chunk: &mut [StaticHtmlAnalysis],
    virtual_children: &mut Vec<Vue3StringifyVirtualChild>,
    operations: &mut Vec<Value>,
) -> usize {
    if current_chunk.is_empty() {
        return 0;
    }
    let mut analysis = StaticHtmlAnalysis {
        html: StaticHtmlBuffer::default(),
        dom_nodes: current_chunk.len(),
        node_count: 0,
        element_with_binding_count: 0,
    };
    for item in current_chunk.iter() {
        analysis.html.append(item.html.clone());
        analysis.node_count += item.node_count;
        analysis.element_with_binding_count += item.element_with_binding_count;
    }
    if !analysis.meets_threshold() {
        return 0;
    }

    let start = current_index.saturating_sub(current_chunk.len());
    let count = current_chunk.len();
    let operation = json!({
        "kind": if is_parent_cached {
            "stringifyParentCachedRange"
        } else {
            "stringifyCachedChildRange"
        },
        "start": start,
        "count": count,
        "html": analysis.html.to_js_expression(),
        "domNodes": analysis.dom_nodes,
    });
    operations.push(operation);
    let delete_count = count.saturating_sub(1);
    if is_parent_cached {
        virtual_children.splice(
            start..start + count,
            [Vue3StringifyVirtualChild::StaticCall],
        );
    } else if delete_count > 0 {
        virtual_children.drain(start + 1..start + count);
    }
    delete_count
}

pub(crate) fn vue3_stringify_analyze_public_node(
    node: &Value,
    context: &Value,
) -> Option<StaticHtmlAnalysis> {
    match json_node_type(node) {
        Some(1) => {
            let tag = json_str(node, "tag").unwrap_or_default();
            if static_html_non_stringifiable_tag(tag)
                || vue3_public_node_has_directive(node, "once")
            {
                return None;
            }
            let ns = vue3_public_element_namespace(node, vuec_ast::HtmlNamespace::Html);
            let mut analysis = StaticHtmlAnalysis {
                html: vue3_stringify_public_node_html_with_ns(
                    node,
                    context,
                    vuec_ast::HtmlNamespace::Html,
                )?,
                dom_nodes: 1,
                node_count: 1,
                element_with_binding_count: (!vue3_public_props(node).is_empty()) as usize,
            };
            for child in vue3_public_children(node) {
                analysis.node_count += 1;
                if json_node_type(child) == Some(1) {
                    if !vue3_public_props(child).is_empty() {
                        analysis.element_with_binding_count += 1;
                    }
                    vue3_stringify_walk_public_element(child, ns, &mut analysis)?;
                }
            }
            Some(analysis)
        }
        Some(12) => Some(StaticHtmlAnalysis {
            html: vue3_stringify_public_node_html(
                node.get("content").unwrap_or(&Value::Null),
                context,
            )?,
            dom_nodes: 1,
            node_count: 1,
            element_with_binding_count: 0,
        }),
        _ => None,
    }
}

pub(crate) fn vue3_stringify_walk_public_element(
    node: &Value,
    parent_ns: vuec_ast::HtmlNamespace,
    analysis: &mut StaticHtmlAnalysis,
) -> Option<()> {
    let tag = json_str(node, "tag").unwrap_or_default();
    if static_html_non_stringifiable_tag(tag) || vue3_public_node_has_directive(node, "once") {
        return None;
    }
    let ns = vue3_public_element_namespace(node, parent_ns);
    let is_option = ns == vuec_ast::HtmlNamespace::Html && tag == "option";
    for prop in vue3_public_props(node) {
        if !vue3_stringify_public_prop_is_allowed(prop, ns, is_option) {
            return None;
        }
    }
    for child in vue3_public_children(node) {
        analysis.node_count += 1;
        if json_node_type(child) == Some(1) {
            if !vue3_public_props(child).is_empty() {
                analysis.element_with_binding_count += 1;
            }
            vue3_stringify_walk_public_element(child, ns, analysis)?;
        }
    }
    Some(())
}

pub(crate) fn vue3_stringify_public_prop_is_allowed(
    prop: &Value,
    ns: vuec_ast::HtmlNamespace,
    is_option: bool,
) -> bool {
    match json_node_type(prop) {
        Some(6) => {
            json_str(prop, "name").is_some_and(|name| static_html_attr_is_stringifiable(name, ns))
        }
        Some(7) if json_str(prop, "name") == Some("bind") => {
            let Some(arg) = prop.get("arg").filter(|arg| !arg.is_null()) else {
                return false;
            };
            if json_node_type(arg) == Some(8) {
                return false;
            }
            let arg_name = json_str(arg, "content").unwrap_or_default();
            if json_bool(arg, "isStatic") && !static_html_attr_is_stringifiable(arg_name, ns) {
                return false;
            }
            let Some(exp) = prop.get("exp").filter(|exp| !exp.is_null()) else {
                return false;
            };
            if json_node_type(exp) == Some(8) {
                return false;
            }
            if json_u64(exp, "constType").unwrap_or(0) < u64::from(VUE3_CONSTANT_CAN_STRINGIFY) {
                return false;
            }
            !(is_option && arg_name == "value" && !json_bool(exp, "isStatic"))
        }
        _ => true,
    }
}

pub(crate) fn vue3_stringify_public_node_html(
    node: &Value,
    context: &Value,
) -> Option<StaticHtmlBuffer> {
    vue3_stringify_public_node_html_with_ns(node, context, vuec_ast::HtmlNamespace::Html)
}

pub(crate) fn vue3_stringify_public_node_html_with_ns(
    node: &Value,
    context: &Value,
    parent_ns: vuec_ast::HtmlNamespace,
) -> Option<StaticHtmlBuffer> {
    match json_node_type(node) {
        Some(1) => vue3_stringify_public_element_html(node, context, parent_ns),
        Some(2) => Some(StaticHtmlBuffer::from_text(escape_static_html_text(
            json_str(node, "content").unwrap_or_default(),
        ))),
        Some(3) => Some(StaticHtmlBuffer::from_text(format!(
            "<!--{}-->",
            escape_static_html_text(json_str(node, "content").unwrap_or_default())
        ))),
        Some(5) => {
            let value = vue3_public_evaluate_constant(node.get("content")?)?.to_display_string()?;
            Some(StaticHtmlBuffer::from_text(escape_static_html_text(&value)))
        }
        Some(8) => {
            let value = vue3_public_evaluate_constant(node)?.to_js_string()?;
            Some(StaticHtmlBuffer::from_text(escape_static_html_text(&value)))
        }
        Some(12) => {
            vue3_stringify_public_node_html_with_ns(node.get("content")?, context, parent_ns)
        }
        _ => None,
    }
}

pub(crate) fn vue3_stringify_public_element_html(
    node: &Value,
    context: &Value,
    parent_ns: vuec_ast::HtmlNamespace,
) -> Option<StaticHtmlBuffer> {
    if json_u64(node, "tagType") != Some(0) || vue3_public_node_has_directive(node, "once") {
        return None;
    }
    let tag = json_str(node, "tag").unwrap_or_default();
    let ns = vue3_public_element_namespace(node, parent_ns);
    if ns == vuec_ast::HtmlNamespace::Html
        && (static_html_non_stringifiable_tag(tag)
            || static_html_is_void_tag(tag) && !vue3_public_children(node).is_empty())
    {
        return None;
    }

    let mut html = StaticHtmlBuffer::default();
    html.push_text("<");
    html.push_text(tag);
    let mut inner_html = None::<String>;
    for prop in vue3_public_props(node) {
        match json_node_type(prop) {
            Some(6) => {
                let name = json_str(prop, "name")?;
                if !static_html_attr_is_stringifiable(name, ns) {
                    return None;
                }
                html.push_text(" ");
                html.push_text(name);
                if let Some(value) = prop.get("value").filter(|value| !value.is_null()) {
                    html.push_text("=\"");
                    html.push_text(escape_static_html_attr(
                        json_str(value, "content").unwrap_or_default(),
                    ));
                    html.push_text("\"");
                }
            }
            Some(7) if json_str(prop, "name") == Some("html") => {
                let source = json_str(prop.get("exp")?, "content")?;
                let value = vue3_public_evaluate_source(source)?;
                inner_html = Some(decode_static_html_entities(&value.to_display_string()?));
            }
            Some(7) if json_str(prop, "name") == Some("text") => {
                let source = json_str(prop.get("exp")?, "content")?;
                let value = vue3_public_evaluate_source(source)?;
                inner_html = Some(escape_static_html_text(&value.to_display_string()?));
            }
            Some(7) if json_str(prop, "name") == Some("bind") => {
                let Some(attr) = vue3_stringify_public_bind_attr(tag, ns, prop)? else {
                    continue;
                };
                html.push_text(" ");
                html.push_text(attr.name);
                html.push_text("=\"");
                html.append(attr.value);
                html.push_text("\"");
            }
            Some(7) => {}
            _ => return None,
        }
    }
    if let Some(scope_id) = json_str(context, "scopeId").filter(|scope_id| !scope_id.is_empty()) {
        html.push_text(" ");
        html.push_text(scope_id);
    }
    html.push_text(">");

    if ns != vuec_ast::HtmlNamespace::Html || !static_html_is_void_tag(tag) {
        if let Some(inner_html) = inner_html.filter(|value| !value.is_empty()) {
            html.push_text(inner_html);
        } else {
            for child in vue3_public_children(node) {
                html.append(vue3_stringify_public_node_html_with_ns(child, context, ns)?);
            }
        }
        html.push_text("</");
        html.push_text(tag);
        html.push_text(">");
    }
    Some(html)
}

pub(crate) fn vue3_stringify_public_bind_attr(
    tag: &str,
    ns: vuec_ast::HtmlNamespace,
    prop: &Value,
) -> Option<Option<StaticHtmlAttr>> {
    let arg = prop.get("arg")?;
    if json_node_type(arg) == Some(8) || !json_bool(arg, "isStatic") {
        return None;
    }
    let name = json_str(arg, "content")?.to_string();
    if !static_html_attr_is_stringifiable(&name, ns) {
        return None;
    }
    if ns == vuec_ast::HtmlNamespace::Html && tag == "option" && name == "value" {
        return None;
    }
    let source = json_str(prop.get("exp")?, "content")?;
    if source.starts_with('_') {
        let mut value = StaticHtmlBuffer::default();
        value.push_expression(source);
        return Some(Some(StaticHtmlAttr { name, value }));
    }
    let value = vue3_public_evaluate_source(source)?;
    if matches!(value, StaticConstValue::Null) {
        return Some(None);
    }
    if static_html_is_boolean_attr(&name) && matches!(value, StaticConstValue::Bool(false)) {
        return Some(None);
    }
    let value = if name == "class" {
        static_const_normalize_class(&value)?
    } else if name == "style" {
        static_const_stringify_style(&value)?
    } else {
        value.to_display_string()?
    };
    Some(Some(StaticHtmlAttr {
        name,
        value: StaticHtmlBuffer::from_text(escape_static_html_attr(&value)),
    }))
}

pub(crate) fn vue3_public_evaluate_constant(node: &Value) -> Option<StaticConstValue> {
    match json_node_type(node) {
        Some(4) => vue3_public_evaluate_source(json_str(node, "content")?),
        Some(8) => {
            let mut output = String::new();
            for child in node.get("children").and_then(Value::as_array)? {
                if child.is_string() {
                    continue;
                }
                match json_node_type(child) {
                    Some(2) => output.push_str(json_str(child, "content").unwrap_or_default()),
                    Some(5) => output.push_str(
                        &vue3_public_evaluate_constant(child.get("content")?)?
                            .to_display_string()?,
                    ),
                    _ => output.push_str(&vue3_public_evaluate_constant(child)?.to_js_string()?),
                }
            }
            Some(StaticConstValue::String(output))
        }
        _ => None,
    }
}

pub(crate) fn vue3_public_evaluate_source(source: &str) -> Option<StaticConstValue> {
    static_const_eval_source(source)
}

pub(crate) fn vue3_public_node_has_directive(node: &Value, name: &str) -> bool {
    vue3_public_props(node)
        .iter()
        .any(|prop| json_node_type(prop) == Some(7) && json_str(prop, "name") == Some(name))
}

pub(crate) fn vue3_public_props(node: &Value) -> &[Value] {
    node.get("props")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub(crate) fn vue3_public_children(node: &Value) -> &[Value] {
    node.get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub(crate) fn vue3_public_namespace(ns: u64) -> vuec_ast::HtmlNamespace {
    match ns {
        1 => vuec_ast::HtmlNamespace::Svg,
        2 => vuec_ast::HtmlNamespace::MathMl,
        _ => vuec_ast::HtmlNamespace::Html,
    }
}

pub(crate) fn vue3_public_element_namespace(
    node: &Value,
    parent_ns: vuec_ast::HtmlNamespace,
) -> vuec_ast::HtmlNamespace {
    let tag = json_str(node, "tag").unwrap_or_default();
    if tag == "svg" {
        return vuec_ast::HtmlNamespace::Svg;
    }
    if tag == "math" {
        return vuec_ast::HtmlNamespace::MathMl;
    }
    if parent_ns == vuec_ast::HtmlNamespace::Svg
        && matches!(tag, "foreignObject" | "desc" | "title")
    {
        return vuec_ast::HtmlNamespace::Html;
    }
    if let Some(ns) = json_u64(node, "ns").filter(|ns| *ns != 0) {
        return vue3_public_namespace(ns);
    }
    parent_ns
}

#[derive(Default)]
pub(crate) struct Vue3CacheStaticState {
    pub(crate) operations: Vec<Value>,
}

pub(crate) fn vue3_cache_static_walk(
    children: &[Value],
    children_path: Vec<String>,
    parent_path: Option<Vec<String>>,
    parent: &Value,
    context: &Value,
    do_not_hoist_node: bool,
    state: &mut Vue3CacheStaticState,
) {
    let mut to_cache = Vec::<usize>::new();

    for (index, child) in children.iter().enumerate() {
        let child_path = vue3_path_child(&children_path, index);
        if json_node_type(child) == Some(1) && json_u64(child, "tagType") == Some(0) {
            let constant_type = if do_not_hoist_node {
                VUE3_CONSTANT_NOT
            } else {
                vue3_constant_type(child, context)
            };
            if constant_type > VUE3_CONSTANT_NOT {
                if constant_type >= VUE3_CONSTANT_CAN_CACHE {
                    if vue3_should_downgrade_static_block(child) {
                        state.operations.push(json!({
                            "kind": "setBlock",
                            "path": vue3_codegen_path(&child_path),
                            "isBlock": false,
                        }));
                    }
                    state.operations.push(json!({
                        "kind": "setPatchFlag",
                        "path": vue3_codegen_path(&child_path),
                        "patchFlag": -1,
                    }));
                    to_cache.push(index);
                    continue;
                }
            } else {
                vue3_project_prop_hoists(child, &child_path, context, state);
            }
        } else if json_node_type(child) == Some(12) {
            let constant_type = if do_not_hoist_node {
                VUE3_CONSTANT_NOT
            } else {
                vue3_constant_type(child, context)
            };
            if constant_type >= VUE3_CONSTANT_CAN_CACHE {
                state.operations.push(json!({
                    "kind": "appendTextCallPatchFlag",
                    "path": vue3_codegen_path(&child_path),
                    "patchFlag": "-1 /* CACHED */",
                }));
                to_cache.push(index);
                continue;
            }
        }

        match json_node_type(child) {
            Some(1) => {
                let child_children = child
                    .get("children")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                vue3_cache_static_walk(
                    child_children,
                    vue3_path_push(&child_path, "children"),
                    Some(child_path.clone()),
                    child,
                    context,
                    false,
                    state,
                );
            }
            Some(11) => {
                let for_children = child
                    .get("children")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                vue3_cache_static_walk(
                    for_children,
                    vue3_path_push(&child_path, "children"),
                    Some(child_path.clone()),
                    child,
                    context,
                    for_children.len() == 1,
                    state,
                );
            }
            Some(9) => {
                if let Some(branches) = child.get("branches").and_then(Value::as_array) {
                    for (branch_index, branch) in branches.iter().enumerate() {
                        let branch_children = branch
                            .get("children")
                            .and_then(Value::as_array)
                            .map(Vec::as_slice)
                            .unwrap_or(&[]);
                        vue3_cache_static_walk(
                            branch_children,
                            vue3_path_push(
                                &vue3_path_child(
                                    &vue3_path_push(&child_path, "branches"),
                                    branch_index,
                                ),
                                "children",
                            ),
                            Some(vue3_path_child(
                                &vue3_path_push(&child_path, "branches"),
                                branch_index,
                            )),
                            branch,
                            context,
                            branch_children.len() == 1,
                            state,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    if vue3_can_cache_children_array(&to_cache, children, parent) {
        let target = if json_u64(parent, "tagType") == Some(0) {
            Some(json!({
                "kind": "cacheChildrenArray",
                "path": vue3_path_push(
                    &vue3_codegen_path(parent_path.as_deref().unwrap_or(&[])),
                    "children"
                ),
                "childrenPath": children_path,
                "needArraySpread": true,
            }))
        } else if json_u64(parent, "tagType") == Some(1) {
            Some(json!({
                "kind": "cacheSlotReturns",
                "ownerPath": parent_path,
                "slot": { "kind": "static", "name": "default" },
                "needArraySpread": true,
            }))
        } else if json_u64(parent, "tagType") == Some(3) {
            parent_path.as_ref().and_then(|template_path| {
                let slot = vue3_template_slot_projection(parent)?;
                Some(json!({
                    "kind": "cacheSlotReturns",
                    "ownerPath": vue3_parent_path(template_path),
                    "slot": slot,
                    "needArraySpread": true,
                }))
            })
        } else {
            None
        };
        if let Some(operation) = target {
            state.operations.push(operation);
            return;
        }
    }

    for index in to_cache {
        state.operations.push(json!({
            "kind": "cacheCodegen",
            "path": vue3_codegen_path(&vue3_path_child(&children_path, index)),
        }));
    }
}

pub(crate) fn vue3_project_prop_hoists(
    node: &Value,
    child_path: &[String],
    context: &Value,
    state: &mut Vue3CacheStaticState,
) {
    let Some(codegen_node) = node.get("codegenNode") else {
        return;
    };
    if json_node_type(codegen_node) != Some(13) {
        return;
    }
    let flag = codegen_node.get("patchFlag");
    let patch_flag_allows_props = flag.is_none_or(Value::is_null)
        || flag.and_then(Value::as_i64) == Some(512)
        || flag.and_then(Value::as_i64) == Some(1);
    if patch_flag_allows_props
        && vue3_generated_props_constant_type(node, context) >= VUE3_CONSTANT_CAN_CACHE
        && !codegen_node.get("props").is_none_or(Value::is_null)
    {
        state.operations.push(json!({
            "kind": "hoistProps",
            "path": vue3_path_push(&vue3_codegen_path(child_path), "props"),
        }));
    }
    if !codegen_node.get("dynamicProps").is_none_or(Value::is_null) {
        state.operations.push(json!({
            "kind": "hoistDynamicProps",
            "path": vue3_path_push(&vue3_codegen_path(child_path), "dynamicProps"),
        }));
    }
}

pub(crate) fn vue3_can_cache_children_array(
    to_cache: &[usize],
    children: &[Value],
    parent: &Value,
) -> bool {
    if to_cache.len() != children.len() || children.is_empty() || json_node_type(parent) != Some(1)
    {
        return false;
    }
    match json_u64(parent, "tagType") {
        Some(0) => {
            let Some(codegen_node) = parent.get("codegenNode") else {
                return false;
            };
            json_node_type(codegen_node) == Some(13)
                && codegen_node
                    .get("children")
                    .and_then(Value::as_array)
                    .is_some()
        }
        Some(1) => parent.get("codegenNode").is_some_and(|codegen_node| {
            json_node_type(codegen_node) == Some(13)
                && vue3_codegen_has_object_slots(codegen_node)
                && vue3_slot_returns_len(
                    codegen_node,
                    &json!({ "kind": "static", "name": "default" }),
                ) == Some(children.len())
        }),
        Some(3) => true,
        _ => false,
    }
}

pub(crate) fn vue3_constant_type(node: &Value, context: &Value) -> u8 {
    match json_node_type(node) {
        Some(1) => vue3_element_constant_type(node, context),
        Some(2) | Some(3) => VUE3_CONSTANT_CAN_STRINGIFY,
        Some(9) | Some(10) | Some(11) => VUE3_CONSTANT_NOT,
        Some(5) | Some(12) => node
            .get("content")
            .map(|content| vue3_constant_type(content, context))
            .unwrap_or(VUE3_CONSTANT_NOT),
        Some(4) => json_u64(node, "constType")
            .map(|value| value as u8)
            .unwrap_or_else(|| {
                if json_bool(node, "isStatic") {
                    VUE3_CONSTANT_CAN_STRINGIFY
                } else {
                    VUE3_CONSTANT_NOT
                }
            }),
        Some(8) => vue3_compound_constant_type(node, context),
        Some(20) => VUE3_CONSTANT_CAN_CACHE,
        _ => VUE3_CONSTANT_NOT,
    }
}

pub(crate) fn vue3_element_constant_type(node: &Value, context: &Value) -> u8 {
    if json_u64(node, "tagType") != Some(0) {
        return VUE3_CONSTANT_NOT;
    }
    let Some(codegen_node) = node.get("codegenNode") else {
        return VUE3_CONSTANT_NOT;
    };
    if json_node_type(codegen_node) != Some(13) {
        return VUE3_CONSTANT_NOT;
    }
    if json_bool(codegen_node, "isBlock")
        && !matches!(
            json_str(node, "tag"),
            Some("svg" | "foreignObject" | "math")
        )
    {
        return VUE3_CONSTANT_NOT;
    }
    if !codegen_node.get("patchFlag").is_none_or(Value::is_null) {
        return VUE3_CONSTANT_NOT;
    }

    let mut return_type = VUE3_CONSTANT_CAN_STRINGIFY;
    let generated_props_type = vue3_generated_props_constant_type(node, context);
    if generated_props_type == VUE3_CONSTANT_NOT {
        return VUE3_CONSTANT_NOT;
    }
    return_type = return_type.min(generated_props_type);

    for child in node
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        let child_type = vue3_constant_type(child, context);
        if child_type == VUE3_CONSTANT_NOT {
            return VUE3_CONSTANT_NOT;
        }
        return_type = return_type.min(child_type);
    }

    if return_type > VUE3_CONSTANT_CAN_SKIP_PATCH {
        for prop in node
            .get("props")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            if json_node_type(prop) == Some(7)
                && json_str(prop, "name") == Some("bind")
                && prop.get("exp").is_some_and(|exp| !exp.is_null())
            {
                let exp_type = vue3_constant_type(prop.get("exp").unwrap_or(&Value::Null), context);
                if exp_type == VUE3_CONSTANT_NOT {
                    return VUE3_CONSTANT_NOT;
                }
                return_type = return_type.min(exp_type);
            }
        }
    }

    if json_bool(codegen_node, "isBlock")
        && node
            .get("props")
            .and_then(Value::as_array)
            .is_some_and(|props| props.iter().any(|prop| json_node_type(prop) == Some(7)))
    {
        return VUE3_CONSTANT_NOT;
    }

    return_type
}

pub(crate) fn vue3_compound_constant_type(node: &Value, context: &Value) -> u8 {
    let mut return_type = VUE3_CONSTANT_CAN_STRINGIFY;
    for child in node
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        if child.is_string() {
            continue;
        }
        let child_type = vue3_constant_type(child, context);
        if child_type == VUE3_CONSTANT_NOT {
            return VUE3_CONSTANT_NOT;
        }
        return_type = return_type.min(child_type);
    }
    return_type
}

pub(crate) fn vue3_generated_props_constant_type(node: &Value, context: &Value) -> u8 {
    let Some(props) = node
        .get("codegenNode")
        .and_then(|codegen| codegen.get("props"))
    else {
        return VUE3_CONSTANT_CAN_STRINGIFY;
    };
    if props.is_null() {
        return VUE3_CONSTANT_CAN_STRINGIFY;
    }
    if json_node_type(props) != Some(15) {
        return VUE3_CONSTANT_NOT;
    }
    let mut return_type = VUE3_CONSTANT_CAN_STRINGIFY;
    for prop in props
        .get("properties")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        let key_type = prop
            .get("key")
            .map(|key| vue3_constant_type(key, context))
            .unwrap_or(VUE3_CONSTANT_NOT);
        if key_type == VUE3_CONSTANT_NOT {
            return VUE3_CONSTANT_NOT;
        }
        return_type = return_type.min(key_type);

        let value = prop.get("value").unwrap_or(&Value::Null);
        let value_type = if json_node_type(value) == Some(4) {
            vue3_constant_type(value, context)
        } else if json_node_type(value) == Some(14) {
            vue3_helper_call_constant_type(value, context)
        } else {
            VUE3_CONSTANT_NOT
        };
        if value_type == VUE3_CONSTANT_NOT {
            return VUE3_CONSTANT_NOT;
        }
        return_type = return_type.min(value_type);
    }
    return_type
}

pub(crate) fn vue3_helper_call_constant_type(value: &Value, context: &Value) -> u8 {
    if json_node_type(value) != Some(14) || !vue3_allow_hoisted_helper_call(value) {
        return VUE3_CONSTANT_NOT;
    }
    let Some(arg) = value
        .get("arguments")
        .and_then(Value::as_array)
        .and_then(|arguments| arguments.first())
    else {
        return VUE3_CONSTANT_NOT;
    };
    if json_node_type(arg) == Some(4) {
        vue3_constant_type(arg, context)
    } else if json_node_type(arg) == Some(14) {
        vue3_helper_call_constant_type(arg, context)
    } else {
        VUE3_CONSTANT_NOT
    }
}

pub(crate) fn vue3_allow_hoisted_helper_call(value: &Value) -> bool {
    value
        .get("callee")
        .and_then(Value::as_str)
        .and_then(public_helper_by_name)
        .is_some_and(|helper| {
            matches!(
                helper,
                RuntimeHelper::Vue3NormalizeClass
                    | RuntimeHelper::Vue3NormalizeStyle
                    | RuntimeHelper::Vue3NormalizeProps
                    | RuntimeHelper::Vue3GuardReactiveProps
            )
        })
}

pub(crate) fn vue3_should_downgrade_static_block(node: &Value) -> bool {
    let Some(codegen_node) = node.get("codegenNode") else {
        return false;
    };
    json_bool(codegen_node, "isBlock")
        && matches!(
            json_str(node, "tag"),
            Some("svg" | "foreignObject" | "math")
        )
        && !node
            .get("props")
            .and_then(Value::as_array)
            .is_some_and(|props| props.iter().any(|prop| json_node_type(prop) == Some(7)))
}

pub(crate) fn vue3_single_element_root(children: &[Value]) -> Option<&Value> {
    let non_comments = children
        .iter()
        .filter(|child| json_node_type(child) != Some(3))
        .collect::<Vec<_>>();
    match non_comments.as_slice() {
        [node] if json_node_type(node) == Some(1) && json_u64(node, "tagType") != Some(2) => {
            Some(*node)
        }
        _ => None,
    }
}

pub(crate) fn vue3_path_child(path: &[String], index: usize) -> Vec<String> {
    let mut out = path.to_vec();
    out.push(index.to_string());
    out
}

pub(crate) fn vue3_path_push(path: &[String], key: &str) -> Vec<String> {
    let mut out = path.to_vec();
    out.push(key.to_string());
    out
}

pub(crate) fn vue3_parent_path(path: &[String]) -> Vec<String> {
    let mut out = path.to_vec();
    out.pop();
    out.pop();
    out
}

pub(crate) fn vue3_codegen_path(path: &[String]) -> Vec<String> {
    vue3_path_push(path, "codegenNode")
}

pub(crate) fn vue3_template_slot_projection(node: &Value) -> Option<Value> {
    let dir = node
        .get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| json_str(prop, "name") == Some("slot"))?;
    let arg = dir.get("arg")?;
    if json_bool(arg, "isStatic") {
        Some(json!({
            "kind": "static",
            "name": json_str(arg, "content").unwrap_or("default"),
        }))
    } else {
        Some(json!({
            "kind": "dynamic",
            "node": arg,
        }))
    }
}

pub(crate) fn vue3_codegen_has_object_slots(codegen_node: &Value) -> bool {
    codegen_node
        .get("children")
        .is_some_and(|children| json_node_type(children) == Some(15))
}

pub(crate) fn vue3_slot_returns_len(codegen_node: &Value, slot: &Value) -> Option<usize> {
    let properties = codegen_node
        .get("children")?
        .get("properties")?
        .as_array()?;
    let property = properties
        .iter()
        .find(|property| vue3_slot_property_matches(property, slot))?;
    property
        .get("value")?
        .get("returns")?
        .as_array()
        .map(Vec::len)
}

pub(crate) fn vue3_slot_property_matches(property: &Value, slot: &Value) -> bool {
    let Some(key) = property.get("key") else {
        return false;
    };
    if json_str(slot, "kind") == Some("static") {
        let name = json_str(slot, "name").unwrap_or("default");
        return json_str(key, "content") == Some(name);
    }
    if json_str(slot, "kind") == Some("dynamic") {
        return property.get("key") == slot.get("node");
    }
    false
}
