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
            let is_refed = parent.is_none_or(|parent| {
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

pub(crate) fn js_ast_child_entries(node: &Value) -> Vec<(String, Vec<Value>, &Value)> {
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
            Some("FunctionDeclaration" | "ClassDeclaration")
                if !json_bool(stmt, "declare") =>
            {
                if let Some(id) = stmt.get("id") {
                    if let Some(name) = json_str(id, "name") {
                        names.push(name.to_string());
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
