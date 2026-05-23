#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vuec_ast::{
    MissingSpanReason, NodeSpan, QuoteKind, RuntimeHelper, TemplateAttribute, Vue3Ast, Vue3AstKind,
    Vue3Directive, Vue3Element, Vue3ElementType, Vue3Expression, Vue3NodeKind, Vue3Prop,
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
            mode: "module".into(),
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
        let tokenizer = if let Some([open, close]) = &options.delimiters {
            HtmlTokenizer::new(&source.source).with_interpolation_delimiters(open, close)
        } else {
            HtmlTokenizer::new(&source.source)
        };
        let tokens = tokenizer.tokenize();
        for token in tokens {
            let current_parent = *stack.last().unwrap_or(&root);
            match token.kind {
                HtmlTokenKind::Text(text) => push_text_and_interpolations(
                    &mut ast,
                    current_parent,
                    source.file_id,
                    source.base_offset + token.start,
                    &text,
                    options,
                ),
                HtmlTokenKind::Comment(value) => {
                    if !options.comments {
                        continue;
                    }
                    let _id = ast.push_child(
                        current_parent,
                        Vue3NodeKind::comment(value),
                        Some(Span::new(
                            source.file_id,
                            source.base_offset + token.start,
                            source.base_offset + token.end,
                        )),
                    );
                }
                HtmlTokenKind::StartTag {
                    name,
                    attributes,
                    self_closing,
                } => {
                    let is_void = options.void_tags.iter().any(|candidate| candidate == &name);
                    let id = ast.push_child(
                        current_parent,
                        vue3_element_kind(
                            name,
                            attributes,
                            self_closing,
                            options,
                            source.file_id,
                            source.base_offset,
                        ),
                        Some(Span::new(
                            source.file_id,
                            source.base_offset + token.start,
                            source.base_offset + token.end,
                        )),
                    );
                    if !self_closing && !is_void {
                        stack.push(id);
                    }
                }
                HtmlTokenKind::EndTag { name } => {
                    while stack.len() > 1 {
                        let Some(node_id) = stack.pop() else {
                            break;
                        };
                        if let Some(node) = ast.node(node_id) {
                            if matches!(&node.kind, Vue3AstKind::Element(element) if element.tag.eq_ignore_ascii_case(&name))
                            {
                                if let Some(node) = ast.node_mut(node_id) {
                                    if let Some(span) = node.span.source_mut() {
                                        span.end =
                                            vuec_source::BytePos(source.base_offset + token.end);
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
                HtmlTokenKind::Cdata(text) => {
                    push_text_and_interpolations(
                        &mut ast,
                        current_parent,
                        source.file_id,
                        source.base_offset + token.start,
                        &text,
                        options,
                    );
                }
                HtmlTokenKind::Doctype(_) | HtmlTokenKind::Eof => {}
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
        let mut walk = vec![(root_id, true)];
        while let Some((node_id, is_root)) = walk.pop() {
            if let Some(node) = ast.node(node_id) {
                for child_id in node.children.clone() {
                    if let Some(child) = ast.node(child_id) {
                        match &child.kind {
                            Vue3AstKind::Element(element) => {
                                has_element = true;
                                if element.tag == "slot" {
                                    ctx.add_helper(RuntimeHelper::Vue3RenderSlot);
                                }
                                if !is_root {
                                    has_nested_element = true;
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
        let helper_order = [
            RuntimeHelper::Vue3ToDisplayString,
            RuntimeHelper::Vue3CreateElementVNode,
            RuntimeHelper::Vue3RenderSlot,
            RuntimeHelper::Vue3OpenBlock,
            RuntimeHelper::Vue3CreateElementBlock,
        ];
        let root_id = ast.root;
        if let Some(root) = ast.node(root_id) {
            let helpers = render_helpers(&helper_order, ctx);
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
            let expr = if root.children.len() == 1 {
                render_node_expr(ast, root.children[0], options, NodeRenderMode::Root)
            } else {
                render_children_array(ast, &root.children, options, true)
            };
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
            result.map = source_map_for_render(&result.code, &ast, &source);
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

fn vue3_element_kind(
    tag: String,
    attributes: Vec<vuec_html::HtmlAttribute>,
    self_closing: bool,
    options: &Vue3CompilerOptions,
    file_id: FileId,
    base_offset: usize,
) -> Vue3NodeKind {
    let props = attributes
        .into_iter()
        .map(|attr| vue3_prop_from_attr(attr, file_id, base_offset))
        .collect::<Vec<_>>();
    let tag_type = vue3_tag_type(&tag, &props, options);
    Vue3NodeKind::Element(Vue3Element {
        tag,
        tag_type,
        ns: vuec_ast::HtmlNamespace::Html,
        props,
        self_closing,
        codegen_node: None,
        ssr_codegen_node: None,
    })
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
    if let Some(parsed) = parse_vue3_directive(&attr.name, name_span) {
        let (directive_name, arg, modifiers, is_dynamic_arg, arg_span, modifier_spans) = parsed;
        Vue3Prop::Directive(Vue3Directive {
            name: directive_name,
            raw_name: attr.name,
            arg: arg.map(Vue3Expression::Raw),
            exp: attr.value.map(Vue3Expression::Raw),
            modifiers,
            is_dynamic_arg,
            span,
            arg_span,
            exp_span: value_content_span.or(value_span),
            modifier_spans,
        })
    } else {
        Vue3Prop::Attribute(vuec_ast::Vue3Attribute {
            name: attr.name,
            value: attr.value,
            span,
            name_span,
            value_span,
            quote,
        })
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
    let (arg, is_dynamic) = if raw_arg.starts_with('[') && raw_arg.ends_with(']') {
        (
            Some(raw_arg[1..raw_arg.len().saturating_sub(1)].to_string()),
            true,
        )
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
                raw_arg.len()
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

fn render_helpers(order: &[RuntimeHelper], ctx: &TransformContext) -> Vec<RuntimeHelper> {
    order
        .iter()
        .copied()
        .filter(|helper| ctx.helpers.contains(helper))
        .collect()
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
        RuntimeHelper::Vue3ToDisplayString => "toDisplayString",
        RuntimeHelper::Vue3RenderList => "renderList",
        RuntimeHelper::Vue3RenderSlot => "renderSlot",
    }
}

fn source_map_for_render(
    code: &str,
    ast: &Vue3Ast,
    source: &TemplateSource,
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
        &mut names,
        &mut segments,
    );
    if segments.is_empty() {
        return None;
    }
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
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    match &node.kind {
        Vue3AstKind::Element(_) => {
            add_vnode_mapping(code, node, base_offset, source, segments, cursor);
            for child_id in &node.children {
                collect_node_source_map(
                    code,
                    ast,
                    *child_id,
                    base_offset,
                    source,
                    names,
                    segments,
                    cursor,
                );
            }
        }
        Vue3AstKind::Interpolation(_) => {
            add_interpolation_mapping(code, node, base_offset, source, names, segments, cursor);
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
    let name = expression.trim().to_string();
    let name_index = if let Some(index) = names.iter().position(|existing| existing == &name) {
        index
    } else {
        names.push(name.clone());
        names.len() - 1
    };
    let local_start = span.start.0.saturating_sub(base_offset);
    let local_end = span.end.0.saturating_sub(base_offset);
    let Some(original_start) = source[local_start..local_end]
        .find(expression.trim())
        .map(|offset| local_start + offset)
    else {
        return;
    };
    let Some(start) = loc_for_offset(source, original_start) else {
        return;
    };
    let Some(end) = loc_for_offset(source, original_start + expression.trim().len()) else {
        return;
    };
    let needle = format!("_ctx.{name}");
    if let Some(offset) = find_code_offset(code, &needle, *cursor) {
        if let Some((line, column)) = loc_for_offset(code, offset) {
            segments.push(SourceMapSegment {
                generated_line: line,
                generated_column: column,
                original_line: start.0,
                original_column: start.1,
                name_index: Some(name_index),
            });
            segments.push(SourceMapSegment {
                generated_line: line,
                generated_column: column + needle.encode_utf16().count() as u32,
                original_line: end.0,
                original_column: end.1,
                name_index: None,
            });
            *cursor = offset + needle.len();
        }
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
    let rendered = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .map(|child| {
            render_node_expr(
                ast,
                child.id,
                options,
                if is_root {
                    NodeRenderMode::Root
                } else {
                    NodeRenderMode::Child
                },
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", rendered.join(", "))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeRenderMode {
    Root,
    Child,
    Cached,
}

fn render_node_expr(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
) -> String {
    let Some(node) = ast.node(node_id) else {
        return "null".into();
    };
    match &node.kind {
        Vue3AstKind::Root(_) => render_children_array(ast, &node.children, options, true),
        Vue3AstKind::Text(text) => quote_text(&text.value),
        Vue3AstKind::Interpolation(interpolation) => {
            format!(
                "_toDisplayString({})",
                rewrite_expression(&interpolation.expression.source_string(), options)
            )
        }
        Vue3AstKind::Comment(comment) => format!("/*{}*/", comment.value),
        Vue3AstKind::Element(element) => {
            let tag = &element.tag;
            let attributes = element.template_attributes();
            if tag == "slot" {
                return render_slot_outlet(&attributes, options);
            }
            let helper = if mode == NodeRenderMode::Root {
                "_createElementBlock"
            } else {
                "_createElementVNode"
            };
            let props = render_props(&attributes, options);
            let children = render_element_children(ast, &node.children, options, mode);
            let has_dynamic_props = has_dynamic_props(&attributes);
            let skip_text_patch = children_literal_const_only(ast, &node.children, options);
            let patch_flag = if mode == NodeRenderMode::Cached {
                ", -1 /* CACHED */"
            } else if has_dynamic_props {
                ", 8 /* PROPS */"
            } else if tag != "template"
                && !skip_text_patch
                && has_dynamic_children(ast, &node.children)
            {
                ", 1 /* TEXT */"
            } else {
                ""
            };
            let attrs = if props.is_empty() {
                "null".into()
            } else {
                props
            };
            let children_arg = if children.is_empty() {
                if patch_flag.is_empty() {
                    String::new()
                } else {
                    ", null".into()
                }
            } else if mode == NodeRenderMode::Root && tag == "template" && children.starts_with('[')
            {
                format!(", {children}")
            } else {
                format!(", {children}")
            };
            if mode == NodeRenderMode::Root {
                format!(
                    "(_openBlock(), {}({}, {}{}{}{}))",
                    helper,
                    quote_string(tag),
                    attrs,
                    children_arg,
                    patch_flag,
                    dynamic_props_arg(&attributes)
                )
            } else {
                format!(
                    "{}({}, {}{}{}{})",
                    helper,
                    quote_string(tag),
                    attrs,
                    children_arg,
                    patch_flag,
                    dynamic_props_arg(&attributes)
                )
            }
        }
        _ => "null".into(),
    }
}

fn render_slot_outlet(attributes: &[TemplateAttribute], options: &Vue3CompilerOptions) -> String {
    let name = attributes
        .iter()
        .find(|attr| attr.name == "name")
        .and_then(|attr| attr.value.as_deref())
        .unwrap_or("default");
    let slots = if options.prefix_identifiers || options.mode == "module" {
        "_ctx.$slots"
    } else {
        "$slots"
    };
    format!("_renderSlot({}, {})", slots, quote_string(name))
}

fn render_element_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    parent_mode: NodeRenderMode,
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
            .map(|child| render_node_expr(ast, child.id, options, NodeRenderMode::Cached))
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
        let rendered = child_nodes
            .iter()
            .map(|child| render_node_expr(ast, child.id, options, NodeRenderMode::Child))
            .collect::<Vec<_>>();
        return rendered.join(" + ");
    }
    let rendered = child_nodes
        .iter()
        .map(|child| render_node_expr(ast, child.id, options, NodeRenderMode::Child))
        .collect::<Vec<_>>();
    if rendered.is_empty() {
        String::new()
    } else if rendered.len() == 1
        && (parent_mode != NodeRenderMode::Root
            || matches!(
                child_nodes[0].kind,
                Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
            ))
    {
        rendered.into_iter().next().unwrap()
    } else {
        if rendered.len() == 1 {
            format!("[\n  {}\n]", rendered[0])
        } else {
            format!("[{}]", rendered.join(", "))
        }
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
            && element.template_attributes().iter().all(|attr| {
                !attr.name.starts_with("v-")
                    && !attr.name.starts_with('@')
                    && !attr.name.starts_with(':')
            })
    )
}

fn has_dynamic_children(ast: &Vue3Ast, children: &[vuec_ast::NodeId]) -> bool {
    children.iter().any(|child_id| {
        ast.node(*child_id).is_some_and(|child| {
            matches!(child.kind, Vue3AstKind::Interpolation(_))
                || matches!(&child.kind, Vue3AstKind::Element(_) if has_dynamic_children(ast, &child.children))
        })
    })
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

fn render_props(attributes: &[TemplateAttribute], options: &Vue3CompilerOptions) -> String {
    let dynamic_event = has_dynamic_props(attributes);
    let props = attributes
        .iter()
        .filter_map(|attr| match &attr.value {
            Some(value) if attr.name.starts_with('@') => {
                let event = attr.name.trim_start_matches('@');
                Some(format!(
                    "{}: {}",
                    json_key(&format!("on{}", capitalize(event))),
                    rewrite_handler_expression(value, options)
                ))
            }
            Some(value) if attr.name.starts_with("v-on:") => {
                let event = attr.name.trim_start_matches("v-on:");
                Some(format!(
                    "{}: {}",
                    json_key(&format!("on{}", capitalize(event))),
                    rewrite_handler_expression(value, options)
                ))
            }
            Some(value) if !attr.name.starts_with("v-") && !attr.name.starts_with(':') => {
                Some(format!("{}: {}", json_key(&attr.name), quote_string(value)))
            }
            None if !attr.name.starts_with("v-")
                && !attr.name.starts_with('@')
                && !attr.name.starts_with(':') =>
            {
                Some(format!("{}: true", json_key(&attr.name)))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if props.is_empty() {
        String::new()
    } else if dynamic_event {
        format!("{{\n  {}\n}}", props.join(",\n  "))
    } else {
        format!("{{ {} }}", props.join(", "))
    }
}

fn has_dynamic_props(attributes: &[TemplateAttribute]) -> bool {
    attributes.iter().any(|attr| {
        attr.value.is_some() && (attr.name.starts_with('@') || attr.name.starts_with("v-on:"))
    })
}

fn dynamic_props_arg(attributes: &[TemplateAttribute]) -> String {
    let props = attributes
        .iter()
        .filter_map(|attr| {
            if attr.value.is_some() && attr.name.starts_with('@') {
                Some(format!(
                    "on{}",
                    capitalize(attr.name.trim_start_matches('@'))
                ))
            } else if attr.value.is_some() && attr.name.starts_with("v-on:") {
                Some(format!(
                    "on{}",
                    capitalize(attr.name.trim_start_matches("v-on:"))
                ))
            } else {
                None
            }
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

fn rewrite_handler_expression(expression: &str, options: &Vue3CompilerOptions) -> String {
    normalize_handler_indent(&rewrite_expression(expression, options))
}

fn rewrite_expression(expression: &str, options: &Vue3CompilerOptions) -> String {
    let expression = expression.trim();
    if !options.prefix_identifiers {
        return expression.to_string();
    }
    rewrite_js_like_expression(expression, options)
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
    let mut scopes = vec![Scope::default()];
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
    output
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
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::text(text),
        Some(Span::new(file_id, start, start + text.len())),
    );
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
}
