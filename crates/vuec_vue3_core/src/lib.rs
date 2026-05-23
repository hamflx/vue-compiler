#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use oxc_ast::ast::{ChainElement, Expression};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vuec_ast::{
    MissingSpanReason, NodeSpan, QuoteKind, RuntimeHelper, Vue3Ast, Vue3AstKind, Vue3Directive,
    Vue3Element, Vue3ElementType, Vue3Expression, Vue3NodeKind, Vue3Prop,
};
use vuec_codegen::{CodeWriter, SourceMapArtifact, SourceMapSegment};
use vuec_html::{HtmlTokenKind, HtmlTokenizer};
use vuec_js::JsAstStore;
use vuec_pass::TransformContext;
use vuec_source::{FileId, Span};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateSource {
    pub filename: String,
    pub source: String,
    pub file_id: FileId,
    pub base_offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3CompilerOptions {
    pub prefix_identifiers: bool,
    pub mode: String,
    pub hoist_static: bool,
    pub cache_handlers: bool,
    pub scope_id: Option<String>,
    pub slotted: bool,
    pub is_ts: bool,
    pub expression_plugins: Vec<String>,
    pub source_map: bool,
    pub comments: bool,
    pub delimiters: Option<[String; 2]>,
    pub void_tags: Vec<String>,
    pub native_tags: Option<Vec<String>>,
    pub custom_elements: Vec<String>,
    pub built_in_components: Vec<String>,
    pub namespaces: BTreeMap<String, vuec_ast::HtmlNamespace>,
    pub whitespace: String,
    pub pre_tags: Vec<String>,
    pub ignore_newline_tags: Vec<String>,
    pub binding_metadata: BTreeMap<String, String>,
    pub props_aliases: BTreeMap<String, String>,
    pub inline: bool,
}

impl Default for Vue3CompilerOptions {
    fn default() -> Self {
        Self {
            prefix_identifiers: false,
            mode: "function".into(),
            hoist_static: false,
            cache_handlers: false,
            scope_id: None,
            slotted: false,
            is_ts: false,
            expression_plugins: Vec::new(),
            source_map: false,
            comments: true,
            delimiters: None,
            void_tags: Vec::new(),
            native_tags: None,
            custom_elements: Vec::new(),
            built_in_components: Vec::new(),
            namespaces: BTreeMap::new(),
            whitespace: "condense".into(),
            pre_tags: Vec::new(),
            ignore_newline_tags: Vec::new(),
            binding_metadata: BTreeMap::new(),
            props_aliases: BTreeMap::new(),
            inline: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodegenResult {
    pub code: String,
    pub map: Option<SourceMapArtifact>,
    pub ast_summary: String,
    pub diagnostics: Vec<String>,
    pub preamble: String,
}

pub struct Vue3Dialect;

impl Vue3Dialect {
    pub fn base_parse(source: TemplateSource, options: &Vue3CompilerOptions) -> Vue3Ast {
        let mut ast = Vue3Ast::new(
            Vue3NodeKind::root(),
            Some(Span::new(
                source.file_id,
                source.base_offset,
                source.base_offset + source.source.len(),
            )),
        );
        let root = ast.root;
        let mut stack = vec![root];
        let mut v_pre_depth = 0usize;
        let mut malformed_start_depth = 0usize;
        let mut namespace_stack = vec![vuec_ast::HtmlNamespace::Html];
        let mut tokenizer = if let Some([open, close]) = &options.delimiters {
            HtmlTokenizer::new(&source.source).with_interpolation_delimiters(open, close)
        } else {
            HtmlTokenizer::new(&source.source)
        };
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
            let current_parent = *stack.last().unwrap_or(&root);
            let current_namespace = namespace_stack
                .last()
                .copied()
                .unwrap_or(vuec_ast::HtmlNamespace::Html);
            match token.kind {
                HtmlTokenKind::Text(text) => {
                    if malformed_start_depth > 0 {
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    if v_pre_depth > 0 {
                        push_text(
                            &mut ast,
                            current_parent,
                            source.file_id,
                            source.base_offset + token.start,
                            &text,
                        );
                    } else {
                        push_text_and_interpolations(
                            &mut ast,
                            current_parent,
                            source.file_id,
                            source.base_offset + token.start,
                            &text,
                            options,
                        );
                    }
                }
                HtmlTokenKind::Comment(value) => {
                    extend_open_element_spans_to(&mut ast, &stack, source.base_offset + token.end);
                    if !options.comments {
                        continue;
                    }
                    let incomplete = source.source[token.start..].starts_with("<!--")
                        && token.end == source.source.len()
                        && !source.source[token.start..token.end].ends_with("-->");
                    if incomplete && value.is_empty() {
                        continue;
                    }
                    let comment_end = if incomplete {
                        token.end + "-->".len()
                    } else {
                        token.end
                    };
                    let _id = ast.push_child(
                        current_parent,
                        Vue3NodeKind::comment(value),
                        Some(Span::new(
                            source.file_id,
                            source.base_offset + token.start,
                            source.base_offset + comment_end,
                        )),
                    );
                }
                HtmlTokenKind::StartTag {
                    name,
                    attributes,
                    self_closing,
                } => {
                    let incomplete =
                        vue3_start_tag_is_incomplete(&source.source, token.start, token.end);
                    if incomplete
                        && token.end == source.source.len()
                        && !stack_is_root_only(&stack, root)
                    {
                        malformed_start_depth += 1;
                        push_incomplete_start_tag_recovery_text(
                            &mut ast,
                            current_parent,
                            &source,
                            token.start,
                            token.end,
                        );
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    let is_void = options.void_tags.iter().any(|candidate| candidate == &name);
                    let namespace = vue3_element_namespace(&name, current_namespace, options);
                    let starts_v_pre =
                        v_pre_depth == 0 && attributes.iter().any(|attr| attr.name == "v-pre");
                    let in_v_pre = v_pre_depth > 0 || starts_v_pre;
                    let id = ast.push_child(
                        current_parent,
                        vue3_element_kind(
                            name,
                            attributes,
                            self_closing,
                            options,
                            source.file_id,
                            source.base_offset,
                            in_v_pre,
                            namespace,
                        ),
                        Some(Span::new(
                            source.file_id,
                            source.base_offset + token.start,
                            source.base_offset + token.end,
                        )),
                    );
                    if !self_closing && !is_void {
                        stack.push(id);
                        namespace_stack.push(namespace);
                        if in_v_pre {
                            v_pre_depth += 1;
                        }
                    }
                }
                HtmlTokenKind::EndTag { name } => {
                    if name.is_empty() {
                        if vue3_empty_end_tag_should_be_text(&source.source, token.start, token.end)
                        {
                            push_text(
                                &mut ast,
                                current_parent,
                                source.file_id,
                                source.base_offset + token.start,
                                &source.source[token.start..token.end],
                            );
                        }
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    if current_parent_raw_text_ignores_end_tag(&ast, current_parent, &name) {
                        push_text(
                            &mut ast,
                            current_parent,
                            source.file_id,
                            source.base_offset + token.start,
                            &source.source[token.start..token.end],
                        );
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    if malformed_start_depth > 0 {
                        malformed_start_depth -= 1;
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    if !stack_has_matching_element(&ast, &stack, &name) {
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    while stack.len() > 1 {
                        let Some(node_id) = stack.pop() else {
                            break;
                        };
                        if namespace_stack.len() > 1 {
                            namespace_stack.pop();
                        }
                        if let Some(node) = ast.node(node_id) {
                            if matches!(&node.kind, Vue3AstKind::Element(element) if element.tag.eq_ignore_ascii_case(&name))
                            {
                                if let Some(node) = ast.node_mut(node_id) {
                                    if let Some(span) = node.span.source_mut() {
                                        span.end =
                                            vuec_source::BytePos(source.base_offset + token.end);
                                    }
                                }
                                if v_pre_depth > 0 {
                                    v_pre_depth -= 1;
                                }
                                break;
                            } else if let Some(node) = ast.node_mut(node_id) {
                                if let Some(span) = node.span.source_mut() {
                                    span.end =
                                        vuec_source::BytePos(source.base_offset + token.start);
                                }
                                if v_pre_depth > 0 {
                                    v_pre_depth -= 1;
                                }
                            }
                        }
                    }
                }
                HtmlTokenKind::Cdata(text) => {
                    extend_open_element_spans_to(&mut ast, &stack, source.base_offset + token.end);
                    if current_namespace != vuec_ast::HtmlNamespace::Html {
                        push_text(
                            &mut ast,
                            current_parent,
                            source.file_id,
                            source.base_offset + token.start + "<![CDATA[".len(),
                            &text,
                        );
                    }
                }
                HtmlTokenKind::BogusQuestionTag => {
                    extend_open_element_spans_to(&mut ast, &stack, source.base_offset + token.end);
                }
                HtmlTokenKind::Doctype(_) | HtmlTokenKind::Eof => {}
            }
            if eof {
                extend_open_element_spans_to(
                    &mut ast,
                    &stack,
                    source.base_offset + source.source.len(),
                );
                break;
            }
        }
        normalize_vue3_parse_text(&mut ast, options);
        ast
    }

    pub fn transform(ast: &mut Vue3Ast, ctx: &mut TransformContext) {
        let root_id = ast.root;
        let mut has_element = false;
        let mut has_nested_element = false;
        let mut has_interpolation = false;
        let mut has_text_call = false;
        let mut has_fragment = false;
        let mut has_render_list = false;
        let mut has_normalize_class = false;
        let mut has_component = false;
        let mut has_component_slots = false;
        let mut has_dynamic_component_slots = false;
        let mut walk = vec![(root_id, true)];
        while let Some((node_id, is_root)) = walk.pop() {
            if let Some(node) = ast.node(node_id) {
                if child_sequence_needs_text_vnode(ast, &node.children) {
                    has_text_call = true;
                }
                for child_id in node.children.clone() {
                    if let Some(child) = ast.node(child_id) {
                        match &child.kind {
                            Vue3AstKind::Element(element) => {
                                has_element = true;
                                if element.tag == "slot" {
                                    ctx.add_helper(RuntimeHelper::Vue3RenderSlot);
                                }
                                if element.tag_type == Vue3ElementType::Component {
                                    has_component = true;
                                    let slot_analysis = analyze_component_slots(ast, child_id);
                                    if slot_analysis.has_slots {
                                        has_component_slots = true;
                                    }
                                    if slot_analysis.has_dynamic_slots {
                                        has_dynamic_component_slots = true;
                                    }
                                }
                                if !is_root {
                                    has_nested_element = true;
                                }
                                for prop in &element.props {
                                    if let Vue3Prop::Directive(dir) = prop {
                                        match dir.name.as_str() {
                                            "for" => {
                                                has_fragment = true;
                                                has_render_list = true;
                                            }
                                            "else" | "else-if" => {
                                                has_fragment = true;
                                            }
                                            "if" => {
                                                ctx.add_helper(
                                                    RuntimeHelper::Vue3CreateCommentVNode,
                                                );
                                            }
                                            "bind"
                                                if dir.arg.as_ref().is_some_and(|arg| {
                                                    arg.source_string() == "class"
                                                }) =>
                                            {
                                                has_normalize_class = true;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                walk.push((child_id, false));
                            }
                            Vue3AstKind::Interpolation(_) => {
                                has_interpolation = true;
                            }
                            Vue3AstKind::Text(_) => {}
                            _ => {}
                        }
                    }
                }
            }
        }
        if has_element {
            ctx.add_helper(RuntimeHelper::Vue3OpenBlock);
            ctx.add_helper(RuntimeHelper::Vue3CreateElementBlock);
        }
        if has_nested_element {
            ctx.add_helper(RuntimeHelper::Vue3CreateElementVNode);
        }
        if has_text_call {
            ctx.add_helper(RuntimeHelper::Vue3CreateTextVNode);
        }
        if has_fragment {
            ctx.add_helper(RuntimeHelper::Vue3Fragment);
        }
        if has_render_list {
            ctx.add_helper(RuntimeHelper::Vue3RenderList);
        }
        if has_normalize_class {
            ctx.add_helper(RuntimeHelper::Vue3NormalizeClass);
        }
        if has_component {
            ctx.add_helper(RuntimeHelper::Vue3ResolveComponent);
            ctx.add_helper(RuntimeHelper::Vue3OpenBlock);
            ctx.add_helper(RuntimeHelper::Vue3CreateBlock);
        }
        if has_component_slots {
            ctx.add_helper(RuntimeHelper::Vue3WithCtx);
        }
        if has_dynamic_component_slots {
            ctx.add_helper(RuntimeHelper::Vue3CreateSlots);
        }
        if has_interpolation {
            ctx.add_helper(RuntimeHelper::Vue3ToDisplayString);
        }
    }

    pub fn generate(
        ast: &Vue3Ast,
        options: &Vue3CompilerOptions,
        ctx: &TransformContext,
    ) -> CodegenResult {
        let mut writer = CodeWriter::new();
        let root_id = ast.root;
        if let Some(root) = ast.node(root_id) {
            let components = collect_component_tags(ast);
            let component_declarations = components
                .iter()
                .map(|component| {
                    format!(
                        "const {} = _resolveComponent({})",
                        component_asset_id(component),
                        quote_string(component)
                    )
                })
                .collect::<Vec<_>>();
            let expr = if root.children.len() == 1 {
                render_node_expr(ast, root.children[0], options, NodeRenderMode::Root)
            } else {
                render_children_array(ast, &root.children, options, true)
            };
            let helper_probe = format!("{}\n{}", component_declarations.join("\n"), expr);
            let helpers = if components.is_empty() {
                render_helpers(vue3_helper_order(false), ctx)
            } else {
                render_helpers_from_code(vue3_helper_order(true), &helper_probe)
            };
            if options.inline {
                writer.push_line("(_ctx, _cache) => {");
            } else if options.mode == "module" {
                if !helpers.is_empty() {
                    writer.push_line(&format!(
                        "import {{ {} }} from \"vue\"",
                        import_helper_aliases(&helpers)
                    ));
                    writer.newline();
                }
                writer.push_line("export function render(_ctx, _cache) {");
            } else if options.prefix_identifiers {
                if !helpers.is_empty() {
                    writer.push_line(&format!("const {{ {} }} = Vue", helper_aliases(&helpers)));
                    writer.newline();
                }
                writer.push_line(&format!(
                    "return function render({}) {{",
                    render_args(options)
                ));
            } else if options.mode == "function" {
                writer.push_line("const _Vue = Vue");
                writer.newline();
                writer.push_line(&format!(
                    "return function render({}) {{",
                    render_args(options)
                ));
            } else {
                writer.push_line(&format!(
                    "export function render({}) {{",
                    render_args(options)
                ));
            }
            writer.indent();
            if !options.inline && !options.prefix_identifiers && options.mode != "module" {
                writer.push_line("with (_ctx) {");
                writer.indent();
                if !helpers.is_empty() {
                    writer.push_line(&format!("const {{ {} }} = _Vue", helper_aliases(&helpers)));
                    writer.newline();
                }
            }
            for declaration in &component_declarations {
                writer.push_line(declaration);
            }
            if !component_declarations.is_empty() {
                writer.newline();
            }
            writer.push_line(&format!("return {}", expr));
            if !options.inline && !options.prefix_identifiers && options.mode != "module" {
                writer.dedent();
                writer.push_line("}");
            }
            writer.dedent();
            writer.push_line("}");
        }
        let code = writer.finish().trim_end().to_string();
        CodegenResult {
            code,
            map: None,
            ast_summary: format!("nodes={}", ast.len()),
            diagnostics: Vec::new(),
            preamble: String::new(),
        }
    }

    pub fn base_compile(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
        let mut ast = Self::base_parse(source.clone(), &options);
        let mut ctx = TransformContext::default();
        Self::transform(&mut ast, &mut ctx);
        Self::finish_compile(ast, source, options, ctx)
    }

    pub fn finish_compile(
        ast: Vue3Ast,
        source: TemplateSource,
        options: Vue3CompilerOptions,
        ctx: TransformContext,
    ) -> CodegenResult {
        let mut result = Self::generate(&ast, &options, &ctx);
        if options.source_map {
            result.map = source_map_for_render(&result.code, &ast, &source, &options);
        }
        result.diagnostics = expression_diagnostics(&ast, &options);
        result.diagnostics.extend(
            ctx.diagnostics
                .into_vec()
                .into_iter()
                .map(|diagnostic| diagnostic.message),
        );
        result
    }

    pub fn compile_dom(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
        let mut result = Self::base_compile(source, options);
        if result.code.is_empty() {
            result.code = "/* dom */".into();
        }
        result
    }

    pub fn compile_ssr(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
        let mut result = Self::base_compile(source, options);
        if !result.code.starts_with("/* ssr */") {
            result.code = format!("/* ssr */\n{}", result.code);
        }
        result
    }
}

pub fn root_codegen_projection(root: &Value) -> Value {
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    match children {
        [] => json!({ "kind": "none" }),
        [_] => root_single_child_codegen_projection(children),
        _ => json!({
            "kind": "fragment",
            "patchFlag": root_fragment_patch_flag(children),
        }),
    }
}

fn root_single_child_codegen_projection(children: &[Value]) -> Value {
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

fn single_element_root(children: &[Value]) -> Option<(usize, &Value)> {
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

fn root_fragment_patch_flag(children: &[Value]) -> u16 {
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

fn json_node_type(value: &Value) -> Option<u64> {
    json_u64(value, "type")
}

fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

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

fn model_assignment_projection(
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

fn model_prop_name_projection(arg: Option<&Value>) -> Value {
    match arg {
        Some(_) => json!({ "kind": "node", "path": "dir.arg" }),
        None => json!({ "kind": "static", "content": "modelValue" }),
    }
}

fn model_event_name_projection(arg: Option<&Value>) -> Value {
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

fn model_update_needs_hydration_event(arg: Option<&Value>, node: &Value) -> bool {
    arg.is_some_and(|arg| json_bool(arg, "isStatic")) && json_u64(node, "tagType") != Some(1)
}

fn model_modifiers_key_projection(arg: Option<&Value>) -> Value {
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

fn model_modifiers_expression(dir: &Value) -> Value {
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

fn should_cache_model_update(exp: &Value, context: &Value) -> bool {
    json_bool(context, "prefixIdentifiers")
        && json_bool(context, "cacheHandlers")
        && !json_bool(context, "inVOnce")
        && !model_has_scope_ref(exp, context)
}

fn model_has_scope_ref(exp: &Value, context: &Value) -> bool {
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

fn model_expression_source(exp: &Value) -> String {
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

fn model_expression_child_source(child: &Value) -> String {
    child
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| model_expression_source(child))
}

fn model_is_member_expression(expression: &str) -> bool {
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

fn model_chain_element_is_member(element: &ChainElement<'_>) -> bool {
    matches!(
        element,
        ChainElement::ComputedMemberExpression(_)
            | ChainElement::StaticMemberExpression(_)
            | ChainElement::PrivateFieldExpression(_)
    )
}

fn context_identifier_count<'a>(context: &'a Value, name: &str) -> i64 {
    context
        .get("identifiers")
        .and_then(|identifiers| identifiers.get(name))
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

fn camelize(value: &str) -> String {
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

fn is_simple_identifier_ascii(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn json_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn json_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn vue3_element_kind(
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

fn vue3_element_namespace(
    tag: &str,
    parent: vuec_ast::HtmlNamespace,
    options: &Vue3CompilerOptions,
) -> vuec_ast::HtmlNamespace {
    options.namespaces.get(tag).copied().unwrap_or(parent)
}

fn vue3_tag_type(tag: &str, props: &[Vue3Prop], options: &Vue3CompilerOptions) -> Vue3ElementType {
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

fn is_template_directive(name: &str) -> bool {
    matches!(name, "if" | "else" | "else-if" | "for" | "slot")
}

fn vue3_prop_from_attr(
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
            exp: attr.value.map(Vue3Expression::Raw),
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

fn vue3_attribute_from_attr(
    attr: vuec_html::HtmlAttribute,
    file_id: FileId,
    base_offset: usize,
) -> Vue3Prop {
    Vue3Prop::Attribute(vue3_attr_from_html(attr, file_id, base_offset).attr)
}

struct ParsedVue3Attribute {
    attr: vuec_ast::Vue3Attribute,
    value_content_span: Option<Span>,
}

fn vue3_attr_from_html(
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

fn parse_vue3_directive(
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

fn split_directive_parts(source: &str, preserve_dots: bool) -> impl Iterator<Item = &str> {
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

fn directive_modifier_spans(
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

fn vue3_start_tag_is_incomplete(source: &str, start: usize, end: usize) -> bool {
    source
        .get(start..end)
        .is_some_and(|slice| !slice.ends_with('>'))
}

fn vue3_empty_end_tag_should_be_text(source: &str, start: usize, end: usize) -> bool {
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

fn stack_is_root_only(stack: &[vuec_ast::NodeId], root: vuec_ast::NodeId) -> bool {
    stack.len() == 1 && stack.first().copied() == Some(root)
}

fn push_incomplete_start_tag_recovery_text(
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

fn incomplete_start_tag_recovery_text_start(slice: &str) -> Option<usize> {
    slice.rfind('/').filter(|index| {
        slice
            .get(index + 1..)
            .is_some_and(|tail| tail.chars().all(char::is_whitespace))
    })
}

fn current_parent_raw_text_ignores_end_tag(
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

fn stack_has_matching_element(ast: &Vue3Ast, stack: &[vuec_ast::NodeId], name: &str) -> bool {
    stack.iter().copied().skip(1).any(|node_id| {
        ast.node(node_id).is_some_and(|node| {
            matches!(
                &node.kind,
                Vue3AstKind::Element(element) if element.tag.eq_ignore_ascii_case(name)
            )
        })
    })
}

fn extend_open_element_spans_to(ast: &mut Vue3Ast, stack: &[vuec_ast::NodeId], end: usize) {
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

fn normalize_vue3_parse_text(ast: &mut Vue3Ast, options: &Vue3CompilerOptions) {
    normalize_class_attribute_values(ast);
    remove_initial_newline_after_ignore_newline_tags(ast, options);
    normalize_text_children(ast, ast.root, options, false);
}

fn normalize_class_attribute_values(ast: &mut Vue3Ast) {
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
            }
        }
    }
}

fn remove_initial_newline_after_ignore_newline_tags(
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
                if text.value.starts_with('\n') {
                    text.value.remove(0);
                    if let Some(span) = child.span.source_mut() {
                        span.start.0 += 1;
                    }
                }
            }
        }
    }
}

fn normalize_text_children(
    ast: &mut Vue3Ast,
    parent_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    in_pre: bool,
) {
    let Some(parent) = ast.node(parent_id) else {
        return;
    };
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
        if text.value.chars().all(char::is_whitespace) {
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

fn should_keep_whitespace_between(
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

fn condense_whitespace(value: &str) -> String {
    let mut out = String::new();
    let mut previous_ws = false;
    for ch in value.chars() {
        if ch.is_whitespace() {
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

fn render_helpers_from_code(order: &[RuntimeHelper], code: &str) -> Vec<RuntimeHelper> {
    order
        .iter()
        .copied()
        .filter(|helper| code.contains(&helper_reference(*helper)))
        .collect()
}

fn render_helpers(order: &[RuntimeHelper], ctx: &TransformContext) -> Vec<RuntimeHelper> {
    order
        .iter()
        .copied()
        .filter(|helper| ctx.helpers.contains(helper))
        .collect()
}

fn vue3_helper_order(components_first: bool) -> &'static [RuntimeHelper] {
    if components_first {
        &[
            RuntimeHelper::Vue3ToDisplayString,
            RuntimeHelper::Vue3CreateTextVNode,
            RuntimeHelper::Vue3CreateElementVNode,
            RuntimeHelper::Vue3ResolveComponent,
            RuntimeHelper::Vue3WithCtx,
            RuntimeHelper::Vue3RenderList,
            RuntimeHelper::Vue3CreateSlots,
            RuntimeHelper::Vue3OpenBlock,
            RuntimeHelper::Vue3CreateBlock,
            RuntimeHelper::Vue3CreateVNode,
            RuntimeHelper::Vue3CreateCommentVNode,
            RuntimeHelper::Vue3Fragment,
            RuntimeHelper::Vue3CreateElementBlock,
            RuntimeHelper::Vue3RenderSlot,
            RuntimeHelper::Vue3NormalizeClass,
        ]
    } else {
        &[
            RuntimeHelper::Vue3ToDisplayString,
            RuntimeHelper::Vue3OpenBlock,
            RuntimeHelper::Vue3CreateElementBlock,
            RuntimeHelper::Vue3CreateCommentVNode,
            RuntimeHelper::Vue3CreateTextVNode,
            RuntimeHelper::Vue3Fragment,
            RuntimeHelper::Vue3RenderList,
            RuntimeHelper::Vue3CreateElementVNode,
            RuntimeHelper::Vue3RenderSlot,
            RuntimeHelper::Vue3NormalizeClass,
            RuntimeHelper::Vue3ResolveComponent,
            RuntimeHelper::Vue3WithCtx,
            RuntimeHelper::Vue3CreateBlock,
            RuntimeHelper::Vue3CreateVNode,
            RuntimeHelper::Vue3CreateSlots,
        ]
    }
}

fn helper_reference(helper: RuntimeHelper) -> String {
    format!("_{}", helper_name(helper))
}

fn helper_aliases(helpers: &[RuntimeHelper]) -> String {
    helpers
        .iter()
        .map(|helper| format!("{}: _{}", helper_name(*helper), helper_name(*helper)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn import_helper_aliases(helpers: &[RuntimeHelper]) -> String {
    helpers
        .iter()
        .map(|helper| format!("{} as _{}", helper_name(*helper), helper_name(*helper)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_args(options: &Vue3CompilerOptions) -> String {
    if options.binding_metadata.is_empty() || options.inline {
        "_ctx, _cache".into()
    } else {
        "_ctx, _cache, $props, $setup, $data, $options".into()
    }
}

fn helper_name(helper: RuntimeHelper) -> &'static str {
    match helper {
        RuntimeHelper::Vue2CreateElement => "createElement",
        RuntimeHelper::Vue2CreateTextVNode => "createTextVNode",
        RuntimeHelper::Vue2ToString => "toString",
        RuntimeHelper::Vue2RenderList => "renderList",
        RuntimeHelper::Vue2ResolveFilter => "resolveFilter",
        RuntimeHelper::Vue3OpenBlock => "openBlock",
        RuntimeHelper::Vue3CreateElementVNode => "createElementVNode",
        RuntimeHelper::Vue3CreateElementBlock => "createElementBlock",
        RuntimeHelper::Vue3CreateCommentVNode => "createCommentVNode",
        RuntimeHelper::Vue3CreateTextVNode => "createTextVNode",
        RuntimeHelper::Vue3Fragment => "Fragment",
        RuntimeHelper::Vue3ToDisplayString => "toDisplayString",
        RuntimeHelper::Vue3RenderList => "renderList",
        RuntimeHelper::Vue3RenderSlot => "renderSlot",
        RuntimeHelper::Vue3NormalizeClass => "normalizeClass",
        RuntimeHelper::Vue3ResolveComponent => "resolveComponent",
        RuntimeHelper::Vue3WithCtx => "withCtx",
        RuntimeHelper::Vue3CreateBlock => "createBlock",
        RuntimeHelper::Vue3CreateVNode => "createVNode",
        RuntimeHelper::Vue3CreateSlots => "createSlots",
    }
}

fn source_map_for_render(
    code: &str,
    ast: &Vue3Ast,
    source: &TemplateSource,
    options: &Vue3CompilerOptions,
) -> Option<SourceMapArtifact> {
    let root = ast.node(ast.root)?;
    let source_name = if source.filename.is_empty() {
        "template.vue.html".to_string()
    } else {
        source.filename.clone()
    };
    let mut names = Vec::new();
    let mut segments = Vec::new();
    collect_source_map_segments(
        code,
        ast,
        &root.children,
        source.base_offset,
        &source.source,
        options,
        &mut names,
        &mut segments,
    );
    if segments.is_empty() {
        return None;
    }
    segments.sort_by_key(|segment| {
        (
            segment.generated_line,
            segment.generated_column,
            segment.original_line,
            segment.original_column,
            segment.name_index.unwrap_or(usize::MAX),
        )
    });
    segments.dedup_by_key(|segment| {
        (
            segment.generated_line,
            segment.generated_column,
            segment.original_line,
            segment.original_column,
            segment.name_index,
        )
    });
    Some(SourceMapArtifact::from_segments(
        None,
        source_name,
        source.source.clone(),
        names,
        segments,
    ))
}

fn collect_source_map_segments(
    code: &str,
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    let mut cursor = 0usize;
    for child_id in children {
        collect_node_source_map(
            code,
            ast,
            *child_id,
            base_offset,
            source,
            options,
            names,
            segments,
            &mut cursor,
        );
    }
}

fn collect_node_source_map(
    code: &str,
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    match &node.kind {
        Vue3AstKind::Element(element) => {
            add_vnode_mapping(code, node, base_offset, source, segments, cursor);
            add_element_prop_mappings(code, element, base_offset, source, options, names, segments);
            for child_id in &node.children {
                collect_node_source_map(
                    code,
                    ast,
                    *child_id,
                    base_offset,
                    source,
                    options,
                    names,
                    segments,
                    cursor,
                );
            }
        }
        Vue3AstKind::Interpolation(_) => {
            add_interpolation_mapping(
                code,
                node,
                base_offset,
                source,
                options,
                names,
                segments,
                cursor,
            );
        }
        _ => {}
    }
}

fn add_vnode_mapping(
    code: &str,
    node: &vuec_ast::Node<Vue3NodeKind>,
    base_offset: usize,
    source: &str,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Some(span) = node.span.source() else {
        return;
    };
    let local_start = span.start.0.saturating_sub(base_offset);
    let local_end = span.end.0.saturating_sub(base_offset);
    let Some(start) = loc_for_offset(source, local_start) else {
        return;
    };
    let Some(end) = loc_for_offset(source, local_end) else {
        return;
    };
    let tag = match &node.kind {
        Vue3AstKind::Element(element) => &element.tag,
        _ => return,
    };
    let block_needle = format!("_createElementBlock(\"{tag}\"");
    let vnode_needle = format!("_createElementVNode(\"{tag}\"");
    let block_offset = find_code_offset(code, &block_needle, *cursor);
    let vnode_offset = find_code_offset(code, &vnode_needle, *cursor);
    let helper_offset = match (block_offset, vnode_offset) {
        (Some(block), Some(vnode)) => block.min(vnode),
        (Some(block), None) => block,
        (None, Some(vnode)) => vnode,
        (None, None) => return,
    };
    if let Some((line, column)) = loc_for_offset(code, helper_offset) {
        segments.push(SourceMapSegment {
            generated_line: line,
            generated_column: column,
            original_line: start.0,
            original_column: start.1,
            name_index: None,
        });
        let tag_needle = format!("\"{tag}\"");
        if let Some(tag_offset) = find_code_offset(code, &tag_needle, helper_offset) {
            if let Some((end_line, end_column)) = loc_for_offset(code, tag_offset) {
                segments.push(SourceMapSegment {
                    generated_line: end_line,
                    generated_column: end_column,
                    original_line: end.0,
                    original_column: end.1,
                    name_index: None,
                });
                *cursor = tag_offset + tag_needle.len();
            }
        } else {
            *cursor = helper_offset + block_needle.len().min(vnode_needle.len());
        }
    }
}

fn add_interpolation_mapping(
    code: &str,
    node: &vuec_ast::Node<Vue3NodeKind>,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Vue3AstKind::Interpolation(interpolation) = &node.kind else {
        return;
    };
    let Some(span) = node.span.source() else {
        return;
    };
    let expression = interpolation.expression.source_string();
    let local_start = span.start.0.saturating_sub(base_offset);
    let local_end = span.end.0.saturating_sub(base_offset);
    let Some(original_start) = source[local_start..local_end]
        .find(expression.trim())
        .map(|offset| local_start + offset)
    else {
        return;
    };
    add_expression_token_mappings(
        code,
        source,
        expression.trim(),
        original_start,
        *cursor,
        uses_prefixed_identifiers(options),
        names,
        segments,
    );
    if let Some(offset) = find_code_offset(code, expression.trim(), *cursor)
        .or_else(|| find_code_offset(code, &format!("_ctx.{}", expression.trim()), *cursor))
    {
        *cursor = offset + expression.trim().len();
    }
}

fn add_element_prop_mappings(
    code: &str,
    element: &Vue3Element,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    for prop in &element.props {
        match prop {
            Vue3Prop::Attribute(attr) => {
                if let Some(span) = attr.name_span {
                    add_direct_mapping(
                        code,
                        source,
                        &attr.name,
                        span.start.0.saturating_sub(base_offset),
                        0,
                        None,
                        segments,
                    );
                }
                if let (Some(value), Some(span)) = (&attr.value, attr.value_span) {
                    add_direct_mapping(
                        code,
                        source,
                        &quote_string(value),
                        span.start.0.saturating_sub(base_offset),
                        0,
                        None,
                        segments,
                    );
                }
            }
            Vue3Prop::Directive(dir) => {
                if dir.name == "bind"
                    && dir
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.source_string() == "class")
                {
                    if let Some(arg_span) = dir.arg_span {
                        add_direct_mapping(
                            code,
                            source,
                            "class:",
                            arg_span.start.0.saturating_sub(base_offset),
                            0,
                            None,
                            segments,
                        );
                    }
                    if let (Some(exp), Some(span)) = (&dir.exp, dir.exp_span) {
                        let expression = exp.source_string();
                        add_expression_token_mappings(
                            code,
                            source,
                            expression.trim(),
                            span.start.0.saturating_sub(base_offset),
                            0,
                            uses_prefixed_identifiers(options),
                            names,
                            segments,
                        );
                    }
                }
                if matches!(dir.name.as_str(), "if" | "else-if" | "for") {
                    if let (Some(exp), Some(span)) = (&dir.exp, dir.exp_span) {
                        let expression = exp.source_string();
                        add_expression_token_mappings(
                            code,
                            source,
                            expression.trim(),
                            span.start.0.saturating_sub(base_offset),
                            0,
                            uses_prefixed_identifiers(options),
                            names,
                            segments,
                        );
                    }
                }
            }
        }
    }
}

fn add_direct_mapping(
    code: &str,
    source: &str,
    generated_needle: &str,
    original_offset: usize,
    generated_from: usize,
    name: Option<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    let Some(generated_offset) = find_code_offset(code, generated_needle, generated_from) else {
        return;
    };
    let Some((generated_line, generated_column)) = loc_for_offset(code, generated_offset) else {
        return;
    };
    let Some((original_line, original_column)) = loc_for_offset(source, original_offset) else {
        return;
    };
    let name_index = name.map(|_| 0);
    segments.push(SourceMapSegment {
        generated_line,
        generated_column,
        original_line,
        original_column,
        name_index,
    });
}

fn add_expression_token_mappings(
    code: &str,
    source: &str,
    expression: &str,
    original_expression_start: usize,
    generated_from: usize,
    precise_members: bool,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    for token in expression_source_map_tokens(expression) {
        let generated_needles = if uses_ctx_prefix_for_generated(code, token) {
            vec![format!("_ctx.{token}"), token.to_string()]
        } else {
            vec![token.to_string(), format!("_ctx.{token}")]
        };
        let generated_offset = generated_needles
            .iter()
            .find_map(|needle| find_code_offset(code, needle, generated_from));
        let Some(generated_offset) = generated_offset else {
            continue;
        };
        let Some(original_relative) = expression.find(token) else {
            continue;
        };
        let original_offset = if precise_members || !is_member_tail_token(expression, token) {
            original_expression_start + original_relative
        } else {
            original_expression_start
        };
        let Some((generated_line, generated_column)) = loc_for_offset(code, generated_offset)
        else {
            continue;
        };
        let Some((original_line, original_column)) = loc_for_offset(source, original_offset) else {
            continue;
        };
        let name_index = Some(name_index(names, token));
        segments.push(SourceMapSegment {
            generated_line,
            generated_column,
            original_line,
            original_column,
            name_index,
        });
    }
}

fn uses_ctx_prefix_for_generated(code: &str, token: &str) -> bool {
    code.contains(&format!("_ctx.{token}"))
}

fn is_member_tail_token(expression: &str, token: &str) -> bool {
    expression
        .match_indices(token)
        .any(|(index, _)| index > 0 && expression[..index].ends_with('.'))
}

fn expression_source_map_tokens(expression: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    for (index, ch) in expression.char_indices() {
        if !is_identifier_start(ch) {
            continue;
        }
        if index > 0
            && expression[..index]
                .chars()
                .last()
                .is_some_and(is_identifier_continue)
        {
            continue;
        }
        let end = expression[index + ch.len_utf8()..]
            .char_indices()
            .find_map(|(offset, current)| {
                (!is_identifier_continue(current)).then_some(index + ch.len_utf8() + offset)
            })
            .unwrap_or(expression.len());
        let token = &expression[index..end];
        if !is_keyword(token) && !is_global_or_literal(token) {
            tokens.push(token);
        }
    }
    if tokens.is_empty() && !expression.is_empty() {
        tokens.push(expression);
    }
    tokens
}

fn name_index(names: &mut Vec<String>, name: &str) -> usize {
    if let Some(index) = names.iter().position(|existing| existing == name) {
        index
    } else {
        names.push(name.to_string());
        names.len() - 1
    }
}

fn loc_for_offset(source: &str, offset: usize) -> Option<(u32, u32)> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let mut line = 0u32;
    let mut line_start = 0usize;
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < offset {
        match bytes[index] {
            b'\r' => {
                if index + 1 < offset && bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                line += 1;
                line_start = index + 1;
            }
            b'\n' => {
                line += 1;
                line_start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    let column = source[line_start..offset].encode_utf16().count() as u32;
    Some((line, column))
}

fn find_code_offset(code: &str, needle: &str, from: usize) -> Option<usize> {
    code.get(from..)?.find(needle).map(|offset| from + offset)
}

fn render_children_array(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    is_root: bool,
) -> String {
    let scope = RenderScope::default();
    let rendered = render_child_sequence(
        ast,
        children,
        options,
        if is_root {
            NodeRenderMode::Root
        } else {
            NodeRenderMode::Child
        },
        &scope,
    );
    render_array(&rendered)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeRenderMode {
    Root,
    Child,
    Cached,
}

#[derive(Clone, Debug, Default)]
struct RenderScope {
    locals: Vec<String>,
}

impl RenderScope {
    fn with_locals(&self, locals: Vec<String>) -> Self {
        let mut next = self.clone();
        for local in locals {
            if !next.locals.iter().any(|existing| existing == &local) {
                next.locals.push(local);
            }
        }
        next
    }
}

fn render_node_expr(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
) -> String {
    render_node_expr_scoped(ast, node_id, options, mode, &RenderScope::default())
}

fn render_node_expr_scoped(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
) -> String {
    let Some(node) = ast.node(node_id) else {
        return "null".into();
    };
    match &node.kind {
        Vue3AstKind::Root(_) => {
            let rendered =
                render_child_sequence(ast, &node.children, options, NodeRenderMode::Root, scope);
            format!("[{}]", rendered.join(", "))
        }
        Vue3AstKind::Text(text) => quote_text(&text.value),
        Vue3AstKind::Interpolation(interpolation) => {
            format!(
                "_toDisplayString({})",
                rewrite_expression_with_scope(
                    &interpolation.expression.source_string(),
                    options,
                    scope
                )
            )
        }
        Vue3AstKind::Comment(comment) => format!("/*{}*/", comment.value),
        Vue3AstKind::Element(element) => {
            if let Some(for_dir) = directive_by_name(element, "for") {
                return render_for_node(ast, node_id, element, for_dir, options, mode, scope);
            }
            if directive_by_name(element, "if").is_some() {
                return render_if_chain(ast, &[node_id], options, mode, scope);
            }
            if is_else_branch(element) {
                return "null".into();
            }
            render_plain_element(ast, node_id, element, options, mode, scope, None)
        }
        _ => "null".into(),
    }
}

fn render_plain_element(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: Option<usize>,
) -> String {
    let tag = &element.tag;
    if tag == "slot" {
        return render_slot_outlet(element, options, scope);
    }
    if element.tag_type == Vue3ElementType::Component {
        return render_component_element(ast, node_id, element, options, mode, scope, branch_key);
    }
    let helper = if mode == NodeRenderMode::Root {
        "_createElementBlock"
    } else {
        "_createElementVNode"
    };
    let props = render_props(element, options, scope, branch_key);
    let children = ast
        .node(node_id)
        .map(|node| render_element_children(ast, &node.children, options, mode, scope))
        .unwrap_or_default();
    let patch_flag = render_patch_flag(ast, node_id, element, options, mode);
    let attrs = if props.is_empty() { None } else { Some(props) };
    let args = render_call_args(
        quote_string(tag),
        attrs.as_deref(),
        (!children.is_empty()).then_some(children.as_str()),
        patch_flag.as_str(),
        dynamic_props_arg(element).as_str(),
    );
    if mode == NodeRenderMode::Root {
        format!("(_openBlock(), {}({}))", helper, args)
    } else {
        format!("{}({})", helper, args)
    }
}

fn render_call_args(
    tag: String,
    props: Option<&str>,
    children: Option<&str>,
    patch_flag: &str,
    dynamic_props: &str,
) -> String {
    let mut args = vec![tag];
    if let Some(props) = props {
        args.push(props.to_string());
    } else if children.is_some() || !patch_flag.is_empty() || !dynamic_props.is_empty() {
        args.push("null".into());
    }
    if let Some(children) = children {
        args.push(children.to_string());
    } else if !patch_flag.is_empty() || !dynamic_props.is_empty() {
        args.push("null".into());
    }
    if !patch_flag.is_empty() {
        args.push(patch_flag.trim_start_matches(", ").to_string());
    }
    if !dynamic_props.is_empty() {
        args.push(dynamic_props.trim_start_matches(", ").to_string());
    }
    args.join(", ")
}

fn render_component_element(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: Option<usize>,
) -> String {
    let tag = component_asset_id(&element.tag);
    let props = render_props(element, options, scope, branch_key);
    let attrs = if props.is_empty() {
        "null".into()
    } else {
        props
    };
    let children = render_component_slots(ast, node_id, options, scope);
    let patch_flag = component_patch_flag(ast, node_id);
    let helper = if mode == NodeRenderMode::Root {
        "_createBlock"
    } else {
        "_createVNode"
    };
    let children_arg = children.map_or_else(String::new, |children| format!(", {children}"));
    if mode == NodeRenderMode::Root {
        format!(
            "(_openBlock(), {}({}, {}{}{}))",
            helper, tag, attrs, children_arg, patch_flag
        )
    } else {
        format!(
            "{}({}, {}{}{})",
            helper, tag, attrs, children_arg, patch_flag
        )
    }
}

#[derive(Clone, Debug, Default)]
struct ComponentSlotAnalysis {
    has_slots: bool,
    has_dynamic_slots: bool,
}

fn analyze_component_slots(ast: &Vue3Ast, node_id: vuec_ast::NodeId) -> ComponentSlotAnalysis {
    let Some(node) = ast.node(node_id) else {
        return ComponentSlotAnalysis::default();
    };
    let visible = visible_children(ast, &node.children);
    if visible.is_empty() {
        return ComponentSlotAnalysis::default();
    }
    let mut analysis = ComponentSlotAnalysis {
        has_slots: true,
        has_dynamic_slots: false,
    };
    for child in visible {
        if let Vue3AstKind::Element(element) = &child.kind {
            if directive_by_name(element, "slot").is_some()
                && (directive_by_name(element, "if").is_some()
                    || directive_by_name(element, "for").is_some()
                    || directive_by_name(element, "else").is_some()
                    || directive_by_name(element, "else-if").is_some()
                    || directive_by_name(element, "slot").is_some_and(|slot| slot.is_dynamic_arg))
            {
                analysis.has_dynamic_slots = true;
            }
        }
    }
    analysis
}

fn render_component_slots(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    let node = ast.node(node_id)?;
    let visible = visible_children(ast, &node.children);
    if visible.is_empty() {
        return None;
    }
    let dynamic_slots = visible.iter().any(|child| {
        matches!(
            &child.kind,
            Vue3AstKind::Element(element)
                if directive_by_name(element, "slot").is_some()
                    && (directive_by_name(element, "if").is_some()
                        || directive_by_name(element, "for").is_some()
                        || directive_by_name(element, "else").is_some()
                        || directive_by_name(element, "else-if").is_some())
        )
    });
    if dynamic_slots {
        Some(render_dynamic_component_slots(
            ast, &visible, options, scope,
        ))
    } else {
        Some(render_stable_component_slots(ast, &visible, options, scope))
    }
}

fn render_stable_component_slots(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let mut slots = Vec::new();
    let mut default_children = Vec::new();
    for child in children {
        if let Vue3AstKind::Element(element) = &child.kind {
            if let Some(slot) = directive_by_name(element, "slot") {
                slots.push(render_static_slot_property(
                    ast, child.id, element, slot, options, scope,
                ));
                continue;
            }
        }
        default_children.push(child.id);
    }
    if !default_children.is_empty() {
        slots.push(render_slot_property(
            "default",
            "()",
            render_slot_children(ast, &default_children, options, scope),
        ));
    }
    slots.push("_: 1 /* STABLE */".into());
    render_object(&slots)
}

fn render_dynamic_component_slots(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let mut dynamic_entries = Vec::new();
    for (index, child) in children.iter().enumerate() {
        let Vue3AstKind::Element(element) = &child.kind else {
            continue;
        };
        let Some(slot) = directive_by_name(element, "slot") else {
            continue;
        };
        if let Some(if_dir) = directive_by_name(element, "if") {
            dynamic_entries.push(render_conditional_dynamic_slot(
                ast, child.id, element, slot, if_dir, options, scope, index,
            ));
        } else if let Some(for_dir) = directive_by_name(element, "for") {
            dynamic_entries.push(render_for_dynamic_slot(
                ast, child.id, element, slot, for_dir, options, scope,
            ));
        } else {
            dynamic_entries.push(render_dynamic_slot_object(
                ast, child.id, element, slot, options, scope, None,
            ));
        }
    }
    format!(
        "_createSlots({{ _: 2 /* DYNAMIC */ }}, {})",
        render_array(&dynamic_entries)
    )
}

fn render_static_slot_property(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    _element: &Vue3Element,
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let name = slot
        .arg
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "default".into());
    let params = slot
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|params| !params.trim().is_empty())
        .map(|params| format!("({})", params.trim()))
        .unwrap_or_else(|| "()".into());
    let slot_scope = slot_function_scope(scope, &params);
    let children = ast
        .node(node_id)
        .map(|node| render_slot_children(ast, &node.children, options, &slot_scope))
        .unwrap_or_else(|| "[]".into());
    render_slot_property(&name, &params, children)
}

fn render_slot_property(name: &str, params: &str, children: String) -> String {
    format!("{}: _withCtx({params} => {children})", json_key(name))
}

fn render_conditional_dynamic_slot(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    slot: &Vue3Directive,
    if_dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    index: usize,
) -> String {
    let condition = if_dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let condition = render_condition(
        &rewrite_expression_with_scope(&condition, options, scope),
        options,
    );
    let slot = render_dynamic_slot_object(ast, node_id, element, slot, options, scope, Some(index));
    format!(
        "{condition}\n  ? {}\n  : undefined",
        indent_after_first_line(&slot, 4)
    )
}

fn render_for_dynamic_slot(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    slot: &Vue3Directive,
    for_dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let expression = for_dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let Some((source, aliases)) = parse_v_for_expression(&expression) else {
        return render_dynamic_slot_object(ast, node_id, element, slot, options, scope, None);
    };
    let source = rewrite_expression_with_scope(&source, options, scope);
    let scoped = scope.with_locals(aliases.clone());
    let params = aliases.join(", ");
    let body = render_dynamic_slot_object(ast, node_id, element, slot, options, &scoped, None);
    format!(
        "_renderList({source}, ({params}) => {{\n  return {}\n}})",
        indent_after_first_line(&body, 2)
    )
}

fn render_dynamic_slot_object(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    _element: &Vue3Element,
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    key: Option<usize>,
) -> String {
    let name = slot_name_expression(slot, options, scope);
    let params = slot
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|params| !params.trim().is_empty())
        .map(|params| format!("({})", params.trim()))
        .unwrap_or_else(|| "()".into());
    let slot_scope = slot_function_scope(scope, &params);
    let children = ast
        .node(node_id)
        .map(|node| render_slot_children(ast, &node.children, options, &slot_scope))
        .unwrap_or_else(|| "[]".into());
    let mut properties = vec![
        format!("name: {name}"),
        format!("fn: _withCtx({params} => {children})"),
    ];
    if let Some(key) = key {
        properties.push(format!("key: {}", quote_string(&key.to_string())));
    }
    render_object(&properties)
}

fn slot_name_expression(
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let Some(arg) = slot.arg.as_ref() else {
        return quote_string("default");
    };
    let name = arg.source_string();
    if slot.is_dynamic_arg {
        rewrite_expression_with_scope(&name, options, scope)
    } else {
        quote_string(&name)
    }
}

fn render_slot_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let rendered = render_child_sequence(ast, children, options, NodeRenderMode::Child, scope);
    render_array(&rendered)
}

fn slot_function_scope(scope: &RenderScope, params: &str) -> RenderScope {
    scope.with_locals(extract_slot_params(params))
}

fn extract_slot_params(params: &str) -> Vec<String> {
    let params = params
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    let mut output = Vec::new();
    let mut ident = String::new();
    for ch in params.chars() {
        if is_identifier_continue(ch) {
            ident.push(ch);
        } else if !ident.is_empty() {
            output.push(std::mem::take(&mut ident));
        }
    }
    if !ident.is_empty() {
        output.push(ident);
    }
    output
}

fn component_patch_flag(ast: &Vue3Ast, node_id: vuec_ast::NodeId) -> String {
    let Some(node) = ast.node(node_id) else {
        return String::new();
    };
    let visible = visible_children(ast, &node.children);
    if visible.iter().any(|child| {
        matches!(
            &child.kind,
            Vue3AstKind::Element(element)
                if directive_by_name(element, "slot").is_some()
                    && (directive_by_name(element, "if").is_some()
                        || directive_by_name(element, "for").is_some()
                        || directive_by_name(element, "else").is_some()
                        || directive_by_name(element, "else-if").is_some())
        )
    }) {
        ", 1024 /* DYNAMIC_SLOTS */".into()
    } else {
        String::new()
    }
}

fn visible_children<'a>(
    ast: &'a Vue3Ast,
    children: &[vuec_ast::NodeId],
) -> Vec<&'a vuec_ast::Node<Vue3NodeKind>> {
    children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| match &child.kind {
            Vue3AstKind::Comment(_) => false,
            Vue3AstKind::Text(text) => !text.value.trim().is_empty(),
            _ => true,
        })
        .collect()
}

fn collect_component_tags(ast: &Vue3Ast) -> Vec<String> {
    let mut tags = Vec::new();
    for node in &ast.nodes {
        if let Vue3AstKind::Element(element) = &node.kind {
            if element.tag_type == Vue3ElementType::Component
                && !tags.iter().any(|tag| tag == &element.tag)
            {
                tags.push(element.tag.clone());
            }
        }
    }
    tags
}

fn component_asset_id(tag: &str) -> String {
    format!("_component_{}", to_valid_asset_part(tag))
}

fn to_valid_asset_part(value: &str) -> String {
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch);
        } else if ch == '-' {
            output.push('_');
        } else {
            output.push_str(&(ch as u32).to_string());
        }
        if index == 0 && ch.is_ascii_digit() {
            output.insert(0, '_');
        }
    }
    output
}

fn render_slot_outlet(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let name = element
        .props
        .iter()
        .find_map(|prop| match prop {
            Vue3Prop::Attribute(attr) if attr.name == "name" => attr.value.as_deref(),
            _ => None,
        })
        .map(quote_string)
        .or_else(|| {
            directive_by_name(element, "bind").and_then(|dir| {
                let arg = dir.arg.as_ref()?.source_string();
                (arg == "name").then(|| {
                    rewrite_expression_with_scope(
                        &dir.exp
                            .as_ref()
                            .map(Vue3Expression::source_string)
                            .unwrap_or_default(),
                        options,
                        scope,
                    )
                })
            })
        })
        .unwrap_or_else(|| quote_string("default"));
    let slots = if options.prefix_identifiers || options.mode == "module" {
        "_ctx.$slots"
    } else {
        "$slots"
    };
    format!("_renderSlot({}, {})", slots, name)
}

fn render_element_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    parent_mode: NodeRenderMode,
    scope: &RenderScope,
) -> String {
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| !matches!(child.kind, Vue3AstKind::Comment(_)))
        .collect::<Vec<_>>();
    if options.hoist_static
        && parent_mode == NodeRenderMode::Root
        && should_cache_children(&child_nodes)
    {
        let rendered = child_nodes
            .iter()
            .map(|child| {
                render_node_expr_scoped(ast, child.id, options, NodeRenderMode::Cached, scope)
            })
            .collect::<Vec<_>>();
        if !rendered.is_empty() {
            return format!(
                "[...(_cache[0] || (_cache[0] = [{}]))]",
                rendered.join(", ")
            );
        }
    }
    if child_nodes.iter().all(|child| {
        matches!(
            child.kind,
            Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
        )
    }) {
        return render_text_sequence_expr(ast, children, options, scope);
    }
    let rendered = render_child_sequence(ast, children, options, NodeRenderMode::Child, scope);
    if rendered.is_empty() {
        String::new()
    } else if rendered.len() == 1
        && child_nodes.first().is_some_and(|child| is_text_like(child))
        && parent_mode != NodeRenderMode::Root
    {
        rendered.into_iter().next().unwrap()
    } else {
        render_array(&rendered)
    }
}

fn render_child_sequence(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
) -> Vec<String> {
    let mut rendered = Vec::new();
    let mut index = 0usize;
    while index < children.len() {
        let child_id = children[index];
        let Some(child) = ast.node(child_id) else {
            index += 1;
            continue;
        };
        if matches!(child.kind, Vue3AstKind::Comment(_)) {
            index += 1;
            continue;
        }
        if is_text_like(child) {
            let start = index;
            index += 1;
            while index < children.len()
                && ast
                    .node(children[index])
                    .is_some_and(|candidate| is_text_like(candidate))
            {
                index += 1;
            }
            rendered.push(render_text_vnode(
                ast,
                &children[start..index],
                options,
                scope,
            ));
            continue;
        }
        if let Vue3AstKind::Element(element) = &child.kind {
            if directive_by_name(element, "if").is_some() {
                let mut branch_ids = vec![child_id];
                index += 1;
                while index < children.len() {
                    let Some(candidate) = ast.node(children[index]) else {
                        index += 1;
                        continue;
                    };
                    if matches!(candidate.kind, Vue3AstKind::Comment(_)) {
                        index += 1;
                        continue;
                    }
                    if let Vue3AstKind::Element(candidate_element) = &candidate.kind {
                        if is_else_branch(candidate_element) {
                            branch_ids.push(children[index]);
                            index += 1;
                            continue;
                        }
                    }
                    break;
                }
                rendered.push(render_if_chain(ast, &branch_ids, options, mode, scope));
                continue;
            }
            if is_else_branch(element) {
                index += 1;
                continue;
            }
        }
        rendered.push(render_node_expr_scoped(ast, child_id, options, mode, scope));
        index += 1;
    }
    rendered
}

fn is_text_like(node: &vuec_ast::Node<Vue3NodeKind>) -> bool {
    matches!(
        node.kind,
        Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
    )
}

fn render_text_sequence_expr(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter_map(|child| match &child.kind {
            Vue3AstKind::Text(text) => Some(quote_text(&text.value)),
            Vue3AstKind::Interpolation(interpolation) => Some(format!(
                "_toDisplayString({})",
                rewrite_expression_with_scope(
                    &interpolation.expression.source_string(),
                    options,
                    scope
                )
            )),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn render_text_vnode(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let expression = render_text_sequence_expr(ast, children, options, scope);
    let has_interpolation = children.iter().any(|child_id| {
        ast.node(*child_id)
            .is_some_and(|child| matches!(child.kind, Vue3AstKind::Interpolation(_)))
    });
    if has_interpolation {
        format!("_createTextVNode({}, 1 /* TEXT */)", expression)
    } else {
        format!("_createTextVNode({})", expression)
    }
}

fn render_if_chain(
    ast: &Vue3Ast,
    branch_ids: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
) -> String {
    fn render_branch(
        ast: &Vue3Ast,
        branch_ids: &[vuec_ast::NodeId],
        index: usize,
        options: &Vue3CompilerOptions,
        mode: NodeRenderMode,
        scope: &RenderScope,
    ) -> String {
        let Some(branch_id) = branch_ids.get(index).copied() else {
            return "_createCommentVNode(\"v-if\", true)".into();
        };
        let Some(node) = ast.node(branch_id) else {
            return "_createCommentVNode(\"v-if\", true)".into();
        };
        let Some(element) = (match &node.kind {
            Vue3AstKind::Element(element) => Some(element),
            _ => None,
        }) else {
            return render_node_expr_scoped(ast, branch_id, options, mode, scope);
        };
        let branch_expr =
            render_if_branch_expr(ast, branch_id, element, options, mode, scope, index);
        let condition = if index == 0 {
            directive_by_name(element, "if")
        } else {
            directive_by_name(element, "else-if")
        };
        if let Some(condition) = condition.and_then(|dir| dir.exp.as_ref()) {
            let condition = render_condition(
                &rewrite_expression_with_scope(&condition.source_string(), options, scope),
                options,
            );
            let alternate = render_branch(ast, branch_ids, index + 1, options, mode, scope);
            format!(
                "{condition}\n  ? {}\n  : {}",
                indent_after_first_line(&branch_expr, 4),
                indent_after_first_line(&alternate, 4)
            )
        } else {
            branch_expr
        }
    }
    render_branch(ast, branch_ids, 0, options, mode, scope)
}

fn render_if_branch_expr(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    _mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: usize,
) -> String {
    if element.tag == "template" {
        let children = ast
            .node(node_id)
            .map(|node| render_fragment_children(ast, &node.children, options, scope))
            .unwrap_or_else(|| "[]".into());
        return format!(
            "(_openBlock(), _createElementBlock(_Fragment, {{ key: {branch_key} }}, {children}, 64 /* STABLE_FRAGMENT */))"
        );
    }
    render_plain_element(
        ast,
        node_id,
        element,
        options,
        NodeRenderMode::Root,
        scope,
        Some(branch_key),
    )
}

fn render_fragment_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let rendered = render_child_sequence(ast, children, options, NodeRenderMode::Child, scope);
    render_array(&rendered)
}

fn render_for_node(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    directive: &Vue3Directive,
    options: &Vue3CompilerOptions,
    _mode: NodeRenderMode,
    scope: &RenderScope,
) -> String {
    let Some(expression) = directive.exp.as_ref().map(Vue3Expression::source_string) else {
        return render_plain_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::Root,
            scope,
            None,
        );
    };
    let parsed = parse_v_for_expression(&expression);
    let Some((source, aliases)) = parsed else {
        return render_plain_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::Root,
            scope,
            None,
        );
    };
    let source = rewrite_expression_with_scope(&source, options, scope);
    let scoped = scope.with_locals(aliases.clone());
    let params = aliases.join(", ");
    let body = render_plain_element(
        ast,
        node_id,
        element,
        options,
        NodeRenderMode::Root,
        &scoped,
        None,
    );
    let body = indent_after_first_line(&body, 2);
    format!(
        "(_openBlock(true), _createElementBlock(_Fragment, null, _renderList({source}, ({params}) => {{\n  return {body}\n}}), 256 /* UNKEYED_FRAGMENT */))"
    )
}

fn render_array(items: &[String]) -> String {
    if items.is_empty() {
        "[]".into()
    } else {
        format!(
            "[\n{}\n]",
            items
                .iter()
                .map(|item| indent_lines(item, 2))
                .collect::<Vec<_>>()
                .join(",\n")
        )
    }
}

fn indent_lines(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    value
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn indent_after_first_line(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    let mut lines = value.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut output = first.to_string();
    for line in lines {
        output.push('\n');
        output.push_str(&prefix);
        output.push_str(line);
    }
    output
}

fn render_condition(condition: &str, options: &Vue3CompilerOptions) -> String {
    if uses_prefixed_identifiers(options) {
        format!("({condition})")
    } else {
        condition.to_string()
    }
}

fn should_cache_children(children: &[&vuec_ast::Node<Vue3NodeKind>]) -> bool {
    !children.is_empty()
        && children
            .iter()
            .all(|child| is_static_element_for_cache(child))
}

fn is_static_element_for_cache(node: &vuec_ast::Node<Vue3NodeKind>) -> bool {
    matches!(
        &node.kind,
        Vue3AstKind::Element(element) if element.tag != "slot"
            && element.props.iter().all(|prop| {
                matches!(prop, Vue3Prop::Attribute(_))
            })
    )
}

fn has_dynamic_text_child(ast: &Vue3Ast, children: &[vuec_ast::NodeId]) -> bool {
    children.iter().any(|child_id| {
        ast.node(*child_id)
            .is_some_and(|child| matches!(child.kind, Vue3AstKind::Interpolation(_)))
    })
}

fn child_sequence_needs_text_vnode(ast: &Vue3Ast, children: &[vuec_ast::NodeId]) -> bool {
    let visible = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| !matches!(child.kind, Vue3AstKind::Comment(_)))
        .collect::<Vec<_>>();
    visible.len() > 1 && !visible.iter().all(|child| is_text_like(child))
}

fn children_literal_const_only(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
) -> bool {
    let mut has_interpolation = false;
    for child_id in children {
        let Some(child) = ast.node(*child_id) else {
            continue;
        };
        match &child.kind {
            Vue3AstKind::Interpolation(interpolation) => {
                has_interpolation = true;
                let expression = interpolation.expression.source_string();
                if options
                    .binding_metadata
                    .get(expression.trim())
                    .map(String::as_str)
                    != Some("literal-const")
                {
                    return false;
                }
            }
            Vue3AstKind::Text(text) if text.value.trim().is_empty() => {}
            Vue3AstKind::Comment(_) => {}
            _ => return false,
        }
    }
    has_interpolation
}

fn render_patch_flag(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
) -> String {
    let children = ast
        .node(node_id)
        .map(|node| node.children.as_slice())
        .unwrap_or(&[]);
    if mode == NodeRenderMode::Cached {
        ", -1 /* CACHED */".into()
    } else if has_class_binding(element) {
        ", 2 /* CLASS */".into()
    } else if has_dynamic_props(element) {
        ", 8 /* PROPS */".into()
    } else if element.tag != "template"
        && !children_literal_const_only(ast, children, options)
        && has_dynamic_text_child(ast, children)
    {
        ", 1 /* TEXT */".into()
    } else {
        String::new()
    }
}

fn render_props(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    branch_key: Option<usize>,
) -> String {
    let dynamic_event = has_dynamic_props(element);
    let mut props = Vec::new();
    if let Some(key) = branch_key {
        props.push(format!("key: {key}"));
    }
    props.extend(element.props.iter().filter_map(|prop| match prop {
        Vue3Prop::Attribute(attr) => match &attr.value {
            Some(value) => Some(format!("{}: {}", json_key(&attr.name), quote_string(value))),
            None => Some(format!("{}: true", json_key(&attr.name))),
        },
        Vue3Prop::Directive(dir) if dir.name == "on" => {
            let event = dir
                .arg
                .as_ref()
                .map(Vue3Expression::source_string)
                .unwrap_or_default();
            let value = dir
                .exp
                .as_ref()
                .map(Vue3Expression::source_string)
                .unwrap_or_default();
            Some(format!(
                "{}: {}",
                json_key(&format!("on{}", capitalize(&event))),
                rewrite_handler_expression_with_scope(&value, options, scope)
            ))
        }
        Vue3Prop::Directive(dir) if dir.name == "bind" => {
            let arg = dir
                .arg
                .as_ref()
                .map(Vue3Expression::source_string)
                .unwrap_or_default();
            let value = dir
                .exp
                .as_ref()
                .map(Vue3Expression::source_string)
                .unwrap_or_default();
            if arg.is_empty() || value.is_empty() {
                return None;
            }
            let expression = rewrite_expression_with_scope(&value, options, scope);
            if arg == "class" {
                Some(format!("class: _normalizeClass({expression})"))
            } else if dir.is_dynamic_arg {
                Some(format!(
                    "[{}]: {}",
                    rewrite_expression_with_scope(&arg, options, scope),
                    expression
                ))
            } else {
                Some(format!("{}: {}", json_key(&arg), expression))
            }
        }
        _ => None,
    }));
    if props.is_empty() {
        String::new()
    } else if dynamic_event {
        format!("{{\n  {}\n}}", props.join(",\n  "))
    } else if props.len() > 1 || has_class_binding(element) {
        render_object(&props)
    } else {
        format!("{{ {} }}", props.join(", "))
    }
}

fn render_object(properties: &[String]) -> String {
    if properties.is_empty() {
        "{}".into()
    } else {
        format!(
            "{{\n{}\n}}",
            properties
                .iter()
                .map(|property| indent_lines(property, 2))
                .collect::<Vec<_>>()
                .join(",\n")
        )
    }
}

fn has_class_binding(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "bind"
                    && dir
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.source_string() == "class")
        )
    })
}

fn has_dynamic_props(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "on"
                    || (dir.name == "bind"
                        && dir
                            .arg
                            .as_ref()
                            .is_none_or(|arg| arg.source_string() != "class"))
        )
    })
}

fn dynamic_props_arg(element: &Vue3Element) -> String {
    let props = element
        .props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "on" => {
                let event = dir
                    .arg
                    .as_ref()
                    .map(Vue3Expression::source_string)
                    .unwrap_or_default();
                Some(format!("on{}", capitalize(&event)))
            }
            Vue3Prop::Directive(dir) if dir.name == "bind" && !has_class_bind_dir(dir) => {
                dir.arg.as_ref().map(Vue3Expression::source_string)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if props.is_empty() {
        String::new()
    } else {
        format!(
            ", [{}]",
            props
                .iter()
                .map(|prop| quote_string(prop))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn has_class_bind_dir(dir: &Vue3Directive) -> bool {
    dir.arg
        .as_ref()
        .is_some_and(|arg| arg.source_string() == "class")
}

fn directive_by_name<'a>(element: &'a Vue3Element, name: &str) -> Option<&'a Vue3Directive> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir) if dir.name == name => Some(dir),
        _ => None,
    })
}

fn is_else_branch(element: &Vue3Element) -> bool {
    directive_by_name(element, "else").is_some() || directive_by_name(element, "else-if").is_some()
}

fn parse_v_for_expression(expression: &str) -> Option<(String, Vec<String>)> {
    let expression = expression.trim();
    let (raw_aliases, source) = expression
        .split_once(" in ")
        .or_else(|| expression.split_once(" of "))?;
    let aliases = raw_aliases
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .filter_map(|alias| {
            let alias = alias.trim();
            (!alias.is_empty()).then(|| alias.to_string())
        })
        .collect::<Vec<_>>();
    if aliases.is_empty() {
        None
    } else {
        Some((source.trim().to_string(), aliases))
    }
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!(
        "{}{}",
        first.to_ascii_uppercase(),
        chars.collect::<String>()
    )
}

fn rewrite_handler_expression_with_scope(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    normalize_handler_indent(&rewrite_expression_with_scope(expression, options, scope))
}

fn rewrite_expression_with_scope(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let expression = expression.trim();
    if !uses_prefixed_identifiers(options) {
        return expression.to_string();
    }
    if scope.locals.is_empty() {
        rewrite_js_like_expression(expression, options)
    } else {
        rewrite_js_like_expression_with_locals(expression, options, &scope.locals)
    }
}

fn uses_prefixed_identifiers(options: &Vue3CompilerOptions) -> bool {
    options.prefix_identifiers || options.mode == "module"
}

fn normalize_handler_indent(expression: &str) -> String {
    let mut lines = expression.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut normalized = String::from(first);
    for line in lines {
        normalized.push('\n');
        normalized.push_str(line.strip_prefix("  ").unwrap_or(line));
    }
    normalized
}

fn rewrite_js_like_expression(expression: &str, options: &Vue3CompilerOptions) -> String {
    let mut output = String::new();
    rewrite_js_like_expression_into(expression, options, Vec::new(), &mut output);
    output
}

fn rewrite_js_like_expression_with_locals(
    expression: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
) -> String {
    let mut output = String::new();
    rewrite_js_like_expression_into(expression, options, locals.to_vec(), &mut output);
    output
}

fn rewrite_js_like_expression_into(
    expression: &str,
    options: &Vue3CompilerOptions,
    root_locals: Vec<String>,
    output: &mut String,
) {
    let mut scopes = vec![Scope {
        locals: root_locals,
    }];
    let mut previous = TokenKind::Other;
    let mut pending_decl: Option<DeclKind> = None;
    let mut pending_function_params = false;
    let mut last_keyword: Option<String> = None;
    let mut paren_depth = 0usize;
    let mut for_pending = false;
    let mut for_header_depth: Option<usize> = None;
    let mut pending_for_block_locals = Vec::<String>::new();
    let mut catch_pending = false;
    let mut catch_param_depth: Option<usize> = None;
    let mut pending_catch_locals = Vec::<String>::new();
    let chars = expression.char_indices().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        let byte = chars[index].0;
        let ch = chars[index].1;
        if ch == '\'' || ch == '"' || ch == '`' {
            let quote = ch;
            output.push(ch);
            index += 1;
            while index < chars.len() {
                let current = chars[index].1;
                output.push(current);
                index += 1;
                if current == '\\' && index < chars.len() {
                    output.push(chars[index].1);
                    index += 1;
                    continue;
                }
                if current == quote {
                    break;
                }
            }
            previous = TokenKind::Other;
            continue;
        }
        if is_identifier_start(ch) {
            let start = byte;
            index += 1;
            while index < chars.len() && is_identifier_continue(chars[index].1) {
                index += 1;
            }
            let end = chars
                .get(index)
                .map_or(expression.len(), |(offset, _)| *offset);
            let ident = &expression[start..end];
            let next = next_non_ws(expression, end);
            let prev = previous;
            if is_keyword(ident) {
                output.push_str(ident);
                match ident {
                    "var" => pending_decl = Some(DeclKind::Var),
                    "let" | "const" => pending_decl = Some(DeclKind::Block),
                    "function" => pending_function_params = true,
                    "for" => for_pending = true,
                    "in" | "of" => pending_decl = None,
                    "catch" => catch_pending = true,
                    _ => {}
                }
                last_keyword = Some(ident.to_string());
                previous = TokenKind::Keyword;
                continue;
            }
            if catch_param_depth.is_some() {
                if next != Some(':') {
                    pending_catch_locals.push(ident.to_string());
                }
                output.push_str(ident);
                previous = TokenKind::Identifier;
                continue;
            }
            if pending_decl.is_some()
                && matches!(
                    prev,
                    TokenKind::Keyword | TokenKind::Comma | TokenKind::OpenParen
                )
            {
                if pending_decl == Some(DeclKind::Var) {
                    if let Some(scope) = scopes.first_mut() {
                        scope.locals.push(ident.to_string());
                    }
                } else if for_header_depth.is_some() {
                    pending_for_block_locals.push(ident.to_string());
                } else if let Some(scope) = scopes.last_mut() {
                    scope.locals.push(ident.to_string());
                }
                output.push_str(ident);
                previous = TokenKind::Identifier;
                continue;
            }
            let skip_property = matches!(prev, TokenKind::Dot)
                || (next == Some(':') && last_keyword.as_deref() != Some("case"))
                || (pending_function_params
                    && matches!(prev, TokenKind::OpenParen | TokenKind::Comma));
            if skip_property
                || is_global_or_literal(ident)
                || is_local(&scopes, ident)
                || pending_for_block_locals.iter().any(|local| local == ident)
            {
                output.push_str(ident);
            } else {
                output.push_str(&rewrite_identifier(ident, options));
            }
            previous = TokenKind::Identifier;
            continue;
        }
        output.push(ch);
        match ch {
            '{' => {
                if !pending_for_block_locals.is_empty() {
                    scopes.push(Scope {
                        locals: std::mem::take(&mut pending_for_block_locals),
                    });
                } else if !pending_catch_locals.is_empty() {
                    scopes.push(Scope {
                        locals: std::mem::take(&mut pending_catch_locals),
                    });
                } else {
                    scopes.push(Scope::default());
                }
                previous = TokenKind::Other;
            }
            '}' => {
                if scopes.len() > 1 {
                    scopes.pop();
                }
                previous = TokenKind::Other;
            }
            '(' => {
                paren_depth += 1;
                if for_pending {
                    for_header_depth = Some(paren_depth);
                    for_pending = false;
                }
                if catch_pending {
                    catch_param_depth = Some(paren_depth);
                    catch_pending = false;
                }
                previous = TokenKind::OpenParen;
            }
            ')' => {
                if catch_param_depth == Some(paren_depth) {
                    catch_param_depth = None;
                }
                if for_header_depth == Some(paren_depth) {
                    for_header_depth = None;
                }
                paren_depth = paren_depth.saturating_sub(1);
                pending_function_params = false;
                previous = TokenKind::Other;
            }
            ',' => previous = TokenKind::Comma,
            '.' => previous = TokenKind::Dot,
            ';' => {
                pending_decl = None;
                previous = TokenKind::Other;
            }
            _ if ch.is_whitespace() => {}
            _ => {
                if ch != ':' {
                    last_keyword = None;
                }
                previous = TokenKind::Other;
            }
        }
        index += 1;
    }
}

#[derive(Clone, Debug, Default)]
struct Scope {
    locals: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeclKind {
    Var,
    Block,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenKind {
    Identifier,
    Keyword,
    OpenParen,
    Comma,
    Dot,
    Other,
}

fn is_local(scopes: &[Scope], ident: &str) -> bool {
    scopes
        .iter()
        .rev()
        .any(|scope| scope.locals.iter().any(|local| local == ident))
}

fn next_non_ws(source: &str, offset: usize) -> Option<char> {
    source.get(offset..)?.chars().find(|ch| !ch.is_whitespace())
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

fn is_keyword(value: &str) -> bool {
    matches!(
        value,
        "const"
            | "let"
            | "var"
            | "function"
            | "return"
            | "if"
            | "else"
            | "for"
            | "in"
            | "of"
            | "try"
            | "catch"
            | "throw"
            | "new"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "async"
            | "await"
    )
}

fn is_global_or_literal(value: &str) -> bool {
    matches!(
        value,
        "true"
            | "false"
            | "null"
            | "undefined"
            | "this"
            | "Infinity"
            | "NaN"
            | "Math"
            | "Number"
            | "Date"
            | "Array"
            | "Object"
            | "Boolean"
            | "String"
            | "RegExp"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "JSON"
            | "Intl"
            | "BigInt"
            | "console"
            | "Error"
            | "TypeError"
            | "Symbol"
            | "Promise"
            | "Reflect"
            | "globalThis"
    )
}

fn rewrite_identifier(ident: &str, options: &Vue3CompilerOptions) -> String {
    match options.binding_metadata.get(ident).map(String::as_str) {
        Some("setup-ref") if options.inline => format!("{ident}.value"),
        Some("setup-maybe-ref") if options.inline => format!("_unref({ident})"),
        Some("setup-const" | "literal-const" | "setup-reactive-const") if options.inline => {
            ident.to_string()
        }
        Some("props") if options.inline => format!("__props.{ident}"),
        Some("props-aliased") if options.inline => {
            let source = options
                .props_aliases
                .get(ident)
                .map_or(ident, String::as_str);
            format!("__props[{}]", quote_string(source))
        }
        Some("props-aliased") => {
            let source = options
                .props_aliases
                .get(ident)
                .map_or(ident, String::as_str);
            format!("$props[{}]", quote_string(source))
        }
        Some("data" | "options") if options.inline => format!("_ctx.{ident}"),
        Some(kind) if kind.starts_with("setup") || kind == "literal-const" => {
            format!("$setup.{ident}")
        }
        Some(kind) => format!("${kind}.{ident}"),
        None => format!("_ctx.{ident}"),
    }
}

fn expression_diagnostics(ast: &Vue3Ast, options: &Vue3CompilerOptions) -> Vec<String> {
    let store = JsAstStore::new();
    let source_type = expression_source_type(options);
    ast.nodes
        .iter()
        .filter_map(|node| match &node.kind {
            Vue3AstKind::Interpolation(interpolation) => {
                Some(interpolation.expression.source_string())
            }
            _ => None,
        })
        .filter_map(|expression| {
            store
                .parse_expression(expression.trim(), source_type)
                .err()
                .map(|err| err.message().to_string())
        })
        .collect()
}

fn expression_source_type(options: &Vue3CompilerOptions) -> oxc_span::SourceType {
    if options.is_ts
        || options
            .expression_plugins
            .iter()
            .any(|plugin| plugin == "typescript")
    {
        oxc_span::SourceType::ts()
    } else {
        oxc_span::SourceType::mjs()
    }
}

fn quote_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn json_key(key: &str) -> String {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
    {
        key.to_string()
    } else {
        quote_string(key)
    }
}

fn quote_text(value: &str) -> String {
    quote_string(value)
}

fn push_text_and_interpolations(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    file_id: FileId,
    token_start: usize,
    text: &str,
    options: &Vue3CompilerOptions,
) {
    let (open_delimiter, close_delimiter) = options
        .delimiters
        .as_ref()
        .map_or(("{{", "}}"), |items| (items[0].as_str(), items[1].as_str()));
    if open_delimiter.is_empty() || close_delimiter.is_empty() {
        push_text(ast, parent, file_id, token_start, text);
        return;
    }
    let mut cursor = 0usize;
    while let Some(open) = text[cursor..].find(open_delimiter) {
        let open = cursor + open;
        let expression_start = open + open_delimiter.len();
        let Some(close_offset) = text[expression_start..].find(close_delimiter) else {
            push_text(ast, parent, file_id, token_start + cursor, &text[cursor..]);
            return;
        };
        if open > cursor {
            push_text(
                ast,
                parent,
                file_id,
                token_start + cursor,
                &text[cursor..open],
            );
        }
        let close = expression_start + close_offset;
        let expression = text[expression_start..close].trim().to_string();
        let _id = ast.push_child(
            parent,
            Vue3NodeKind::interpolation(expression),
            Some(Span::new(
                file_id,
                token_start + open,
                token_start + close + close_delimiter.len(),
            )),
        );
        cursor = close + close_delimiter.len();
    }
    if cursor < text.len() {
        push_text(ast, parent, file_id, token_start + cursor, &text[cursor..]);
    }
}

fn push_text(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    file_id: FileId,
    start: usize,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let decoded = decode_html_text_entities(text);
    let previous = ast
        .node(parent)
        .and_then(|node| node.children.last().copied());
    if let Some(previous) = previous {
        if let Some(node) = ast.node_mut(previous) {
            if let Vue3AstKind::Text(existing) = &mut node.kind {
                existing.value.push_str(&decoded);
                if let Some(span) = node.span.source_mut() {
                    span.end = vuec_source::BytePos(start + text.len());
                }
                return;
            }
        }
    }
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::text(decoded),
        Some(Span::new(file_id, start, start + text.len())),
    );
}

fn decode_html_text_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    text.replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
}

pub fn base_compile(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
    Vue3Dialect::base_compile(source, options)
}

pub fn compile_dom(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
    Vue3Dialect::compile_dom(source, options)
}

pub fn compile_ssr(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
    Vue3Dialect::compile_ssr(source, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_transform_generate_roundtrip() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div>hello</div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(source, Vue3CompilerOptions::default());
        assert!(result.code.contains("render"));
        assert!(result.ast_summary.contains("nodes="));
    }

    #[test]
    fn ssr_wraps_code() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = compile_ssr(source, Vue3CompilerOptions::default());
        assert!(result.code.starts_with("/* ssr */"));
    }

    #[test]
    fn template_base_offset_maps_nodes_to_original_file_spans() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div>{{ msg }}</div>".into(),
            file_id: FileId(7),
            base_offset: 42,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        assert_eq!(root.span.source(), Some(Span::new(FileId(7), 42, 62)));
        let element = ast.node(root.children[0]).expect("element");
        assert_eq!(element.span.source(), Some(Span::new(FileId(7), 42, 62)));
        let interpolation = ast.node(element.children[0]).expect("interpolation child");
        assert_eq!(
            interpolation.span.source(),
            Some(Span::new(FileId(7), 47, 56))
        );
    }

    #[test]
    fn base_compile_uses_binding_metadata_for_prefixed_interpolations() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "function".into(),
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("props".into(), "props".into());
        options
            .binding_metadata
            .insert("setup".into(), "setup-maybe-ref".into());
        options
            .binding_metadata
            .insert("literal".into(), "literal-const".into());
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div>{{ props }} {{ setup }} {{ literal }}</div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(source, options);
        assert!(result.code.contains("$props.props"));
        assert!(result.code.contains("$setup.setup"));
        assert!(result.code.contains("$setup.literal"));
    }

    #[test]
    fn base_compile_rewrites_event_handler_statement_scopes() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div @click=\"() => {\n        for (const x in list) {\n          log(x)\n        }\n        error(x)\n      }\"/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "function".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("for (const x in _ctx.list)"));
        assert!(result.code.contains("_ctx.log(x)"));
        assert!(result.code.contains("_ctx.error(_ctx.x)"));
    }

    #[test]
    fn base_compile_generates_core_integration_directives() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div id="foo" :class="bar.baz">
  {{ world.burn() }}
  <div v-if="ok">yes</div>
  <template v-else>no</template>
  <div v-for="(value, index) in list"><span>{{ value + index }}</span></div>
</div>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                source_map: true,
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("class: _normalizeClass(bar.baz)"));
        assert!(result.code.contains("ok\n        ? (_openBlock()"));
        assert!(result.code.contains("_renderList(list, (value, index) =>"));
        assert!(result
            .code
            .contains("_toDisplayString(value + index), 1 /* TEXT */"));
        let map = result.map.expect("source map");
        assert_eq!(map.sources, vec!["foo.vue"]);
        assert_eq!(
            map.sources_content,
            Some(vec![Some(
                r#"<div id="foo" :class="bar.baz">
  {{ world.burn() }}
  <div v-if="ok">yes</div>
  <template v-else>no</template>
  <div v-for="(value, index) in list"><span>{{ value + index }}</span></div>
</div>"#
                    .into()
            )])
        );
    }

    #[test]
    fn base_compile_keeps_v_for_aliases_local_when_prefixed() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-for="(value, index) in list">{{ value + index }}</div>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "function".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result
            .code
            .contains("_renderList(_ctx.list, (value, index) =>"));
        assert!(result.code.contains("_toDisplayString(value + index)"));
        assert!(!result.code.contains("_ctx.value + _ctx.index"));
    }

    #[test]
    fn base_compile_wraps_component_default_slot_with_ctx() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<Child><div/></Child>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                scope_id: Some("test".into()),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result
            .code
            .contains("const _component_Child = _resolveComponent(\"Child\")"));
        assert!(result.code.contains("default: _withCtx(() => ["));
        assert!(result.code.contains("_createElementVNode(\"div\")"));
    }

    #[test]
    fn base_compile_wraps_named_component_slots_with_ctx() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Child>
        <template #foo="{ msg }">{{ msg }}</template>
        <template #bar><div/></template>
      </Child>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                scope_id: Some("test".into()),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("foo: _withCtx(({ msg }) => ["));
        assert!(result
            .code
            .contains("_createTextVNode(_toDisplayString(msg), 1 /* TEXT */)"));
        assert!(result.code.contains("bar: _withCtx(() => ["));
        assert!(result.code.contains("_: 1 /* STABLE */"));
    }

    #[test]
    fn base_compile_wraps_dynamic_component_slots_with_create_slots() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Child>
        <template #foo v-if="ok"><div/></template>
        <template v-for="i in list" #[i]><div/></template>
      </Child>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                scope_id: Some("test".into()),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result
            .code
            .contains("_createSlots({ _: 2 /* DYNAMIC */ }, ["));
        assert!(result.code.contains("name: \"foo\""));
        assert!(result.code.contains("fn: _withCtx(() => ["));
        assert!(result.code.contains("name: i"));
        assert!(result.code.contains(", 1024 /* DYNAMIC_SLOTS */"));
    }

    #[test]
    fn root_codegen_projection_uses_child_for_slot_outlet() {
        let root = json!({
            "children": [{
                "type": 1,
                "tagType": 2,
                "codegenNode": { "type": 14 }
            }]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "child", "index": 0 })
        );
    }

    #[test]
    fn root_codegen_projection_uses_single_element_codegen_as_block() {
        let root = json!({
            "children": [{
                "type": 1,
                "tagType": 0,
                "codegenNode": { "type": 13 }
            }]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "childCodegen", "index": 0, "asBlock": true })
        );
    }

    #[test]
    fn root_codegen_projection_preserves_non_element_child() {
        let root = json!({ "children": [{ "type": 11, "codegenNode": { "type": 13 } }] });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "child", "index": 0 })
        );
    }

    #[test]
    fn root_codegen_projection_marks_single_visible_root_fragment() {
        let root = json!({
            "children": [
                { "type": 3 },
                { "type": 1, "tagType": 0 },
                { "type": 3 }
            ]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "fragment", "patchFlag": 2112 })
        );
    }

    #[test]
    fn transform_model_projection_emits_model_value_and_update_props() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "model",
                    "loc": { "source": "model" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));

        assert_eq!(
            projection["props"][0]["key"],
            json!({ "kind": "static", "content": "modelValue" })
        );
        assert_eq!(projection["props"][0]["dynamic"], json!(true));
        assert_eq!(
            projection["props"][1]["key"],
            json!({ "kind": "static", "content": "onUpdate:modelValue" })
        );
        assert_eq!(
            projection["props"][1]["value"]["children"][0],
            json!("$event => ((")
        );
    }

    #[test]
    fn transform_model_projection_handles_dynamic_argument() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "_ctx.model",
                    "loc": { "source": "model" }
                },
                "arg": {
                    "type": 4,
                    "content": "_ctx.value",
                    "isStatic": false
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": { "prefixIdentifiers": true }
        }));

        assert_eq!(
            projection["props"][0]["key"],
            json!({ "kind": "node", "path": "dir.arg" })
        );
        assert_eq!(
            projection["props"][1]["key"],
            json!({
                "kind": "compound",
                "children": ["\"onUpdate:\" + ", { "kind": "node", "path": "dir.arg" }]
            })
        );
    }

    #[test]
    fn transform_model_projection_reports_invalid_expression_errors() {
        let no_expression = transform_model_projection(&json!({
            "dir": { "modifiers": [] },
            "node": { "tagType": 0 },
            "context": {}
        }));
        assert_eq!(no_expression["errors"], json!([41]));

        let malformed = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "a + b",
                    "loc": { "source": "a + b" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));
        assert_eq!(malformed["errors"], json!([42]));
    }

    #[test]
    fn transform_model_projection_tracks_cache_and_scope_refs() {
        let cached = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "_ctx.foo",
                    "loc": { "source": "foo" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {}
            }
        }));
        assert_eq!(cached["props"][1]["cache"], json!(true));
        assert_eq!(cached["props"][1]["dynamic"], json!(false));

        let scoped = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 8,
                    "loc": { "source": "foo[i]" },
                    "children": [
                        { "type": 4, "content": "_ctx.foo" },
                        "[",
                        { "type": 4, "content": "i" },
                        "]"
                    ]
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": { "i": 1 }
            }
        }));
        assert_eq!(scoped["props"][1]["cache"], json!(false));
        assert_eq!(scoped["props"][1]["dynamic"], json!(true));
    }

    #[test]
    fn transform_model_projection_generates_component_modifiers() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "foo",
                    "loc": { "source": "foo" }
                },
                "arg": {
                    "type": 4,
                    "content": "bar",
                    "isStatic": true
                },
                "modifiers": [
                    { "content": "trim" },
                    { "content": "bar-baz" }
                ]
            },
            "node": { "tagType": 1 },
            "context": {}
        }));

        assert_eq!(
            projection["props"][2]["key"],
            json!({ "kind": "static", "content": "barModifiers" })
        );
        assert_eq!(
            projection["props"][2]["value"]["content"],
            json!("{ trim: true, \"bar-baz\": true }")
        );
    }

    #[test]
    fn transform_model_projection_marks_static_argument_hydration_event() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "model",
                    "loc": { "source": "model" }
                },
                "arg": {
                    "type": 4,
                    "content": "foo-value",
                    "isStatic": true
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));

        assert_eq!(projection["props"][1]["hydrate"], json!(true));
    }

    #[test]
    fn base_parse_decodes_builtin_text_entities() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "&gt;&lt;&amp;&apos;&quot;&foo;".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let text = ast.node(root.children[0]).expect("text");
        assert!(matches!(
            &text.kind,
            Vue3AstKind::Text(value) if value.value == "><&'\"&foo;"
        ));
        assert_eq!(text.span.source(), Some(Span::new(FileId(0), 0, 30)));
    }

    #[test]
    fn base_parse_preserves_raw_content_inside_v_pre() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-pre :id="foo"><Comp/>{{ bar }}</div><div :id="foo"><Comp/>{{ bar }}</div>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let with_pre = ast.node(root.children[0]).expect("v-pre div");
        let Vue3AstKind::Element(with_pre_element) = &with_pre.kind else {
            panic!("expected element");
        };
        assert_eq!(with_pre_element.props.len(), 1);
        assert!(matches!(
            &with_pre_element.props[0],
            Vue3Prop::Attribute(attr) if attr.name == ":id" && attr.value.as_deref() == Some("foo")
        ));
        let raw_component = ast.node(with_pre.children[0]).expect("raw component");
        assert!(matches!(
            &raw_component.kind,
            Vue3AstKind::Element(element)
                if element.tag == "Comp" && element.tag_type == Vue3ElementType::Element
        ));
        let raw_text = ast.node(with_pre.children[1]).expect("raw interpolation");
        assert!(matches!(
            &raw_text.kind,
            Vue3AstKind::Text(text) if text.value == "{{ bar }}"
        ));

        let without_pre = ast.node(root.children[1]).expect("normal div");
        let Vue3AstKind::Element(without_pre_element) = &without_pre.kind else {
            panic!("expected element");
        };
        assert!(matches!(
            &without_pre_element.props[0],
            Vue3Prop::Directive(dir) if dir.name == "bind"
        ));
        let component = ast.node(without_pre.children[0]).expect("component");
        assert!(matches!(
            &component.kind,
            Vue3AstKind::Element(element)
                if element.tag == "Comp" && element.tag_type == Vue3ElementType::Component
        ));
        let interpolation = ast.node(without_pre.children[1]).expect("interpolation");
        assert!(matches!(interpolation.kind, Vue3AstKind::Interpolation(_)));
    }

    #[test]
    fn base_parse_splits_half_open_interpolations_inside_v_pre() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div v-pre><span>{{ number </span><span>}}</span></div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let div = ast.node(root.children[0]).expect("div");
        let first_span = ast.node(div.children[0]).expect("first span");
        let second_span = ast.node(div.children[1]).expect("second span");

        assert!(matches!(
            &ast.node(first_span.children[0]).expect("first text").kind,
            Vue3AstKind::Text(text) if text.value == "{{ number "
        ));
        assert!(matches!(
            &ast.node(second_span.children[0]).expect("second text").kind,
            Vue3AstKind::Text(text) if text.value == "}}"
        ));
    }

    #[test]
    fn base_parse_preserves_inter_element_whitespace_in_preserve_mode() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div/> \n <div/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                whitespace: "preserve".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        assert_eq!(root.children.len(), 3);
        assert!(matches!(
            &ast.node(root.children[1]).expect("whitespace text").kind,
            Vue3AstKind::Text(text) if text.value == " "
        ));
    }

    #[test]
    fn base_parse_preserves_text_inside_configured_pre_tag() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<pre>\n  foo  bar  </pre><span>\n  foo   bar</span>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                pre_tags: vec!["pre".into()],
                ignore_newline_tags: vec!["pre".into()],
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let pre = ast.node(root.children[0]).expect("pre");
        let span = ast.node(root.children[1]).expect("span");
        assert!(matches!(
            &ast.node(pre.children[0]).expect("pre text").kind,
            Vue3AstKind::Text(text) if text.value == "  foo  bar  "
        ));
        assert!(matches!(
            &ast.node(span.children[0]).expect("span text").kind,
            Vue3AstKind::Text(text) if text.value == " foo bar"
        ));
    }

    #[test]
    fn base_parse_extends_open_element_spans_to_eof() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        let div = ast.node(template.children[0]).expect("div");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 15)));
        assert_eq!(div.span.source(), Some(Span::new(FileId(0), 10, 15)));
    }

    #[test]
    fn base_parse_recovers_from_incomplete_child_start_tag_at_eof() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><div id=abc /".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 23)));
        assert_eq!(template.children.len(), 1);
        assert!(matches!(
            &ast.node(template.children[0]).expect("recovered text").kind,
            Vue3AstKind::Text(text) if text.value == "/"
        ));
    }

    #[test]
    fn base_parse_treats_empty_incomplete_end_tag_as_text() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template></".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 12)));
        assert!(matches!(
            &ast.node(template.children[0]).expect("end tag text").kind,
            Vue3AstKind::Text(text) if text.value == "</"
        ));
    }

    #[test]
    fn base_parse_uses_configured_namespace_for_cdata_text() {
        let mut namespaces = BTreeMap::new();
        namespaces.insert("svg".into(), vuec_ast::HtmlNamespace::Svg);
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><svg><![CDATA[cdata]]></svg></template>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                namespaces,
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        let svg = ast.node(template.children[0]).expect("svg");
        assert!(matches!(
            &svg.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Svg
        ));
        assert!(matches!(
            &ast.node(svg.children[0]).expect("cdata text").kind,
            Vue3AstKind::Text(text) if text.value == "cdata"
        ));
        assert_eq!(
            ast.node(svg.children[0]).expect("cdata text").span.source(),
            Some(Span::new(FileId(0), 24, 29))
        );
    }

    #[test]
    fn base_parse_drops_cdata_children_in_html_namespace() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><![CDATA[cdata]]></template>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert!(template.children.is_empty());
    }

    #[test]
    fn base_parse_keeps_non_matching_end_tag_as_text_in_textarea() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<textarea></div></textarea>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let textarea = ast.node(root.children[0]).expect("textarea");
        assert_eq!(textarea.span.source(), Some(Span::new(FileId(0), 0, 27)));
        assert!(matches!(
            &ast.node(textarea.children[0]).expect("raw end tag text").kind,
            Vue3AstKind::Text(text) if text.value == "</div>"
        ));
        assert_eq!(
            ast.node(textarea.children[0])
                .expect("raw end tag text")
                .span
                .source(),
            Some(Span::new(FileId(0), 10, 16))
        );
    }

    #[test]
    fn base_parse_extends_open_span_across_invalid_end_tags() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template></div></template>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 27)));
        assert!(template.children.is_empty());
    }
}
