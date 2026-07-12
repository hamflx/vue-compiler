use crate::*;

/// Parses, transforms, and generates Vue 3 compiler-core output.
pub fn base_compile(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
    Vue3Dialect::base_compile(source, options)
}

/// Compiles a Vue 3 template for DOM render output.
pub fn compile_dom(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
    Vue3Dialect::compile_dom(source, options)
}

/// Compiles a Vue 3 template for SSR render output.
pub fn compile_ssr(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
    Vue3Dialect::compile_ssr(source, options)
}

/// Generates render code from a hydrated public AST JSON value.
pub fn generate_public_ast(ast: &Value, options: &Vue3CompilerOptions) -> CodegenResult {
    PublicAstCodegen::new(ast, options).generate()
}

pub(crate) struct PublicAstCodegen<'a> {
    pub(crate) root: &'a Value,
    pub(crate) options: &'a Vue3CompilerOptions,
    pub(crate) code: String,
    pub(crate) indent: usize,
    pub(crate) pure: bool,
}

impl<'a> PublicAstCodegen<'a> {
    pub(crate) fn new(root: &'a Value, options: &'a Vue3CompilerOptions) -> Self {
        Self {
            root,
            options,
            code: String::new(),
            indent: 0,
            pure: false,
        }
    }

    pub(crate) fn generate(mut self) -> CodegenResult {
        self.gen_preamble();
        let preamble = if self.options.inline {
            std::mem::take(&mut self.code)
        } else {
            String::new()
        };
        let name = if self.options.ssr {
            "ssrRender"
        } else {
            "render"
        };
        let ssr = self.options.ssr;
        let args = if ssr {
            "_ctx, _push, _parent, _attrs".to_string()
        } else if self.options.binding_metadata.is_empty() || self.options.inline {
            "_ctx, _cache".to_string()
        } else {
            "_ctx, _cache, $props, $setup, $data, $options".to_string()
        };
        if self.options.inline {
            self.push(&format!("({args}) => {{"));
        } else {
            self.push(&format!("function {name}({args}) {{"));
        }
        self.indent();
        let use_with = !self.options.prefix_identifiers && self.options.mode != "module";
        if use_with {
            self.push("with (_ctx) {");
            self.indent();
            if !self.helpers().is_empty() {
                self.push(&format!(
                    "const {{ {} }} = _Vue",
                    public_helper_aliases(&self.helpers())
                ));
                self.push("\n\n");
                self.push_indent();
            }
        }
        self.gen_assets();
        if !ssr {
            self.push("return ");
        }
        let node = self.root.get("codegenNode").unwrap_or(&Value::Null);
        self.gen_node(node);
        if use_with {
            self.dedent();
            self.push("}");
        }
        self.dedent();
        self.push("}");
        CodegenResult {
            ast_summary: "public-ast-generate".to_string(),
            code: self.code,
            preamble,
            map: None,
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn gen_preamble(&mut self) {
        let helpers = self.helpers();
        let ssr_helpers = self.ssr_helpers();
        if self.options.mode == "module" {
            if !helpers.is_empty() {
                if self.options.optimize_imports {
                    self.push(&format!(
                        "import {{ {} }} from \"vue\"",
                        helpers
                            .iter()
                            .map(|helper| helper_name(*helper))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    self.newline();
                    self.newline();
                    self.push("// Binding optimization for webpack code-split");
                    self.newline();
                    self.push(&format!(
                        "const {}",
                        helpers
                            .iter()
                            .map(|helper| format!(
                                "_{} = {}",
                                helper_name(*helper),
                                helper_name(*helper)
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    self.newline();
                } else {
                    self.push(&format!(
                        "import {{ {} }} from \"vue\"",
                        import_helper_aliases(&helpers)
                    ));
                    self.newline();
                }
            }
            if !ssr_helpers.is_empty() {
                self.push(&format!(
                    "import {{ {} }} from \"vue/server-renderer\"",
                    import_helper_aliases(&ssr_helpers)
                ));
                self.newline();
            }
            self.gen_imports();
            self.gen_hoists();
            if self.code.is_empty() {
                self.push("\n");
            } else {
                if !self.code.ends_with('\n') {
                    self.newline();
                }
                if !self.code.ends_with("\n\n") {
                    self.newline();
                }
            }
            if !self.options.inline {
                self.push("export ");
            }
            return;
        }
        if !helpers.is_empty() {
            if self.options.prefix_identifiers {
                self.push(&format!(
                    "const {{ {} }} = {}",
                    public_helper_aliases(&helpers),
                    if self.options.ssr {
                        "require(\"vue\")"
                    } else {
                        "Vue"
                    }
                ));
                self.newline();
            } else {
                self.push(if self.options.ssr {
                    "const _Vue = require(\"vue\")"
                } else {
                    "const _Vue = Vue"
                });
                self.newline();
                let static_helpers = hoist_static_helpers(&helpers);
                if self.has_hoists() && !static_helpers.is_empty() {
                    self.push(&format!(
                        "const {{ {} }} = _Vue",
                        public_helper_aliases(&static_helpers)
                    ));
                    self.newline();
                }
            }
        }
        if !ssr_helpers.is_empty() {
            self.push(&format!(
                "const {{ {} }} = require(\"vue/server-renderer\")",
                public_helper_aliases(&ssr_helpers)
            ));
            self.newline();
        }
        self.gen_hoists();
        if self.code.is_empty() {
            self.push("\n");
        } else {
            self.newline();
        }
        if !self.options.inline {
            self.push("return ");
        }
    }

    pub(crate) fn gen_imports(&mut self) {
        let imports = public_asset_imports(self.root);
        if imports.is_empty() {
            return;
        }
        for (name, path) in imports {
            self.push(&format!("import {name} from '{}'", path));
            self.newline();
        }
        self.newline();
        self.newline();
    }

    pub(crate) fn gen_assets(&mut self) {
        let components = json_string_array(self.root.get("components"));
        let directives = json_string_array(self.root.get("directives"));
        if !components.is_empty() {
            for (index, raw) in components.iter().enumerate() {
                let maybe_self = raw.ends_with("__self");
                let id = raw.strip_suffix("__self").unwrap_or(raw);
                self.push(&format!(
                    "const {} = _resolveComponent({}{})",
                    component_asset_id(id),
                    quote_string(id),
                    if maybe_self { ", true" } else { "" }
                ));
                if index + 1 < components.len() {
                    self.newline();
                }
            }
            if !directives.is_empty() || json_u64(self.root, "temps").unwrap_or_default() > 0 {
                self.newline();
            }
        }
        if !directives.is_empty() {
            for (index, raw) in directives.iter().enumerate() {
                self.push(&format!(
                    "const {} = _resolveDirective({})",
                    directive_asset_id(raw),
                    quote_string(raw)
                ));
                if index + 1 < directives.len() {
                    self.newline();
                }
            }
            if json_u64(self.root, "temps").unwrap_or_default() > 0 {
                self.newline();
            }
        }
        if let Some(temps) = json_u64(self.root, "temps").filter(|value| *value > 0) {
            self.push("let ");
            for index in 0..temps {
                if index > 0 {
                    self.push(", ");
                }
                self.push(&format!("_temp{index}"));
            }
        }
        if !components.is_empty()
            || !directives.is_empty()
            || json_u64(self.root, "temps").unwrap_or_default() > 0
        {
            self.push("\n\n");
            self.push_indent();
        }
    }

    pub(crate) fn gen_hoists(&mut self) {
        let Some(hoists) = self.root.get("hoists").and_then(Value::as_array) else {
            return;
        };
        if hoists.is_empty() {
            return;
        }
        let previous = self.pure;
        self.pure = true;
        if !self.code.ends_with("\n\n\n") {
            self.newline();
        }
        for (index, exp) in hoists.iter().enumerate() {
            if exp.is_null() {
                continue;
            }
            self.push(&format!("const _hoisted_{} = ", index + 1));
            self.gen_node(exp);
            self.newline();
        }
        self.pure = previous;
    }

    pub(crate) fn gen_node(&mut self, node: &Value) {
        match node {
            Value::Null => self.push("null"),
            Value::String(value) => self.gen_raw_string(value),
            Value::Array(items) => self.gen_node_list_as_array(items),
            Value::Object(_) => match json_u64(node, "type") {
                Some(1) | Some(9) | Some(11) => {
                    self.gen_node(node.get("codegenNode").unwrap_or(&Value::Null));
                }
                Some(2) => self.push(&quote_string(json_str(node, "content").unwrap_or(""))),
                Some(3) => {
                    if self.pure {
                        self.push("/*@__PURE__*/");
                    }
                    self.push(&format!(
                        "_createCommentVNode({})",
                        quote_string(json_str(node, "content").unwrap_or(""))
                    ));
                }
                Some(4) => {
                    let content = json_str(node, "content").unwrap_or("");
                    if json_bool(node, "isStatic") {
                        self.push(&quote_string(content));
                    } else {
                        self.push(content);
                    }
                }
                Some(5) => {
                    if self.pure {
                        self.push("/*@__PURE__*/");
                    }
                    self.push("_toDisplayString(");
                    self.gen_node(node.get("content").unwrap_or(&Value::Null));
                    self.push(")");
                }
                Some(8) => {
                    for child in node
                        .get("children")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        self.gen_node(child);
                    }
                }
                Some(12) => self.gen_node(node.get("codegenNode").unwrap_or(&Value::Null)),
                Some(13) => self.gen_vnode_call(node),
                Some(14) => self.gen_call_expression(node),
                Some(15) => self.gen_object_expression(node),
                Some(17) => self.gen_node_list_as_array(
                    node.get("elements")
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                ),
                Some(18) => self.gen_function_expression(node),
                Some(19) => self.gen_conditional_expression(node),
                Some(20) => self.gen_cache_expression(node),
                Some(21) => self.gen_node_list(
                    node.get("body")
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                    true,
                    false,
                ),
                Some(22) => self.gen_template_literal(node),
                Some(23) => self.gen_if_statement(node),
                Some(24) => {
                    self.gen_node(node.get("left").unwrap_or(&Value::Null));
                    self.push(" = ");
                    self.gen_node(node.get("right").unwrap_or(&Value::Null));
                }
                Some(25) => {
                    self.push("(");
                    self.gen_node_list(
                        node.get("expressions")
                            .and_then(Value::as_array)
                            .map(Vec::as_slice)
                            .unwrap_or(&[]),
                        false,
                        true,
                    );
                    self.push(")");
                }
                Some(26) => {
                    self.push("return ");
                    let returns = node.get("returns").unwrap_or(&Value::Null);
                    if let Some(items) = returns.as_array() {
                        self.gen_node_list_as_array(items);
                    } else {
                        self.gen_node(returns);
                    }
                }
                _ => self.push("null"),
            },
            _ => self.push("null"),
        }
    }

    pub(crate) fn gen_node_list(&mut self, nodes: &[Value], multilines: bool, comma: bool) {
        for (index, node) in nodes.iter().enumerate() {
            self.gen_node(node);
            if index + 1 < nodes.len() {
                if multilines {
                    if comma {
                        self.push(",");
                    }
                    self.newline();
                } else if comma {
                    self.push(", ");
                }
            }
        }
    }

    pub(crate) fn gen_node_list_as_array(&mut self, nodes: &[Value]) {
        let multilines = nodes.len() > 3
            || nodes
                .iter()
                .any(|node| node.as_array().is_some() || !public_is_text_like(node));
        self.push("[");
        if multilines {
            self.indent();
        }
        self.gen_node_list(nodes, multilines, true);
        if multilines {
            self.dedent();
        }
        self.push("]");
    }

    pub(crate) fn gen_vnode_call(&mut self, node: &Value) {
        let is_block = json_bool(node, "isBlock");
        let is_component = json_bool(node, "isComponent");
        let call = if is_block {
            if self.in_ssr() || is_component {
                "_createBlock"
            } else {
                "_createElementBlock"
            }
        } else if self.in_ssr() || is_component {
            "_createVNode"
        } else {
            "_createElementVNode"
        };
        if node.get("directives").is_some_and(|value| !value.is_null()) {
            self.push("_withDirectives(");
        }
        if is_block {
            self.push(&format!(
                "(_openBlock({}), ",
                if json_bool(node, "disableTracking") {
                    "true"
                } else {
                    ""
                }
            ));
        }
        if self.pure {
            self.push("/*@__PURE__*/");
        }
        self.push(call);
        self.push("(");
        let args = nullable_args(vec![
            node.get("tag").cloned().unwrap_or(Value::Null),
            node.get("props").cloned().unwrap_or(Value::Null),
            node.get("children").cloned().unwrap_or(Value::Null),
            node.get("patchFlag")
                .map(public_patch_flag_value)
                .unwrap_or(Value::Null),
            node.get("dynamicProps").cloned().unwrap_or(Value::Null),
        ]);
        self.gen_node_list(&args, false, true);
        self.push(")");
        if is_block {
            self.push(")");
        }
        if let Some(directives) = node.get("directives").filter(|value| !value.is_null()) {
            self.push(", ");
            self.gen_node(directives);
            self.push(")");
        }
    }

    pub(crate) fn gen_call_expression(&mut self, node: &Value) {
        let callee = match node.get("callee") {
            Some(Value::String(value)) if public_helper_by_name(value).is_some() => {
                format!("_{}", helper_name(public_helper_by_name(value).unwrap()))
            }
            Some(Value::String(value)) => value.clone(),
            _ => String::new(),
        };
        if self.pure {
            self.push("/*@__PURE__*/");
        }
        self.push(&callee);
        self.push("(");
        self.gen_node_list(
            node.get("arguments")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            false,
            true,
        );
        self.push(")");
    }

    pub(crate) fn gen_object_expression(&mut self, node: &Value) {
        let properties = node
            .get("properties")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if properties.is_empty() {
            self.push("{}");
            return;
        }
        let multilines = properties.len() > 1
            || properties
                .iter()
                .any(public_object_property_prefers_multiline);
        self.push(if multilines { "{" } else { "{ " });
        if multilines {
            self.indent();
        }
        for (index, prop) in properties.iter().enumerate() {
            self.gen_property_key(prop.get("key").unwrap_or(&Value::Null));
            self.push(": ");
            self.gen_node(prop.get("value").unwrap_or(&Value::Null));
            if index + 1 < properties.len() {
                self.push(",");
                self.newline();
            }
        }
        if multilines {
            self.dedent();
        }
        self.push(if multilines { "}" } else { " }" });
    }

    pub(crate) fn gen_property_key(&mut self, key: &Value) {
        if json_u64(key, "type") == Some(8) {
            self.push("[");
            self.gen_node(key);
            self.push("]");
        } else if json_u64(key, "type") == Some(4) && json_bool(key, "isStatic") {
            let content = json_str(key, "content").unwrap_or("");
            if is_simple_identifier(content) {
                self.push(content);
            } else {
                self.push(&quote_string(content));
            }
        } else if json_u64(key, "type") == Some(4) {
            self.push("[");
            self.push(json_str(key, "content").unwrap_or(""));
            self.push("]");
        } else {
            self.push("[");
            self.gen_node(key);
            self.push("]");
        }
    }

    pub(crate) fn gen_function_expression(&mut self, node: &Value) {
        if json_bool(node, "isSlot") {
            self.push("_withCtx(");
        }
        self.push("(");
        match node.get("params") {
            Some(Value::Array(params)) => self.gen_node_list(params, false, true),
            Some(param) if !param.is_null() => self.gen_node(param),
            _ => {}
        }
        self.push(") => ");
        if json_bool(node, "newline") || node.get("body").is_some_and(|value| !value.is_null()) {
            self.push("{");
            self.indent();
        }
        if let Some(returns) = node.get("returns").filter(|value| !value.is_null()) {
            if json_bool(node, "newline") {
                self.push("return ");
            }
            if let Some(items) = returns.as_array() {
                self.gen_node_list_as_array(items);
            } else {
                self.gen_node(returns);
            }
        } else if let Some(body) = node.get("body").filter(|value| !value.is_null()) {
            self.gen_node(body);
        }
        if json_bool(node, "newline") || node.get("body").is_some_and(|value| !value.is_null()) {
            self.dedent();
            self.push("}");
        }
        if json_bool(node, "isSlot") {
            self.push(")");
        }
    }

    pub(crate) fn gen_conditional_expression(&mut self, node: &Value) {
        let test = node.get("test").unwrap_or(&Value::Null);
        let alternate = node.get("alternate").unwrap_or(&Value::Null);
        let nested = json_u64(alternate, "type") == Some(19);
        if json_u64(test, "type") != Some(4)
            || (!json_bool(test, "isStatic")
                && !is_simple_identifier(json_str(test, "content").unwrap_or("")))
        {
            self.push("(");
            self.gen_node(test);
            self.push(")");
        } else {
            self.gen_node(test);
        }
        if !json_bool(node, "newline") && node.get("newline").is_some() {
            self.push(" ? ");
            self.gen_node(node.get("consequent").unwrap_or(&Value::Null));
            self.push(" : ");
            self.gen_node(alternate);
            return;
        }
        self.indent += 1;
        self.newline();
        self.push("? ");
        self.indent += 1;
        self.gen_node(node.get("consequent").unwrap_or(&Value::Null));
        self.indent = self.indent.saturating_sub(1);
        self.newline();
        self.push(": ");
        if !nested {
            self.indent += 1;
        }
        self.gen_node(alternate);
        if !nested {
            self.indent = self.indent.saturating_sub(1);
        }
        self.indent = self.indent.saturating_sub(1);
    }

    pub(crate) fn gen_cache_expression(&mut self, node: &Value) {
        if json_bool(node, "needArraySpread") {
            self.push("[...(");
        }
        let index = json_u64(node, "index").unwrap_or_default();
        self.push(&format!("_cache[{index}] || ("));
        if json_bool(node, "needPauseTracking") {
            self.indent();
            self.push(&format!(
                "_setBlockTracking(-1{}),",
                if json_bool(node, "inVOnce") {
                    ", true"
                } else {
                    ""
                }
            ));
            self.newline();
            self.push(&format!("(_cache[{index}] = "));
            self.gen_node(node.get("value").unwrap_or(&Value::Null));
            self.push(&format!(").cacheIndex = {index},"));
            self.newline();
            self.push("_setBlockTracking(1),");
            self.newline();
            self.push(&format!("_cache[{index}]"));
            self.dedent();
        } else {
            self.push(&format!("_cache[{index}] = "));
            self.gen_node(node.get("value").unwrap_or(&Value::Null));
        }
        self.push(")");
        if json_bool(node, "needArraySpread") {
            self.push(")]");
        }
    }

    pub(crate) fn gen_template_literal(&mut self, node: &Value) {
        self.push("`");
        let elements = node
            .get("elements")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let multilines = elements.len() > 3;
        for element in elements {
            if let Some(text) = element.as_str() {
                self.push(
                    &text
                        .replace('\\', "\\\\")
                        .replace('`', "\\`")
                        .replace('$', "\\$"),
                );
            } else {
                self.push("${");
                if multilines {
                    self.indent();
                }
                self.gen_node(element);
                if multilines {
                    self.dedent();
                }
                self.push("}");
            }
        }
        self.push("`");
    }

    pub(crate) fn gen_if_statement(&mut self, node: &Value) {
        self.push("if (");
        self.gen_node(node.get("test").unwrap_or(&Value::Null));
        self.push(") {");
        self.indent();
        self.gen_node(node.get("consequent").unwrap_or(&Value::Null));
        self.dedent();
        self.push("}");
        if let Some(alternate) = node.get("alternate").filter(|value| !value.is_null()) {
            self.push(" else ");
            if json_u64(alternate, "type") == Some(23) {
                self.gen_if_statement(alternate);
            } else {
                self.push("{");
                self.indent();
                self.gen_node(alternate);
                self.dedent();
                self.push("}");
            }
        }
    }

    pub(crate) fn helpers(&self) -> Vec<RuntimeHelper> {
        let mut helpers = json_string_array(self.root.get("helpers"))
            .into_iter()
            .filter_map(|name| public_helper_by_name(&name))
            .filter(|helper| !is_ssr_helper(*helper))
            .collect::<Vec<_>>();
        helpers.dedup();
        helpers
    }

    pub(crate) fn ssr_helpers(&self) -> Vec<RuntimeHelper> {
        let mut helpers = json_string_array(self.root.get("ssrHelpers"))
            .into_iter()
            .filter_map(|name| public_helper_by_name(&name))
            .chain(
                json_string_array(self.root.get("helpers"))
                    .into_iter()
                    .filter_map(|name| public_helper_by_name(&name))
                    .filter(|helper| is_ssr_helper(*helper)),
            )
            .collect::<Vec<_>>();
        helpers.dedup();
        apply_public_ssr_helper_order_preferences(&mut helpers);
        helpers
    }

    pub(crate) fn has_hoists(&self) -> bool {
        self.root
            .get("hoists")
            .and_then(Value::as_array)
            .is_some_and(|hoists| hoists.iter().any(|exp| !exp.is_null()))
    }

    pub(crate) fn in_ssr(&self) -> bool {
        self.options.ssr
    }

    pub(crate) fn gen_raw_string(&mut self, value: &str) {
        if let Some(helper) = public_helper_by_name(value) {
            self.push(&format!("_{}", helper_name(helper)));
        } else {
            self.push(value);
        }
    }

    pub(crate) fn push(&mut self, value: &str) {
        self.code.push_str(value);
    }

    pub(crate) fn newline(&mut self) {
        self.code.push('\n');
        self.push_indent();
    }

    pub(crate) fn push_indent(&mut self) {
        for _ in 0..self.indent {
            self.code.push_str("  ");
        }
    }

    pub(crate) fn indent(&mut self) {
        self.indent += 1;
        self.newline();
    }

    pub(crate) fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
        self.newline();
    }
}

pub(crate) fn public_helper_by_name(name: &str) -> Option<RuntimeHelper> {
    match name {
        "resolveDirective" => Some(RuntimeHelper::Vue3ResolveDirective),
        "RESOLVE_DIRECTIVE" => Some(RuntimeHelper::Vue3ResolveDirective),
        "withDirectives" => Some(RuntimeHelper::Vue3WithDirectives),
        "WITH_DIRECTIVES" => Some(RuntimeHelper::Vue3WithDirectives),
        "setBlockTracking" => Some(RuntimeHelper::Vue3SetBlockTracking),
        "SET_BLOCK_TRACKING" => Some(RuntimeHelper::Vue3SetBlockTracking),
        "openBlock" => Some(RuntimeHelper::Vue3OpenBlock),
        "OPEN_BLOCK" => Some(RuntimeHelper::Vue3OpenBlock),
        "createBlock" => Some(RuntimeHelper::Vue3CreateBlock),
        "CREATE_BLOCK" => Some(RuntimeHelper::Vue3CreateBlock),
        "createElementBlock" => Some(RuntimeHelper::Vue3CreateElementBlock),
        "CREATE_ELEMENT_BLOCK" => Some(RuntimeHelper::Vue3CreateElementBlock),
        "createVNode" => Some(RuntimeHelper::Vue3CreateVNode),
        "CREATE_VNODE" => Some(RuntimeHelper::Vue3CreateVNode),
        "createElementVNode" => Some(RuntimeHelper::Vue3CreateElementVNode),
        "CREATE_ELEMENT_VNODE" => Some(RuntimeHelper::Vue3CreateElementVNode),
        "createCommentVNode" => Some(RuntimeHelper::Vue3CreateCommentVNode),
        "CREATE_COMMENT" => Some(RuntimeHelper::Vue3CreateCommentVNode),
        "createTextVNode" => Some(RuntimeHelper::Vue3CreateTextVNode),
        "CREATE_TEXT" => Some(RuntimeHelper::Vue3CreateTextVNode),
        "BaseTransition" => Some(RuntimeHelper::Vue3BaseTransition),
        "BASE_TRANSITION" => Some(RuntimeHelper::Vue3BaseTransition),
        "Transition" => Some(RuntimeHelper::Vue3Transition),
        "TRANSITION" => Some(RuntimeHelper::Vue3Transition),
        "TransitionGroup" => Some(RuntimeHelper::Vue3TransitionGroup),
        "TRANSITION_GROUP" => Some(RuntimeHelper::Vue3TransitionGroup),
        "Teleport" => Some(RuntimeHelper::Vue3Teleport),
        "TELEPORT" => Some(RuntimeHelper::Vue3Teleport),
        "Suspense" => Some(RuntimeHelper::Vue3Suspense),
        "SUSPENSE" => Some(RuntimeHelper::Vue3Suspense),
        "KeepAlive" => Some(RuntimeHelper::Vue3KeepAlive),
        "KEEP_ALIVE" => Some(RuntimeHelper::Vue3KeepAlive),
        "Fragment" => Some(RuntimeHelper::Vue3Fragment),
        "FRAGMENT" => Some(RuntimeHelper::Vue3Fragment),
        "toDisplayString" => Some(RuntimeHelper::Vue3ToDisplayString),
        "TO_DISPLAY_STRING" => Some(RuntimeHelper::Vue3ToDisplayString),
        "renderList" => Some(RuntimeHelper::Vue3RenderList),
        "RENDER_LIST" => Some(RuntimeHelper::Vue3RenderList),
        "renderSlot" => Some(RuntimeHelper::Vue3RenderSlot),
        "RENDER_SLOT" => Some(RuntimeHelper::Vue3RenderSlot),
        "normalizeClass" => Some(RuntimeHelper::Vue3NormalizeClass),
        "NORMALIZE_CLASS" => Some(RuntimeHelper::Vue3NormalizeClass),
        "normalizeProps" => Some(RuntimeHelper::Vue3NormalizeProps),
        "NORMALIZE_PROPS" => Some(RuntimeHelper::Vue3NormalizeProps),
        "normalizeStyle" => Some(RuntimeHelper::Vue3NormalizeStyle),
        "NORMALIZE_STYLE" => Some(RuntimeHelper::Vue3NormalizeStyle),
        "guardReactiveProps" => Some(RuntimeHelper::Vue3GuardReactiveProps),
        "GUARD_REACTIVE_PROPS" => Some(RuntimeHelper::Vue3GuardReactiveProps),
        "mergeProps" => Some(RuntimeHelper::Vue3MergeProps),
        "MERGE_PROPS" => Some(RuntimeHelper::Vue3MergeProps),
        "resolveComponent" => Some(RuntimeHelper::Vue3ResolveComponent),
        "RESOLVE_COMPONENT" => Some(RuntimeHelper::Vue3ResolveComponent),
        "resolveDynamicComponent" => Some(RuntimeHelper::Vue3ResolveDynamicComponent),
        "RESOLVE_DYNAMIC_COMPONENT" => Some(RuntimeHelper::Vue3ResolveDynamicComponent),
        "withCtx" => Some(RuntimeHelper::Vue3WithCtx),
        "WITH_CTX" => Some(RuntimeHelper::Vue3WithCtx),
        "createSlots" => Some(RuntimeHelper::Vue3CreateSlots),
        "CREATE_SLOTS" => Some(RuntimeHelper::Vue3CreateSlots),
        "createStaticVNode" => Some(RuntimeHelper::Vue3CreateStaticVNode),
        "CREATE_STATIC" => Some(RuntimeHelper::Vue3CreateStaticVNode),
        "withMemo" => Some(RuntimeHelper::Vue3WithMemo),
        "WITH_MEMO" => Some(RuntimeHelper::Vue3WithMemo),
        "isMemoSame" => Some(RuntimeHelper::Vue3IsMemoSame),
        "IS_MEMO_SAME" => Some(RuntimeHelper::Vue3IsMemoSame),
        "toHandlers" => Some(RuntimeHelper::Vue3ToHandlers),
        "TO_HANDLERS" => Some(RuntimeHelper::Vue3ToHandlers),
        "camelize" => Some(RuntimeHelper::Vue3Camelize),
        "CAMELIZE" => Some(RuntimeHelper::Vue3Camelize),
        "capitalize" => Some(RuntimeHelper::Vue3Capitalize),
        "CAPITALIZE" => Some(RuntimeHelper::Vue3Capitalize),
        "toHandlerKey" => Some(RuntimeHelper::Vue3ToHandlerKey),
        "TO_HANDLER_KEY" => Some(RuntimeHelper::Vue3ToHandlerKey),
        "pushScopeId" => Some(RuntimeHelper::Vue3PushScopeId),
        "PUSH_SCOPE_ID" => Some(RuntimeHelper::Vue3PushScopeId),
        "popScopeId" => Some(RuntimeHelper::Vue3PopScopeId),
        "POP_SCOPE_ID" => Some(RuntimeHelper::Vue3PopScopeId),
        "unref" => Some(RuntimeHelper::Vue3Unref),
        "UNREF" => Some(RuntimeHelper::Vue3Unref),
        "isRef" => Some(RuntimeHelper::Vue3IsRef),
        "IS_REF" => Some(RuntimeHelper::Vue3IsRef),
        "vModelRadio" => Some(RuntimeHelper::Vue3VModelRadio),
        "V_MODEL_RADIO" => Some(RuntimeHelper::Vue3VModelRadio),
        "vModelCheckbox" => Some(RuntimeHelper::Vue3VModelCheckbox),
        "V_MODEL_CHECKBOX" => Some(RuntimeHelper::Vue3VModelCheckbox),
        "vModelText" => Some(RuntimeHelper::Vue3VModelText),
        "V_MODEL_TEXT" => Some(RuntimeHelper::Vue3VModelText),
        "vModelSelect" => Some(RuntimeHelper::Vue3VModelSelect),
        "V_MODEL_SELECT" => Some(RuntimeHelper::Vue3VModelSelect),
        "vModelDynamic" => Some(RuntimeHelper::Vue3VModelDynamic),
        "V_MODEL_DYNAMIC" => Some(RuntimeHelper::Vue3VModelDynamic),
        "withModifiers" => Some(RuntimeHelper::Vue3WithModifiers),
        "V_ON_WITH_MODIFIERS" => Some(RuntimeHelper::Vue3WithModifiers),
        "withKeys" => Some(RuntimeHelper::Vue3WithKeys),
        "V_ON_WITH_KEYS" => Some(RuntimeHelper::Vue3WithKeys),
        "vShow" => Some(RuntimeHelper::Vue3VShow),
        "V_SHOW" => Some(RuntimeHelper::Vue3VShow),
        "ssrInterpolate" => Some(RuntimeHelper::Vue3SsrInterpolate),
        "SSR_INTERPOLATE" => Some(RuntimeHelper::Vue3SsrInterpolate),
        "ssrRenderVNode" => Some(RuntimeHelper::Vue3SsrRenderVNode),
        "SSR_RENDER_VNODE" => Some(RuntimeHelper::Vue3SsrRenderVNode),
        "ssrRenderComponent" => Some(RuntimeHelper::Vue3SsrRenderComponent),
        "SSR_RENDER_COMPONENT" => Some(RuntimeHelper::Vue3SsrRenderComponent),
        "ssrRenderSlot" => Some(RuntimeHelper::Vue3SsrRenderSlot),
        "SSR_RENDER_SLOT" => Some(RuntimeHelper::Vue3SsrRenderSlot),
        "ssrRenderSlotInner" => Some(RuntimeHelper::Vue3SsrRenderSlotInner),
        "SSR_RENDER_SLOT_INNER" => Some(RuntimeHelper::Vue3SsrRenderSlotInner),
        "ssrRenderClass" => Some(RuntimeHelper::Vue3SsrRenderClass),
        "SSR_RENDER_CLASS" => Some(RuntimeHelper::Vue3SsrRenderClass),
        "ssrRenderStyle" => Some(RuntimeHelper::Vue3SsrRenderStyle),
        "SSR_RENDER_STYLE" => Some(RuntimeHelper::Vue3SsrRenderStyle),
        "ssrRenderAttrs" => Some(RuntimeHelper::Vue3SsrRenderAttrs),
        "SSR_RENDER_ATTRS" => Some(RuntimeHelper::Vue3SsrRenderAttrs),
        "ssrRenderAttr" => Some(RuntimeHelper::Vue3SsrRenderAttr),
        "SSR_RENDER_ATTR" => Some(RuntimeHelper::Vue3SsrRenderAttr),
        "ssrRenderDynamicAttr" => Some(RuntimeHelper::Vue3SsrRenderDynamicAttr),
        "SSR_RENDER_DYNAMIC_ATTR" => Some(RuntimeHelper::Vue3SsrRenderDynamicAttr),
        "ssrRenderList" => Some(RuntimeHelper::Vue3SsrRenderList),
        "SSR_RENDER_LIST" => Some(RuntimeHelper::Vue3SsrRenderList),
        "ssrIncludeBooleanAttr" => Some(RuntimeHelper::Vue3SsrIncludeBooleanAttr),
        "SSR_INCLUDE_BOOLEAN_ATTR" => Some(RuntimeHelper::Vue3SsrIncludeBooleanAttr),
        "ssrLooseEqual" => Some(RuntimeHelper::Vue3SsrLooseEqual),
        "SSR_LOOSE_EQUAL" => Some(RuntimeHelper::Vue3SsrLooseEqual),
        "ssrLooseContain" => Some(RuntimeHelper::Vue3SsrLooseContain),
        "SSR_LOOSE_CONTAIN" => Some(RuntimeHelper::Vue3SsrLooseContain),
        "ssrRenderDynamicModel" => Some(RuntimeHelper::Vue3SsrRenderDynamicModel),
        "SSR_RENDER_DYNAMIC_MODEL" => Some(RuntimeHelper::Vue3SsrRenderDynamicModel),
        "ssrGetDynamicModelProps" => Some(RuntimeHelper::Vue3SsrGetDynamicModelProps),
        "SSR_GET_DYNAMIC_MODEL_PROPS" => Some(RuntimeHelper::Vue3SsrGetDynamicModelProps),
        "ssrRenderTeleport" => Some(RuntimeHelper::Vue3SsrRenderTeleport),
        "SSR_RENDER_TELEPORT" => Some(RuntimeHelper::Vue3SsrRenderTeleport),
        "ssrRenderSuspense" => Some(RuntimeHelper::Vue3SsrRenderSuspense),
        "SSR_RENDER_SUSPENSE" => Some(RuntimeHelper::Vue3SsrRenderSuspense),
        "ssrGetDirectiveProps" => Some(RuntimeHelper::Vue3SsrGetDirectiveProps),
        "SSR_GET_DIRECTIVE_PROPS" => Some(RuntimeHelper::Vue3SsrGetDirectiveProps),
        _ => None,
    }
}

pub(crate) fn public_helper_aliases(helpers: &[RuntimeHelper]) -> String {
    helpers
        .iter()
        .map(|helper| format!("{}: _{}", helper_name(*helper), helper_name(*helper)))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn hoist_static_helpers(helpers: &[RuntimeHelper]) -> Vec<RuntimeHelper> {
    [
        RuntimeHelper::Vue3CreateVNode,
        RuntimeHelper::Vue3CreateElementVNode,
        RuntimeHelper::Vue3CreateCommentVNode,
        RuntimeHelper::Vue3CreateTextVNode,
        RuntimeHelper::Vue3CreateStaticVNode,
    ]
    .into_iter()
    .filter(|helper| helpers.contains(helper))
    .collect()
}

pub(crate) fn is_ssr_helper(helper: RuntimeHelper) -> bool {
    matches!(
        helper,
        RuntimeHelper::Vue3SsrInterpolate
            | RuntimeHelper::Vue3SsrRenderVNode
            | RuntimeHelper::Vue3SsrRenderComponent
            | RuntimeHelper::Vue3SsrRenderSlot
            | RuntimeHelper::Vue3SsrRenderSlotInner
            | RuntimeHelper::Vue3SsrRenderClass
            | RuntimeHelper::Vue3SsrRenderStyle
            | RuntimeHelper::Vue3SsrRenderAttrs
            | RuntimeHelper::Vue3SsrRenderAttr
            | RuntimeHelper::Vue3SsrRenderDynamicAttr
            | RuntimeHelper::Vue3SsrRenderList
            | RuntimeHelper::Vue3SsrIncludeBooleanAttr
            | RuntimeHelper::Vue3SsrLooseEqual
            | RuntimeHelper::Vue3SsrLooseContain
            | RuntimeHelper::Vue3SsrRenderDynamicModel
            | RuntimeHelper::Vue3SsrGetDynamicModelProps
            | RuntimeHelper::Vue3SsrRenderTeleport
            | RuntimeHelper::Vue3SsrRenderSuspense
            | RuntimeHelper::Vue3SsrGetDirectiveProps
    )
}

pub(crate) fn apply_public_ssr_helper_order_preferences(helpers: &mut Vec<RuntimeHelper>) {
    move_helper_before_if_present(
        helpers,
        RuntimeHelper::Vue3SsrRenderAttrs,
        RuntimeHelper::Vue3SsrInterpolate,
    );
}

pub(crate) fn json_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn public_asset_imports(root: &Value) -> Vec<(String, String)> {
    root.get("imports")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let path = json_str(item, "path")?.to_string();
            let name = item
                .get("exp")
                .and_then(|exp| json_str(exp, "content"))
                .or_else(|| json_str(item, "name"))?;
            Some((name.to_string(), path))
        })
        .collect()
}

pub(crate) fn directive_asset_id(name: &str) -> String {
    to_valid_asset_id(name, "directive")
}

pub(crate) fn nullable_args(args: Vec<Value>) -> Vec<Value> {
    let last = args.iter().rposition(|arg| !arg.is_null());
    match last {
        Some(last) => args
            .into_iter()
            .take(last + 1)
            .map(|arg| {
                if arg.is_null() {
                    Value::String("null".to_string())
                } else {
                    arg
                }
            })
            .collect(),
        None => Vec::new(),
    }
}

pub(crate) fn public_patch_flag_value(value: &Value) -> Value {
    if let Some(number) = value.as_i64() {
        if number == 0 {
            return value.clone();
        }
        return Value::String(public_patch_flag_text(number as i32));
    }
    value.clone()
}

pub(crate) fn public_patch_flag_text(flag: i32) -> String {
    let names = patch_flag_names(flag);
    if names.is_empty() {
        flag.to_string()
    } else {
        format!("{flag} /* {} */", names.join(", "))
    }
}

pub(crate) fn patch_flag_names(flag: i32) -> Vec<&'static str> {
    if flag == -1 {
        return vec!["CACHED"];
    }
    if flag == -2 {
        return vec!["BAIL"];
    }
    [
        (1, "TEXT"),
        (2, "CLASS"),
        (4, "STYLE"),
        (8, "PROPS"),
        (16, "FULL_PROPS"),
        (32, "NEED_HYDRATION"),
        (64, "STABLE_FRAGMENT"),
        (128, "KEYED_FRAGMENT"),
        (256, "UNKEYED_FRAGMENT"),
        (512, "NEED_PATCH"),
        (1024, "DYNAMIC_SLOTS"),
        (2048, "DEV_ROOT_FRAGMENT"),
    ]
    .into_iter()
    .filter_map(|(bit, name)| if flag & bit != 0 { Some(name) } else { None })
    .collect()
}

pub(crate) fn public_is_text_like(node: &Value) -> bool {
    node.as_str().is_some() || matches!(json_u64(node, "type"), Some(2 | 4 | 5 | 8))
}

pub(crate) fn public_object_property_prefers_multiline(prop: &Value) -> bool {
    let value = prop.get("value").unwrap_or(&Value::Null);
    if json_u64(value, "type") != Some(4) {
        return true;
    }
    let content = json_str(value, "content").unwrap_or("");
    if content.contains('\n') {
        return true;
    }
    prop.get("key").is_some_and(|key| {
        json_u64(key, "type") == Some(4) && json_str(key, "content") == Some("key")
    }) && content.contains('(')
}

pub(crate) fn is_simple_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}
