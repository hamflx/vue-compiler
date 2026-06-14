use crate::*;

pub(crate) fn root_single_child_codegen_projection(children: &[Value]) -> Value {
    if let Some((index, child)) = single_element_root(children) {
        if child
            .get("codegenNode")
            .is_some_and(|value| !value.is_null())
        {
            return json!({
                "kind": "childCodegen",
                "index": index,
                "asBlock": child
                    .get("codegenNode")
                    .and_then(json_node_type)
                    == Some(13),
            });
        }
    }
    json!({ "kind": "child", "index": 0 })
}

pub(crate) fn single_element_root(children: &[Value]) -> Option<(usize, &Value)> {
    let mut element = None;
    for (index, child) in children.iter().enumerate() {
        if json_node_type(child) == Some(3) {
            continue;
        }
        if json_node_type(child) != Some(1) || json_u64(child, "tagType") == Some(2) {
            return None;
        }
        if element.replace((index, child)).is_some() {
            return None;
        }
    }
    element
}

pub(crate) fn root_fragment_patch_flag(children: &[Value]) -> u16 {
    let visible = children
        .iter()
        .filter(|child| json_node_type(child) != Some(3))
        .count();
    if visible == 1
        && children
            .iter()
            .any(|child| json_node_type(child) == Some(3))
    {
        64 | 2048
    } else {
        64
    }
}

pub(crate) fn json_node_type(value: &Value) -> Option<u64> {
    json_u64(value, "type")
}

pub(crate) fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

pub(crate) fn json_usize(value: &Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
}

pub(crate) const VUE3_CONSTANT_NOT: u8 = 0;
pub(crate) const VUE3_CONSTANT_CAN_SKIP_PATCH: u8 = 1;
pub(crate) const VUE3_CONSTANT_CAN_CACHE: u8 = 2;
pub(crate) const VUE3_CONSTANT_CAN_STRINGIFY: u8 = 3;

/// Projects Vue 3 `getConstantType` behavior for public bridge callers.
pub fn get_constant_type_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    json!({
        "constantType": vue3_constant_type(node, context),
    })
}

/// Projects Vue 3 `isMemberExpression` behavior for public bridge callers.
pub fn is_member_expression_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let source = model_expression_source(node);
    let mode = json_str(payload, "mode").unwrap_or("node");
    let is_member = if mode == "browser" {
        transform_on_is_member_expression_lexer(&source)
    } else {
        transform_on_is_member_expression(&source, context)
    };
    json!({
        "isMemberExpression": is_member,
    })
}

/// Projects function-type detection for public bridge callers.
pub fn is_function_type_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let is_function_type = js_ast_type(node).is_some_and(js_ast_is_function_type);
    json!({
        "isFunctionType": is_function_type,
    })
}

/// Projects `advancePositionWithClone` source-location behavior.
pub fn advance_position_with_clone_projection(payload: &Value) -> Value {
    let pos = payload.get("pos").unwrap_or(&Value::Null);
    let source = json_str(payload, "source").unwrap_or("");
    let count =
        json_usize(payload, "numberOfCharacters").unwrap_or_else(|| source.encode_utf16().count());
    advance_position_value(pos, source, count)
}

/// Projects `advancePositionWithMutation` source-location behavior.
pub fn advance_position_with_mutation_projection(payload: &Value) -> Value {
    advance_position_with_clone_projection(payload)
}

/// Projects Vue 3 `toValidAssetId` behavior.
pub fn to_valid_asset_id_projection(payload: &Value) -> Value {
    let name = json_str(payload, "name").unwrap_or("");
    let asset_type = json_str(payload, "type").unwrap_or("");
    json!({
        "id": to_valid_asset_id(name, asset_type),
    })
}

/// Projects identifier extraction for public AST utility callers.
pub fn extract_identifiers_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let mut identifiers = Vec::new();
    js_ast_extract_identifiers(node, &mut Vec::new(), &mut identifiers);
    json!({
        "identifiers": identifiers,
    })
}

pub(crate) fn advance_position_value(
    pos: &Value,
    source: &str,
    number_of_characters: usize,
) -> Value {
    let mut offset = json_usize(pos, "offset").unwrap_or_default();
    let mut line = json_usize(pos, "line").unwrap_or(1);
    let mut column = json_usize(pos, "column").unwrap_or(1);
    let utf16 = source.encode_utf16().collect::<Vec<_>>();
    let count = number_of_characters.min(utf16.len());
    let mut lines_count = 0usize;
    let mut last_newline_pos = None;
    for (index, unit) in utf16.iter().take(count).enumerate() {
        if *unit == b'\n' as u16 {
            lines_count += 1;
            last_newline_pos = Some(index);
        }
    }
    offset += number_of_characters;
    line += lines_count;
    column = match last_newline_pos {
        Some(index) => number_of_characters - index,
        None => column + number_of_characters,
    };
    json!({
        "offset": offset,
        "line": line,
        "column": column,
    })
}

/// Projects static-property detection for public AST utility callers.
pub fn is_static_property_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    json!({
        "isStaticProperty": js_ast_is_static_property(node),
    })
}

/// Projects destructure-assignment detection for public AST utility callers.
pub fn is_in_destructure_assignment_projection(payload: &Value) -> Value {
    let parent = payload.get("parent").unwrap_or(&Value::Null);
    let parent_stack = payload
        .get("parentStack")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    json!({
        "isInDestructureAssignment": js_ast_is_in_destructure_assignment(parent, &parent_stack),
    })
}

/// Projects referenced-identifier detection for public AST utility callers.
pub fn is_referenced_identifier_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let parent = payload.get("parent").unwrap_or(&Value::Null);
    let parent_stack = payload
        .get("parentStack")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let relation = json_str(payload, "relation");
    json!({
        "isReferencedIdentifier": js_ast_is_referenced_identifier(
            node,
            parent,
            &parent_stack,
            relation,
        ),
    })
}

/// Projects identifier walking for public AST utility callers.
pub fn walk_identifiers_projection(payload: &Value) -> Value {
    let root = payload.get("root").unwrap_or(&Value::Null);
    let include_all = json_bool(payload, "includeAll");
    let mut known_ids = payload
        .get("knownIds")
        .and_then(Value::as_object)
        .map(|known| {
            known
                .iter()
                .map(|(name, count)| (name.clone(), count.as_i64().unwrap_or_default()))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut events = Vec::new();
    let mut parent_stack = Vec::new();
    js_ast_walk_identifiers(
        root,
        Vec::new(),
        None,
        None,
        &mut parent_stack,
        include_all,
        &mut known_ids,
        &mut events,
    );
    json!({
        "identifiers": events,
        "knownIds": known_ids,
    })
}

pub(crate) fn js_ast_extract_identifiers(
    node: &Value,
    path: &mut Vec<Value>,
    out: &mut Vec<Value>,
) {
    if let Some(items) = node.as_array() {
        for (index, item) in items.iter().enumerate() {
            path.push(json!(index));
            js_ast_extract_identifiers(item, path, out);
            path.pop();
        }
        return;
    }

    match js_ast_type(node) {
        Some("Identifier") => {
            out.push(json!({
                "name": json_str(node, "name").unwrap_or(""),
                "start": json_u64(node, "start"),
                "end": json_u64(node, "end"),
                "path": path.clone(),
            }));
        }
        Some("MemberExpression" | "OptionalMemberExpression") => {
            path.push(json!("object"));
            if let Some(object) = node.get("object") {
                js_ast_extract_identifiers(object, path, out);
            }
            path.pop();
        }
        Some("ObjectPattern") => {
            if let Some(properties) = node.get("properties").and_then(Value::as_array) {
                for (index, prop) in properties.iter().enumerate() {
                    path.push(json!("properties"));
                    path.push(json!(index));
                    if js_ast_type(prop) == Some("RestElement") {
                        path.push(json!("argument"));
                        if let Some(argument) = prop.get("argument") {
                            js_ast_extract_identifiers(argument, path, out);
                        }
                        path.pop();
                    } else {
                        path.push(json!("value"));
                        if let Some(value) = prop.get("value") {
                            js_ast_extract_identifiers(value, path, out);
                        }
                        path.pop();
                    }
                    path.pop();
                    path.pop();
                }
            }
        }
        Some("ObjectProperty" | "Property") => {
            path.push(json!("value"));
            if let Some(value) = node.get("value") {
                js_ast_extract_identifiers(value, path, out);
            }
            path.pop();
        }
        Some("ArrayPattern") => {
            if let Some(elements) = node.get("elements").and_then(Value::as_array) {
                for (index, element) in elements.iter().enumerate() {
                    if element.is_null() {
                        continue;
                    }
                    path.push(json!("elements"));
                    path.push(json!(index));
                    js_ast_extract_identifiers(element, path, out);
                    path.pop();
                    path.pop();
                }
            }
        }
        Some("RestElement") => {
            path.push(json!("argument"));
            if let Some(argument) = node.get("argument") {
                js_ast_extract_identifiers(argument, path, out);
            }
            path.pop();
        }
        Some("AssignmentPattern") => {
            path.push(json!("left"));
            if let Some(left) = node.get("left") {
                js_ast_extract_identifiers(left, path, out);
            }
            path.pop();
        }
        Some("TSParameterProperty") => {
            path.push(json!("parameter"));
            if let Some(parameter) = node.get("parameter") {
                js_ast_extract_identifiers(parameter, path, out);
            }
            path.pop();
        }
        Some(kind) if js_ast_is_ts_expression_wrapper(kind) => {
            path.push(json!("expression"));
            if let Some(expression) = node.get("expression") {
                js_ast_extract_identifiers(expression, path, out);
            }
            path.pop();
        }
        _ => {}
    }
}

#[derive(Clone)]
pub(crate) struct JsAstAncestor<'a> {
    pub(crate) node: &'a Value,
    pub(crate) path: Vec<Value>,
}

pub(crate) fn js_ast_walk_identifiers<'a>(
    node: &'a Value,
    path: Vec<Value>,
    parent: Option<&'a Value>,
    relation: Option<&str>,
    parent_stack: &mut Vec<JsAstAncestor<'a>>,
    include_all: bool,
    known_ids: &mut BTreeMap<String, i64>,
    events: &mut Vec<Value>,
) {
    if node.is_null() {
        return;
    }
    if let Some(parent_type) = parent.and_then(js_ast_type) {
        if parent_type.starts_with("TS") && !js_ast_is_ts_expression_wrapper(parent_type) {
            return;
        }
    }

    let mut scope_ids = js_ast_scope_identifiers(node);
    scope_ids.sort();
    scope_ids.dedup();
    for name in &scope_ids {
        *known_ids.entry(name.clone()).or_insert(0) += 1;
    }

    if js_ast_type(node) == Some("Identifier") {
        let name = json_str(node, "name").unwrap_or("");
        if name != "arguments" {
            let is_local = known_ids.get(name).copied().unwrap_or_default() > 0;
            let stack_nodes = parent_stack
                .iter()
                .map(|ancestor| ancestor.node.clone())
                .collect::<Vec<_>>();
            let is_refed = parent.map_or(true, |parent| {
                js_ast_is_referenced_identifier(node, parent, &stack_nodes, relation)
            });
            if include_all || (is_refed && !is_local) {
                let parent_path = parent_stack.last().map(|ancestor| ancestor.path.clone());
                let parent_stack_paths = parent_stack
                    .iter()
                    .map(|ancestor| ancestor.path.clone())
                    .collect::<Vec<_>>();
                events.push(json!({
                    "name": name,
                    "start": json_u64(node, "start"),
                    "end": json_u64(node, "end"),
                    "path": path,
                    "parentPath": parent_path,
                    "parentStackPaths": parent_stack_paths,
                    "isReferenced": is_refed,
                    "isLocal": is_local,
                }));
            }
        }
    } else {
        let children = js_ast_child_entries(node);
        parent_stack.push(JsAstAncestor {
            node,
            path: path.clone(),
        });
        for (child_relation, child_path, child) in children {
            let mut full_child_path = path.clone();
            full_child_path.extend(child_path);
            js_ast_walk_identifiers(
                child,
                full_child_path,
                Some(node),
                Some(&child_relation),
                parent_stack,
                include_all,
                known_ids,
                events,
            );
        }
        parent_stack.pop();
    }

    for name in scope_ids {
        if let Some(count) = known_ids.get_mut(&name) {
            *count -= 1;
            if *count <= 0 {
                known_ids.remove(&name);
            }
        }
    }
}

pub(crate) fn js_ast_child_entries<'a>(node: &'a Value) -> Vec<(String, Vec<Value>, &'a Value)> {
    const KEYS: &[&str] = &[
        "body",
        "declarations",
        "declaration",
        "id",
        "init",
        "test",
        "update",
        "left",
        "right",
        "argument",
        "arguments",
        "callee",
        "object",
        "property",
        "properties",
        "key",
        "value",
        "elements",
        "expressions",
        "params",
        "consequent",
        "alternate",
        "cases",
        "discriminant",
        "handler",
        "finalizer",
        "block",
        "param",
        "specifiers",
        "local",
        "imported",
        "source",
        "superClass",
        "quasi",
        "tag",
        "expression",
    ];

    let mut out = Vec::new();
    for key in KEYS {
        let Some(value) = node.get(*key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for (index, item) in items.iter().enumerate() {
                if item.is_object() {
                    out.push(((*key).to_string(), vec![json!(*key), json!(index)], item));
                }
            }
        } else if value.is_object() {
            out.push(((*key).to_string(), vec![json!(*key)], value));
        }
    }
    out
}

pub(crate) fn js_ast_scope_identifiers(node: &Value) -> Vec<String> {
    match js_ast_type(node) {
        Some(kind) if js_ast_is_function_type(kind) => node
            .get("params")
            .map(js_ast_extract_identifier_names)
            .unwrap_or_default(),
        Some("BlockStatement") | Some("SwitchCase") => js_ast_walk_block_declaration_names(node),
        Some("SwitchStatement") => js_ast_walk_switch_statement_names(node, false),
        Some("CatchClause") => node
            .get("param")
            .map(js_ast_extract_identifier_names)
            .unwrap_or_default(),
        Some("ForStatement" | "ForOfStatement" | "ForInStatement") => {
            js_ast_walk_for_statement_names(node, false)
        }
        _ => Vec::new(),
    }
}

pub(crate) fn js_ast_walk_block_declaration_names(block: &Value) -> Vec<String> {
    let body = if js_ast_type(block) == Some("SwitchCase") {
        block.get("consequent")
    } else {
        block.get("body")
    };
    let Some(body) = body.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for stmt in body {
        match js_ast_type(stmt) {
            Some("VariableDeclaration") if !json_bool(stmt, "declare") => {
                if let Some(declarations) = stmt.get("declarations").and_then(Value::as_array) {
                    for decl in declarations {
                        if let Some(id) = decl.get("id") {
                            names.extend(js_ast_extract_identifier_names(id));
                        }
                    }
                }
            }
            Some("FunctionDeclaration" | "ClassDeclaration") => {
                if !json_bool(stmt, "declare") {
                    if let Some(id) = stmt.get("id") {
                        if let Some(name) = json_str(id, "name") {
                            names.push(name.to_string());
                        }
                    }
                }
            }
            Some("ForStatement" | "ForOfStatement" | "ForInStatement") => {
                names.extend(js_ast_walk_for_statement_names(stmt, true));
            }
            Some("SwitchStatement") => names.extend(js_ast_walk_switch_statement_names(stmt, true)),
            _ => {}
        }
    }
    names
}

pub(crate) fn js_ast_walk_for_statement_names(stmt: &Value, is_var: bool) -> Vec<String> {
    let variable = if js_ast_type(stmt) == Some("ForStatement") {
        stmt.get("init")
    } else {
        stmt.get("left")
    };
    let Some(variable) = variable else {
        return Vec::new();
    };
    if js_ast_type(variable) != Some("VariableDeclaration") {
        return Vec::new();
    }
    let kind_is_var = json_str(variable, "kind") == Some("var");
    if kind_is_var != is_var {
        return Vec::new();
    }
    let mut names = Vec::new();
    if let Some(declarations) = variable.get("declarations").and_then(Value::as_array) {
        for decl in declarations {
            if let Some(id) = decl.get("id") {
                names.extend(js_ast_extract_identifier_names(id));
            }
        }
    }
    names
}

pub(crate) fn js_ast_walk_switch_statement_names(stmt: &Value, is_var: bool) -> Vec<String> {
    let Some(cases) = stmt.get("cases").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for case in cases {
        if let Some(consequent) = case.get("consequent").and_then(Value::as_array) {
            for stmt in consequent {
                if js_ast_type(stmt) == Some("VariableDeclaration")
                    && (json_str(stmt, "kind") == Some("var")) == is_var
                {
                    if let Some(declarations) = stmt.get("declarations").and_then(Value::as_array) {
                        for decl in declarations {
                            if let Some(id) = decl.get("id") {
                                names.extend(js_ast_extract_identifier_names(id));
                            }
                        }
                    }
                }
            }
        }
        names.extend(js_ast_walk_block_declaration_names(case));
    }
    names
}

pub(crate) fn js_ast_extract_identifier_names(node: &Value) -> Vec<String> {
    let mut identifiers = Vec::new();
    js_ast_extract_identifiers(node, &mut Vec::new(), &mut identifiers);
    identifiers
        .into_iter()
        .filter_map(|ident| {
            ident
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

pub(crate) fn js_ast_is_static_property(node: &Value) -> bool {
    matches!(
        js_ast_type(node),
        Some("ObjectProperty" | "ObjectMethod" | "Property")
    ) && !json_bool(node, "computed")
}

pub(crate) fn js_ast_is_referenced_identifier(
    id: &Value,
    parent: &Value,
    parent_stack: &[Value],
    relation: Option<&str>,
) -> bool {
    if parent.is_null() {
        return true;
    }
    if json_str(id, "name") == Some("arguments") {
        return false;
    }
    let grandparent = parent_stack
        .iter()
        .rev()
        .find(|ancestor| *ancestor != parent);
    if js_ast_is_referenced(parent, grandparent, relation) {
        return true;
    }
    match js_ast_type(parent) {
        Some("AssignmentExpression" | "AssignmentPattern") => true,
        Some("ObjectProperty" | "Property") => {
            relation != Some("key") && js_ast_is_in_destructure_assignment(parent, parent_stack)
        }
        Some("ArrayPattern") => js_ast_is_in_destructure_assignment(parent, parent_stack),
        _ => false,
    }
}

pub(crate) fn js_ast_is_referenced(
    parent: &Value,
    grandparent: Option<&Value>,
    relation: Option<&str>,
) -> bool {
    match js_ast_type(parent) {
        Some("MemberExpression" | "OptionalMemberExpression") => {
            if relation == Some("property") {
                json_bool(parent, "computed")
            } else {
                relation == Some("object")
            }
        }
        Some("JSXMemberExpression") => relation == Some("object"),
        Some("VariableDeclarator") => relation == Some("init"),
        Some("ArrowFunctionExpression") => relation == Some("body"),
        Some("PrivateName") => false,
        Some("ClassMethod" | "ClassPrivateMethod" | "ObjectMethod") => {
            relation == Some("key") && json_bool(parent, "computed")
        }
        Some("ObjectProperty" | "Property") => {
            if relation == Some("key") {
                json_bool(parent, "computed")
            } else {
                grandparent.and_then(js_ast_type) != Some("ObjectPattern")
            }
        }
        Some("ClassProperty" | "PropertyDefinition") => {
            relation != Some("key") || json_bool(parent, "computed")
        }
        Some("ClassPrivateProperty") => relation != Some("key"),
        Some("ClassDeclaration" | "ClassExpression") => relation == Some("superClass"),
        Some("AssignmentExpression") => relation == Some("right"),
        Some("AssignmentPattern") => relation == Some("right"),
        Some(
            "LabeledStatement"
            | "CatchClause"
            | "RestElement"
            | "BreakStatement"
            | "ContinueStatement"
            | "FunctionDeclaration"
            | "FunctionExpression"
            | "ExportNamespaceSpecifier"
            | "ExportDefaultSpecifier"
            | "ImportDefaultSpecifier"
            | "ImportNamespaceSpecifier"
            | "ImportSpecifier"
            | "ImportAttribute"
            | "JSXAttribute"
            | "ObjectPattern"
            | "ArrayPattern"
            | "MetaProperty"
            | "PrivateIdentifier",
        ) => false,
        Some("ExportSpecifier") => {
            if grandparent
                .and_then(|node| node.get("source"))
                .is_some_and(|source| !source.is_null())
            {
                false
            } else {
                relation == Some("local")
            }
        }
        Some("ObjectTypeProperty") => relation != Some("key"),
        Some("TSEnumMember") => relation != Some("id"),
        Some("TSPropertySignature") => relation != Some("key") || json_bool(parent, "computed"),
        _ => true,
    }
}

pub(crate) fn js_ast_is_in_destructure_assignment(parent: &Value, parent_stack: &[Value]) -> bool {
    if !matches!(
        js_ast_type(parent),
        Some("ObjectProperty" | "Property" | "ArrayPattern")
    ) {
        return false;
    }
    for ancestor in parent_stack.iter().rev() {
        match js_ast_type(ancestor) {
            Some("AssignmentExpression") => return true,
            Some("ObjectProperty" | "Property") => {}
            Some(kind) if kind.ends_with("Pattern") => {}
            _ => break,
        }
    }
    false
}

pub(crate) fn js_ast_type(node: &Value) -> Option<&str> {
    json_str(node, "type")
}

pub(crate) fn js_ast_is_ts_expression_wrapper(kind: &str) -> bool {
    matches!(
        kind,
        "TSAsExpression"
            | "TSTypeAssertion"
            | "TSNonNullExpression"
            | "TSInstantiationExpression"
            | "TSSatisfiesExpression"
    )
}

pub(crate) fn js_ast_is_function_type(kind: &str) -> bool {
    kind.ends_with("FunctionExpression")
        || kind == "FunctionDeclaration"
        || kind.ends_with("Method")
}

/// Projects Rust-backed `processExpression` behavior for bridge callers.
pub fn process_expression_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let raw = json_str(node, "content").unwrap_or("");
    let as_params = json_bool(payload, "asParams");
    let as_raw_statements = json_bool(payload, "asRawStatements");
    if json_node_type(node) != Some(4)
        || json_bool(node, "isStatic")
        || !json_bool(context, "prefixIdentifiers")
        || raw.trim().is_empty()
    {
        return json!({ "kind": "unchanged" });
    }

    if process_expression_is_static_literal(raw) {
        return json!({
            "kind": "setConstType",
            "constType": 3,
        });
    }

    let options = vue3_options_from_transform_context(context);
    let locals = process_expression_locals(payload, context);
    if as_params {
        if is_simple_identifier_ascii(raw) {
            return json!({
                "kind": "setConstType",
                "constType": 2,
            });
        }
        return process_expression_params_projection(raw, node, context, &options);
    }
    let literal = matches!(raw, "true" | "false" | "null" | "this");
    if is_simple_identifier_ascii(raw) {
        let is_local = locals.iter().any(|local| local == raw);
        let is_global = is_global_or_literal(raw);
        if !as_params
            && !is_local
            && !literal
            && (!is_global || options.binding_metadata.contains_key(raw))
        {
            let content =
                process_expression_rewrite_identifier(raw, &options, None, None, false, &[]);
            return json!({
                "kind": "simple",
                "content": content,
                "isStatic": false,
                "constType": if process_expression_is_const_binding(raw, &options) { 1 } else { 0 },
                "loc": node.get("loc").cloned().unwrap_or(Value::Null),
                "helpers": vue3_for_helpers_for_content(&content),
            });
        }
        if !is_local {
            return json!({
                "kind": "setConstType",
                "constType": if literal { 3 } else { 2 },
            });
        }
        return json!({ "kind": "unchanged" });
    }

    let source = if as_raw_statements {
        format!(" {raw} ")
    } else {
        format!("({raw}){}", if as_params { "=>{}" } else { "" })
    };
    let store = JsAstStore::new();
    let parse_ok = if process_expression_uses_supported_external_plugin(raw, context) {
        true
    } else if as_raw_statements {
        let parsed = store.parse_program(&source, transform_on_source_type(context));
        !parsed.panicked && parsed.errors.is_empty()
    } else {
        store
            .parse_expression(&source, transform_on_source_type(context))
            .is_ok()
    };
    if !parse_ok {
        return json!({
            "kind": "error",
            "code": 46,
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
            "message": "Error parsing JavaScript expression: Unexpected token",
        });
    }

    let mut effective_locals = locals;
    if !as_raw_statements {
        effective_locals.extend(transform_on_root_function_locals(raw));
        effective_locals.sort();
        effective_locals.dedup();
    }
    let children = process_expression_compound_children(
        raw,
        &options,
        &effective_locals,
        node.get("loc").unwrap_or(&Value::Null),
    );
    if children.is_empty() {
        return json!({
            "kind": "setConstType",
            "constType": 3,
        });
    }
    let rewritten = process_expression_rewrite_source(raw, &options, &effective_locals);
    let mut helper_source = rewritten.clone();
    for child in &children {
        if let Some(content) = child.get("content").and_then(Value::as_str) {
            helper_source.push_str(content);
        }
    }
    json!({
        "kind": "compound",
        "children": children,
        "loc": node.get("loc").cloned().unwrap_or(Value::Null),
        "identifiers": effective_locals,
        "helpers": vue3_for_helpers_for_content(&helper_source),
    })
}

/// Projects Rust-backed `transformExpression` behavior for bridge callers.
pub fn transform_expression_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let mut operations = Vec::<Value>::new();
    match json_node_type(node) {
        Some(5) => {
            let Some(content) = node.get("content") else {
                return json!({ "operations": operations });
            };
            operations.push(json!({
                "kind": "process",
                "path": ["content"],
                "projection": process_expression_projection(&json!({
                    "node": content,
                    "context": context,
                })),
            }));
        }
        Some(1) => {
            let memo_index = node
                .get("props")
                .and_then(Value::as_array)
                .and_then(|props| {
                    props.iter().position(|prop| {
                        json_node_type(prop) == Some(7) && json_str(prop, "name") == Some("memo")
                    })
                });
            for (index, dir) in node
                .get("props")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .enumerate()
            {
                if json_node_type(dir) != Some(7) || json_str(dir, "name") == Some("for") {
                    continue;
                }
                let arg = dir.get("arg").unwrap_or(&Value::Null);
                if let Some(exp) = dir.get("exp").filter(|exp| json_node_type(exp) == Some(4)) {
                    let skip_on_arg = json_str(dir, "name") == Some("on") && !arg.is_null();
                    let skip_memo_key = memo_index.is_some()
                        && json_node_type(arg) == Some(4)
                        && json_str(arg, "content") == Some("key");
                    if !skip_on_arg && !skip_memo_key {
                        operations.push(json!({
                            "kind": "process",
                            "path": ["props", index.to_string(), "exp"],
                            "projection": process_expression_projection(&json!({
                                "node": exp,
                                "context": context,
                                "asParams": json_str(dir, "name") == Some("slot"),
                            })),
                        }));
                    }
                }
                if json_node_type(arg) == Some(4) && !json_bool(arg, "isStatic") {
                    operations.push(json!({
                        "kind": "process",
                        "path": ["props", index.to_string(), "arg"],
                        "projection": process_expression_projection(&json!({
                            "node": arg,
                            "context": context,
                        })),
                    }));
                }
            }
        }
        _ => {}
    }
    json!({ "operations": operations })
}

/// Projects Rust-backed `transformOnce` behavior for bridge callers.
pub fn transform_once_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    if json_node_type(node) != Some(1)
        || vue3_directive(node, "once", true).is_none()
        || json_bool(payload, "seen")
        || json_bool(context, "inVOnce")
        || json_bool(context, "inSSR")
    {
        return json!({ "kind": "noop" });
    }
    json!({
        "kind": "enter",
        "helper": "SET_BLOCK_TRACKING",
        "markSeen": true,
        "enterInVOnce": true,
        "exit": {
            "restoreInVOnce": false,
            "cacheCodegen": true,
            "isVNode": true,
            "inVOnce": true,
        }
    })
}

/// Projects Rust-backed `transformMemo` behavior for bridge callers.
pub fn transform_memo_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let Some(dir) = vue3_directive(node, "memo", false) else {
        return json!({ "kind": "noop" });
    };
    if json_node_type(node) != Some(1) || json_bool(payload, "seen") || json_bool(context, "inSSR")
    {
        return json!({ "kind": "noop" });
    }
    json!({
        "kind": "enter",
        "markSeen": true,
        "exit": {
            "wrapMemo": true,
            "convertToBlock": json_u64(node, "tagType") != Some(1),
            "helper": "WITH_MEMO",
            "exp": dir.get("exp").cloned().unwrap_or(Value::Null),
            "cacheIndex": json_u64(context, "cachedLength").unwrap_or(0),
        }
    })
}

/// Projects Rust-backed static-cache analysis for bridge callers.
pub fn cache_static_projection(payload: &Value) -> Value {
    let root = payload.get("root").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let do_not_hoist_root = vue3_single_element_root(children).is_some();
    let mut state = Vue3CacheStaticState::default();
    vue3_cache_static_walk(
        children,
        vec!["children".to_string()],
        None,
        root,
        context,
        do_not_hoist_root,
        &mut state,
    );
    json!({
        "operations": state.operations,
    })
}

/// Projects Rust-backed `stringifyStatic` transform-hoist behavior for public AST callers.
pub fn stringify_static_projection(payload: &Value) -> Value {
    let children = payload
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let context = payload.get("context").unwrap_or(&Value::Null);
    let parent = payload.get("parent").unwrap_or(&Value::Null);
    if json_usize(
        context
            .get("scopes")
            .unwrap_or_else(|| context.get("scope").unwrap_or(&Value::Null)),
        "vSlot",
    )
    .unwrap_or(0)
        > 0
    {
        return json!({ "operations": [] });
    }

    let is_parent_cached = vue3_stringify_parent_is_cached(parent);
    let mut virtual_children = (0..children.len())
        .map(Vue3StringifyVirtualChild::Original)
        .collect::<Vec<_>>();
    let mut operations = Vec::new();
    let mut current_chunk = Vec::<StaticHtmlAnalysis>::new();
    let mut index = 0usize;
    while index < virtual_children.len() {
        let child = match virtual_children[index] {
            Vue3StringifyVirtualChild::Original(original) => children.get(original),
            Vue3StringifyVirtualChild::StaticCall => None,
        };
        if let Some(child) = child {
            let is_cached = is_parent_cached || vue3_stringify_cached_node(child).is_some();
            if is_cached {
                if let Some(analysis) = vue3_stringify_analyze_public_node(child, context) {
                    current_chunk.push(analysis);
                    index += 1;
                    continue;
                }
            }
        }

        let delete_count = vue3_stringify_flush_public_chunk(
            index,
            is_parent_cached,
            &mut current_chunk,
            &mut virtual_children,
            &mut operations,
        );
        current_chunk.clear();
        index = index.saturating_sub(delete_count) + 1;
    }
    vue3_stringify_flush_public_chunk(
        virtual_children.len(),
        is_parent_cached,
        &mut current_chunk,
        &mut virtual_children,
        &mut operations,
    );

    json!({ "operations": operations })
}

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

/// Projects Rust-backed `transformModel` behavior for bridge callers.
pub fn transform_model_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let Some(exp) = dir.get("exp").filter(|value| !value.is_null()) else {
        return json!({ "errors": [41], "props": [] });
    };

    let raw_exp = exp
        .get("loc")
        .and_then(|loc| loc.get("source"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| json_str(exp, "content").unwrap_or(""))
        .trim();
    let exp_string = json_str(exp, "content").unwrap_or(raw_exp);
    let binding_type = context
        .get("bindingMetadata")
        .and_then(|metadata| metadata.get(raw_exp))
        .and_then(Value::as_str);

    if matches!(binding_type, Some("props" | "props-aliased")) {
        return json!({ "errors": [44], "props": [] });
    }
    if matches!(binding_type, Some("literal-const" | "setup-const")) {
        return json!({ "errors": [45], "props": [] });
    }

    let maybe_ref = json_bool(context, "inline")
        && matches!(
            binding_type,
            Some("setup-let" | "setup-ref" | "setup-maybe-ref")
        );
    if exp_string.trim().is_empty() || (!model_is_member_expression(raw_exp) && !maybe_ref) {
        return json!({ "errors": [42], "props": [] });
    }
    if json_bool(context, "prefixIdentifiers")
        && is_simple_identifier_ascii(exp_string)
        && context_identifier_count(context, exp_string) > 0
    {
        return json!({ "errors": [43], "props": [] });
    }

    let arg = dir.get("arg").filter(|value| !value.is_null());
    let event_arg = if json_bool(context, "isTS") {
        "($event: any)"
    } else {
        "$event"
    };
    let assignment = model_assignment_projection(exp, raw_exp, event_arg, binding_type, maybe_ref);
    let mut props = vec![
        json!({
            "kind": "modelValue",
            "key": model_prop_name_projection(arg),
            "value": { "kind": "node", "path": "dir.exp" },
            "dynamic": true,
        }),
        json!({
            "kind": "modelUpdate",
            "key": model_event_name_projection(arg),
            "value": assignment,
            "cache": should_cache_model_update(exp, context),
            "dynamic": !should_cache_model_update(exp, context),
            "hydrate": model_update_needs_hydration_event(arg, node),
        }),
    ];

    if dir
        .get("modifiers")
        .and_then(Value::as_array)
        .is_some_and(|modifiers| !modifiers.is_empty())
        && json_u64(node, "tagType") == Some(1)
    {
        props.push(json!({
            "kind": "modelModifiers",
            "key": model_modifiers_key_projection(arg),
            "value": model_modifiers_expression(dir),
            "dynamic": false,
        }));
    }

    json!({
        "errors": [],
        "props": props,
    })
}

/// Projects Rust-backed `transformBind` behavior for bridge callers.
pub fn transform_bind_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let arg = dir.get("arg").filter(|value| !value.is_null());
    let mut exp = dir.get("exp").filter(|value| !value.is_null());

    if let Some(current_exp) = exp {
        if json_node_type(current_exp) == Some(4)
            && json_str(current_exp, "content")
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            if !json_bool(context, "browser") {
                return json!({
                    "errors": [{ "code": 34, "loc": "dir" }],
                    "props": [{
                        "key": transform_bind_raw_arg_projection(arg, dir),
                        "value": transform_bind_empty_expression_value(dir),
                    }],
                });
            }
            exp = None;
        }
    }

    json!({
        "errors": [],
        "props": [{
            "key": transform_bind_key_projection(arg, dir, context),
            "value": exp
                .map(|_| json!({ "kind": "node", "path": "dir.exp" }))
                .unwrap_or_else(|| json!({ "kind": "undefined" })),
        }],
    })
}

/// Projects Rust-backed v-bind shorthand behavior for bridge callers.
pub fn transform_v_bind_shorthand_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    if json_node_type(node) != Some(1) {
        return json!({ "operations": [] });
    }
    let context = payload.get("context").unwrap_or(&Value::Null);
    let operations = node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, prop)| {
            transform_v_bind_shorthand_operation(index, prop, json_bool(context, "browser"))
        })
        .collect::<Vec<_>>();

    json!({ "operations": operations })
}

/// Projects Rust-backed `transformOn` behavior for bridge callers.
pub fn transform_on_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let arg = dir.get("arg").filter(|value| !value.is_null());
    let mut errors = Vec::<Value>::new();

    if dir.get("exp").is_none_or(Value::is_null)
        && dir
            .get("modifiers")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        errors.push(json!({ "code": 35, "loc": "dir" }));
    }

    let event_name = transform_on_event_name_projection(arg, node, &mut errors);
    let handler = transform_on_handler_projection(dir, node, context);
    let cache = json_bool(&handler, "cache");
    let value = handler
        .get("value")
        .cloned()
        .unwrap_or_else(|| transform_on_empty_handler_projection(dir));

    json!({
        "errors": errors,
        "props": [{
            "key": event_name,
            "value": value,
            "cache": cache,
            "valueConstant": transform_on_projection_const_type(&value) > 0,
            "handlerKey": true,
            "dynamicKey": arg.is_some_and(|arg| !json_bool(arg, "isStatic")),
            "ignoreDynamicKeyForNormalize": true,
        }],
    })
}

/// Projects Rust-backed `transformIf` behavior for bridge callers.
pub fn transform_if_projection(payload: &Value) -> Value {
    if json_str(payload, "phase") == Some("branchCodegen") {
        return transform_if_branch_codegen_projection(payload);
    }
    transform_if_process_projection(payload)
}

/// Projects Rust-backed `transformFor` behavior for bridge callers.
pub fn transform_for_projection(payload: &Value) -> Value {
    if json_str(payload, "phase") == Some("codegen") {
        return transform_for_codegen_projection(payload);
    }
    if json_str(payload, "phase") == Some("exitCodegen") {
        return transform_for_exit_codegen_projection(payload);
    }

    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let mut errors = Vec::<Value>::new();
    let Some(exp) = dir.get("exp").filter(|value| !value.is_null()) else {
        errors.push(json!({ "code": 31, "loc": "dir" }));
        return json!({ "errors": errors });
    };
    let raw = json_str(exp, "content")
        .or_else(|| exp.get("loc").and_then(|loc| json_str(loc, "source")))
        .unwrap_or("");
    let Some(parsed) = parse_vue3_for_expression(raw) else {
        errors.push(json!({ "code": 32, "loc": "dir" }));
        return json!({ "errors": errors });
    };

    let mut source = vue3_for_expression_projection(
        &parsed.source.content,
        exp,
        parsed.source.start,
        parsed.source.end,
        Vue3ForAstMode::Expression,
    );
    let mut value = parsed.value.as_ref().map(|part| {
        vue3_for_expression_projection(
            &part.content,
            exp,
            part.start,
            part.end,
            Vue3ForAstMode::Params,
        )
    });
    let mut key = parsed.key.as_ref().map(|part| {
        vue3_for_expression_projection(
            &part.content,
            exp,
            part.start,
            part.end,
            Vue3ForAstMode::Params,
        )
    });
    let mut index = parsed.index.as_ref().map(|part| {
        vue3_for_expression_projection(
            &part.content,
            exp,
            part.start,
            part.end,
            Vue3ForAstMode::Params,
        )
    });

    if json_bool(context, "prefixIdentifiers") {
        let options = vue3_options_from_transform_context(context);
        let locals = transform_context_locals(context);
        source = vue3_for_rewrite_projection_node(
            &parsed.source.content,
            &options,
            &locals,
            source["loc"].clone(),
            Vue3ForAstMode::Expression,
            false,
        );
        let scoped = parsed
            .all_alias_locals()
            .into_iter()
            .chain(locals)
            .collect::<Vec<_>>();
        if let Some(part) = parsed.value.as_ref() {
            value = Some(vue3_for_rewrite_projection_node(
                &part.content,
                &options,
                &scoped,
                value
                    .as_ref()
                    .and_then(|node| node.get("loc"))
                    .cloned()
                    .unwrap_or(Value::Null),
                Vue3ForAstMode::Params,
                true,
            ));
        }
        if let Some(part) = parsed.key.as_ref() {
            key = Some(vue3_for_rewrite_projection_node(
                &part.content,
                &options,
                &scoped,
                key.as_ref()
                    .and_then(|node| node.get("loc"))
                    .cloned()
                    .unwrap_or(Value::Null),
                Vue3ForAstMode::Params,
                true,
            ));
        }
        if let Some(part) = parsed.index.as_ref() {
            index = Some(vue3_for_rewrite_projection_node(
                &part.content,
                &options,
                &scoped,
                index
                    .as_ref()
                    .and_then(|node| node.get("loc"))
                    .cloned()
                    .unwrap_or(Value::Null),
                Vue3ForAstMode::Params,
                true,
            ));
        }
    }

    let parse_result = json!({
        "source": source,
        "value": value,
        "key": key,
        "index": index,
        "finalized": true,
    });
    let template_key_errors = vue3_for_template_key_errors(node);

    json!({
        "errors": errors,
        "parseResult": parse_result,
        "locals": parsed.all_alias_locals(),
        "children": if json_u64(node, "tagType") == Some(3) { "template" } else { "self" },
        "templateKeyErrors": template_key_errors,
    })
}

/// Projects Rust-backed slot-scope tracking for bridge callers.
pub fn track_slot_scopes_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let Some(slot) = vue3_slot_directive(node, false) else {
        return json!({ "track": false });
    };
    let locals = slot
        .get("exp")
        .filter(|exp| !exp.is_null())
        .map(vue3_slot_param_locals)
        .unwrap_or_default();
    json!({
        "track": true,
        "slotProps": slot.get("exp").cloned().unwrap_or(Value::Null),
        "locals": locals,
    })
}

/// Projects Rust-backed `v-for` slot-scope tracking for bridge callers.
pub fn track_v_for_slot_scopes_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    if json_node_type(node) != Some(1)
        || json_u64(node, "tagType") != Some(3)
        || vue3_slot_directive(node, true).is_none()
    {
        return json!({ "track": false });
    }
    let Some(dir) = vue3_directive(node, "for", true) else {
        return json!({ "track": false });
    };
    let context = payload.get("context").unwrap_or(&Value::Null);
    let projection = vue3_for_parse_result_projection(node, dir, context);
    if projection.get("parseResult").is_none() {
        return json!({ "track": false, "errors": projection.get("errors").cloned().unwrap_or_else(|| json!([])) });
    }
    json!({
        "track": true,
        "dir": dir,
        "parseResult": projection["parseResult"].clone(),
        "locals": projection["locals"].clone(),
    })
}

/// Projects Rust-backed slot outlet transforms for bridge callers.
pub fn transform_slot_outlet_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    if json_node_type(node) != Some(1) || json_u64(node, "tagType") != Some(2) {
        return json!({ "transform": false });
    }
    let context = payload.get("context").unwrap_or(&Value::Null);
    let process = process_slot_outlet_projection(node, context);
    let has_children = node
        .get("children")
        .and_then(Value::as_array)
        .is_some_and(|children| !children.is_empty());
    let mut expected_len = 2;
    if has_children {
        expected_len = 4;
    }
    if json_str(context, "scopeId").is_some() && !json_bool(context, "slotted") {
        expected_len = 5;
    }

    json!({
        "transform": true,
        "process": process,
        "codegen": {
            "slots": if json_bool(context, "prefixIdentifiers") { "_ctx.$slots" } else { "$slots" },
            "expectedLen": expected_len,
            "hasChildren": has_children,
            "helper": "RENDER_SLOT",
        },
    })
}

pub(crate) fn process_slot_outlet_projection(node: &Value, context: &Value) -> Value {
    let mut slot_name = json!({ "kind": "literal", "value": "\"default\"" });
    let mut non_name_props = Vec::<Value>::new();
    let mut mutations = Vec::<Value>::new();

    for (index, prop) in node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        if json_node_type(prop) == Some(6) {
            if prop.get("value").is_none_or(Value::is_null) {
                continue;
            }
            if json_str(prop, "name") == Some("name") {
                let content = prop
                    .get("value")
                    .and_then(|value| json_str(value, "content"))
                    .unwrap_or("");
                slot_name = json!({ "kind": "literal", "value": quote_string(content) });
            } else {
                if let Some(name) = json_str(prop, "name") {
                    let camel = camelize(name);
                    if camel != name {
                        mutations.push(json!({
                            "kind": "setPropName",
                            "index": index,
                            "name": camel,
                        }));
                    }
                }
                non_name_props.push(json!(index));
            }
            continue;
        }

        if json_str(prop, "name") == Some("bind")
            && prop.get("arg").is_some_and(|arg| {
                json_node_type(arg) == Some(4)
                    && json_bool(arg, "isStatic")
                    && json_str(arg, "content") == Some("name")
            })
        {
            if prop.get("exp").is_some_and(|exp| !exp.is_null()) {
                slot_name =
                    json!({ "kind": "node", "path": "props", "index": index, "field": "exp" });
            } else if prop
                .get("arg")
                .is_some_and(|arg| json_node_type(arg) == Some(4))
            {
                let name = prop
                    .get("arg")
                    .and_then(|arg| json_str(arg, "content"))
                    .map(camelize)
                    .unwrap_or_default();
                let loc = prop
                    .get("arg")
                    .and_then(|arg| arg.get("loc"))
                    .cloned()
                    .unwrap_or(Value::Null);
                let exp = json!({
                    "kind": "simple",
                    "content": name,
                    "isStatic": false,
                    "loc": loc,
                });
                mutations.push(json!({
                    "kind": "setDirectiveExp",
                    "index": index,
                    "value": process_slot_outlet_maybe_process_expression(exp, context),
                }));
                slot_name =
                    json!({ "kind": "node", "path": "props", "index": index, "field": "exp" });
            }
            continue;
        }

        if json_str(prop, "name") == Some("bind")
            && prop
                .get("arg")
                .is_some_and(|arg| json_node_type(arg) == Some(4) && json_bool(arg, "isStatic"))
        {
            let content = prop
                .get("arg")
                .and_then(|arg| json_str(arg, "content"))
                .unwrap_or("");
            let camel = camelize(content);
            if camel != content {
                mutations.push(json!({
                    "kind": "setDirectiveArgContent",
                    "index": index,
                    "content": camel,
                }));
            }
        }
        non_name_props.push(json!(index));
    }

    json!({
        "slotName": slot_name,
        "nonNameProps": non_name_props,
        "mutations": mutations,
    })
}

pub(crate) fn process_slot_outlet_maybe_process_expression(node: Value, context: &Value) -> Value {
    if !json_bool(context, "prefixIdentifiers") {
        return node;
    }
    let processed = process_expression_projection(&json!({
        "node": {
            "type": 4,
            "content": json_str(&node, "content").unwrap_or(""),
            "isStatic": json_bool(&node, "isStatic"),
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
        },
        "context": context,
    }));
    match json_str(&processed, "kind") {
        Some("simple") | Some("compound") => processed,
        _ => node,
    }
}

/// Projects Rust-backed `buildSlots` behavior for bridge callers.
pub fn build_slots_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut properties = Vec::<Value>::new();
    let mut dynamic_slots = Vec::<Value>::new();
    let mut errors = Vec::<Value>::new();
    let mut has_dynamic_slots = json_usize(context, "vSlotDepth").unwrap_or_default() > 0
        || json_usize(context, "vForDepth").unwrap_or_default() > 0;

    if !json_bool(context, "ssr") && json_bool(context, "prefixIdentifiers") {
        has_dynamic_slots = vue3_component_slot_scope_ref(node, children, context);
    }

    let on_component_slot = vue3_slot_directive(node, true);
    if let Some(slot) = on_component_slot {
        if slot
            .get("arg")
            .filter(|arg| !arg.is_null())
            .is_some_and(|arg| !json_bool(arg, "isStatic"))
        {
            has_dynamic_slots = true;
        }
        properties.push(json!({
            "kind": "property",
            "key": vue3_slot_name_projection(slot, context),
            "params": slot.get("exp").cloned().unwrap_or(Value::Null),
            "indices": vue3_all_child_indices(children),
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
        }));
    }

    let mut has_template_slots = false;
    let mut has_named_default_slot = false;
    let mut implicit_default_indices = Vec::<usize>::new();
    let mut seen_slot_names = Vec::<String>::new();
    let mut conditional_branch_index = 0usize;

    for (index, child) in children.iter().enumerate() {
        let Some(slot_dir) = vue3_template_slot_directive(child) else {
            if json_node_type(child) != Some(3) {
                implicit_default_indices.push(index);
            }
            continue;
        };

        if on_component_slot.is_some() {
            errors.push(
                json!({ "code": 37, "loc": slot_dir.get("loc").cloned().unwrap_or(Value::Null) }),
            );
            break;
        }

        has_template_slots = true;
        let slot_name = vue3_slot_name_projection(slot_dir, context);
        let static_slot_name = vue3_static_slot_name(slot_dir);
        if static_slot_name.is_none() {
            has_dynamic_slots = true;
        }
        let slot = vue3_slot_function_projection(slot_dir, &[index], child);

        if let Some(if_dir) = vue3_directive(child, "if", false) {
            has_dynamic_slots = true;
            dynamic_slots.push(json!({
                "kind": "conditional",
                "test": vue3_slot_condition_projection(if_dir, context),
                "consequent": vue3_dynamic_slot_projection(slot_name, slot, Some(conditional_branch_index)),
                "alternate": vue3_default_fallback_projection(),
            }));
            conditional_branch_index += 1;
            continue;
        }

        if let Some(else_dir) = vue3_else_slot_directive(child) {
            if let Some(previous) = vue3_previous_non_comment_or_whitespace(children, index) {
                if vue3_template_has_if_like_slot_directive(previous) {
                    let alternate = if json_str(else_dir, "name") == Some("else-if") {
                        json!({
                            "kind": "conditional",
                            "test": vue3_slot_condition_projection(else_dir, context),
                            "consequent": vue3_dynamic_slot_projection(slot_name, slot, Some(conditional_branch_index)),
                            "alternate": vue3_default_fallback_projection(),
                        })
                    } else {
                        vue3_dynamic_slot_projection(
                            slot_name,
                            slot,
                            Some(conditional_branch_index),
                        )
                    };
                    vue3_append_slot_conditional_alternate(&mut dynamic_slots, alternate);
                    conditional_branch_index += 1;
                } else {
                    errors.push(json!({ "code": 30, "loc": else_dir.get("loc").cloned().unwrap_or(Value::Null) }));
                }
            } else {
                errors.push(json!({ "code": 30, "loc": else_dir.get("loc").cloned().unwrap_or(Value::Null) }));
            }
            continue;
        }

        if let Some(for_dir) = vue3_directive(child, "for", true) {
            has_dynamic_slots = true;
            let parsed_projection = vue3_slot_for_parse_result_projection(child, for_dir, context);
            if let Some(parse_result) = parsed_projection.get("parseResult") {
                dynamic_slots.push(json!({
                    "kind": "for",
                    "source": parse_result["source"].clone(),
                    "params": {
                        "value": parse_result["value"].clone(),
                        "key": parse_result["key"].clone(),
                        "index": parse_result["index"].clone(),
                    },
                    "slot": vue3_dynamic_slot_projection(slot_name, slot, None),
                }));
            } else {
                errors.push(json!({ "code": 32, "loc": for_dir.get("loc").cloned().unwrap_or(Value::Null) }));
            }
            continue;
        }

        if let Some(name) = static_slot_name {
            if seen_slot_names.iter().any(|seen| seen == &name) {
                errors.push(json!({ "code": 38, "loc": slot_dir.get("loc").cloned().unwrap_or(Value::Null) }));
                continue;
            }
            if name == "default" {
                has_named_default_slot = true;
            }
            seen_slot_names.push(name);
        }
        properties.push(json!({
            "kind": "property",
            "key": slot_name,
            "params": slot_dir.get("exp").cloned().unwrap_or(Value::Null),
            "indices": [index],
            "unwrapTemplate": true,
            "loc": child.get("loc").cloned().unwrap_or_else(|| node.get("loc").cloned().unwrap_or(Value::Null)),
        }));
    }

    if on_component_slot.is_none() {
        if !has_template_slots {
            properties.push(json!({
                "kind": "property",
                "key": vue3_static_slot_key("default"),
                "params": Value::Null,
                "indices": vue3_all_child_indices(children),
                "loc": node.get("loc").cloned().unwrap_or(Value::Null),
                "nonScoped": true,
            }));
        } else if !implicit_default_indices.is_empty()
            && !vue3_all_indices_are_whitespace_text(children, &implicit_default_indices)
        {
            if has_named_default_slot {
                if let Some(child) = implicit_default_indices
                    .first()
                    .and_then(|index| children.get(*index))
                {
                    errors.push(json!({ "code": 39, "loc": child.get("loc").cloned().unwrap_or(Value::Null) }));
                }
            } else {
                properties.push(json!({
                    "kind": "property",
                    "key": vue3_static_slot_key("default"),
                    "params": Value::Null,
                    "indices": implicit_default_indices,
                    "loc": node.get("loc").cloned().unwrap_or(Value::Null),
                    "nonScoped": true,
                }));
            }
        }
    }

    let slot_flag = if has_dynamic_slots {
        2
    } else if vue3_has_forwarded_slots(children) {
        3
    } else {
        1
    };

    json!({
        "properties": properties,
        "dynamicSlots": dynamic_slots,
        "slotFlag": slot_flag,
        "slotFlagText": vue3_slot_flag_text(slot_flag),
        "hasDynamicSlots": has_dynamic_slots,
        "errors": errors,
    })
}

pub(crate) fn vue3_for_parse_result_projection(
    node: &Value,
    dir: &Value,
    context: &Value,
) -> Value {
    transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }))
}

pub(crate) fn vue3_slot_for_parse_result_projection(
    node: &Value,
    dir: &Value,
    context: &Value,
) -> Value {
    if let Some(parse_result) = dir.get("forParseResult").filter(|value| !value.is_null()) {
        return json!({
            "parseResult": {
                "source": parse_result.get("source").cloned().unwrap_or(Value::Null),
                "value": parse_result.get("value").cloned().unwrap_or(Value::Null),
                "key": parse_result.get("key").cloned().unwrap_or(Value::Null),
                "index": parse_result.get("index").cloned().unwrap_or(Value::Null),
                "finalized": parse_result.get("finalized").and_then(Value::as_bool).unwrap_or(true),
            }
        });
    }
    vue3_for_parse_result_projection(node, dir, context)
}

pub(crate) fn vue3_directive<'a>(
    node: &'a Value,
    name: &str,
    allow_empty: bool,
) -> Option<&'a Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            json_node_type(prop) == Some(7)
                && json_str(prop, "name") == Some(name)
                && (allow_empty || prop.get("exp").is_some_and(|exp| !exp.is_null()))
        })
}

pub(crate) fn vue3_slot_directive(node: &Value, allow_empty: bool) -> Option<&Value> {
    vue3_directive(node, "slot", allow_empty)
}

pub(crate) fn vue3_template_slot_directive(node: &Value) -> Option<&Value> {
    if json_node_type(node) == Some(1) && json_u64(node, "tagType") == Some(3) {
        vue3_slot_directive(node, true)
    } else {
        None
    }
}

pub(crate) fn vue3_else_slot_directive(node: &Value) -> Option<&Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            json_node_type(prop) == Some(7)
                && matches!(json_str(prop, "name"), Some("else") | Some("else-if"))
        })
}

pub(crate) fn vue3_template_has_if_like_slot_directive(node: &Value) -> bool {
    vue3_template_slot_directive(node).is_some()
        && node
            .get("props")
            .and_then(Value::as_array)
            .is_some_and(|props| {
                props.iter().any(|prop| {
                    json_node_type(prop) == Some(7)
                        && matches!(json_str(prop, "name"), Some("if") | Some("else-if"))
                })
            })
}

pub(crate) fn vue3_previous_non_comment_or_whitespace(
    children: &[Value],
    index: usize,
) -> Option<&Value> {
    children[..index]
        .iter()
        .rev()
        .find(|child| !vue3_is_comment_or_whitespace(child))
}

pub(crate) fn vue3_is_comment_or_whitespace(node: &Value) -> bool {
    json_node_type(node) == Some(3) || vue3_is_whitespace_text(node)
}

pub(crate) fn vue3_is_whitespace_text(node: &Value) -> bool {
    match json_node_type(node) {
        Some(2) => json_str(node, "content").is_some_and(|content| {
            content
                .chars()
                .all(|ch| matches!(ch, '\t' | '\r' | '\n' | '\u{000C}' | ' '))
        }),
        Some(12) => node.get("content").is_some_and(vue3_is_whitespace_text),
        _ => false,
    }
}

pub(crate) fn vue3_all_indices_are_whitespace_text(children: &[Value], indices: &[usize]) -> bool {
    indices
        .iter()
        .filter_map(|index| children.get(*index))
        .all(vue3_is_whitespace_text)
}

pub(crate) fn vue3_all_child_indices(children: &[Value]) -> Vec<usize> {
    (0..children.len()).collect()
}

pub(crate) fn vue3_slot_name_projection(slot: &Value, context: &Value) -> Value {
    let Some(arg) = slot.get("arg").filter(|arg| !arg.is_null()) else {
        return vue3_static_slot_key("default");
    };
    if json_bool(arg, "isStatic") {
        return vue3_static_slot_key(json_str(arg, "content").unwrap_or("default"));
    }
    let _ = context;
    arg.clone()
}

pub(crate) fn vue3_static_slot_name(slot: &Value) -> Option<String> {
    let Some(arg) = slot.get("arg").filter(|arg| !arg.is_null()) else {
        return Some("default".to_string());
    };
    json_bool(arg, "isStatic").then(|| json_str(arg, "content").unwrap_or("default").to_string())
}

pub(crate) fn vue3_static_slot_key(name: &str) -> Value {
    json!({
        "kind": "simple",
        "content": name,
        "isStatic": true,
        "constType": 3,
    })
}

pub(crate) fn vue3_slot_param_locals(exp: &Value) -> Vec<String> {
    let source = model_expression_source(exp);
    vue3_for_alias_locals(source.trim())
}

pub(crate) fn vue3_slot_condition_projection(dir: &Value, context: &Value) -> Value {
    let Some(exp) = dir.get("exp").filter(|exp| !exp.is_null()) else {
        return json!({ "kind": "undefined" });
    };
    let _ = context;
    exp.clone()
}

pub(crate) fn vue3_slot_function_projection(
    slot_dir: &Value,
    indices: &[usize],
    child: &Value,
) -> Value {
    json!({
        "kind": "slotFunction",
        "params": slot_dir.get("exp").cloned().unwrap_or(Value::Null),
        "indices": indices,
        "unwrapTemplate": true,
        "loc": child.get("loc").cloned().unwrap_or(Value::Null),
    })
}

pub(crate) fn vue3_dynamic_slot_projection(name: Value, slot: Value, key: Option<usize>) -> Value {
    let mut value = json!({
        "kind": "dynamicSlot",
        "name": name,
        "slot": slot,
    });
    if let Some(key) = key {
        value["key"] = json!(key.to_string());
    }
    value
}

pub(crate) fn vue3_default_fallback_projection() -> Value {
    json!({
        "kind": "simple",
        "content": "undefined",
        "isStatic": false,
        "constType": 0,
    })
}

pub(crate) fn vue3_append_slot_conditional_alternate(
    dynamic_slots: &mut [Value],
    alternate: Value,
) {
    let Some(last) = dynamic_slots.last_mut() else {
        return;
    };
    let mut target = last;
    loop {
        let nested = target
            .get("alternate")
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str)
            == Some("conditional");
        if !nested {
            target["alternate"] = alternate;
            break;
        }
        target = target.get_mut("alternate").expect("checked alternate");
    }
}

pub(crate) fn vue3_slot_flag_text(flag: u8) -> &'static str {
    match flag {
        1 => "STABLE",
        2 => "DYNAMIC",
        3 => "FORWARDED",
        _ => "",
    }
}

pub(crate) fn vue3_has_forwarded_slots(children: &[Value]) -> bool {
    children.iter().any(|child| match json_node_type(child) {
        Some(1) => {
            json_u64(child, "tagType") == Some(2)
                || child
                    .get("children")
                    .and_then(Value::as_array)
                    .is_some_and(|children| vue3_has_forwarded_slots(children))
        }
        Some(9) => child
            .get("branches")
            .and_then(Value::as_array)
            .is_some_and(|branches| vue3_has_forwarded_slots(branches)),
        Some(10) | Some(11) => child
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| vue3_has_forwarded_slots(children)),
        _ => false,
    })
}

pub(crate) fn vue3_component_slot_scope_ref(
    node: &Value,
    children: &[Value],
    context: &Value,
) -> bool {
    let mut names = transform_context_locals(context);
    if let Some(slot) = vue3_slot_directive(node, false) {
        if let Some(exp) = slot.get("exp").filter(|exp| !exp.is_null()) {
            let slot_locals = vue3_slot_param_locals(exp);
            names.retain(|name| !slot_locals.iter().any(|local| local == name));
        }
    }
    if names.is_empty() {
        return false;
    }
    node.get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props.iter().any(|prop| {
                json_str(prop, "name") == Some("slot")
                    && (prop
                        .get("arg")
                        .is_some_and(|arg| vue3_node_source_contains_any(arg, &names))
                        || prop
                            .get("exp")
                            .is_some_and(|exp| vue3_node_source_contains_any(exp, &names)))
            })
        })
        || children
            .iter()
            .any(|child| vue3_node_source_contains_any(child, &names))
}

pub(crate) fn vue3_node_source_contains_any(node: &Value, names: &[String]) -> bool {
    if node.is_null() {
        return false;
    }
    match json_node_type(node) {
        Some(1) => {
            if node
                .get("props")
                .and_then(Value::as_array)
                .is_some_and(|props| {
                    props.iter().any(|prop| {
                        json_node_type(prop) == Some(7)
                            && (prop
                                .get("arg")
                                .is_some_and(|arg| vue3_node_source_contains_any(arg, names))
                                || prop
                                    .get("exp")
                                    .is_some_and(|exp| vue3_node_source_contains_any(exp, names)))
                    })
                })
            {
                return true;
            }
            node.get("children")
                .and_then(Value::as_array)
                .is_some_and(|children| {
                    children
                        .iter()
                        .any(|child| vue3_node_source_contains_any(child, names))
                })
        }
        Some(11) => {
            if node
                .get("source")
                .is_some_and(|source| vue3_node_source_contains_any(source, names))
            {
                return true;
            }
            node.get("children")
                .and_then(Value::as_array)
                .is_some_and(|children| {
                    children
                        .iter()
                        .any(|child| vue3_node_source_contains_any(child, names))
                })
        }
        Some(9) => node
            .get("branches")
            .and_then(Value::as_array)
            .is_some_and(|branches| {
                branches
                    .iter()
                    .any(|branch| vue3_node_source_contains_any(branch, names))
            }),
        Some(10) => {
            if node
                .get("condition")
                .is_some_and(|condition| vue3_node_source_contains_any(condition, names))
            {
                return true;
            }
            node.get("children")
                .and_then(Value::as_array)
                .is_some_and(|children| {
                    children
                        .iter()
                        .any(|child| vue3_node_source_contains_any(child, names))
                })
        }
        Some(4) => {
            let content = json_str(node, "content").unwrap_or("");
            !json_bool(node, "isStatic")
                && (names
                    .iter()
                    .any(|name| source_contains_identifier(content, name))
                    || node
                        .get("loc")
                        .and_then(|loc| json_str(loc, "source"))
                        .is_some_and(|source| {
                            names
                                .iter()
                                .any(|name| source_contains_identifier(source, name))
                        }))
        }
        Some(8) => node
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| {
                children
                    .iter()
                    .filter(|child| child.is_object())
                    .any(|child| vue3_node_source_contains_any(child, names))
            }),
        Some(5) | Some(12) => node
            .get("content")
            .is_some_and(|content| vue3_node_source_contains_any(content, names)),
        Some(2) | Some(3) | Some(20) => false,
        _ => false,
    }
}

pub(crate) fn source_contains_identifier(source: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut search_start = 0usize;
    while let Some(offset) = source[search_start..].find(name) {
        let start = search_start + offset;
        let end = start + name.len();
        let before = source[..start].chars().next_back();
        let after = source[end..].chars().next();
        if before.is_none_or(|ch| !is_identifier_continue(ch))
            && after.is_none_or(|ch| !is_identifier_continue(ch))
        {
            return true;
        }
        search_start = end;
    }
    false
}

pub(crate) fn transform_for_codegen_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let for_node = payload.get("forNode").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let source = for_node.get("source").unwrap_or(&Value::Null);
    let is_stable_fragment = json_node_type(source) == Some(4)
        && json_u64(source, "constType").is_some_and(|value| value > 0);
    let key_projection = vue3_for_key_property_projection(node, context);
    json!({
        "keyProperty": key_projection,
        "fragmentFlag": if is_stable_fragment {
            64
        } else if !key_projection.is_null() {
            128
        } else {
            256
        },
        "disableTracking": !is_stable_fragment,
        "isStableFragment": is_stable_fragment,
    })
}

pub(crate) fn transform_for_exit_codegen_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let for_node = payload.get("forNode").unwrap_or(&Value::Null);
    let children = for_node
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if vue3_for_is_slot_outlet_summary(node) {
        return json!({ "kind": "slotOutlet", "path": "node" });
    }
    if json_u64(node, "tagType") == Some(3)
        && node
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| {
                children.len() == 1 && vue3_for_is_slot_outlet_summary(&children[0])
            })
    {
        return json!({ "kind": "slotOutlet", "path": "templateChild", "index": 0 });
    }
    let need_fragment_wrapper =
        children.len() != 1 || children.first().and_then(json_node_type) != Some(1);
    if need_fragment_wrapper {
        return json!({ "kind": "fragmentWrapper", "patchFlag": 64 });
    }
    json!({
        "kind": "singleElement",
        "childBlockIsBlock": !json_bool(payload, "isStableFragment"),
    })
}

pub(crate) fn vue3_for_is_slot_outlet_summary(node: &Value) -> bool {
    json_node_type(node) == Some(1) && json_u64(node, "tagType") == Some(2)
}

pub(crate) fn vue3_for_key_property_projection(node: &Value, context: &Value) -> Value {
    let Some((prop, is_directive)) = vue3_for_key_prop(node) else {
        return Value::Null;
    };
    let value = if is_directive {
        let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) else {
            return Value::Null;
        };
        let raw = json_str(exp, "content")
            .or_else(|| exp.get("loc").and_then(|loc| json_str(loc, "source")))
            .unwrap_or("");
        if json_bool(context, "prefixIdentifiers") {
            let options = vue3_options_from_transform_context(context);
            let locals = transform_context_locals(context);
            vue3_for_rewrite_projection_node(
                raw,
                &options,
                &locals,
                exp.get("loc").cloned().unwrap_or(Value::Null),
                Vue3ForAstMode::Expression,
                false,
            )
        } else {
            vue3_for_expression_projection(raw, exp, 0, raw.len(), Vue3ForAstMode::Expression)
        }
    } else {
        let Some(value) = prop.get("value").filter(|value| !value.is_null()) else {
            return Value::Null;
        };
        let content = json_str(value, "content").unwrap_or("");
        json!({
            "kind": "simple",
            "content": content,
            "isStatic": true,
            "constType": 3,
            "loc": value.get("loc").cloned().unwrap_or_else(|| prop.get("loc").cloned().unwrap_or(Value::Null)),
            "astMode": "expression",
        })
    };
    json!({ "value": value })
}

pub(crate) fn vue3_for_key_prop(node: &Value) -> Option<(&Value, bool)> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|prop| match json_node_type(prop) {
            Some(6) if json_str(prop, "name") == Some("key") => Some((prop, false)),
            Some(7)
                if json_str(prop, "name") == Some("bind")
                    && prop.get("arg").is_some_and(|arg| {
                        json_str(arg, "content") == Some("key") && json_bool(arg, "isStatic")
                    }) =>
            {
                Some((prop, true))
            }
            _ => None,
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3ForParsed {
    pub(crate) source: Vue3ForPart,
    pub(crate) value: Option<Vue3ForPart>,
    pub(crate) key: Option<Vue3ForPart>,
    pub(crate) index: Option<Vue3ForPart>,
}

impl Vue3ForParsed {
    pub(crate) fn all_alias_locals(&self) -> Vec<String> {
        let mut locals = Vec::new();
        for part in [&self.value, &self.key, &self.index].into_iter().flatten() {
            for local in vue3_for_alias_locals(&part.content) {
                if !locals.iter().any(|existing| existing == &local) {
                    locals.push(local);
                }
            }
        }
        locals
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3ForPart {
    pub(crate) content: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Vue3ForAstMode {
    Expression,
    Params,
}

pub(crate) fn parse_vue3_for_expression(source: &str) -> Option<Vue3ForParsed> {
    let Vue3ForMatch { lhs_end, rhs_start } = find_vue3_for_match(source)?;
    let rhs_end = trim_end_offset(source, rhs_start, source.len());
    if rhs_start >= rhs_end {
        return None;
    }
    let (alias_start, alias_end) = vue3_for_alias_content_span(source, 0, lhs_end);
    let aliases = split_vue3_for_aliases(source, alias_start, alias_end);
    Some(Vue3ForParsed {
        source: Vue3ForPart {
            content: source[rhs_start..rhs_end].to_string(),
            start: rhs_start,
            end: rhs_end,
        },
        value: aliases.first().and_then(|segment| segment.part(source)),
        key: aliases.get(1).and_then(|segment| segment.part(source)),
        index: aliases.get(2).and_then(|segment| segment.part(source)),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Vue3ForMatch {
    pub(crate) lhs_end: usize,
    pub(crate) rhs_start: usize,
}

pub(crate) fn find_vue3_for_match(source: &str) -> Option<Vue3ForMatch> {
    for (operator_start, _) in source.char_indices() {
        let operator_len = if source[operator_start..].starts_with("in") {
            2
        } else if source[operator_start..].starts_with("of") {
            2
        } else {
            continue;
        };
        if operator_start == 0 || !previous_char_is_whitespace(source, operator_start) {
            continue;
        }
        let after_operator = operator_start + operator_len;
        if !source[after_operator..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
        {
            continue;
        }
        let Some(rhs_start) = source[after_operator..]
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
            .map(|(index, _)| after_operator + index)
        else {
            continue;
        };
        let lhs_end = trim_end_offset(source, 0, operator_start);
        return Some(Vue3ForMatch { lhs_end, rhs_start });
    }
    None
}

pub(crate) fn previous_char_is_whitespace(source: &str, offset: usize) -> bool {
    source[..offset]
        .chars()
        .next_back()
        .is_some_and(char::is_whitespace)
}

pub(crate) fn vue3_for_alias_content_span(
    source: &str,
    start: usize,
    end: usize,
) -> (usize, usize) {
    let mut start = trim_start_offset(source, start, end);
    let mut end = trim_end_offset(source, start, end);
    if source[start..end].starts_with('(') && source[start..end].ends_with(')') {
        start += '('.len_utf8();
        end = end.saturating_sub(')'.len_utf8());
    }
    start = trim_start_offset(source, start, end);
    end = trim_end_offset(source, start, end);
    (start, end)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3ForAliasSegment {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Vue3ForAliasSegment {
    pub(crate) fn part(&self, source: &str) -> Option<Vue3ForPart> {
        let start = trim_start_offset(source, self.start, self.end);
        let end = trim_end_offset(source, start, self.end);
        (start < end).then(|| Vue3ForPart {
            content: source[start..end].to_string(),
            start,
            end,
        })
    }
}

pub(crate) fn split_vue3_for_aliases(
    source: &str,
    alias_start: usize,
    alias_end: usize,
) -> Vec<Vue3ForAliasSegment> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut quote = None::<char>;
    let mut start = alias_start;
    let mut escaped = false;
    for (index, ch) in source[alias_start..alias_end].char_indices() {
        let index = alias_start + index;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                items.push(Vue3ForAliasSegment { start, end: index });
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    items.push(Vue3ForAliasSegment {
        start,
        end: alias_end,
    });
    items
}

pub(crate) fn trim_start_offset(source: &str, start: usize, end: usize) -> usize {
    source[start..end]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(index, _)| start + index)
        .unwrap_or(end)
}

pub(crate) fn trim_end_offset(source: &str, start: usize, end: usize) -> usize {
    let mut trimmed = end;
    for (index, ch) in source[start..end].char_indices().rev() {
        if !ch.is_whitespace() {
            trimmed = start + index + ch.len_utf8();
            break;
        }
        trimmed = start + index;
    }
    trimmed
}

pub(crate) fn vue3_for_expression_projection(
    content: &str,
    exp: &Value,
    start: usize,
    end: usize,
    ast_mode: Vue3ForAstMode,
) -> Value {
    json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": 0,
        "loc": vue3_for_exp_loc(exp, start, end),
        "astMode": vue3_for_ast_mode_name(ast_mode),
    })
}

pub(crate) fn vue3_for_rewrite_projection_node(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
    loc: Value,
    ast_mode: Vue3ForAstMode,
    force_compound_for_complex: bool,
) -> Value {
    let rewritten = if locals.is_empty() {
        rewrite_js_like_expression(raw, options)
    } else {
        rewrite_js_like_expression_with_locals(raw, options, locals)
    };
    let children = vue3_for_compound_children(raw, options, locals, ast_mode, &loc);
    let is_simple = is_simple_identifier_ascii(raw.trim())
        || children.is_empty()
        || (!force_compound_for_complex && rewritten == raw.trim());
    if is_simple {
        return vue3_for_simple_projection(
            rewritten.trim(),
            loc,
            vue3_for_const_type(rewritten.trim()),
            ast_mode,
        );
    }
    let helpers = vue3_for_helpers_for_content(&rewritten);
    let mut value = json!({
        "kind": "compound",
        "children": children,
        "loc": loc,
        "astMode": vue3_for_ast_mode_name(ast_mode),
    });
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

pub(crate) fn vue3_for_simple_projection(
    content: &str,
    loc: Value,
    const_type: u8,
    ast_mode: Vue3ForAstMode,
) -> Value {
    let mut value = json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": const_type,
        "loc": loc,
        "astMode": vue3_for_ast_mode_name(ast_mode),
    });
    let helpers = vue3_for_helpers_for_content(content);
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

pub(crate) fn vue3_for_compound_children(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
    ast_mode: Vue3ForAstMode,
    loc: &Value,
) -> Vec<Value> {
    let mut children = Vec::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut index = 0usize;
    let mut last = 0usize;
    let chars = raw.char_indices().collect::<Vec<_>>();
    while index < chars.len() {
        let start = chars[index].0;
        let ch = chars[index].1;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if !is_identifier_start(ch) {
            index += 1;
            continue;
        }
        index += 1;
        while index < chars.len() && is_identifier_continue(chars[index].1) {
            index += 1;
        }
        let end = chars.get(index).map_or(raw.len(), |(offset, _)| *offset);
        let ident = &raw[start..end];
        let Some(replacement) = vue3_for_identifier_projection_content(
            raw, start, end, ident, options, locals, ast_mode,
        ) else {
            continue;
        };
        if last < start {
            children.push(json!(raw[last..start]));
        }
        children.push(vue3_for_simple_projection(
            &replacement,
            vue3_for_child_loc(loc, raw, start, end),
            if replacement == ident {
                3
            } else {
                vue3_for_const_type(&replacement)
            },
            ast_mode,
        ));
        last = end;
    }
    if last < raw.len() {
        children.push(json!(raw[last..].to_string()));
    }
    children
}

pub(crate) fn vue3_for_identifier_projection_content(
    raw: &str,
    start: usize,
    end: usize,
    ident: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
    ast_mode: Vue3ForAstMode,
) -> Option<String> {
    if is_keyword(ident) || is_global_or_literal(ident) {
        return None;
    }
    let prev = previous_non_ws(raw, start);
    let next = next_non_ws(raw, end);
    if next == Some(':') {
        return None;
    }
    if prev == Some('.') {
        return Some(ident.to_string());
    }
    if locals.iter().any(|local| local == ident) {
        return Some(ident.to_string());
    }
    if ast_mode == Vue3ForAstMode::Params && next == Some('=') {
        return Some(ident.to_string());
    }
    Some(rewrite_identifier(ident, options))
}

pub(crate) fn previous_non_ws(source: &str, offset: usize) -> Option<char> {
    source[..offset]
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace())
}

pub(crate) fn vue3_for_exp_loc(exp: &Value, start: usize, end: usize) -> Value {
    let loc = exp.get("loc").unwrap_or(&Value::Null);
    let source = json_str(loc, "source")
        .or_else(|| json_str(exp, "content"))
        .unwrap_or("");
    vue3_for_loc_from_start(loc.get("start").unwrap_or(&Value::Null), source, start, end)
}

pub(crate) fn vue3_for_child_loc(
    parent_loc: &Value,
    source: &str,
    start: usize,
    end: usize,
) -> Value {
    vue3_for_loc_from_start(
        parent_loc.get("start").unwrap_or(&Value::Null),
        source,
        start,
        end,
    )
}

pub(crate) fn vue3_for_loc_from_start(
    start_pos: &Value,
    source: &str,
    start: usize,
    end: usize,
) -> Value {
    let start = start.min(source.len());
    let end = end.min(source.len()).max(start);
    json!({
        "start": vue3_for_advance_position(start_pos, source, start),
        "end": vue3_for_advance_position(start_pos, source, end),
        "source": source.get(start..end).unwrap_or_default(),
    })
}

pub(crate) fn vue3_for_advance_position(start_pos: &Value, source: &str, amount: usize) -> Value {
    let mut offset = start_pos
        .get("offset")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let mut line = start_pos.get("line").and_then(Value::as_i64).unwrap_or(1);
    let mut column = start_pos.get("column").and_then(Value::as_i64).unwrap_or(1);
    let mut index = 0usize;
    for ch in source.chars() {
        if index >= amount {
            break;
        }
        let len = ch.len_utf8();
        if index + len > amount {
            offset += (amount - index) as i64;
            column += (amount - index) as i64;
            return json!({ "offset": offset, "line": line, "column": column });
        }
        index += len;
        offset += len as i64;
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    if amount > index {
        offset += (amount - index) as i64;
        column += (amount - index) as i64;
    }
    json!({ "offset": offset, "line": line, "column": column })
}

pub(crate) fn vue3_for_ast_mode_name(mode: Vue3ForAstMode) -> &'static str {
    match mode {
        Vue3ForAstMode::Expression => "expression",
        Vue3ForAstMode::Params => "params",
    }
}

pub(crate) fn vue3_for_const_type(content: &str) -> u8 {
    let content = content.trim();
    if matches!(content, "true" | "false" | "null") {
        return 3;
    }
    if (content.starts_with('"') && content.ends_with('"'))
        || (content.starts_with('\'') && content.ends_with('\''))
        || content.parse::<f64>().is_ok()
    {
        return 3;
    }
    0
}

pub(crate) fn vue3_for_helpers_for_content(content: &str) -> Vec<&'static str> {
    let mut helpers = Vec::new();
    if content.contains("_unref(") {
        helpers.push("UNREF");
    }
    if content.contains("_isRef(") {
        helpers.push("IS_REF");
    }
    helpers
}

pub(crate) fn vue3_for_alias_locals(alias: &str) -> Vec<String> {
    let store = JsAstStore::new();
    let wrapped = format!("({alias})=>{{}}");
    if let Ok(Expression::ArrowFunctionExpression(function)) =
        store.parse_expression(&wrapped, oxc_span::SourceType::ts())
    {
        let mut locals = Vec::new();
        for param in &function.params.items {
            collect_vue3_for_binding_pattern(&param.pattern, &mut locals);
        }
        if let Some(rest) = &function.params.rest {
            collect_vue3_for_binding_pattern(&rest.rest.argument, &mut locals);
        }
        locals.sort();
        locals.dedup();
        return locals;
    }
    extract_v_for_alias_locals(alias)
}

pub(crate) fn collect_vue3_for_binding_pattern(
    pattern: &BindingPattern<'_>,
    locals: &mut Vec<String>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            locals.push(identifier.name.to_string());
        }
        BindingPattern::ObjectPattern(object) => {
            for property in &object.properties {
                collect_vue3_for_binding_pattern(&property.value, locals);
            }
            if let Some(rest) = &object.rest {
                collect_vue3_for_binding_pattern(&rest.argument, locals);
            }
        }
        BindingPattern::ArrayPattern(array) => {
            for element in array.elements.iter().flatten() {
                collect_vue3_for_binding_pattern(element, locals);
            }
            if let Some(rest) = &array.rest {
                collect_vue3_for_binding_pattern(&rest.argument, locals);
            }
        }
        BindingPattern::AssignmentPattern(assignment) => {
            collect_vue3_for_binding_pattern(&assignment.left, locals);
        }
    }
}

pub(crate) fn vue3_for_template_key_errors(node: &Value) -> Vec<Value> {
    if json_u64(node, "tagType") != Some(3) {
        return Vec::new();
    }
    node.get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|child| json_node_type(child) == Some(1))
        .filter(|child| !vue3_for_child_has_structural_directive(child))
        .filter_map(vue3_for_child_key_loc)
        .take(1)
        .map(|loc| json!({ "code": 33, "loc": loc }))
        .collect()
}

pub(crate) fn vue3_for_child_has_structural_directive(node: &Value) -> bool {
    node.get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props.iter().any(|prop| {
                json_node_type(prop) == Some(7)
                    && matches!(
                        json_str(prop, "name"),
                        Some("for" | "if" | "else" | "else-if")
                    )
            })
        })
}

pub(crate) fn vue3_for_child_key_loc(node: &Value) -> Option<Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| match json_node_type(prop) {
            Some(6) => json_str(prop, "name") == Some("key"),
            Some(7) => {
                json_str(prop, "name") == Some("bind")
                    && prop.get("arg").is_some_and(|arg| {
                        json_str(arg, "content") == Some("key") && json_bool(arg, "isStatic")
                    })
            }
            _ => false,
        })
        .and_then(|prop| prop.get("loc").cloned())
}

/// Projects Rust-backed component type resolution for bridge callers.
pub fn resolve_component_type_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let ssr = json_bool(payload, "ssr");
    let mut tag = json_str(node, "tag").unwrap_or("").to_string();
    let is_explicit_dynamic = matches!(tag.as_str(), "component" | "Component");
    let is_prop = resolve_component_is_prop(node);

    if let Some(is_prop) = is_prop {
        if is_explicit_dynamic || json_bool(context, "compatIsOnElement") {
            if let Some(exp) = resolve_component_is_prop_expression(is_prop, context) {
                return json!({
                    "kind": "dynamic",
                    "helper": "RESOLVE_DYNAMIC_COMPONENT",
                    "argument": exp,
                });
            }
        } else if json_node_type(is_prop) == Some(6)
            && is_prop
                .get("value")
                .and_then(|value| json_str(value, "content"))
                .is_some_and(|value| value.starts_with("vue:"))
        {
            tag = is_prop
                .get("value")
                .and_then(|value| json_str(value, "content"))
                .map(|value| value[4..].to_string())
                .unwrap_or(tag);
        }
    }

    if let Some(helper) = vue3_core_component_helper(&tag) {
        return json!({
            "kind": "helper",
            "helper": helper,
            "registerHelper": !ssr,
        });
    }
    if let Some(projection) = context
        .get("builtInComponents")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find_map(|component| {
                if component.as_str() == Some(&tag) {
                    return Some(json!({
                        "kind": "helper",
                        "helper": tag,
                        "registerHelper": !ssr,
                    }));
                }
                let component_tag = component.get("tag").and_then(Value::as_str)?;
                (component_tag == tag).then(|| {
                    json!({
                        "kind": "helper",
                        "helperName": component.get("helperName").and_then(Value::as_str).unwrap_or(component_tag),
                        "registerHelper": !ssr,
                    })
                })
            })
        })
    {
        return projection;
    }

    if let Some(from_setup) = resolve_setup_reference(&tag, context) {
        return from_setup;
    }
    if let Some(dot_index) = tag.find('.') {
        if dot_index > 0 {
            if let Some(mut namespace) = resolve_setup_reference(&tag[..dot_index], context) {
                if let Some(content) = json_str(&namespace, "content") {
                    let resolved = format!("{}{}", content, &tag[dot_index..]);
                    namespace["content"] = json!(resolved);
                    return namespace;
                }
            }
        }
    }

    let self_name = json_str(context, "selfName");
    let component_name =
        if self_name.is_some_and(|self_name| capitalize(&camelize(&tag)) == self_name) {
            format!("{tag}__self")
        } else {
            tag.clone()
        };
    json!({
        "kind": "asset",
        "helper": "RESOLVE_COMPONENT",
        "component": component_name,
        "assetId": component_asset_id(&tag),
    })
}

/// Projects Rust-backed element prop transform behavior for bridge callers.
pub fn transform_element_props_projection(payload: &Value) -> Value {
    let props = payload
        .get("props")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let has_children = json_bool(payload, "hasChildren");
    let is_component = json_bool(payload, "isComponent");
    let is_dynamic_component = json_bool(payload, "isDynamicComponent");
    let in_ssr = json_bool(context, "inSSR");
    let in_v_for = context
        .get("vForDepth")
        .and_then(Value::as_u64)
        .is_some_and(|depth| depth > 0);
    let inline_template_refs = inline_template_ref_projections(props, context);
    let mut patch_flag = 0u16;
    let mut dynamic_prop_names = Vec::<String>::new();
    let mut has_ref = false;
    let mut has_class_binding = false;
    let mut has_style_binding = false;
    let mut has_hydration_event_binding = false;
    let mut has_dynamic_keys = false;
    let mut has_vnode_hook = false;
    let mut should_use_block = false;
    let mut normalize_props = false;
    let mut guard_reactive_props = false;
    let mut normalize_class = false;
    let mut normalize_style = false;
    let mut has_runtime_directives = false;
    let mut has_dynamic_object = false;
    let mut has_normalize_dynamic_keys = false;
    let ref_for_marker = in_v_for
        && props.iter().any(|prop| {
            (matches!(
                json_str(prop, "kind"),
                Some("attribute") | Some("directiveProp")
            ) && json_str(prop, "name") == Some("ref"))
                || json_str(prop, "kind") == Some("objectBind")
        });

    for prop in props {
        match json_str(prop, "kind") {
            Some("attribute") => {
                if json_str(prop, "name") == Some("ref") {
                    has_ref = true;
                }
            }
            Some("objectBind") => {
                has_dynamic_keys = true;
                has_normalize_dynamic_keys = true;
                has_dynamic_object = true;
            }
            Some("objectOn") => {
                has_dynamic_keys = true;
                has_normalize_dynamic_keys = true;
                has_dynamic_object = true;
            }
            Some("runtimeDirective") => {
                has_runtime_directives = true;
                if has_children {
                    should_use_block = true;
                }
            }
            Some("directiveProp") => {
                if json_bool(prop, "dynamicKey") {
                    has_dynamic_keys = true;
                    if !json_bool(prop, "ignoreDynamicKeyForNormalize") {
                        has_normalize_dynamic_keys = true;
                    }
                } else if let Some(name) = json_str(prop, "name") {
                    let value_constant = json_bool(prop, "valueConstant");
                    let value_cached = json_bool(prop, "valueCached");
                    let is_event = prop_name_is_event_handler(name);
                    if is_event
                        && (!is_component || is_dynamic_component)
                        && name.to_ascii_lowercase() != "onclick"
                        && name != "onUpdate:modelValue"
                        && !prop_name_is_reserved(name)
                    {
                        has_hydration_event_binding = true;
                    }
                    if is_event && prop_name_is_reserved(name) {
                        has_vnode_hook = true;
                    }
                    if !value_cached && !value_constant {
                        if name == "ref" {
                            has_ref = true;
                        } else if name == "class" {
                            has_class_binding = true;
                        } else if name == "style" {
                            has_style_binding = true;
                        } else if name != "key"
                            && !dynamic_prop_names.iter().any(|existing| existing == name)
                        {
                            dynamic_prop_names.push(name.to_string());
                        }
                        if is_component
                            && matches!(name, "class" | "style")
                            && !dynamic_prop_names.iter().any(|existing| existing == name)
                        {
                            dynamic_prop_names.push(name.to_string());
                        }
                    }
                }
                if json_bool(prop, "propModifier") {
                    patch_flag |= 32;
                }
                if json_bool(prop, "forceBlock") {
                    should_use_block = true;
                }
            }
            _ => {}
        }
    }

    if has_dynamic_keys {
        patch_flag |= 16;
    } else {
        if has_class_binding && !is_component {
            patch_flag |= 2;
        }
        if has_style_binding && !is_component {
            patch_flag |= 4;
        }
        if !dynamic_prop_names.is_empty() {
            patch_flag |= 8;
        }
        if has_hydration_event_binding {
            patch_flag |= 32;
        }
    }

    if !should_use_block
        && (patch_flag == 0 || patch_flag == 32)
        && (has_ref || has_vnode_hook || has_runtime_directives)
    {
        patch_flag |= 512;
    }

    if !in_ssr {
        normalize_class = has_class_binding || props.iter().any(prop_requires_normalize_class);
        normalize_style = has_style_binding
            || props.iter().any(prop_requires_normalize_style)
            || props
                .iter()
                .filter(|prop| prop_output_name(prop) == Some("style"))
                .count()
                > 1;
        if has_dynamic_object {
            normalize_props = true;
            guard_reactive_props = true;
        } else if has_normalize_dynamic_keys {
            normalize_props = true;
        }
    }

    json!({
        "patchFlag": patch_flag,
        "dynamicPropNames": dynamic_prop_names,
        "shouldUseBlock": should_use_block,
        "normalizeProps": normalize_props,
        "guardReactiveProps": guard_reactive_props,
        "normalizeClass": normalize_class,
        "normalizeStyle": normalize_style,
        "refForMarker": ref_for_marker,
        "inlineTemplateRefs": inline_template_refs,
    })
}

/// Projects Rust-backed directive argument building for bridge callers.
pub fn build_directive_args_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let need_runtime = payload.get("needRuntime").unwrap_or(&Value::Null);
    let runtime = if let Some(helper) = need_runtime.get("helper").and_then(Value::as_str) {
        json!({ "kind": "helper", "helper": helper })
    } else if let Some(helper_name) = need_runtime.get("helperName").and_then(Value::as_str) {
        json!({ "kind": "helper", "helperName": helper_name })
    } else {
        json!({
            "kind": "asset",
            "name": json_str(dir, "name").unwrap_or(""),
        })
    };
    let modifiers = dir
        .get("modifiers")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter_map(|modifier| {
            modifier
                .as_str()
                .or_else(|| modifier.get("content").and_then(Value::as_str))
                .map(|name| json!({ "name": name }))
        })
        .collect::<Vec<_>>();
    json!({
        "runtime": runtime,
        "includeExp": dir.get("exp").is_some_and(|exp| !exp.is_null()),
        "includeArg": dir.get("arg").is_some_and(|arg| !arg.is_null()),
        "modifiers": modifiers,
    })
}

/// Projects Rust-backed built-in element child transform behavior.
pub fn transform_element_children_projection(payload: &Value) -> Value {
    let tag = json_str(payload, "tag").unwrap_or("");
    let children = payload
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    match public_helper_by_name(tag) {
        Some(RuntimeHelper::Vue3Suspense | RuntimeHelper::Vue3BaseTransition) => {
            let slots = component_slot_projections(children);
            json!({
                "kind": "slots",
                "slots": slots,
                "slotFlag": "1 /* STABLE */",
                "patchFlag": null,
                "shouldUseBlock": public_helper_by_name(tag) == Some(RuntimeHelper::Vue3Suspense),
            })
        }
        Some(RuntimeHelper::Vue3KeepAlive) => json!({
            "kind": "children",
            "patchFlag": 1024,
            "shouldUseBlock": true,
        }),
        _ => json!({ "kind": "default" }),
    }
}

/// Projects Rust-backed text transform behavior for bridge callers.
pub fn transform_text_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    if !matches!(json_node_type(node), Some(0 | 1 | 10 | 11)) {
        return json!({ "operations": [] });
    }
    let Some(source_children) = node.get("children").and_then(Value::as_array) else {
        return json!({ "operations": [] });
    };
    let mut children = source_children.clone();
    let mut operations = Vec::new();
    let mut has_text = false;
    let mut index = 0usize;
    while index < children.len() {
        if !vue3_is_text_node(&children[index]) {
            index += 1;
            continue;
        }
        has_text = true;
        let start = index;
        let mut end = index;
        while end + 1 < children.len() && vue3_is_text_node(&children[end + 1]) {
            end += 1;
        }
        if end > start {
            let compound = vue3_text_compound(&children[start..=end]);
            operations.push(json!({
                "kind": "mergeText",
                "start": start,
                "end": end,
            }));
            children.splice(start..=end, std::iter::once(compound));
            index = start + 1;
        } else {
            index += 1;
        }
    }

    if !has_text {
        return json!({ "operations": operations });
    }

    let single_plain_element_text = children.len() == 1
        && json_node_type(node) == Some(1)
        && json_u64(node, "tagType") == Some(0)
        && !vue3_text_has_untransformed_custom_directive(node, context)
        && !(json_bool(context, "compat") && json_str(node, "tag") == Some("template"));
    if children.len() == 1 && (json_node_type(node) == Some(0) || single_plain_element_text) {
        return json!({ "operations": operations });
    }

    let ssr = json_bool(context, "ssr");
    for (index, child) in children.iter().enumerate() {
        if !(vue3_is_text_node(child) || json_node_type(child) == Some(8)) {
            continue;
        }
        let patch_flag = (!ssr && vue3_constant_type(child, context) == VUE3_CONSTANT_NOT)
            .then_some("1 /* TEXT */");
        operations.push(json!({
            "kind": "wrapTextCall",
            "index": index,
            "includeContent": !(json_node_type(child) == Some(2)
                && json_str(child, "content") == Some(" ")),
            "patchFlag": patch_flag,
        }));
    }

    json!({ "operations": operations })
}

pub(crate) fn vue3_is_text_node(node: &Value) -> bool {
    matches!(json_node_type(node), Some(2 | 5))
}

pub(crate) fn vue3_text_compound(children: &[Value]) -> Value {
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
            .unwrap_or(Value::Null),
    })
}

pub(crate) fn vue3_text_has_untransformed_custom_directive(node: &Value, context: &Value) -> bool {
    let transformed = context
        .get("directiveTransforms")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    node.get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props.iter().any(|prop| {
                json_node_type(prop) == Some(7)
                    && json_str(prop, "name")
                        .is_some_and(|name| !transformed.iter().any(|known| *known == name))
            })
        })
}

pub(crate) fn component_slot_projections(children: &[Value]) -> Vec<Value> {
    let mut slots = Vec::new();
    let mut plain_indices = Vec::new();
    for (index, child) in children.iter().enumerate() {
        if json_str(child, "tag") == Some("template") {
            if let Some(slot_name) = template_slot_name(child) {
                slots.push(json!({
                    "name": slot_name,
                    "indices": [index],
                    "unwrapTemplate": true,
                }));
                continue;
            }
        }
        plain_indices.push(index);
    }
    if !plain_indices.is_empty() {
        slots.insert(
            0,
            json!({
                "name": "default",
                "indices": plain_indices,
                "unwrapTemplate": false,
            }),
        );
    }
    slots
}

pub(crate) fn template_slot_name(node: &Value) -> Option<&str> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|prop| {
            if json_str(prop, "name") == Some("slot") {
                prop.get("arg")
                    .and_then(|arg| arg.get("content"))
                    .and_then(Value::as_str)
            } else {
                None
            }
        })
}

pub(crate) fn inline_template_ref_projections(props: &[Value], context: &Value) -> Vec<Value> {
    if !json_bool(context, "inline") {
        return Vec::new();
    }
    let Some(binding_metadata) = context.get("bindingMetadata").and_then(Value::as_object) else {
        return Vec::new();
    };
    props
        .iter()
        .filter_map(|prop| {
            if json_str(prop, "kind") != Some("attribute") || json_str(prop, "name") != Some("ref")
            {
                return None;
            }
            let content = json_str(prop, "value")?;
            let binding = binding_metadata.get(content).and_then(Value::as_str)?;
            if matches!(binding, "setup-let" | "setup-ref" | "setup-maybe-ref") {
                Some(json!({ "content": content }))
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn prop_requires_normalize_style(prop: &Value) -> bool {
    json_str(prop, "kind") == Some("directiveProp")
        && json_str(prop, "name") == Some("style")
        && (json_bool(prop, "valueStartsWithArray")
            || prop.get("valueType").and_then(Value::as_u64) == Some(17))
}

pub(crate) fn prop_requires_normalize_class(prop: &Value) -> bool {
    json_str(prop, "kind") == Some("directiveProp")
        && json_str(prop, "name") == Some("class")
        && !json_bool(prop, "valueStatic")
}

pub(crate) fn prop_output_name(prop: &Value) -> Option<&str> {
    match json_str(prop, "kind") {
        Some("attribute") | Some("directiveProp") => json_str(prop, "name"),
        _ => None,
    }
}

pub(crate) fn prop_name_is_event_handler(name: &str) -> bool {
    name.starts_with("on")
        && name
            .chars()
            .nth(2)
            .is_some_and(|ch| !matches!(ch, 'a'..='z' | '-' | ':'))
}

pub(crate) fn prop_name_is_reserved(name: &str) -> bool {
    matches!(name, "key" | "ref" | "ref_for" | "ref_key")
        || name.starts_with("onVnode")
        || name.starts_with("onUpdate:")
}

pub(crate) fn resolve_component_is_prop(node: &Value) -> Option<&Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            if json_node_type(prop) == Some(6) {
                json_str(prop, "name") == Some("is")
            } else {
                json_str(prop, "name") == Some("bind")
                    && prop.get("arg").is_some_and(|arg| {
                        json_bool(arg, "isStatic") && json_str(arg, "content") == Some("is")
                    })
            }
        })
}

pub(crate) fn resolve_component_is_prop_expression(prop: &Value, context: &Value) -> Option<Value> {
    if json_node_type(prop) == Some(6) {
        return prop
            .get("value")
            .and_then(|value| json_str(value, "content").map(|content| (value, content)))
            .map(|(value, content)| {
                json!({
                    "kind": "simple",
                    "content": content,
                    "isStatic": true,
                    "constType": 3,
                    "loc": value.get("loc").cloned().unwrap_or(Value::Null),
                })
            });
    }

    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
        return Some(exp.clone());
    }

    let content = if json_bool(context, "prefixIdentifiers") {
        rewrite_js_like_expression("is", &vue3_options_from_transform_context(context))
    } else {
        "is".to_string()
    };
    Some(json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": 0,
        "loc": prop
            .get("arg")
            .and_then(|arg| arg.get("loc"))
            .cloned()
            .unwrap_or(Value::Null),
    }))
}

pub(crate) fn vue3_core_component_helper(tag: &str) -> Option<&'static str> {
    match tag {
        "Teleport" | "teleport" => Some("TELEPORT"),
        "Suspense" | "suspense" => Some("SUSPENSE"),
        "KeepAlive" | "keep-alive" => Some("KEEP_ALIVE"),
        "BaseTransition" | "base-transition" => Some("BASE_TRANSITION"),
        _ => None,
    }
}

pub(crate) fn resolve_setup_reference(name: &str, context: &Value) -> Option<Value> {
    let bindings = context.get("bindingMetadata")?;
    if context.get("isScriptSetup").and_then(Value::as_bool) == Some(false) {
        return None;
    }

    let camel_name = camelize(name);
    let pascal_name = capitalize(&camel_name);
    let from_const = binding_with_type(
        bindings,
        &[name, &camel_name, &pascal_name],
        &["setup-const", "setup-reactive-const", "literal-const"],
    );
    if let Some(name) = from_const {
        return Some(json!({
            "kind": "expression",
            "content": if json_bool(context, "inline") {
                name.to_string()
            } else {
                format!("$setup[{}]", quote_string(name))
            },
        }));
    }

    let from_maybe_ref = binding_with_type(
        bindings,
        &[name, &camel_name, &pascal_name],
        &["setup-let", "setup-ref", "setup-maybe-ref"],
    );
    if let Some(name) = from_maybe_ref {
        return Some(json!({
            "kind": "expression",
            "content": if json_bool(context, "inline") {
                format!("_unref({name})")
            } else {
                format!("$setup[{}]", quote_string(name))
            },
            "helpers": if json_bool(context, "inline") {
                json!(["UNREF"])
            } else {
                json!([])
            },
        }));
    }

    let from_props = binding_with_type(bindings, &[name, &camel_name, &pascal_name], &["props"]);
    if let Some(name) = from_props {
        return Some(json!({
            "kind": "expression",
            "content": format!(
                "_unref({}[{}])",
                if json_bool(context, "inline") { "__props" } else { "$props" },
                quote_string(name),
            ),
            "helpers": ["UNREF"],
        }));
    }

    None
}

pub(crate) fn binding_with_type<'a>(
    bindings: &'a Value,
    names: &[&'a str],
    types: &[&str],
) -> Option<&'a str> {
    names.iter().copied().find(|name| {
        bindings
            .get(*name)
            .and_then(Value::as_str)
            .is_some_and(|binding_type| types.contains(&binding_type))
    })
}

pub(crate) fn transform_if_process_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let siblings = payload
        .get("siblings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let node_index = payload
        .get("nodeIndex")
        .and_then(Value::as_u64)
        .map(|index| index as usize);
    let dir_name = json_str(dir, "name").unwrap_or("");
    let mut errors = Vec::<Value>::new();
    let condition = transform_if_condition_projection(dir, node, context, &mut errors);
    let branch = json!({
        "condition": condition,
        "children": if json_u64(node, "tagType") == Some(3) && !json_node_has_directive(node, "for") {
            "template"
        } else {
            "self"
        },
        "isTemplateIf": json_u64(node, "tagType") == Some(3),
    });

    if dir_name == "if" {
        return json!({
            "errors": errors,
            "branch": branch,
            "action": {
                "kind": "create",
                "keyBase": node_index
                    .map(|index| transform_if_previous_key_base(siblings, index))
                    .unwrap_or_default(),
            },
        });
    }

    let Some(node_index) = node_index else {
        errors.push(json!({ "code": 30, "loc": "node" }));
        return json!({
            "errors": errors,
            "branch": branch,
            "action": { "kind": "noop" },
        });
    };

    let mut remove_indices = Vec::<usize>::new();
    let mut comment_indices = Vec::<usize>::new();
    let mut scan_index = node_index as isize - 1;
    while scan_index >= 0 {
        let index = scan_index as usize;
        let sibling = &siblings[index];
        if transform_if_is_comment_or_whitespace(sibling) {
            remove_indices.push(index);
            if json_node_type(sibling) == Some(3) {
                comment_indices.insert(0, index);
            }
            scan_index -= 1;
            continue;
        }

        if json_node_type(sibling) == Some(9) {
            if transform_if_last_branch_is_else(sibling) {
                errors.push(json!({ "code": 30, "loc": "node" }));
            }
            let current_key = payload.get("currentUserKey").unwrap_or(&Value::Null);
            if !current_key.is_null() {
                for branch in sibling
                    .get("branches")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if transform_if_same_key(
                        branch.get("userKey").unwrap_or(&Value::Null),
                        current_key,
                    ) {
                        errors.push(json!({ "code": 29, "loc": "userKey" }));
                    }
                }
            }
            let parent = payload.get("parent").unwrap_or(&Value::Null);
            if transform_if_parent_is_transition(parent) {
                comment_indices.clear();
            }
            return json!({
                "errors": errors,
                "branch": branch,
                "action": {
                    "kind": "append",
                    "targetIndex": index,
                    "removeIndices": remove_indices,
                    "commentIndices": comment_indices,
                },
            });
        }

        errors.push(json!({ "code": 30, "loc": "node" }));
        return json!({
            "errors": errors,
            "branch": branch,
            "action": { "kind": "noop" },
        });
    }

    errors.push(json!({ "code": 30, "loc": "node" }));
    json!({
        "errors": errors,
        "branch": branch,
        "action": { "kind": "noop" },
    })
}

pub(crate) fn transform_if_condition_projection(
    dir: &Value,
    node: &Value,
    context: &Value,
    errors: &mut Vec<Value>,
) -> Value {
    if json_str(dir, "name") == Some("else") {
        return Value::Null;
    }
    let exp = dir.get("exp").filter(|value| !value.is_null());
    let raw_content = exp.and_then(|exp| json_str(exp, "content")).unwrap_or("");
    let missing = exp.is_none() || raw_content.trim().is_empty();
    if missing {
        errors.push(json!({ "code": 28, "loc": "dir" }));
        return json!({
            "kind": "simple",
            "content": "true",
            "isStatic": false,
            "constType": 0,
            "loc": exp
                .and_then(|exp| exp.get("loc"))
                .or_else(|| node.get("loc"))
                .cloned()
                .unwrap_or(Value::Null),
        });
    }

    if !json_bool(context, "prefixIdentifiers") {
        return Value::Null;
    }

    let options = vue3_options_from_transform_context(context);
    let locals = transform_context_locals(context);
    let rewritten = if locals.is_empty() {
        rewrite_js_like_expression(raw_content, &options)
    } else {
        rewrite_js_like_expression_with_locals(raw_content, &options, &locals)
    };
    json!({
        "kind": "simple",
        "content": rewritten,
        "isStatic": false,
        "constType": 0,
        "loc": exp
            .and_then(|exp| exp.get("loc"))
            .cloned()
            .unwrap_or(Value::Null),
    })
}

pub(crate) fn transform_if_branch_codegen_projection(payload: &Value) -> Value {
    let branch = payload.get("branch").unwrap_or(&Value::Null);
    let children = branch
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let first = children.first();
    let need_fragment_wrapper = children.len() != 1
        || first
            .and_then(|child| json_node_type(child))
            .is_some_and(|node_type| node_type != 1);
    if need_fragment_wrapper {
        if children.len() == 1 && first.and_then(|child| json_node_type(child)) == Some(11) {
            return json!({ "kind": "for" });
        }
        let mut patch_flag = 64u16;
        if !json_bool(branch, "isTemplateIf")
            && children
                .iter()
                .filter(|child| json_node_type(child) != Some(3))
                .count()
                == 1
        {
            patch_flag |= 2048;
        }
        return json!({
            "kind": "fragment",
            "patchFlag": patch_flag,
        });
    }

    json!({
        "kind": "single",
        "convertToBlock": first
            .and_then(|child| json_u64(child, "memoedCodegenType"))
            == Some(13),
    })
}

pub(crate) fn transform_if_previous_key_base(siblings: &[Value], node_index: usize) -> usize {
    siblings
        .iter()
        .take(node_index)
        .filter(|sibling| json_node_type(sibling) == Some(9))
        .map(|sibling| {
            sibling
                .get("branches")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or_default()
        })
        .sum()
}

pub(crate) fn transform_if_is_comment_or_whitespace(node: &Value) -> bool {
    match json_node_type(node) {
        Some(3) => true,
        Some(2) => {
            let content_is_ascii_whitespace =
                json_str(node, "content").is_some_and(transform_if_is_ascii_html_whitespace);
            let loc_is_ascii_whitespace = json_str(node, "locSource")
                .map(transform_if_is_ascii_html_whitespace)
                .unwrap_or(true);
            content_is_ascii_whitespace && loc_is_ascii_whitespace
        }
        Some(12) => node
            .get("content")
            .is_some_and(transform_if_is_comment_or_whitespace),
        _ => false,
    }
}

pub(crate) fn transform_if_is_ascii_html_whitespace(content: &str) -> bool {
    content
        .bytes()
        .all(|byte| matches!(byte, b'\t' | b'\n' | b'\x0C' | b'\r' | b' '))
}

pub(crate) fn transform_if_last_branch_is_else(if_node: &Value) -> bool {
    if_node
        .get("branches")
        .and_then(Value::as_array)
        .and_then(|branches| branches.last())
        .is_some_and(|branch| !json_bool(branch, "hasCondition"))
}

pub(crate) fn transform_if_same_key(a: &Value, b: &Value) -> bool {
    if a.is_null() || b.is_null() || json_node_type(a) != json_node_type(b) {
        return false;
    }
    match json_node_type(a) {
        Some(6) => {
            a.get("value").and_then(|value| json_str(value, "content"))
                == b.get("value").and_then(|value| json_str(value, "content"))
        }
        Some(7) => {
            let a_exp = a.get("exp").unwrap_or(&Value::Null);
            let b_exp = b.get("exp").unwrap_or(&Value::Null);
            json_node_type(a_exp) == json_node_type(b_exp)
                && json_bool(a_exp, "isStatic") == json_bool(b_exp, "isStatic")
                && json_str(a_exp, "content") == json_str(b_exp, "content")
        }
        _ => false,
    }
}

pub(crate) fn transform_if_parent_is_transition(parent: &Value) -> bool {
    json_node_type(parent) == Some(1)
        && matches!(json_str(parent, "tag"), Some("transition" | "Transition"))
}

pub(crate) fn json_node_has_directive(node: &Value, name: &str) -> bool {
    node.get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props
                .iter()
                .any(|prop| json_node_type(prop) == Some(7) && json_str(prop, "name") == Some(name))
        })
}

pub(crate) fn vue3_options_from_transform_context(context: &Value) -> Vue3CompilerOptions {
    let mut options = Vue3CompilerOptions {
        prefix_identifiers: json_bool(context, "prefixIdentifiers"),
        inline: json_bool(context, "inline"),
        is_ts: json_bool(context, "isTS"),
        ..Vue3CompilerOptions::default()
    };
    if let Some(metadata) = context.get("bindingMetadata").and_then(Value::as_object) {
        for (key, value) in metadata {
            if key == "__propsAliases" {
                if let Some(aliases) = value.as_object() {
                    options.props_aliases = aliases
                        .iter()
                        .filter_map(|(alias, source)| {
                            source
                                .as_str()
                                .map(|source| (alias.clone(), source.to_string()))
                        })
                        .collect();
                }
            } else if let Some(kind) = value.as_str() {
                options
                    .binding_metadata
                    .insert(key.clone(), kind.to_string());
            }
        }
    }
    options
}

pub(crate) fn transform_context_locals(context: &Value) -> Vec<String> {
    context
        .get("identifiers")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|identifiers| identifiers.iter())
        .filter(|(_, count)| count.as_i64().unwrap_or_default() > 0)
        .map(|(name, _)| name.clone())
        .collect()
}

pub(crate) fn model_assignment_projection(
    exp: &Value,
    raw_exp: &str,
    event_arg: &str,
    binding_type: Option<&str>,
    maybe_ref: bool,
) -> Value {
    if maybe_ref {
        if binding_type == Some("setup-ref") {
            return json!({
                "kind": "compound",
                "children": [
                    format!("{event_arg} => (("),
                    { "kind": "simple", "content": raw_exp, "isStatic": false, "loc": exp.get("loc").cloned().unwrap_or(Value::Null) },
                    ").value = $event)"
                ]
            });
        }
        let alt_assignment = if binding_type == Some("setup-let") {
            format!("{raw_exp} = $event")
        } else {
            "null".to_string()
        };
        return json!({
            "kind": "compound",
            "children": [
                format!("{event_arg} => (_isRef({raw_exp}) ? ("),
                { "kind": "simple", "content": raw_exp, "isStatic": false, "loc": exp.get("loc").cloned().unwrap_or(Value::Null) },
                format!(").value = $event : {alt_assignment})")
            ],
            "helpers": ["IS_REF"]
        });
    }

    json!({
        "kind": "compound",
        "children": [
            format!("{event_arg} => (("),
            { "kind": "node", "path": "dir.exp" },
            ") = $event)"
        ]
    })
}

pub(crate) fn render_inline_model_assignment(
    raw: &str,
    event_arg: &str,
    binding_type: Option<&str>,
    options: &Vue3CompilerOptions,
    fallback_target: impl FnOnce() -> String,
) -> String {
    if !options.inline || !is_simple_identifier_ascii(raw) {
        let target = fallback_target();
        return format!("{event_arg} => (({target}) = $event)");
    }
    match binding_type {
        Some("setup-ref") => format!("{event_arg} => (({raw}).value = $event)"),
        Some("setup-maybe-ref") => {
            format!("{event_arg} => (_isRef({raw}) ? ({raw}).value = $event : null)")
        }
        Some("setup-let") => {
            format!("{event_arg} => (_isRef({raw}) ? ({raw}).value = $event : {raw} = $event)")
        }
        _ => {
            let target = fallback_target();
            format!("{event_arg} => (({target}) = $event)")
        }
    }
}

pub(crate) fn model_prop_name_projection(arg: Option<&Value>) -> Value {
    match arg {
        Some(_) => json!({ "kind": "node", "path": "dir.arg" }),
        None => json!({ "kind": "static", "content": "modelValue" }),
    }
}

pub(crate) fn model_event_name_projection(arg: Option<&Value>) -> Value {
    match arg {
        Some(arg) if json_bool(arg, "isStatic") => json!({
            "kind": "static",
            "content": format!("onUpdate:{}", camelize(json_str(arg, "content").unwrap_or(""))),
        }),
        Some(_) => json!({
            "kind": "compound",
            "children": [
                "\"onUpdate:\" + ",
                { "kind": "node", "path": "dir.arg" }
            ],
        }),
        None => json!({ "kind": "static", "content": "onUpdate:modelValue" }),
    }
}

pub(crate) fn model_update_needs_hydration_event(arg: Option<&Value>, node: &Value) -> bool {
    arg.is_some_and(|arg| json_bool(arg, "isStatic")) && json_u64(node, "tagType") != Some(1)
}

pub(crate) fn model_modifiers_key_projection(arg: Option<&Value>) -> Value {
    match arg {
        Some(arg) if json_bool(arg, "isStatic") => json!({
            "kind": "static",
            "content": format!("{}Modifiers", json_str(arg, "content").unwrap_or("")),
        }),
        Some(_) => json!({
            "kind": "compound",
            "children": [
                { "kind": "node", "path": "dir.arg" },
                " + \"Modifiers\""
            ],
        }),
        None => json!({ "kind": "static", "content": "modelModifiers" }),
    }
}

pub(crate) fn model_modifiers_expression(dir: &Value) -> Value {
    let modifiers = dir
        .get("modifiers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|modifier| json_str(modifier, "content"))
        .map(|modifier| {
            if is_simple_identifier_ascii(modifier) {
                format!("{modifier}: true")
            } else {
                format!("{}: true", quote_string(modifier))
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    json!({
        "kind": "simple",
        "content": format!("{{ {modifiers} }}"),
        "isStatic": false,
        "constType": 2,
    })
}

pub(crate) fn should_cache_model_update(exp: &Value, context: &Value) -> bool {
    json_bool(context, "prefixIdentifiers")
        && json_bool(context, "cacheHandlers")
        && !json_bool(context, "inVOnce")
        && !model_has_scope_ref(exp, context)
}

pub(crate) fn model_has_scope_ref(exp: &Value, context: &Value) -> bool {
    let source = model_expression_source(exp);
    context
        .get("identifiers")
        .and_then(Value::as_object)
        .is_some_and(|identifiers| {
            identifiers.iter().any(|(name, count)| {
                count.as_i64().unwrap_or_default() > 0 && source.contains(name)
            })
        })
}

pub(crate) fn model_expression_source(exp: &Value) -> String {
    if let Some(content) = json_str(exp, "content") {
        return content.to_string();
    }
    if let Some(children) = exp.get("children").and_then(Value::as_array) {
        return children
            .iter()
            .map(model_expression_child_source)
            .collect::<String>();
    }
    exp.get("loc")
        .and_then(|loc| loc.get("source"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(crate) fn model_expression_child_source(child: &Value) -> String {
    child
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| model_expression_source(child))
}

pub(crate) fn transform_bind_key_projection(
    arg: Option<&Value>,
    dir: &Value,
    context: &Value,
) -> Value {
    let mut key = transform_bind_guarded_arg_projection(arg, dir);
    if directive_has_modifier(dir, "camel") {
        key = transform_bind_camel_projection(key);
    }
    if !json_bool(context, "inSSR") {
        if directive_has_modifier(dir, "prop") {
            key = transform_bind_prefix_projection(key, ".");
        }
        if directive_has_modifier(dir, "attr") {
            key = transform_bind_prefix_projection(key, "^");
        }
    }
    key
}

pub(crate) fn transform_bind_raw_arg_projection(arg: Option<&Value>, dir: &Value) -> Value {
    let loc = arg
        .and_then(|arg| arg.get("loc").cloned())
        .unwrap_or_else(|| dir.get("loc").cloned().unwrap_or(Value::Null));
    match arg {
        Some(_) => json!({ "kind": "node", "path": "dir.arg", "loc": loc }),
        None => json!({
            "kind": "simple",
            "content": "",
            "isStatic": true,
            "loc": loc,
        }),
    }
}

pub(crate) fn transform_bind_guarded_arg_projection(arg: Option<&Value>, dir: &Value) -> Value {
    let loc = arg
        .and_then(|arg| arg.get("loc").cloned())
        .unwrap_or_else(|| dir.get("loc").cloned().unwrap_or(Value::Null));
    let Some(arg) = arg else {
        return json!({
            "kind": "simple",
            "content": "",
            "isStatic": true,
            "loc": loc,
        });
    };

    if json_node_type(arg) == Some(4) {
        let content = json_str(arg, "content").unwrap_or("");
        if json_bool(arg, "isStatic") {
            return json!({
                "kind": "simple",
                "content": content,
                "isStatic": true,
                "loc": loc,
            });
        }
        return json!({
            "kind": "simple",
            "content": if content.is_empty() { "\"\"".to_string() } else { format!("{content} || \"\"") },
            "isStatic": false,
            "loc": loc,
            "constType": arg.get("constType").cloned().unwrap_or(json!(0)),
        });
    }

    json!({
        "kind": "compound",
        "children": [
            "(",
            { "kind": "node", "path": "dir.arg.children" },
            ") || \"\"",
        ],
        "loc": loc,
        "constType": arg.get("constType").cloned().unwrap_or(json!(0)),
    })
}

pub(crate) fn transform_bind_empty_expression_value(dir: &Value) -> Value {
    json!({
        "kind": "simple",
        "content": "",
        "isStatic": true,
        "loc": dir.get("loc").cloned().unwrap_or(Value::Null),
    })
}

pub(crate) fn transform_bind_camel_projection(key: Value) -> Value {
    match json_str(&key, "kind") {
        Some("simple") if json_bool(&key, "isStatic") => {
            let mut next = key;
            let content = json_str(&next, "content").unwrap_or("").to_string();
            next["content"] = json!(camelize(&content));
            next
        }
        Some("simple") => {
            let mut next = key;
            let content = json_str(&next, "content").unwrap_or("").to_string();
            next["content"] = json!(format!("_camelize({content})"));
            next["helpers"] = json!(["CAMELIZE"]);
            next
        }
        Some("compound") => {
            let children = key.get("children").cloned().unwrap_or_else(|| json!([]));
            let mut next = key;
            next["children"] = json!([
                { "kind": "helperString", "helper": "CAMELIZE" },
                { "kind": "children", "children": children },
                ")",
            ]);
            next
        }
        _ => key,
    }
}

pub(crate) fn transform_bind_prefix_projection(key: Value, prefix: &str) -> Value {
    match json_str(&key, "kind") {
        Some("simple") if json_bool(&key, "isStatic") => {
            let mut next = key;
            let content = json_str(&next, "content").unwrap_or("").to_string();
            next["content"] = json!(format!("{prefix}{content}"));
            next
        }
        Some("simple") => {
            let mut next = key;
            let content = json_str(&next, "content").unwrap_or("").to_string();
            next["content"] = json!(format!("`{prefix}${{{content}}}`"));
            next
        }
        Some("compound") => {
            let children = key.get("children").cloned().unwrap_or_else(|| json!([]));
            let mut next = key;
            next["children"] = json!([
                format!("'{prefix}' + ("),
                { "kind": "children", "children": children },
                ")",
            ]);
            next
        }
        _ => key,
    }
}

pub(crate) fn transform_v_bind_shorthand_operation(
    index: usize,
    prop: &Value,
    browser: bool,
) -> Option<Value> {
    if json_node_type(prop) != Some(7)
        || json_str(prop, "name") != Some("bind")
        || prop.get("arg").is_none_or(Value::is_null)
        || !transform_v_bind_shorthand_needs_expansion(prop, browser)
    {
        return None;
    }

    let arg = prop.get("arg").unwrap_or(&Value::Null);
    let loc = arg.get("loc").cloned().unwrap_or(Value::Null);
    if json_node_type(arg) != Some(4) || !json_bool(arg, "isStatic") {
        return Some(json!({
            "kind": "setExp",
            "index": index,
            "exp": {
                "kind": "simple",
                "content": "",
                "isStatic": true,
                "loc": loc,
            },
            "errors": [{ "code": 53, "loc": "arg" }],
        }));
    }

    let prop_name = camelize(json_str(arg, "content").unwrap_or(""));
    if !transform_v_bind_shorthand_valid_first_char(&prop_name) {
        return None;
    }
    Some(json!({
        "kind": "setExp",
        "index": index,
        "exp": {
            "kind": "simple",
            "content": prop_name,
            "isStatic": false,
            "loc": loc,
        },
        "errors": [],
    }))
}

pub(crate) fn transform_v_bind_shorthand_needs_expansion(prop: &Value, browser: bool) -> bool {
    match prop.get("exp").filter(|value| !value.is_null()) {
        None => true,
        Some(exp) => {
            browser
                && json_node_type(exp) == Some(4)
                && json_str(exp, "content").unwrap_or("").trim().is_empty()
        }
    }
}

pub(crate) fn transform_v_bind_shorthand_valid_first_char(value: &str) -> bool {
    value.chars().next().is_some_and(|ch| {
        ch == '-' || ch == '_' || ch == '$' || ch.is_ascii_alphabetic() || ch >= '\u{00a0}'
    })
}

pub(crate) fn directive_has_modifier(dir: &Value, name: &str) -> bool {
    dir.get("modifiers")
        .and_then(Value::as_array)
        .is_some_and(|modifiers| {
            modifiers.iter().any(|modifier| {
                modifier.as_str().or_else(|| json_str(modifier, "content")) == Some(name)
            })
        })
}

pub(crate) fn transform_on_event_name_projection(
    arg: Option<&Value>,
    node: &Value,
    errors: &mut Vec<Value>,
) -> Value {
    let Some(arg) = arg else {
        return json!({ "kind": "static", "content": "on" });
    };
    if json_node_type(arg) == Some(4) {
        if json_bool(arg, "isStatic") {
            let mut raw_name = json_str(arg, "content").unwrap_or("").to_string();
            if raw_name.starts_with("vnode") {
                errors.push(json!({ "code": 52, "loc": "arg" }));
            }
            if let Some(rest) = raw_name.strip_prefix("vue:") {
                raw_name = format!("vnode-{rest}");
            }
            let event_string = if json_u64(node, "tagType") != Some(0)
                || raw_name.starts_with("vnode")
                || !raw_name.chars().any(|ch| ch.is_ascii_uppercase())
            {
                to_handler_key(&camelize(&raw_name))
            } else {
                format!("on:{raw_name}")
            };
            return json!({
                "kind": "simple",
                "content": event_string,
                "isStatic": true,
                "loc": arg.get("loc").cloned().unwrap_or(Value::Null),
            });
        }
        return json!({
            "kind": "compound",
            "children": [
                { "kind": "helperString", "helper": "TO_HANDLER_KEY" },
                { "kind": "node", "path": "dir.arg" },
                ")",
            ],
        });
    }
    json!({
        "kind": "compound",
        "children": [
            { "kind": "helperString", "helper": "TO_HANDLER_KEY" },
            { "kind": "node", "path": "dir.arg.children" },
            ")",
        ],
        "loc": arg.get("loc").cloned().unwrap_or(Value::Null),
    })
}

pub(crate) fn transform_on_handler_projection(dir: &Value, node: &Value, context: &Value) -> Value {
    let Some(exp) = dir.get("exp").filter(|value| !value.is_null()) else {
        return json!({ "cache": json_bool(context, "cacheHandlers") && !json_bool(context, "inVOnce") });
    };
    let raw = transform_on_expression_source(exp);
    if raw.trim().is_empty() {
        return json!({ "cache": json_bool(context, "cacheHandlers") && !json_bool(context, "inVOnce") });
    }

    let is_member = transform_on_is_member_expression(&raw, context);
    let is_fn = transform_on_is_fn_expression(&raw, context);
    let is_inline = !is_member && !is_fn;
    let has_multiple_statements = raw.contains(';');
    let mut processed = json!({ "kind": "node", "path": "dir.exp" });
    let mut should_cache = false;

    if json_bool(context, "prefixIdentifiers") {
        let options = vue3_options_from_transform_context(context);
        let mut locals = transform_context_locals(context);
        if is_inline {
            locals.push("$event".to_string());
        }
        processed = transform_on_rewrite_expression_node(
            &raw,
            exp,
            &options,
            &locals,
            has_multiple_statements,
        );
        should_cache = json_bool(context, "cacheHandlers")
            && !json_bool(context, "inVOnce")
            && transform_on_projection_const_type(&processed) == 0
            && !(is_member && json_u64(node, "tagType") == Some(1))
            && !transform_on_has_scope_ref(&processed, context);
        if should_cache && is_member {
            processed = transform_on_member_invocation_projection(processed);
        }
    }

    if is_inline || (should_cache && is_member) {
        processed = transform_on_wrap_handler_projection(
            processed,
            is_inline,
            has_multiple_statements,
            json_bool(context, "isTS"),
        );
    }

    json!({
        "value": processed,
        "cache": should_cache,
        "isInlineStatement": is_inline,
        "isMemberExpression": is_member,
        "isFunctionExpression": is_fn,
    })
}

pub(crate) fn transform_on_empty_handler_projection(dir: &Value) -> Value {
    json!({
        "kind": "simple",
        "content": "() => {}",
        "isStatic": false,
        "loc": dir.get("loc").cloned().unwrap_or(Value::Null),
    })
}

pub(crate) fn transform_on_rewrite_expression_node(
    raw: &str,
    exp: &Value,
    options: &Vue3CompilerOptions,
    locals: &[String],
    as_raw_statements: bool,
) -> Value {
    let trimmed = raw.trim();
    let loc = exp.get("loc").cloned().unwrap_or(Value::Null);
    let mut effective_locals = locals.to_vec();
    effective_locals.extend(transform_on_root_function_locals(raw));
    effective_locals.sort();
    effective_locals.dedup();
    let rewritten = if effective_locals.is_empty() {
        rewrite_js_like_expression(raw, options)
    } else {
        rewrite_js_like_expression_with_locals(raw, options, &effective_locals)
    };
    let children = process_expression_compound_children(raw, options, &effective_locals, &loc);
    let const_type = transform_on_const_type(trimmed, rewritten.trim(), options);
    if is_simple_identifier_ascii(trimmed) || (children.is_empty() && !as_raw_statements) {
        return transform_on_simple_projection(rewritten.trim(), exp, const_type);
    }
    let helpers = vue3_for_helpers_for_content(&rewritten);
    let mut value = json!({
        "kind": "compound",
        "children": children,
        "loc": loc,
        "constType": const_type,
    });
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

pub(crate) fn transform_on_simple_projection(content: &str, exp: &Value, const_type: u8) -> Value {
    let mut value = json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": const_type,
        "loc": exp.get("loc").cloned().unwrap_or(Value::Null),
    });
    let helpers = vue3_for_helpers_for_content(content);
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

pub(crate) fn transform_on_const_type(
    raw: &str,
    rewritten: &str,
    options: &Vue3CompilerOptions,
) -> u8 {
    if is_simple_identifier_ascii(raw)
        && matches!(
            options.binding_metadata.get(raw).map(String::as_str),
            Some("setup-const" | "literal-const")
        )
    {
        return 1;
    }
    vue3_for_const_type(rewritten)
}

pub(crate) fn transform_on_member_invocation_projection(processed: Value) -> Value {
    match json_str(&processed, "kind") {
        Some("simple") => {
            let content = json_str(&processed, "content").unwrap_or("").to_string();
            let mut next = processed;
            next["content"] = json!(format!("{content} && {content}(...args)"));
            next
        }
        Some("compound") => {
            let children = processed
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut next_children = children.clone();
            next_children.push(json!(" && "));
            next_children.extend(children);
            next_children.push(json!("(...args)"));
            let mut next = processed;
            next["children"] = json!(next_children);
            next
        }
        _ => processed,
    }
}

pub(crate) fn transform_on_wrap_handler_projection(
    processed: Value,
    is_inline: bool,
    has_multiple_statements: bool,
    is_ts: bool,
) -> Value {
    let param = if is_inline {
        if is_ts {
            "($event: any)"
        } else {
            "$event"
        }
    } else if is_ts {
        "\n//@ts-ignore\n(...args)"
    } else {
        "(...args)"
    };
    json!({
        "kind": "compound",
        "children": [
            format!("{param} => {}", if has_multiple_statements { "{" } else { "(" }),
            processed,
            if has_multiple_statements { "}" } else { ")" },
        ],
    })
}

pub(crate) fn transform_on_expression_source(exp: &Value) -> String {
    if let Some(content) = json_str(exp, "content") {
        return content.to_string();
    }
    exp.get("loc")
        .and_then(|loc| json_str(loc, "source"))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| model_expression_source(exp))
}

pub(crate) fn process_expression_locals(payload: &Value, context: &Value) -> Vec<String> {
    if let Some(locals) = payload.get("localVars").and_then(Value::as_object) {
        return locals
            .iter()
            .filter(|(_, count)| count.as_i64().unwrap_or(1) > 0)
            .map(|(name, _)| name.clone())
            .collect();
    }
    transform_context_locals(context)
}

pub(crate) fn process_expression_is_const_binding(
    raw: &str,
    options: &Vue3CompilerOptions,
) -> bool {
    matches!(
        options.binding_metadata.get(raw).map(String::as_str),
        Some("setup-const" | "literal-const")
    )
}

pub(crate) fn process_expression_is_static_literal(raw: &str) -> bool {
    let trimmed = raw.trim();
    matches!(trimmed, "true" | "false" | "null" | "this")
        || trimmed.ends_with('n')
            && trimmed[..trimmed.len().saturating_sub(1)]
                .parse::<i128>()
                .is_ok()
        || trimmed.parse::<f64>().is_ok()
}

pub(crate) fn process_expression_uses_supported_external_plugin(
    raw: &str,
    context: &Value,
) -> bool {
    raw.contains("|>")
        && context
            .get("expressionPlugins")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|plugin| {
                plugin
                    .as_str()
                    .is_some_and(|name| name == "pipelineOperator")
                    || plugin
                        .as_array()
                        .and_then(|items| items.first())
                        .and_then(Value::as_str)
                        == Some("pipelineOperator")
            })
}

pub(crate) fn process_expression_rewrite_source(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
) -> String {
    let mut identifiers = process_expression_identifier_spans(raw, options, locals);
    identifiers.sort_by_key(|identifier| (identifier.start, identifier.end));
    let mut filtered = Vec::<ProcessExpressionIdentifier>::new();
    for identifier in identifiers {
        if identifier.start >= identifier.end || identifier.end > raw.len() {
            continue;
        }
        if filtered
            .last()
            .is_some_and(|previous| identifier.start < previous.end)
        {
            continue;
        }
        filtered.push(identifier);
    }

    if filtered.is_empty() {
        return raw.to_string();
    }

    let mut output = String::new();
    let mut last_end = 0usize;
    for identifier in &filtered {
        output.push_str(raw.get(last_end..identifier.start).unwrap_or_default());
        if let Some(prefix) = &identifier.prefix {
            output.push_str(prefix);
        }
        let content = parenthesize_rewritten_identifier_for_new_expression(
            raw,
            identifier.start,
            identifier.end,
            &identifier.content,
        );
        output.push_str(&content);
        last_end = identifier.end;
    }
    output.push_str(raw.get(last_end..).unwrap_or_default());
    output
}

pub(crate) fn parenthesize_rewritten_identifier_for_new_expression(
    raw: &str,
    start: usize,
    end: usize,
    content: &str,
) -> String {
    if !content.starts_with("_unref(") || !process_expression_is_in_new_expression(raw, start) {
        return content.to_string();
    }
    match next_non_ws(raw, end) {
        Some('.') | Some('(') => format!("({content})"),
        _ => content.to_string(),
    }
}

pub(crate) fn process_expression_compound_children(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
    loc: &Value,
) -> Vec<Value> {
    let mut identifiers = process_expression_identifier_spans(raw, options, locals);
    identifiers.sort_by_key(|identifier| (identifier.start, identifier.end));
    let mut filtered = Vec::<ProcessExpressionIdentifier>::new();
    for identifier in identifiers {
        if identifier.start >= identifier.end || identifier.end > raw.len() {
            continue;
        }
        if filtered
            .last()
            .is_some_and(|previous| identifier.start < previous.end)
        {
            continue;
        }
        filtered.push(identifier);
    }

    let mut children = Vec::new();
    for (index, identifier) in filtered.iter().enumerate() {
        let leading_start = filtered
            .get(index.wrapping_sub(1))
            .map(|last| last.end)
            .unwrap_or(0);
        if leading_start < identifier.start || identifier.prefix.is_some() {
            children.push(json!(format!(
                "{}{}",
                raw.get(leading_start..identifier.start).unwrap_or_default(),
                identifier.prefix.as_deref().unwrap_or("")
            )));
        }
        let source = raw
            .get(identifier.start..identifier.end)
            .unwrap_or_default()
            .to_string();
        children.push(json!({
            "kind": "simple",
            "content": identifier.content,
            "isStatic": false,
            "constType": if identifier.is_constant { 3 } else { 0 },
            "loc": vue3_for_child_loc(loc, raw, identifier.start, identifier.end),
        }));
        if index + 1 == filtered.len() && identifier.end < raw.len() {
            children.push(json!(raw[identifier.end..].to_string()));
        }
        let _ = source;
    }
    children
}

pub(crate) fn process_expression_params_projection(
    raw: &str,
    node: &Value,
    context: &Value,
    options: &Vue3CompilerOptions,
) -> Value {
    let source = format!("({raw})=>{{}}");
    let store = JsAstStore::new();
    if store
        .parse_expression(&source, transform_on_source_type(context))
        .is_err()
    {
        return json!({
            "kind": "error",
            "code": 46,
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
            "message": "Error parsing JavaScript expression: Unexpected token",
        });
    }
    let children =
        process_expression_params_children(raw, options, node.get("loc").unwrap_or(&Value::Null));
    if children.is_empty() {
        return json!({
            "kind": "setConstType",
            "constType": 3,
        });
    }
    let identifiers = vue3_for_alias_locals(raw);
    let mut helper_source = String::new();
    for child in &children {
        if let Some(content) = child.get("content").and_then(Value::as_str) {
            helper_source.push_str(content);
        }
    }
    json!({
        "kind": "compound",
        "children": children,
        "loc": node.get("loc").cloned().unwrap_or(Value::Null),
        "identifiers": identifiers,
        "helpers": vue3_for_helpers_for_content(&helper_source),
    })
}

pub(crate) fn process_expression_params_children(
    raw: &str,
    options: &Vue3CompilerOptions,
    loc: &Value,
) -> Vec<Value> {
    let mut identifiers = process_expression_param_identifier_spans(raw, (0, raw.len()), options);
    identifiers.sort_by_key(|identifier| (identifier.start, identifier.end));
    let mut filtered = Vec::<ProcessExpressionIdentifier>::new();
    for identifier in identifiers {
        if filtered
            .last()
            .is_some_and(|previous| identifier.start < previous.end)
        {
            continue;
        }
        filtered.push(identifier);
    }
    process_expression_children_from_identifiers(raw, loc, &filtered)
}

pub(crate) fn process_expression_children_from_identifiers(
    raw: &str,
    loc: &Value,
    identifiers: &[ProcessExpressionIdentifier],
) -> Vec<Value> {
    let mut children = Vec::new();
    for (index, identifier) in identifiers.iter().enumerate() {
        let leading_start = identifiers
            .get(index.wrapping_sub(1))
            .map(|last| last.end)
            .unwrap_or(0);
        if leading_start < identifier.start || identifier.prefix.is_some() {
            children.push(json!(format!(
                "{}{}",
                raw.get(leading_start..identifier.start).unwrap_or_default(),
                identifier.prefix.as_deref().unwrap_or("")
            )));
        }
        children.push(json!({
            "kind": "simple",
            "content": identifier.content,
            "isStatic": false,
            "constType": if identifier.is_constant { 3 } else { 0 },
            "loc": vue3_for_child_loc(loc, raw, identifier.start, identifier.end),
        }));
        if index + 1 == identifiers.len() && identifier.end < raw.len() {
            children.push(json!(raw[identifier.end..].to_string()));
        }
    }
    children
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessExpressionIdentifier {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) content: String,
    pub(crate) prefix: Option<String>,
    pub(crate) is_constant: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessExpressionArrowBinding {
    pub(crate) name: String,
    pub(crate) param_start: usize,
    pub(crate) param_end: usize,
    pub(crate) body_start: usize,
    pub(crate) body_end: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessExpressionAssignmentRhs<'a> {
    pub(crate) operator: &'a str,
    pub(crate) source: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProcessExpressionUpdate {
    pub(crate) operator: &'static str,
    pub(crate) prefix: bool,
}

pub(crate) fn process_expression_identifier_spans(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
) -> Vec<ProcessExpressionIdentifier> {
    let mut spans = Vec::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let chars = raw.char_indices().collect::<Vec<_>>();
    let mut index = 0usize;
    let arrow_bindings = process_expression_arrow_bindings(raw);
    while index < chars.len() {
        let start = chars[index].0;
        let ch = chars[index].1;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if !is_identifier_start(ch) {
            index += 1;
            continue;
        }
        index += 1;
        while index < chars.len() && is_identifier_continue(chars[index].1) {
            index += 1;
        }
        let end = chars.get(index).map_or(raw.len(), |(offset, _)| *offset);
        let ident = &raw[start..end];
        let prev = previous_non_ws(raw, start);
        let next = next_non_ws(raw, end);
        if is_keyword(ident) {
            continue;
        }
        if matches!(ident, "true" | "false" | "null" | "this") {
            continue;
        }
        let local = locals.iter().any(|local| local == ident);
        let property_key = next == Some(':') && prev != Some('?');
        let static_member = prev == Some('.');
        let function_name = process_expression_function_name(raw, start);
        let method_name = process_expression_method_name(raw, start, end);
        if method_name {
            continue;
        }
        let arrow_param = process_expression_is_arrow_param(&arrow_bindings, ident, start, end);
        let arrow_local = process_expression_is_arrow_local(&arrow_bindings, ident, start, end);
        let function_param =
            arrow_param || function_name || process_expression_is_function_param(raw, start);
        if property_key && !function_param {
            continue;
        }
        let is_global = is_global_or_literal(ident);
        let assignment_rhs = process_expression_assignment_rhs(raw, start, end);
        let update_argument = process_expression_update_argument(raw, start, end);
        let destructure_assignment = process_expression_is_destructure_assignment(raw, start);
        let content = if static_member || local || function_param || arrow_local || is_global {
            if !static_member
                && !local
                && !function_param
                && arrow_local
                && (assignment_rhs.is_some() || update_argument.is_some() || destructure_assignment)
                && options.binding_metadata.contains_key(ident)
            {
                process_expression_rewrite_identifier(
                    ident,
                    options,
                    assignment_rhs.as_ref(),
                    update_argument,
                    destructure_assignment,
                    locals,
                )
            } else {
                ident.to_string()
            }
        } else {
            process_expression_rewrite_identifier(
                ident,
                options,
                assignment_rhs.as_ref(),
                update_argument,
                destructure_assignment,
                locals,
            )
        };
        let (replacement_start, replacement_end) = if let Some(update) =
            update_argument.filter(|update| content != ident && content.contains(update.operator))
        {
            process_expression_update_range(raw, start, end, update).unwrap_or((start, end))
        } else {
            (start, end)
        };
        let object_shorthand = process_expression_object_shorthand(raw, start, end);
        let prefix = if property_key && content != ident
            || object_shorthand
                && (content != ident
                    || destructure_assignment
                        && options
                            .binding_metadata
                            .get(ident)
                            .is_some_and(|kind| kind == "setup-let"))
        {
            Some(format!("{ident}: "))
        } else {
            None
        };
        let dynamic_static_reference = (static_member || is_global)
            && process_expression_dynamic_static_reference(raw, start, end);
        spans.push(ProcessExpressionIdentifier {
            start: replacement_start,
            end: replacement_end,
            content,
            prefix,
            is_constant: ((static_member || is_global) && !dynamic_static_reference)
                || function_param
                || arrow_local,
        });
    }
    spans
}

pub(crate) fn process_expression_param_identifier_spans(
    raw: &str,
    range: (usize, usize),
    options: &Vue3CompilerOptions,
) -> Vec<ProcessExpressionIdentifier> {
    let mut spans = Vec::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let chars = raw[range.0..range.1]
        .char_indices()
        .map(|(offset, ch)| (range.0 + offset, ch))
        .collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        let start = chars[index].0;
        let ch = chars[index].1;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if !is_identifier_start(ch) {
            index += 1;
            continue;
        }
        index += 1;
        while index < chars.len() && is_identifier_continue(chars[index].1) {
            index += 1;
        }
        let end = chars.get(index).map_or(range.1, |(offset, _)| *offset);
        let ident = &raw[start..end];
        if is_keyword(ident) || next_non_ws(raw, end) == Some(':') {
            continue;
        }
        if process_expression_param_default_rhs(raw, range.0, start) {
            let content = if is_global_or_literal(ident) {
                ident.to_string()
            } else {
                process_expression_rewrite_identifier(ident, options, None, None, false, &[])
            };
            spans.push(ProcessExpressionIdentifier {
                start,
                end,
                content,
                prefix: None,
                is_constant: is_global_or_literal(ident),
            });
        } else {
            spans.push(ProcessExpressionIdentifier {
                start,
                end,
                content: ident.to_string(),
                prefix: None,
                is_constant: true,
            });
        }
    }
    spans
}

pub(crate) fn process_expression_function_name(raw: &str, start: usize) -> bool {
    let prefix = raw[..start].trim_end();
    prefix.ends_with("function") || prefix.ends_with("function*")
}

pub(crate) fn process_expression_method_name(raw: &str, start: usize, end: usize) -> bool {
    if !previous_non_ws(raw, start).is_some_and(|prev| matches!(prev, '{' | ',')) {
        return false;
    }
    let Some(open) = next_non_ws_index(raw, end).filter(|(_, ch)| *ch == '(') else {
        return false;
    };
    let Some(close) = find_matching_forward(raw, open.0, '(', ')') else {
        return false;
    };
    next_non_ws_index(raw, close + 1).is_some_and(|(_, ch)| ch == '{')
}

pub(crate) fn process_expression_is_function_param(raw: &str, start: usize) -> bool {
    let prefix = raw[..start].trim_end();
    let Some(open) = prefix.rfind('(') else {
        return false;
    };
    if prefix[open + 1..].contains(')') {
        return false;
    }
    let before_open = prefix[..open].trim_end();
    before_open.ends_with("function") || before_open.ends_with("function*")
}

pub(crate) fn process_expression_is_arrow_param(
    bindings: &[ProcessExpressionArrowBinding],
    ident: &str,
    start: usize,
    end: usize,
) -> bool {
    bindings.iter().any(|binding| {
        binding.name == ident && binding.param_start == start && binding.param_end == end
    })
}

pub(crate) fn process_expression_is_arrow_local(
    bindings: &[ProcessExpressionArrowBinding],
    ident: &str,
    start: usize,
    end: usize,
) -> bool {
    bindings.iter().any(|binding| {
        binding.name == ident && binding.body_start <= start && end <= binding.body_end
    })
}

pub(crate) fn process_expression_is_in_new_expression(raw: &str, start: usize) -> bool {
    let head = raw.get(..start).unwrap_or("").trim_end();
    if head.ends_with("new") {
        return head
            .strip_suffix("new")
            .and_then(|before| before.chars().next_back())
            .is_none_or(|ch| !is_identifier_continue(ch));
    }
    let mut depth = 0usize;
    for (index, ch) in head.char_indices().rev() {
        match ch {
            ')' | ']' => depth += 1,
            '(' | '[' => {
                depth = depth.saturating_sub(1);
            }
            '.' if depth == 0 => {
                return process_expression_is_in_new_expression(raw, index);
            }
            ch if ch.is_whitespace() && depth == 0 => {
                let prefix = head.get(..index).unwrap_or("").trim_end();
                return prefix.ends_with("new")
                    && prefix
                        .strip_suffix("new")
                        .and_then(|before| before.chars().next_back())
                        .is_none_or(|before| !is_identifier_continue(before));
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn process_expression_object_shorthand(raw: &str, start: usize, end: usize) -> bool {
    previous_non_ws(raw, start).is_some_and(|prev| matches!(prev, '{' | ','))
        && next_non_ws(raw, end).is_some_and(|next| matches!(next, '}' | ','))
}

pub(crate) fn process_expression_rewrite_identifier(
    ident: &str,
    options: &Vue3CompilerOptions,
    assignment_rhs: Option<&ProcessExpressionAssignmentRhs<'_>>,
    update_argument: Option<ProcessExpressionUpdate>,
    destructure_assignment: bool,
    locals: &[String],
) -> String {
    match options.binding_metadata.get(ident).map(String::as_str) {
        Some("setup-ref") if options.inline => {
            if let Some(update) = update_argument {
                let prefix = if update.prefix { update.operator } else { "" };
                let postfix = if update.prefix { "" } else { update.operator };
                format!("{prefix}{ident}.value{postfix}")
            } else {
                format!("{ident}.value")
            }
        }
        Some("setup-maybe-ref") if options.inline => {
            if let Some(update) = update_argument {
                let prefix = if update.prefix { update.operator } else { "" };
                let postfix = if update.prefix { "" } else { update.operator };
                format!("{prefix}{ident}.value{postfix}")
            } else if assignment_rhs.is_some() || destructure_assignment {
                format!("{ident}.value")
            } else {
                format!("_unref({ident})")
            }
        }
        Some("setup-let") if options.inline => {
            if let Some(rhs) = assignment_rhs {
                let rewritten_rhs = process_expression_rewrite_source(rhs.source, options, locals);
                format!(
                    "_isRef({ident}) ? {ident}.value {} {} : {ident}",
                    rhs.operator,
                    rewritten_rhs.trim()
                )
            } else if let Some(update) = update_argument {
                let prefix = if update.prefix { update.operator } else { "" };
                let postfix = if update.prefix { "" } else { update.operator };
                format!(
                    "_isRef({ident}) ? {prefix}{ident}.value{postfix} : {prefix}{ident}{postfix}"
                )
            } else if destructure_assignment {
                ident.to_string()
            } else {
                format!("_unref({ident})")
            }
        }
        _ => rewrite_identifier(ident, options),
    }
}

pub(crate) fn process_expression_arrow_bindings(raw: &str) -> Vec<ProcessExpressionArrowBinding> {
    let mut bindings = Vec::new();
    for arrow in process_expression_arrow_offsets(raw) {
        let Some(param_range) = process_expression_arrow_param_range(raw, arrow) else {
            continue;
        };
        let body_start = skip_ws_forward(raw, arrow + 2);
        let body_end = process_expression_arrow_body_end(raw, body_start);
        for (param_start, param_end) in process_expression_param_binding_spans(raw, param_range) {
            bindings.push(ProcessExpressionArrowBinding {
                name: raw[param_start..param_end].to_string(),
                param_start,
                param_end,
                body_start,
                body_end,
            });
        }
    }
    bindings
}

pub(crate) fn process_expression_arrow_offsets(raw: &str) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let chars = raw.char_indices().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        let (offset, ch) = chars[index];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if ch == '=' && raw[offset..].starts_with("=>") {
            offsets.push(offset);
            index += 2;
            continue;
        }
        index += 1;
    }
    offsets
}

pub(crate) fn process_expression_arrow_param_range(
    raw: &str,
    arrow: usize,
) -> Option<(usize, usize)> {
    let (param_end, end_char) = previous_non_ws_index(raw, arrow)?;
    if end_char == ')' {
        let open = find_matching_backward(raw, param_end, '(', ')')?;
        return Some((open + 1, param_end));
    }
    if !is_identifier_continue(end_char) {
        return None;
    }
    let mut start = param_end;
    while start > 0 {
        let Some((prev, ch)) = previous_char(raw, start) else {
            break;
        };
        if !is_identifier_continue(ch) {
            break;
        }
        start = prev;
    }
    Some((start, param_end + end_char.len_utf8()))
}

pub(crate) fn process_expression_param_binding_spans(
    raw: &str,
    range: (usize, usize),
) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let chars = raw[range.0..range.1]
        .char_indices()
        .map(|(offset, ch)| (range.0 + offset, ch))
        .collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        let start = chars[index].0;
        let ch = chars[index].1;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if !is_identifier_start(ch) {
            index += 1;
            continue;
        }
        index += 1;
        while index < chars.len() && is_identifier_continue(chars[index].1) {
            index += 1;
        }
        let end = chars.get(index).map_or(range.1, |(offset, _)| *offset);
        let ident = &raw[start..end];
        if is_keyword(ident)
            || process_expression_param_default_rhs(raw, range.0, start)
            || next_non_ws(raw, end) == Some(':')
        {
            continue;
        }
        spans.push((start, end));
    }
    spans
}

pub(crate) fn process_expression_param_default_rhs(
    raw: &str,
    range_start: usize,
    start: usize,
) -> bool {
    let mut offset = start;
    while offset > range_start {
        let Some((prev, ch)) = previous_char(raw, offset) else {
            break;
        };
        if ch.is_whitespace() {
            offset = prev;
            continue;
        }
        return ch == '=';
    }
    false
}

pub(crate) fn process_expression_arrow_body_end(raw: &str, body_start: usize) -> usize {
    if raw[body_start..].starts_with('{') {
        return find_matching_forward(raw, body_start, '{', '}')
            .map(|end| end + 1)
            .unwrap_or(raw.len());
    }
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut depth = 0usize;
    for (offset, ch) in raw[body_start..].char_indices() {
        let absolute = body_start + offset;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => return absolute,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' | ';' if depth == 0 => return absolute,
            _ => {}
        }
    }
    raw.len()
}

pub(crate) fn process_expression_assignment_rhs<'a>(
    raw: &'a str,
    start: usize,
    end: usize,
) -> Option<ProcessExpressionAssignmentRhs<'a>> {
    if !process_expression_assignment_can_start(raw, start) {
        return None;
    }
    let operator_start = skip_ws_forward(raw, end);
    let operator = process_expression_assignment_operator(raw, operator_start)?;
    let rhs_start = skip_ws_forward(raw, operator_start + operator.len());
    let rhs_end = process_expression_assignment_rhs_end(raw, rhs_start);
    let source = raw.get(rhs_start..rhs_end)?.trim();
    (!source.is_empty()).then_some(ProcessExpressionAssignmentRhs { operator, source })
}

pub(crate) fn process_expression_assignment_can_start(raw: &str, start: usize) -> bool {
    if previous_non_ws(raw, start).is_none_or(|prev| matches!(prev, '(' | '{' | '[' | ',' | ';')) {
        return true;
    }
    let Some((prev_index, prev)) = previous_non_ws_index(raw, start) else {
        return true;
    };
    if !raw[prev_index + prev.len_utf8()..start]
        .chars()
        .any(is_line_terminator)
    {
        return false;
    }
    process_expression_token_can_end_statement(raw, prev_index, prev)
}

pub(crate) fn process_expression_token_can_end_statement(
    raw: &str,
    index: usize,
    ch: char,
) -> bool {
    ch == ')'
        || ch == ']'
        || ch == '}'
        || ch == '\''
        || ch == '"'
        || ch == '`'
        || is_identifier_continue(ch)
        || ch.is_ascii_digit()
        || raw[..index + ch.len_utf8()].trim_end().ends_with("++")
        || raw[..index + ch.len_utf8()].trim_end().ends_with("--")
}

pub(crate) fn process_expression_assignment_operator(raw: &str, start: usize) -> Option<&str> {
    [
        ">>>=", "<<=", ">>=", "**=", "&&=", "||=", "??=", "+=", "-=", "*=", "/=", "%=", "&=", "|=",
        "^=", "=",
    ]
    .into_iter()
    .find(|operator| raw[start..].starts_with(operator) && !raw[start..].starts_with("=>"))
}

pub(crate) fn process_expression_assignment_rhs_end(raw: &str, rhs_start: usize) -> usize {
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut depth = 0usize;
    for (offset, ch) in raw[rhs_start..].char_indices() {
        let absolute = rhs_start + offset;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => return raw[..absolute].trim_end().len(),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' | ';' if depth == 0 => return raw[..absolute].trim_end().len(),
            '\n' | '\r'
                if depth == 0
                    && process_expression_line_terminator_can_end_rhs(raw, rhs_start, absolute) =>
            {
                return raw[..absolute].trim_end().len();
            }
            _ => {}
        }
    }
    raw.trim_end().len()
}

pub(crate) fn process_expression_line_terminator_can_end_rhs(
    raw: &str,
    rhs_start: usize,
    offset: usize,
) -> bool {
    if raw[rhs_start..offset].trim().is_empty() {
        return false;
    }
    let mut next_offset = offset;
    while next_offset < raw.len() {
        let Some(ch) = raw[next_offset..].chars().next() else {
            break;
        };
        if !is_line_terminator(ch) {
            break;
        }
        next_offset += ch.len_utf8();
    }
    !next_non_ws(raw, next_offset).is_some_and(process_expression_token_continues_expression)
}

pub(crate) fn process_expression_token_continues_expression(ch: char) -> bool {
    matches!(
        ch,
        '.' | '?'
            | ':'
            | '+'
            | '-'
            | '*'
            | '/'
            | '%'
            | '&'
            | '|'
            | '^'
            | '='
            | '<'
            | '>'
            | '('
            | '['
    )
}

pub(crate) fn is_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r')
}

pub(crate) fn process_expression_is_destructure_assignment(raw: &str, start: usize) -> bool {
    let Some(open) = process_expression_destructure_open_before(raw, start) else {
        return false;
    };
    let close_ch = match raw.as_bytes().get(open) {
        Some(b'{') => '}',
        Some(b'[') => ']',
        _ => return false,
    };
    let Some(close) = find_matching_forward(raw, open, raw.as_bytes()[open] as char, close_ch)
    else {
        return false;
    };
    if !(open < start && start < close) {
        return false;
    }
    next_non_ws(raw, close + close_ch.len_utf8()) == Some('=')
}

pub(crate) fn process_expression_destructure_open_before(raw: &str, start: usize) -> Option<usize> {
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut stack = Vec::<(usize, char)>::new();
    for (offset, ch) in raw[..start].char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => stack.push((offset, ch)),
            ')' => pop_matching_open(&mut stack, '('),
            ']' => pop_matching_open(&mut stack, '['),
            '}' => pop_matching_open(&mut stack, '{'),
            _ => {}
        }
    }
    stack
        .into_iter()
        .rev()
        .find_map(|(offset, ch)| matches!(ch, '{' | '[').then_some(offset))
}

pub(crate) fn pop_matching_open(stack: &mut Vec<(usize, char)>, expected: char) {
    if stack.last().is_some_and(|(_, ch)| *ch == expected) {
        stack.pop();
    }
}

pub(crate) fn process_expression_dynamic_static_reference(
    raw: &str,
    start: usize,
    end: usize,
) -> bool {
    next_non_ws(raw, end) == Some('(') || process_expression_preceded_by_new(raw, start)
}

pub(crate) fn process_expression_preceded_by_new(raw: &str, start: usize) -> bool {
    let prefix = raw[..start].trim_end();
    prefix.strip_suffix("new").is_some_and(|before| {
        before
            .chars()
            .last()
            .is_none_or(|ch| !is_identifier_continue(ch))
    })
}

pub(crate) fn skip_ws_forward(raw: &str, mut offset: usize) -> usize {
    while offset < raw.len() {
        let Some(ch) = raw[offset..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        offset += ch.len_utf8();
    }
    offset
}

pub(crate) fn previous_non_ws_index(source: &str, offset: usize) -> Option<(usize, char)> {
    source[..offset]
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_whitespace())
}

pub(crate) fn next_non_ws_index(source: &str, offset: usize) -> Option<(usize, char)> {
    source[offset..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(relative, ch)| (offset + relative, ch))
}

pub(crate) fn previous_char(source: &str, offset: usize) -> Option<(usize, char)> {
    source[..offset].char_indices().next_back()
}

pub(crate) fn find_matching_forward(
    raw: &str,
    open: usize,
    open_ch: char,
    close_ch: char,
) -> Option<usize> {
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut depth = 0usize;
    for (offset, ch) in raw[open..].char_indices() {
        let absolute = open + offset;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        if ch == open_ch {
            depth += 1;
        } else if ch == close_ch {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(absolute);
            }
        }
    }
    None
}

pub(crate) fn find_matching_backward(
    raw: &str,
    close: usize,
    open_ch: char,
    close_ch: char,
) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in raw[..=close].char_indices().rev() {
        if ch == close_ch {
            depth += 1;
        } else if ch == open_ch {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(offset);
            }
        }
    }
    None
}

pub(crate) fn process_expression_update_argument(
    raw: &str,
    start: usize,
    end: usize,
) -> Option<ProcessExpressionUpdate> {
    if let Some(tail) = raw.get(end..).map(str::trim_start) {
        if tail.starts_with("++") {
            return Some(ProcessExpressionUpdate {
                operator: "++",
                prefix: false,
            });
        }
        if tail.starts_with("--") {
            return Some(ProcessExpressionUpdate {
                operator: "--",
                prefix: false,
            });
        }
    }
    if let Some(head) = raw.get(..start).map(str::trim_end) {
        if head.ends_with("++") {
            return Some(ProcessExpressionUpdate {
                operator: "++",
                prefix: true,
            });
        }
        if head.ends_with("--") {
            return Some(ProcessExpressionUpdate {
                operator: "--",
                prefix: true,
            });
        }
    }
    None
}

pub(crate) fn transform_on_projection_const_type(projection: &Value) -> u64 {
    projection
        .get("constType")
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

pub(crate) fn transform_on_has_scope_ref(exp: &Value, context: &Value) -> bool {
    let source = model_expression_source(exp);
    context
        .get("identifiers")
        .and_then(Value::as_object)
        .is_some_and(|identifiers| {
            identifiers.iter().any(|(name, count)| {
                count.as_i64().unwrap_or_default() > 0 && source_contains_identifier(&source, name)
            })
        })
}

pub(crate) fn transform_on_is_member_expression(expression: &str, context: &Value) -> bool {
    let store = JsAstStore::new();
    let wrapped = format!("({})", expression.trim());
    match store.parse_expression(&wrapped, transform_on_source_type(context)) {
        Ok(expression) => transform_on_expression_is_member(&expression),
        Err(_) if json_bool(context, "allowLexerFallback") => {
            transform_on_is_member_expression_lexer(expression)
        }
        Err(_) => false,
    }
}

pub(crate) fn transform_on_expression_is_member(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Identifier(identifier) => identifier.name != "undefined",
        Expression::ComputedMemberExpression(_)
        | Expression::StaticMemberExpression(_)
        | Expression::PrivateFieldExpression(_) => true,
        Expression::ChainExpression(chain) => {
            transform_on_chain_element_is_member(&chain.expression)
        }
        Expression::ParenthesizedExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSAsExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        _ => false,
    }
}

pub(crate) fn transform_on_chain_element_is_member(element: &ChainElement<'_>) -> bool {
    matches!(
        element,
        ChainElement::ComputedMemberExpression(_)
            | ChainElement::StaticMemberExpression(_)
            | ChainElement::PrivateFieldExpression(_)
            | ChainElement::TSNonNullExpression(_)
    )
}

pub(crate) fn transform_on_is_fn_expression(expression: &str, context: &Value) -> bool {
    let trimmed = expression.trim_start();
    if transform_on_is_fn_expression_lexer(trimmed) {
        return true;
    }
    let store = JsAstStore::new();
    store
        .parse_expression(expression.trim(), transform_on_source_type(context))
        .map(|expression| transform_on_expression_is_fn(&expression))
        .unwrap_or(false)
}

pub(crate) fn transform_on_expression_is_fn(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => true,
        Expression::TSAsExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        _ => false,
    }
}

pub(crate) fn transform_on_is_fn_expression_lexer(expression: &str) -> bool {
    expression.starts_with("function")
        || expression.starts_with("async function")
        || expression
            .find("=>")
            .is_some_and(|index| transform_on_arrow_prefix_is_fn_like(&expression[..index]))
}

pub(crate) fn transform_on_arrow_prefix_is_fn_like(prefix: &str) -> bool {
    let prefix = prefix.trim();
    let prefix = prefix.strip_prefix("async").unwrap_or(prefix).trim();
    if prefix.starts_with('(') {
        return prefix.ends_with(')');
    }
    is_simple_identifier_ascii(prefix)
}

pub(crate) fn transform_on_root_function_locals(expression: &str) -> Vec<String> {
    let store = JsAstStore::new();
    store
        .parse_expression(expression.trim(), oxc_span::SourceType::ts())
        .map(|expression| {
            let mut locals = Vec::new();
            transform_on_collect_root_function_locals(&expression, &mut locals);
            locals.sort();
            locals.dedup();
            locals
        })
        .unwrap_or_else(|_| transform_on_root_function_locals_lexer(expression))
}

pub(crate) fn transform_on_collect_root_function_locals(
    expression: &Expression<'_>,
    locals: &mut Vec<String>,
) {
    match expression {
        Expression::ArrowFunctionExpression(function) => {
            for param in &function.params.items {
                collect_vue3_for_binding_pattern(&param.pattern, locals);
            }
            if let Some(rest) = &function.params.rest {
                collect_vue3_for_binding_pattern(&rest.rest.argument, locals);
            }
        }
        Expression::FunctionExpression(function) => {
            for param in &function.params.items {
                collect_vue3_for_binding_pattern(&param.pattern, locals);
            }
            if let Some(rest) = &function.params.rest {
                collect_vue3_for_binding_pattern(&rest.rest.argument, locals);
            }
        }
        Expression::TSAsExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSSatisfiesExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSTypeAssertion(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSNonNullExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSInstantiationExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        _ => {}
    }
}

pub(crate) fn transform_on_root_function_locals_lexer(expression: &str) -> Vec<String> {
    let trimmed = expression.trim_start();
    let Some(arrow_index) = trimmed.find("=>") else {
        return Vec::new();
    };
    let mut params = trimmed[..arrow_index].trim();
    params = params.strip_prefix("async").unwrap_or(params).trim();
    if params.starts_with('(') && params.ends_with(')') {
        params = &params[1..params.len() - 1];
    }
    split_top_level_like(params, ',')
        .into_iter()
        .flat_map(extract_slot_params)
        .collect()
}

pub(crate) fn transform_on_source_type(context: &Value) -> oxc_span::SourceType {
    let _ = context;
    oxc_span::SourceType::ts()
}

pub(crate) fn transform_on_is_member_expression_lexer(expression: &str) -> bool {
    let path = normalize_member_expression_whitespace(expression.trim());
    if path.is_empty() {
        return false;
    }
    let mut depth_square = 0usize;
    let mut depth_paren = 0usize;
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut chars = path.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '[' => depth_square += 1,
            ']' => {
                if depth_square == 0 {
                    return false;
                }
                depth_square -= 1;
            }
            '(' => depth_paren += 1,
            ')' => {
                if chars.peek().is_none() {
                    return false;
                }
                if depth_paren == 0 {
                    return false;
                }
                depth_paren -= 1;
            }
            _ if depth_square == 0 && depth_paren == 0 => {
                let valid = if index == 0 {
                    is_identifier_start(ch)
                } else {
                    is_identifier_continue(ch) || matches!(ch, '.' | '?')
                };
                if !valid {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth_square == 0 && depth_paren == 0 && quote.is_none()
}

pub(crate) fn normalize_member_expression_whitespace(expression: &str) -> String {
    let mut output = String::new();
    let chars = expression.chars().collect::<Vec<_>>();
    for (index, ch) in chars.iter().copied().enumerate() {
        if ch.is_whitespace() {
            let prev = chars[..index]
                .iter()
                .rev()
                .find(|candidate| !candidate.is_whitespace())
                .copied();
            let next = chars[index + 1..]
                .iter()
                .find(|candidate| !candidate.is_whitespace())
                .copied();
            if matches!(prev, Some('.' | '[')) || matches!(next, Some('.' | '[')) {
                continue;
            }
        }
        output.push(ch);
    }
    output
}

/// Returns whether a `v-model` expression is assignable as a member expression.
pub fn model_is_member_expression(expression: &str) -> bool {
    let store = JsAstStore::new();
    store
        .parse_expression(expression, oxc_span::SourceType::mjs())
        .map(|expression| match expression {
            Expression::Identifier(_) => true,
            Expression::ComputedMemberExpression(_)
            | Expression::StaticMemberExpression(_)
            | Expression::PrivateFieldExpression(_) => true,
            Expression::ChainExpression(chain) => model_chain_element_is_member(&chain.expression),
            _ => false,
        })
        .unwrap_or(false)
}

pub(crate) fn model_chain_element_is_member(element: &ChainElement<'_>) -> bool {
    matches!(
        element,
        ChainElement::ComputedMemberExpression(_)
            | ChainElement::StaticMemberExpression(_)
            | ChainElement::PrivateFieldExpression(_)
    )
}

pub(crate) fn context_identifier_count<'a>(context: &'a Value, name: &str) -> i64 {
    context
        .get("identifiers")
        .and_then(|identifiers| identifiers.get(name))
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

pub(crate) fn camelize(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase_next = false;
    for ch in value.chars() {
        if uppercase_next {
            output.extend(ch.to_uppercase());
            uppercase_next = false;
        } else if ch == '-' {
            uppercase_next = true;
        } else {
            output.push(ch);
        }
    }
    output
}

pub(crate) fn setup_reference_name_for_tag(
    tag: &str,
    options: &Vue3CompilerOptions,
) -> Option<String> {
    setup_reference_name(tag, options)
}

pub(crate) fn setup_reference_name(name: &str, options: &Vue3CompilerOptions) -> Option<String> {
    let camel_name = camelize(name);
    let pascal_name = capitalize(&camel_name);
    for candidate in [name.to_string(), camel_name, pascal_name] {
        if options
            .binding_metadata
            .get(&candidate)
            .is_some_and(|kind| {
                matches!(
                    kind.as_str(),
                    "setup-const"
                        | "setup-reactive-const"
                        | "literal-const"
                        | "setup-let"
                        | "setup-ref"
                        | "setup-maybe-ref"
                        | "props"
                )
            })
        {
            return Some(candidate);
        }
    }
    None
}

pub(crate) fn to_handler_key(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!("on{}", capitalize(value))
    }
}

pub(crate) fn is_simple_identifier_ascii(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

pub(crate) fn json_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(crate) fn json_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub(crate) fn vue3_element_kind(
    tag: String,
    attributes: Vec<vuec_html::HtmlAttribute>,
    self_closing: bool,
    options: &Vue3CompilerOptions,
    file_id: FileId,
    base_offset: usize,
    in_v_pre: bool,
    namespace: vuec_ast::HtmlNamespace,
) -> Vue3NodeKind {
    let props = attributes
        .into_iter()
        .filter(|attr| !(in_v_pre && attr.name == "v-pre"))
        .map(|attr| {
            if in_v_pre {
                vue3_attribute_from_attr(attr, file_id, base_offset)
            } else {
                vue3_prop_from_attr(attr, file_id, base_offset)
            }
        })
        .collect::<Vec<_>>();
    let tag_type = if in_v_pre {
        Vue3ElementType::Element
    } else {
        vue3_tag_type(&tag, &props, options)
    };
    Vue3NodeKind::Element(Vue3Element {
        tag,
        tag_type,
        ns: namespace,
        props,
        self_closing,
        codegen_node: None,
        ssr_codegen_node: None,
    })
}

pub(crate) fn vue3_element_namespace(
    ast: &Vue3Ast,
    parent_id: vuec_ast::NodeId,
    tag: &str,
    parent: vuec_ast::HtmlNamespace,
    options: &Vue3CompilerOptions,
) -> vuec_ast::HtmlNamespace {
    if let Some(namespace) = options.namespaces.get(tag).copied() {
        return namespace;
    }
    let parent_element = ast.node(parent_id).and_then(|node| match &node.kind {
        Vue3AstKind::Element(element) => Some(element),
        _ => None,
    });
    let namespace = resolve_html_namespace(
        tag,
        html_namespace_to_html(parent),
        parent_element.map(|element| element.tag.as_str()),
        parent_element.is_some_and(|element| {
            vue3_element_has_attr_value(
                element,
                "encoding",
                &["text/html", "application/xhtml+xml"],
            )
        }),
        options.dom_namespaces,
    );
    html_namespace_from_html(namespace)
}

pub(crate) fn vue3_element_has_attr_value(
    element: &vuec_ast::Vue3Element,
    name: &str,
    values: &[&str],
) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Attribute(attr)
                if attr.name == name
                    && attr
                        .value
                        .as_deref()
                        .is_some_and(|value| values.iter().any(|candidate| *candidate == value))
        )
    })
}

pub(crate) fn html_namespace_to_html(
    namespace: vuec_ast::HtmlNamespace,
) -> vuec_html::HtmlNamespace {
    match namespace {
        vuec_ast::HtmlNamespace::Html => vuec_html::HtmlNamespace::Html,
        vuec_ast::HtmlNamespace::Svg => vuec_html::HtmlNamespace::Svg,
        vuec_ast::HtmlNamespace::MathMl => vuec_html::HtmlNamespace::MathMl,
    }
}

pub(crate) fn html_namespace_from_html(
    namespace: vuec_html::HtmlNamespace,
) -> vuec_ast::HtmlNamespace {
    match namespace {
        vuec_html::HtmlNamespace::Html => vuec_ast::HtmlNamespace::Html,
        vuec_html::HtmlNamespace::Svg => vuec_ast::HtmlNamespace::Svg,
        vuec_html::HtmlNamespace::MathMl => vuec_ast::HtmlNamespace::MathMl,
    }
}

pub(crate) fn vue3_tag_type(
    tag: &str,
    props: &[Vue3Prop],
    options: &Vue3CompilerOptions,
) -> Vue3ElementType {
    if options
        .custom_elements
        .iter()
        .any(|candidate| candidate == tag)
    {
        return Vue3ElementType::Element;
    }
    if tag == "slot" {
        return Vue3ElementType::SlotOutlet;
    }
    if tag == "template" {
        return if props.iter().any(
            |prop| matches!(prop, Vue3Prop::Directive(dir) if is_template_directive(&dir.name)),
        ) {
            Vue3ElementType::Template
        } else {
            Vue3ElementType::Element
        };
    }
    if options
        .built_in_components
        .iter()
        .any(|candidate| candidate == tag)
    {
        return Vue3ElementType::Component;
    }
    if vue3_core_component_helper(tag).is_some() || matches!(tag, "component" | "Component") {
        return Vue3ElementType::Component;
    }
    if setup_reference_name_for_tag(tag, options).is_some() {
        return Vue3ElementType::Component;
    }
    if props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Attribute(attr)
                if attr.name == "is"
                    && attr
                        .value
                        .as_deref()
                        .is_some_and(|value| value.starts_with("vue:"))
        )
    }) {
        return Vue3ElementType::Component;
    }
    if options
        .native_tags
        .as_ref()
        .is_some_and(|native_tags| !native_tags.iter().any(|candidate| candidate == tag))
    {
        return Vue3ElementType::Component;
    }
    if tag.chars().next().is_some_and(|ch| ch.is_ascii_uppercase()) {
        return Vue3ElementType::Component;
    }
    Vue3ElementType::Element
}

pub(crate) fn is_template_directive(name: &str) -> bool {
    matches!(name, "if" | "else" | "else-if" | "for" | "slot")
}

pub(crate) fn vue3_prop_from_attr(
    attr: vuec_html::HtmlAttribute,
    file_id: FileId,
    base_offset: usize,
) -> Vue3Prop {
    let parsed_attr = vue3_attr_from_html(attr, file_id, base_offset);
    let attr = parsed_attr.attr;
    if let Some(parsed) = parse_vue3_directive(&attr.name, attr.name_span) {
        let (directive_name, arg, modifiers, is_dynamic_arg, arg_span, modifier_spans) = parsed;
        Vue3Prop::Directive(Vue3Directive {
            name: directive_name,
            raw_name: attr.name,
            arg: arg.map(Vue3Expression::Raw),
            exp: attr
                .value
                .map(|value| Vue3Expression::Raw(decode_html_attr_entities(&value))),
            modifiers,
            is_dynamic_arg,
            span: attr.span,
            arg_span,
            exp_span: parsed_attr.value_content_span.or(attr.value_span),
            modifier_spans,
        })
    } else {
        Vue3Prop::Attribute(attr)
    }
}

pub(crate) fn vue3_attribute_from_attr(
    attr: vuec_html::HtmlAttribute,
    file_id: FileId,
    base_offset: usize,
) -> Vue3Prop {
    Vue3Prop::Attribute(vue3_attr_from_html(attr, file_id, base_offset).attr)
}

pub(crate) struct ParsedVue3Attribute {
    pub(crate) attr: vuec_ast::Vue3Attribute,
    pub(crate) value_content_span: Option<Span>,
}

pub(crate) fn vue3_attr_from_html(
    attr: vuec_html::HtmlAttribute,
    file_id: FileId,
    base_offset: usize,
) -> ParsedVue3Attribute {
    let span = Some(Span::new(
        file_id,
        base_offset + attr.start,
        base_offset + attr.end,
    ));
    let name_span = Some(Span::new(
        file_id,
        base_offset + attr.name_start,
        base_offset + attr.name_end,
    ));
    let value_span = attr
        .value_start
        .zip(attr.value_end)
        .map(|(start, end)| Span::new(file_id, base_offset + start, base_offset + end));
    let value_content_span = attr
        .value_content_start
        .zip(attr.value_content_end)
        .map(|(start, end)| Span::new(file_id, base_offset + start, base_offset + end));
    let quote = attr.quote.map(|quote| match quote {
        vuec_html::HtmlQuoteKind::Double => QuoteKind::Double,
        vuec_html::HtmlQuoteKind::Single => QuoteKind::Single,
        vuec_html::HtmlQuoteKind::Unquoted => QuoteKind::Unquoted,
    });
    ParsedVue3Attribute {
        attr: vuec_ast::Vue3Attribute {
            name: attr.name,
            value: attr.value,
            span,
            name_span,
            value_span,
            quote,
        },
        value_content_span,
    }
}

pub(crate) fn parse_vue3_directive(
    raw: &str,
    name_span: Option<Span>,
) -> Option<(
    String,
    Option<String>,
    Vec<String>,
    bool,
    Option<Span>,
    Vec<NodeSpan>,
)> {
    let mut body = raw;
    let mut name = None;
    let mut arg_offset = 0usize;
    if let Some(rest) = raw.strip_prefix("v-") {
        if let Some((head, tail)) = rest.split_once(':') {
            name = Some(head.to_string());
            body = tail;
            arg_offset = 2 + head.len() + 1;
        } else {
            let mut parts = split_directive_parts(rest, false);
            let directive = parts.next().unwrap_or_default();
            if directive.is_empty() {
                return None;
            }
            let modifiers = parts.collect::<Vec<_>>();
            let modifier_spans = directive_modifier_spans(raw, &modifiers, name_span);
            return Some((
                directive.to_string(),
                None,
                modifiers.into_iter().map(ToOwned::to_owned).collect(),
                false,
                None,
                modifier_spans,
            ));
        }
    } else if let Some(rest) = raw.strip_prefix(':') {
        name = Some("bind".to_string());
        body = rest;
        arg_offset = 1;
    } else if let Some(rest) = raw.strip_prefix('@') {
        name = Some("on".to_string());
        body = rest;
        arg_offset = 1;
    } else if let Some(rest) = raw.strip_prefix('#') {
        name = Some("slot".to_string());
        body = rest;
        arg_offset = 1;
    } else if let Some(rest) = raw.strip_prefix('.') {
        name = Some("bind".to_string());
        body = rest;
        arg_offset = 1;
    }
    let name = name?;
    if name.is_empty() {
        return None;
    }
    let preserve_arg_dots = name == "slot";
    let mut parts = split_directive_parts(body, preserve_arg_dots);
    let raw_arg = parts.next().unwrap_or_default();
    let modifiers = if raw.starts_with('.') {
        let mut values = vec!["prop".to_string()];
        values.extend(parts.map(ToOwned::to_owned));
        values
    } else {
        parts.map(ToOwned::to_owned).collect::<Vec<_>>()
    };
    let (arg, is_dynamic) = if raw_arg.starts_with('[') {
        let content_end = if raw_arg.ends_with(']') {
            raw_arg.len().saturating_sub(1)
        } else {
            raw_arg.len()
        };
        let content = raw_arg[1..content_end]
            .trim_end_matches(|ch: char| ch.is_whitespace() || ch == '/')
            .to_string();
        (Some(content), true)
    } else if raw_arg.is_empty() {
        (None, false)
    } else {
        (Some(raw_arg.to_string()), false)
    };
    let arg_span = arg.as_ref().and_then(|_| {
        name_span.map(|span| {
            let arg_start = if is_dynamic && raw_arg.starts_with('[') {
                arg_offset
            } else {
                arg_offset
                    + raw_arg
                        .find(arg.as_deref().unwrap_or_default())
                        .unwrap_or(0)
            };
            let arg_len = if is_dynamic {
                raw_arg.len() + usize::from(!raw_arg.ends_with(']'))
            } else {
                arg.as_deref().unwrap_or_default().len()
            };
            Span::new(
                span.file_id,
                span.start.0 + arg_start,
                span.start.0 + arg_start + arg_len,
            )
        })
    });
    let modifier_spans = if raw.starts_with('.') {
        let mut spans = vec![NodeSpan::missing(MissingSpanReason::Synthetic)];
        let modifier_refs = modifiers
            .iter()
            .skip(1)
            .map(String::as_str)
            .collect::<Vec<_>>();
        spans.extend(directive_modifier_spans(raw, &modifier_refs, name_span));
        spans
    } else {
        let modifier_refs = modifiers.iter().map(String::as_str).collect::<Vec<_>>();
        directive_modifier_spans(raw, &modifier_refs, name_span)
    };
    Some((name, arg, modifiers, is_dynamic, arg_span, modifier_spans))
}

pub(crate) fn split_directive_parts(
    source: &str,
    preserve_dots: bool,
) -> impl Iterator<Item = &str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut bracket_depth = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            '.' if bracket_depth == 0 && !preserve_dots => {
                parts.push(&source[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&source[start..]);
    parts.into_iter()
}

pub(crate) fn directive_modifier_spans(
    raw: &str,
    modifiers: &[&str],
    name_span: Option<Span>,
) -> Vec<NodeSpan> {
    let Some(name_span) = name_span else {
        return Vec::new();
    };
    let mut spans = Vec::new();
    let mut search_start = 0usize;
    for modifier in modifiers {
        let needle = format!(".{modifier}");
        if let Some(offset) = raw[search_start..].find(&needle) {
            let start = search_start + offset + 1;
            spans.push(NodeSpan::from(Span::new(
                name_span.file_id,
                name_span.start.0 + start,
                name_span.start.0 + start + modifier.len(),
            )));
            search_start = start + modifier.len();
        }
    }
    spans
}

pub(crate) fn vue3_start_tag_is_incomplete(source: &str, start: usize, end: usize) -> bool {
    source
        .get(start..end)
        .is_some_and(|slice| !slice.ends_with('>'))
}

pub(crate) fn vue3_empty_end_tag_should_be_text(source: &str, start: usize, end: usize) -> bool {
    let Some(slice) = source.get(start..end) else {
        return false;
    };
    if slice.ends_with('>') {
        return false;
    }
    slice
        .strip_prefix("</")
        .is_some_and(|after_slash| after_slash.trim().is_empty())
}

pub(crate) fn stack_is_root_only(stack: &[vuec_ast::NodeId], root: vuec_ast::NodeId) -> bool {
    stack.len() == 1 && stack.first().copied() == Some(root)
}

pub(crate) fn push_incomplete_start_tag_recovery_text(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    source: &TemplateSource,
    token_start: usize,
    token_end: usize,
) {
    let Some(slice) = source.source.get(token_start..token_end) else {
        return;
    };
    let Some(local_start) = incomplete_start_tag_recovery_text_start(slice) else {
        return;
    };
    let text = &slice[local_start..];
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::text(decode_html_text_entities(text)),
        Some(Span::new(
            source.file_id,
            source.base_offset + token_start + local_start,
            source.base_offset + token_start + local_start + text.len(),
        )),
    );
}

pub(crate) fn incomplete_start_tag_recovery_text_start(slice: &str) -> Option<usize> {
    slice.rfind('/').filter(|index| {
        slice
            .get(index + 1..)
            .is_some_and(|tail| tail.chars().all(char::is_whitespace))
    })
}

/// Returns the Vue 3 raw-text parsing mode for a tag and namespace.
pub fn vue3_raw_text_kind(
    tag: &str,
    namespace: vuec_ast::HtmlNamespace,
    in_v_pre: bool,
) -> Option<HtmlTextMode> {
    match raw_text_mode_for_tag(tag, html_namespace_to_html(namespace), in_v_pre) {
        HtmlTextMode::Data => None,
        mode => Some(mode),
    }
}

pub(crate) fn vue3_is_sfc_plain_template(
    tag: &str,
    parent: vuec_ast::NodeId,
    root: vuec_ast::NodeId,
    attributes: &[vuec_html::HtmlAttribute],
    options: &Vue3CompilerOptions,
) -> bool {
    if parent != root || tag != "template" || options.sfc_plain_template_langs.is_empty() {
        return false;
    }
    let Some(lang) = attributes
        .iter()
        .find(|attr| attr.name == "lang")
        .and_then(|attr| attr.value.as_deref())
    else {
        return false;
    };
    vue3_sfc_plain_template_lang(lang, options)
}

pub(crate) fn vue3_is_sfc_custom_block(
    tag: &str,
    parent: vuec_ast::NodeId,
    root: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
) -> bool {
    options.sfc_parse_mode && parent == root && tag != "template"
}

pub(crate) fn current_parent_raw_text_ignores_end_tag(
    ast: &Vue3Ast,
    parent: vuec_ast::NodeId,
    name: &str,
) -> bool {
    let Some(node) = ast.node(parent) else {
        return false;
    };
    matches!(
        &node.kind,
        Vue3AstKind::Element(element)
            if matches!(element.tag.as_str(), "textarea" | "title")
                && !element.tag.eq_ignore_ascii_case(name)
    )
}

pub(crate) fn stack_has_matching_element(
    ast: &Vue3Ast,
    stack: &[vuec_ast::NodeId],
    name: &str,
) -> bool {
    stack.iter().copied().skip(1).any(|node_id| {
        ast.node(node_id).is_some_and(|node| {
            matches!(
                &node.kind,
                Vue3AstKind::Element(element) if element.tag.eq_ignore_ascii_case(name)
            )
        })
    })
}

pub(crate) fn extend_open_element_spans_to(
    ast: &mut Vue3Ast,
    stack: &[vuec_ast::NodeId],
    end: usize,
) {
    for node_id in stack.iter().copied().skip(1) {
        let Some(node) = ast.node_mut(node_id) else {
            continue;
        };
        if !matches!(node.kind, Vue3AstKind::Element(_)) {
            continue;
        }
        if let Some(span) = node.span.source_mut() {
            if span.end.0 < end {
                span.end = vuec_source::BytePos(end);
            }
        }
    }
}

pub(crate) fn normalize_vue3_parse_text(ast: &mut Vue3Ast, options: &Vue3CompilerOptions) {
    normalize_class_attribute_values(ast);
    remove_initial_newline_after_ignore_newline_tags(ast, options);
    normalize_text_children(ast, ast.root, options, false);
}

pub(crate) fn normalize_class_attribute_values(ast: &mut Vue3Ast) {
    for node in &mut ast.nodes {
        let Vue3AstKind::Element(element) = &mut node.kind else {
            continue;
        };
        for prop in &mut element.props {
            let Vue3Prop::Attribute(attr) = prop else {
                continue;
            };
            if attr.name == "class" {
                if let Some(value) = &mut attr.value {
                    *value = value.split_whitespace().collect::<Vec<_>>().join(" ");
                }
            } else if let Some(value) = &mut attr.value {
                *value = decode_html_attr_entities(value);
            }
        }
    }
}

pub(crate) fn remove_initial_newline_after_ignore_newline_tags(
    ast: &mut Vue3Ast,
    options: &Vue3CompilerOptions,
) {
    let element_ids = ast
        .nodes
        .iter()
        .filter_map(|node| matches!(node.kind, Vue3AstKind::Element(_)).then_some(node.id))
        .collect::<Vec<_>>();
    for node_id in element_ids {
        let should_ignore = ast.node(node_id).is_some_and(|node| {
            matches!(
                &node.kind,
                Vue3AstKind::Element(element)
                    if options.ignore_newline_tags.iter().any(|tag| tag == &element.tag)
            )
        });
        if !should_ignore {
            continue;
        }
        let Some(first_child) = ast
            .node(node_id)
            .and_then(|node| node.children.first().copied())
        else {
            continue;
        };
        if let Some(child) = ast.node_mut(first_child) {
            if let Vue3AstKind::Text(text) = &mut child.kind {
                if text.value.starts_with("\r\n") {
                    text.value.drain(..2);
                } else if text.value.starts_with('\n') {
                    text.value.remove(0);
                }
            }
        }
    }
}

pub(crate) fn normalize_text_children(
    ast: &mut Vue3Ast,
    parent_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    in_pre: bool,
) {
    let Some(parent) = ast.node(parent_id) else {
        return;
    };
    if sfc_raw_text_parent(ast, parent_id, options) {
        return;
    }
    let parent_tag = match &parent.kind {
        Vue3AstKind::Element(element) => Some(element.tag.clone()),
        _ => None,
    };
    let parent_is_pre = parent_tag
        .as_ref()
        .is_some_and(|tag| options.pre_tags.iter().any(|pre| pre == tag));
    let preserve_text = in_pre || parent_is_pre || parent_tag.as_deref() == Some("textarea");
    let original_children = parent.children.clone();
    for child_id in &original_children {
        if matches!(
            ast.node(*child_id).map(|node| &node.kind),
            Some(Vue3AstKind::Element(_)) | Some(Vue3AstKind::Root(_))
        ) {
            normalize_text_children(ast, *child_id, options, preserve_text);
        }
    }
    if preserve_text {
        return;
    }
    let child_kinds = original_children
        .iter()
        .map(|child_id| ast.node(*child_id).map(|node| node.kind.clone()))
        .collect::<Vec<_>>();
    let mut keep_flags = vec![true; original_children.len()];
    let mut updated_texts = vec![None; original_children.len()];
    let mut retained_indices = Vec::new();
    for (index, child_kind) in child_kinds.iter().enumerate() {
        let Some(Vue3AstKind::Text(text)) = child_kind.as_ref() else {
            retained_indices.push(index);
            continue;
        };
        if text.value.chars().all(is_vue3_html_whitespace) {
            let prev = retained_indices
                .last()
                .and_then(|idx| child_kinds.get(*idx))
                .and_then(Option::as_ref);
            let next = child_kinds.get(index + 1).and_then(Option::as_ref);
            let keep = should_keep_whitespace_between(prev, next, &text.value, options);
            keep_flags[index] = keep;
            if keep {
                updated_texts[index] = Some(" ".into());
                retained_indices.push(index);
            }
        } else {
            if options.whitespace == "condense" {
                updated_texts[index] = Some(condense_whitespace(&text.value));
            }
            retained_indices.push(index);
        }
    }
    for (index, child_id) in original_children.iter().copied().enumerate() {
        if let Some(node) = ast.node_mut(child_id) {
            if let Some(new_value) = updated_texts[index].take() {
                if let Vue3AstKind::Text(text) = &mut node.kind {
                    text.value = new_value;
                }
            }
            if !keep_flags[index] {
                node.parent = None;
                node.index_in_parent = 0;
            }
        }
    }
    let retained = original_children
        .into_iter()
        .enumerate()
        .filter_map(|(index, child_id)| keep_flags[index].then_some(child_id))
        .collect::<Vec<_>>();
    ast.replace_children(parent_id, retained);
}

pub(crate) fn sfc_raw_text_parent(
    ast: &Vue3Ast,
    parent_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
) -> bool {
    if !options.sfc_parse_mode {
        return false;
    }
    let Some(parent) = ast.node(parent_id) else {
        return false;
    };
    if parent.parent != Some(ast.root) {
        return false;
    }
    matches!(
        &parent.kind,
        Vue3AstKind::Element(element)
            if element.tag != "template" || vue3_sfc_plain_template_element(element, options)
    )
}

pub(crate) fn vue3_sfc_plain_template_element(
    element: &vuec_ast::Vue3Element,
    options: &Vue3CompilerOptions,
) -> bool {
    element.tag == "template"
        && element.props.iter().any(|prop| {
            matches!(
                prop,
                Vue3Prop::Attribute(attr)
                    if attr.name == "lang"
                        && attr
                            .value
                            .as_deref()
                            .is_some_and(|lang| vue3_sfc_plain_template_lang(lang, options))
            )
        })
}

pub(crate) fn vue3_sfc_plain_template_lang(lang: &str, options: &Vue3CompilerOptions) -> bool {
    !lang.is_empty()
        && ((options.sfc_parse_mode && lang != "html")
            || options
                .sfc_plain_template_langs
                .iter()
                .any(|candidate| candidate == lang))
}

pub(crate) fn should_keep_whitespace_between(
    prev: Option<&Vue3AstKind>,
    next: Option<&Vue3AstKind>,
    value: &str,
    options: &Vue3CompilerOptions,
) -> bool {
    let (Some(prev), Some(next)) = (prev, next) else {
        return false;
    };
    let prev_is_element = matches!(prev, Vue3AstKind::Element(_));
    let next_is_element = matches!(next, Vue3AstKind::Element(_));
    if options.whitespace == "preserve" {
        return true;
    }
    let prev_is_comment = matches!(prev, Vue3AstKind::Comment(_));
    let next_is_comment = matches!(next, Vue3AstKind::Comment(_));
    if prev_is_comment && (next_is_comment || next_is_element) {
        return false;
    }
    if prev_is_element && (next_is_comment || (next_is_element && value.contains('\n'))) {
        return false;
    }
    true
}

pub(crate) fn condense_whitespace(value: &str) -> String {
    let mut out = String::new();
    let mut previous_ws = false;
    for ch in value.chars() {
        if is_vue3_html_whitespace(ch) {
            if !previous_ws {
                out.push(' ');
            }
            previous_ws = true;
        } else {
            out.push(ch);
            previous_ws = false;
        }
    }
    out
}

pub(crate) fn is_vue3_html_whitespace(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\u{000C}' | '\r' | ' ')
}
