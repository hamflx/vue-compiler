//! Vue 2 template compiler implementation.
//!
//! This crate owns the Rust-backed Vue 2 template parser, optimizer, render
//! code generator, public AST projection, SFC asset URL option support, and
//! official-style warning/code-frame result types used by the bridge layers.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vuec_ast::{
    GeneratedReason, Hir, HirBinding, HirConstness, HirDirectiveUse, HirElement, HirEvent, HirExpr,
    HirFor, HirFragment, HirIf, HirIfBranch, HirInterpolation, HirNodeKind, HirObjectBinding,
    HirObjectListeners, HirPropSegment, HirProps, HirRef, HirSlotOutlet, HirStaticAttr, HirTag,
    HtmlNamespace, JsExprId, JsPatternId, JsStmtId, LoweringMap, MirExpr, MissingSpanReason,
    NodeId, NodeSpan, Vue2Ast, Vue2AstKind, Vue2BindWrap, Vue2ComponentModelMir, Vue2CreateElement,
    Vue2DataObject, Vue2DataProp, Vue2DirectiveRuntime, Vue2ForMir, Vue2IfMir, Vue2IfMirBranch,
    Vue2InlineTemplate, Vue2Mir, Vue2MirKind, Vue2NodeKind, Vue2NormalizationType, Vue2Once,
    Vue2RenderStatic, Vue2ScopedSlot, Vue2SlotOutlet, Vue2TextCall, Vue2ValidationData,
};
use vuec_diagnostics::{Diagnostic, DiagnosticSink, Severity};
use vuec_html::{HtmlAttribute, HtmlTokenKind, HtmlTokenizer};
use vuec_js::{parse_vue2_filter_expression, rewrite_vue2_filter_expression, JsAstStore};
use vuec_source::{FileId, Span};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Options controlling Vue 2 template parsing, optimization, and codegen.
pub struct Vue2CompileOptions {
    /// Enabled Vue 2 compiler module names.
    pub modules: Vec<String>,
    /// Enabled custom directive transform names.
    pub directives: Vec<String>,
    /// Whether compiler warnings should be reported.
    pub warn: bool,
    /// Whether warnings and errors should include byte ranges.
    pub output_source_range: bool,
    /// Whether comments should be preserved in the public AST and codegen.
    pub comments: bool,
    /// Custom interpolation delimiters.
    pub delimiters: Option<[String; 2]>,
    /// Whitespace handling mode.
    pub whitespace: Option<String>,
    /// Whether text whitespace should be preserved.
    pub preserve_whitespace: bool,
    /// Whether newlines are decoded in normal attributes.
    pub should_decode_newlines: bool,
    /// Whether newlines are decoded in href-like attributes.
    pub should_decode_newlines_for_href: bool,
    /// Whether static optimization should run.
    pub optimize: bool,
    /// Whether built-in must-use-prop behavior is disabled.
    pub disable_default_must_use_prop: bool,
    /// Per-tag namespace overrides.
    pub tag_namespaces: BTreeMap<String, String>,
    /// Whether default Vue 2 tag namespace rules are enabled.
    pub use_default_tag_namespaces: bool,
    /// Optional reserved-tag allow-list.
    pub reserved_tags: Option<Vec<String>>,
    /// Whether default Vue 2 reserved-tag rules are enabled.
    pub use_default_reserved_tags: bool,
    /// Script binding metadata used by SFC/template integration.
    pub bindings: BTreeMap<String, String>,
    /// Whether binding metadata came from script setup.
    pub bindings_is_script_setup: bool,
    /// Optional SFC asset URL transform configuration.
    pub sfc_asset_url_transform: Option<Vue2SfcAssetUrlTransformOptions>,
}

impl Default for Vue2CompileOptions {
    fn default() -> Self {
        Self {
            modules: Vec::new(),
            directives: Vec::new(),
            warn: true,
            output_source_range: false,
            comments: false,
            delimiters: None,
            whitespace: None,
            preserve_whitespace: true,
            should_decode_newlines: false,
            should_decode_newlines_for_href: false,
            optimize: true,
            disable_default_must_use_prop: false,
            tag_namespaces: BTreeMap::new(),
            use_default_tag_namespaces: true,
            reserved_tags: None,
            use_default_reserved_tags: true,
            bindings: BTreeMap::new(),
            bindings_is_script_setup: true,
            sfc_asset_url_transform: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2.7 SFC template asset URL transform options.
pub struct Vue2SfcAssetUrlTransformOptions {
    /// Optional base path to prefix transformed relative URLs.
    pub base: Option<String>,
    /// Whether absolute URLs should also be transformed.
    pub include_absolute: bool,
    /// Tag-to-attribute map that identifies URL-bearing attributes.
    pub tags: BTreeMap<String, Vec<String>>,
}

impl Default for Vue2SfcAssetUrlTransformOptions {
    fn default() -> Self {
        Self {
            base: None,
            include_absolute: false,
            tags: vue2_sfc_default_asset_url_tags(),
        }
    }
}

/// Returns the default Vue 2.7 SFC asset URL tag and attribute map.
pub fn vue2_sfc_default_asset_url_tags() -> BTreeMap<String, Vec<String>> {
    [
        ("audio", vec!["src"]),
        ("video", vec!["src", "poster"]),
        ("source", vec!["src"]),
        ("img", vec!["src"]),
        ("image", vec!["xlink:href", "href"]),
        ("use", vec!["xlink:href", "href"]),
    ]
    .into_iter()
    .map(|(tag, attrs)| {
        (
            tag.to_string(),
            attrs.into_iter().map(str::to_string).collect(),
        )
    })
    .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 compiler warning or tip.
pub struct Vue2Warning {
    /// Warning message text.
    pub msg: String,
    /// Optional start byte offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    /// Optional end byte offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
    /// Whether this warning is a Vue 2 tip.
    pub tip: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 compiler error.
pub struct Vue2Error {
    /// Error message text.
    pub msg: String,
    /// Optional start byte offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    /// Optional end byte offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Full Vue 2 compile result.
pub struct Vue2CompiledResult {
    /// Canonical arena-backed public AST projection.
    pub ast: Vue2Ast,
    /// Compatibility element tree used by Vue 2 codegen projections.
    pub element_ast: Option<Vue2Element>,
    /// Generated Vue 2 render function body.
    pub render: String,
    /// Generated static render function bodies.
    pub static_render_fns: Vec<String>,
    /// Compile errors in official-style public shape.
    pub errors: Vec<Vue2Error>,
    /// Compile tips and warnings in official-style public shape.
    pub tips: Vec<Vue2Warning>,
    /// Rendered diagnostic messages.
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 `compileToFunctions`-style result.
pub struct Vue2FunctionResult {
    /// Generated Vue 2 render function body.
    pub render: String,
    /// Generated static render function bodies.
    pub static_render_fns: Vec<String>,
    /// Public warning and tip list.
    pub warnings: Vec<Vue2Warning>,
    /// Rendered error strings.
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 codegen-only result.
pub struct Vue2CodegenResult {
    /// Generated Vue 2 render function body.
    pub render: String,
    /// Generated static render function bodies.
    pub static_render_fns: Vec<String>,
}

/// Result of projecting the Vue 2 compatibility parser tree into canonical AST.
pub struct Vue2AstProjectionResult {
    /// Canonical arena-backed Vue 2 AST.
    pub ast: Vue2Ast,
    /// JavaScript side store referenced by AST expression, statement, and pattern ids.
    pub js: JsAstStore,
}

/// Result of the structural Vue 2 lowering contract.
///
/// This is separate from the current exact render emitter so the AST -> HIR ->
/// Vue2Mir boundary can be verified without changing public codegen parity.
pub struct Vue2LoweringResult {
    /// Lowered shared HIR document.
    pub hir: Hir,
    /// Lowered Vue 2 target MIR document.
    pub mir: Vue2Mir,
    /// AST-to-HIR and HIR-to-MIR edge map.
    pub map: LoweringMap,
    /// JavaScript side store used by AST/HIR/MIR ids.
    pub js: JsAstStore,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 parsed attribute.
pub struct Vue2Attribute {
    /// Attribute name.
    pub name: String,
    /// Attribute value.
    pub value: String,
    /// Source span for the attribute.
    pub span: Option<Span>,
    /// Whether the attribute name or value is dynamic.
    pub dynamic: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 parsed directive.
pub struct Vue2Directive {
    /// Normalized directive name.
    pub name: String,
    /// Raw source directive name.
    pub raw_name: String,
    /// Optional directive expression.
    pub value: Option<String>,
    /// Optional directive argument.
    pub arg: Option<String>,
    /// Whether the directive argument is dynamic.
    pub is_dynamic_arg: bool,
    /// Directive modifiers keyed by modifier name.
    pub modifiers: BTreeMap<String, bool>,
    /// Source span for the directive.
    pub span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 event handler metadata.
pub struct Vue2EventHandler {
    /// Handler expression.
    pub value: String,
    /// Event modifiers keyed by modifier name.
    pub modifiers: BTreeMap<String, bool>,
    /// Original modifier order.
    #[serde(default)]
    pub modifier_order: Vec<String>,
    /// Whether object-style modifier syntax was present.
    #[serde(default)]
    pub has_modifier_object: bool,
    /// Whether the event name is dynamic.
    pub dynamic: bool,
    /// Source span for the event directive.
    pub span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// One Vue 2 `v-if` / `v-else-if` / `v-else` branch.
pub struct Vue2IfCondition {
    /// Optional branch expression.
    pub exp: Option<String>,
    /// Branch root element.
    pub block: Box<Vue2Element>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Node in the Vue 2 compatibility element tree.
pub enum Vue2Node {
    /// Element child node.
    Element(Box<Vue2Element>),
    /// Text, interpolation, or comment child node.
    Text(Vue2Text),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 text, interpolation, or comment node.
pub struct Vue2Text {
    /// Raw or generated text content.
    pub text: String,
    /// Optional interpolation expression.
    pub expression: Option<String>,
    /// Whether this text node is a comment.
    pub is_comment: bool,
    /// Source span for this text node.
    pub span: Option<Span>,
    /// Static analysis marker.
    pub static_node: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 element node used by parsing, optimization, and codegen.
pub struct Vue2Element {
    /// Element tag name.
    pub tag: String,
    /// Processed attribute list.
    pub attrs_list: Vec<Vue2Attribute>,
    /// Raw attribute list before directive/module processing.
    #[serde(default)]
    pub raw_attrs_list: Vec<Vue2Attribute>,
    /// Processed attribute map.
    pub attrs_map: BTreeMap<String, String>,
    /// Raw attribute map preserving attribute metadata.
    pub raw_attrs_map: BTreeMap<String, Vue2Attribute>,
    /// Runtime attrs emitted to `data.attrs`.
    pub attrs: Vec<Vue2Attribute>,
    /// Runtime props emitted to `data.domProps` or component props.
    pub props: Vec<Vue2Attribute>,
    /// Dynamic attributes that affect runtime patching.
    pub dynamic_attrs: Vec<Vue2Attribute>,
    /// Custom and built-in directives attached to the element.
    pub directives: Vec<Vue2Directive>,
    /// Component or DOM event listeners.
    pub events: BTreeMap<String, Vec<Vue2EventHandler>>,
    /// Native event listeners for component nodes.
    pub native_events: BTreeMap<String, Vec<Vue2EventHandler>>,
    /// Child nodes.
    pub children: Vec<Vue2Node>,
    /// Source span for the element.
    pub span: Option<Span>,
    /// Optional namespace such as SVG or MathML.
    pub ns: Option<String>,
    /// Whether the element has no data bindings or children requiring data.
    pub plain: bool,
    /// Whether the element is forbidden in the current context.
    pub forbidden: bool,
    /// Whether `v-pre` applies to this element.
    pub pre: bool,
    /// Whether `v-once` applies to this element.
    pub once: bool,
    /// Whether this element has runtime bindings.
    pub has_bindings: bool,
    /// `v-if` expression.
    pub if_exp: Option<String>,
    /// Source span for the `v-if` directive.
    #[serde(default)]
    pub if_span: Option<Span>,
    /// `v-else-if` expression.
    pub elseif: Option<String>,
    /// Source span for the `v-else-if` directive.
    #[serde(default)]
    pub elseif_span: Option<Span>,
    /// Whether this branch is `v-else`.
    pub else_branch: bool,
    /// Source span for the `v-else` directive.
    #[serde(default)]
    pub else_span: Option<Span>,
    /// Ordered conditional branches.
    pub if_conditions: Vec<Vue2IfCondition>,
    /// `v-for` source expression.
    pub for_exp: Option<String>,
    /// Source span for the `v-for` directive.
    #[serde(default)]
    pub for_span: Option<Span>,
    /// Primary `v-for` alias.
    pub alias: Option<String>,
    /// First `v-for` iterator alias.
    pub iterator1: Option<String>,
    /// Second `v-for` iterator alias.
    pub iterator2: Option<String>,
    /// Key expression.
    pub key: Option<String>,
    /// Source span for the key binding.
    #[serde(default)]
    pub key_span: Option<Span>,
    /// Ref expression.
    pub ref_name: Option<String>,
    /// Whether the ref appears inside a `v-for`.
    pub ref_in_for: bool,
    /// Legacy slot name.
    pub slot_name: Option<String>,
    /// Slot target expression.
    pub slot_target: Option<String>,
    /// Whether the slot target is dynamic.
    pub slot_target_dynamic: bool,
    /// Scoped slot expression.
    pub slot_scope: Option<String>,
    /// Whether this uses the new `v-slot` syntax.
    #[serde(default)]
    pub slot_new_syntax: bool,
    /// Scoped slots keyed by slot name.
    pub scoped_slots: BTreeMap<String, Vue2Element>,
    /// Dynamic component expression.
    pub component: Option<String>,
    /// Whether this component uses `inline-template`.
    pub inline_template: bool,
    /// Static class expression.
    pub static_class: Option<String>,
    /// Dynamic class expression.
    pub class_binding: Option<String>,
    /// Static style expression.
    pub static_style: Option<String>,
    /// Dynamic style expression.
    pub style_binding: Option<String>,
    /// Component `v-model` metadata.
    pub model: Option<Vue2ComponentModel>,
    /// Data wrapper produced by custom module transforms.
    pub wrap_data: Option<Vue2DataWrap>,
    /// Listener wrapper expression.
    pub wrap_listeners: Option<String>,
    /// Validation metadata for legacy validation directives.
    pub validate: Option<Vue2Validation>,
    /// Validation rules attached to the element.
    pub validators: Vec<Vue2Validator>,
    /// Whether this element is static.
    pub static_node: bool,
    /// Whether this element is a static root.
    pub static_root: bool,
    /// Whether this static node appears inside `v-for`.
    pub static_in_for: bool,
    #[serde(default)]
    static_processed: bool,
    #[serde(default)]
    once_processed: bool,
    #[serde(default)]
    for_processed: bool,
    #[serde(default)]
    if_processed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 component `v-model` codegen metadata.
pub struct Vue2ComponentModel {
    /// Runtime model value expression.
    pub value: String,
    /// Runtime update callback expression.
    pub callback: String,
    /// Original model expression.
    pub expression: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 data wrapper emitted by module transforms.
pub enum Vue2DataWrap {
    /// Wraps generated data with `_b(...)` semantics.
    Bind {
        /// Bound object expression.
        value: String,
        /// Whether `.prop` handling applies.
        prop: bool,
        /// Whether `.sync` handling applies.
        sync: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 validation directive metadata.
pub struct Vue2Validation {
    /// Field expression being validated.
    pub field: String,
    /// Validation groups.
    pub groups: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 validation rule metadata.
pub struct Vue2Validator {
    /// Validator name.
    pub name: String,
    /// Validator rule expression.
    pub rule: String,
}

/// Stateful Vue 2 compiler facade.
pub struct Vue2Compiler {
    js: JsAstStore,
}

impl Vue2Compiler {
    /// Creates a new Vue 2 compiler facade.
    pub fn new() -> Self {
        Self {
            js: JsAstStore::new(),
        }
    }

    /// Parses, optimizes, and generates a Vue 2 template.
    pub fn compile(&self, template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
        let template = template.trim();
        let mut diagnostics = DiagnosticSink::default();
        let mut element_ast = parse_element_tree(&mut diagnostics, template, &options);
        collect_element_warnings(element_ast.as_ref(), &mut diagnostics);
        if options.optimize {
            if let Some(root) = element_ast.as_mut() {
                optimize(root, &options);
            }
        }
        let mut static_render_fns = Vec::new();
        validate_expressions(element_ast.as_ref(), &mut diagnostics);
        let projection = project_public_ast(template, element_ast.as_ref());
        let lowered = lower_vue2_ast_to_mir(&projection.ast, projection.js);
        let render =
            generate_render_mir(&lowered.mir, &lowered.js, &options, &mut static_render_fns);
        let ast = projection.ast;
        let diagnostics_messages = diagnostics
            .as_slice()
            .iter()
            .map(render_diagnostic_message)
            .collect();
        let (errors, tips) = split_compilation_issues(&diagnostics);
        Vue2CompiledResult {
            ast,
            element_ast,
            render,
            static_render_fns,
            errors,
            tips,
            diagnostics: diagnostics_messages,
        }
    }

    /// Compiles a template into official-style function result fields.
    pub fn compile_to_functions(
        &self,
        template: &str,
        options: Vue2CompileOptions,
    ) -> Vue2FunctionResult {
        let compiled = self.compile(template, options);
        Vue2FunctionResult {
            render: compiled.render,
            static_render_fns: compiled.static_render_fns,
            warnings: compiled.tips,
            errors: compiled.diagnostics,
        }
    }

    /// Compiles a template for the Vue 2 SSR render entry shape.
    pub fn compile_ssr(&self, template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
        let mut compiled = self.compile(template, options);
        compiled.render = format!(
            "function ssrRender(_ctx, _push, _parent, _attrs){{return {}}}",
            compiled.render
        );
        compiled
    }

    /// Generates render code from an existing Vue 2 element tree.
    pub fn generate(
        &self,
        element: Option<&Vue2Element>,
        options: &Vue2CompileOptions,
    ) -> Vue2CodegenResult {
        generate(element, options)
    }

    /// Projects the compatibility element tree into canonical Vue 2 AST.
    pub fn project_ast(
        &self,
        template: &str,
        element: Option<&Vue2Element>,
    ) -> Vue2AstProjectionResult {
        project_vue2_public_ast(template, element)
    }

    /// Lowers canonical Vue 2 AST to shared HIR and Vue 2 target MIR.
    pub fn lower_to_mir(ast: &Vue2Ast, js: JsAstStore) -> Vue2LoweringResult {
        lower_vue2_ast_to_mir(ast, js)
    }

    /// Returns the JavaScript side store used by this compiler.
    pub fn js(&self) -> &JsAstStore {
        &self.js
    }
}

impl Default for Vue2Compiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Parses, optimizes, and generates a Vue 2 template.
pub fn compile(template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
    Vue2Compiler::new().compile(template, options)
}

/// Compiles a template into official-style function result fields.
pub fn compile_to_functions(template: &str, options: Vue2CompileOptions) -> Vue2FunctionResult {
    Vue2Compiler::new().compile_to_functions(template, options)
}

/// Compiles a template for the Vue 2 SSR render entry shape.
pub fn compile_ssr(template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
    Vue2Compiler::new().compile_ssr(template, options)
}

/// Generates render code from an existing Vue 2 element tree.
pub fn generate(element: Option<&Vue2Element>, options: &Vue2CompileOptions) -> Vue2CodegenResult {
    let mut static_render_fns = Vec::new();
    let projected = project_public_ast("", element);
    let lowered = lower_vue2_ast_to_mir(&projected.ast, projected.js);
    let render = generate_render_mir(&lowered.mir, &lowered.js, options, &mut static_render_fns);
    Vue2CodegenResult {
        render,
        static_render_fns,
    }
}

/// Projects a Vue 2 compatibility parser tree into the canonical arena AST.
pub fn project_vue2_public_ast(
    template: &str,
    element_ast: Option<&Vue2Element>,
) -> Vue2AstProjectionResult {
    project_public_ast(template, element_ast)
}

/// Lowers canonical Vue 2 AST to shared HIR and Vue 2 target MIR.
pub fn lower_vue2_ast_to_mir(ast: &Vue2Ast, js: JsAstStore) -> Vue2LoweringResult {
    let root_span = ast
        .root_node()
        .map(|node| node.span.clone())
        .unwrap_or_else(|| NodeSpan::missing(MissingSpanReason::LoweringGap));
    let mut state = Vue2LoweringState {
        hir: Hir::new(HirNodeKind::Root(vuec_ast::HirRoot), root_span.clone()),
        mir: Vue2Mir::new(Vue2MirKind::Root(vuec_ast::Vue2MirRoot), root_span),
        map: LoweringMap::default(),
        js,
        static_render_index: 0,
        once_id: 0,
    };
    state.map.record_ast_to_hir(ast.root, state.hir.root);
    state.map.record_hir_to_mir(state.hir.root, state.mir.root);

    if let Some(root) = ast.root_node() {
        lower_vue2_child_sequence(
            &root.children,
            ast,
            state.hir.root,
            state.mir.root,
            &mut state,
        );
    }

    Vue2LoweringResult {
        hir: state.hir,
        mir: state.mir,
        map: state.map,
        js: state.js,
    }
}

/// Marks static nodes and static roots in a Vue 2 element tree.
pub fn optimize(root: &mut Vue2Element, options: &Vue2CompileOptions) {
    mark_static_element(root, options);
    mark_static_roots(root, false, options);
}

/// Generates a Vue 2 style source code frame for a byte range.
pub fn generate_code_frame(source: &str, start: usize, end: usize) -> String {
    let source_lines = source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect::<Vec<_>>();
    let mut count = 0usize;
    let mut rendered = Vec::new();
    for (index, line) in source_lines.iter().enumerate() {
        count += line.len() + 1;
        if count < start {
            continue;
        }

        let mut output_index = index.saturating_sub(2);
        while output_index <= index + 2 || end > count {
            let Some(output_line) = source_lines.get(output_index) else {
                output_index += 1;
                continue;
            };
            rendered.push(format!(
                "{}{}|  {}",
                output_index + 1,
                " ".repeat(3usize.saturating_sub((output_index + 1).to_string().len())),
                output_line
            ));
            let line_len = output_line.len();
            if output_index == index {
                let pad = start.saturating_sub(count - line.len() - 1);
                let width = if end > count {
                    line_len.saturating_sub(pad)
                } else {
                    end.saturating_sub(start)
                };
                rendered.push(format!("   |  {}{}", " ".repeat(pad), "^".repeat(width)));
            } else if output_index > index {
                if end > count {
                    let width = (end - count).min(line_len);
                    rendered.push(format!("   |  {}", "^".repeat(width)));
                }
                count += line_len + 1;
            }
            output_index += 1;
        }
        break;
    }
    rendered.join("\n")
}

fn parse_element_tree(
    diagnostics: &mut DiagnosticSink,
    template: &str,
    options: &Vue2CompileOptions,
) -> Option<Vue2Element> {
    let mut tokenizer = HtmlTokenizer::new(template);
    let mut stack: Vec<Vue2Element> = Vec::new();
    let mut root: Option<Vue2Element> = None;
    let mut in_v_pre = false;

    loop {
        let in_pre_tag = stack.iter().any(|element| element.tag == "pre");
        if let Some(parent) = stack.last_mut() {
            if is_text_tag(&parent.tag) {
                let tag = parent.tag.clone();
                let raw = consume_raw_text(template, &mut tokenizer, &tag);
                if !raw.text.is_empty() {
                    push_text_node(
                        parent,
                        &raw.text,
                        raw.start,
                        raw.text_end,
                        options,
                        in_v_pre,
                        in_pre_tag,
                    );
                }
                if raw.has_end_tag {
                    close_until_matching_end_tag(
                        &tag,
                        &mut stack,
                        &mut root,
                        diagnostics,
                        options,
                        &mut in_v_pre,
                    );
                }
                if raw.reached_eof {
                    break;
                }
                continue;
            }
        }

        let token = tokenizer.next_token();
        match token.kind {
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let mut element = create_element(name, attributes, token.start, token.end);
                if let Some(namespace) = namespace_for_tag(&element.tag, options) {
                    element.ns = Some(namespace);
                } else if let Some(parent) = stack.last() {
                    element.ns = parent.ns.clone();
                }
                if is_forbidden_tag(&element) {
                    element.forbidden = true;
                    diagnostics.push(vue2_warning(
                        "W_VUE2_FORBIDDEN_TAG",
                        format!(
                            "Templates should only be responsible for mapping the state to the UI. Avoid placing tags with side-effects in your templates, such as <{}>, as they will not be parsed.",
                            element.tag
                        ),
                        element.span,
                    ));
                }

                if !in_v_pre {
                    process_pre(&mut element);
                    in_v_pre = element.pre;
                }
                if in_v_pre {
                    process_raw_attrs(&mut element);
                } else {
                    process_structural_directives(&mut element, diagnostics);
                    process_element(&mut element, diagnostics, options);
                }
                if self_closing || is_unary_tag(&element.tag) {
                    close_element(
                        element,
                        &mut stack,
                        &mut root,
                        diagnostics,
                        options,
                        &mut in_v_pre,
                    );
                } else {
                    stack.push(element);
                }
            }
            HtmlTokenKind::EndTag { name } => {
                close_until_matching_end_tag(
                    &name,
                    &mut stack,
                    &mut root,
                    diagnostics,
                    options,
                    &mut in_v_pre,
                );
            }
            HtmlTokenKind::Text(text) | HtmlTokenKind::Cdata(text) => {
                if let Some(parent) = stack.last_mut() {
                    push_text_node(
                        parent,
                        &text,
                        token.start,
                        token.end,
                        options,
                        in_v_pre,
                        in_pre_tag,
                    );
                } else if !text.trim().is_empty() {
                    let message = if text == template {
                        "Component template requires a root element, rather than just text."
                            .to_string()
                    } else {
                        format!(
                            "text \"{}\" outside root element will be ignored.",
                            text.trim()
                        )
                    };
                    diagnostics.push(vue2_warning(
                        "W_VUE2_TEXT_OUTSIDE_ROOT",
                        message,
                        Some(Span::new(FileId(0), token.start, token.end)),
                    ));
                }
            }
            HtmlTokenKind::Comment(text) if options.comments => {
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(Vue2Node::Text(Vue2Text {
                        text,
                        expression: None,
                        is_comment: true,
                        span: Some(Span::new(FileId(0), token.start, token.end)),
                        static_node: true,
                    }));
                }
            }
            HtmlTokenKind::Comment(_)
            | HtmlTokenKind::BogusQuestionTag
            | HtmlTokenKind::Doctype(_) => {}
            HtmlTokenKind::Eof => break,
        }
    }

    while let Some(element) = stack.pop() {
        diagnostics.push(vue2_error(
            "E_VUE2_UNCLOSED_TAG",
            format!("tag <{}> has no matching end tag.", element.tag),
            element.span,
        ));
        close_element(
            element,
            &mut stack,
            &mut root,
            diagnostics,
            options,
            &mut in_v_pre,
        );
    }

    root
}

struct RawText {
    text: String,
    start: usize,
    text_end: usize,
    has_end_tag: bool,
    reached_eof: bool,
}

fn consume_raw_text(template: &str, tokenizer: &mut HtmlTokenizer<'_>, tag: &str) -> RawText {
    let start = tokenizer.cursor();
    let close_tag = format!("</{tag}");
    let lower = template[start..].to_ascii_lowercase();
    let Some(relative_end) = lower.find(&close_tag) else {
        tokenizer.set_cursor(template.len());
        return RawText {
            text: template[start..].to_string(),
            start,
            text_end: template.len(),
            has_end_tag: false,
            reached_eof: true,
        };
    };
    let text_end = start + relative_end;
    tokenizer.set_cursor(text_end);
    let end_tag = tokenizer.next_token();
    RawText {
        text: template[start..text_end].to_string(),
        start,
        text_end,
        has_end_tag: matches!(end_tag.kind, HtmlTokenKind::EndTag { ref name } if name.eq_ignore_ascii_case(tag)),
        reached_eof: false,
    }
}

fn close_until_matching_end_tag(
    name: &str,
    stack: &mut Vec<Vue2Element>,
    root: &mut Option<Vue2Element>,
    diagnostics: &mut DiagnosticSink,
    options: &Vue2CompileOptions,
    in_v_pre: &mut bool,
) {
    let Some(mut index) = stack.iter().rposition(|element| element.tag == name) else {
        return;
    };
    while stack.len() > index + 1 {
        let Some(element) = stack.pop() else {
            return;
        };
        diagnostics.push(vue2_error(
            "E_VUE2_UNCLOSED_TAG",
            format!("tag <{}> has no matching end tag.", element.tag),
            element.span,
        ));
        if element.pre {
            *in_v_pre = false;
        }
        close_element(element, stack, root, diagnostics, options, in_v_pre);
        index = index.min(stack.len());
    }
    let Some(element) = stack.pop() else {
        return;
    };
    if element.pre {
        *in_v_pre = false;
    }
    close_element(element, stack, root, diagnostics, options, in_v_pre);
}

fn create_element(
    tag: String,
    attributes: Vec<HtmlAttribute>,
    start: usize,
    end: usize,
) -> Vue2Element {
    let attrs_list = attributes
        .into_iter()
        .map(|attr| Vue2Attribute {
            name: attr.name,
            value: attr.value.unwrap_or_default(),
            span: Some(Span::new(FileId(0), attr.start, attr.end)),
            dynamic: false,
        })
        .collect::<Vec<_>>();
    let mut attrs_map = BTreeMap::new();
    let mut raw_attrs_map = BTreeMap::new();
    for attr in &attrs_list {
        attrs_map.insert(attr.name.clone(), attr.value.clone());
        raw_attrs_map.insert(attr.name.clone(), attr.clone());
    }
    Vue2Element {
        tag,
        raw_attrs_list: attrs_list.clone(),
        attrs_list,
        attrs_map,
        raw_attrs_map,
        attrs: Vec::new(),
        props: Vec::new(),
        dynamic_attrs: Vec::new(),
        directives: Vec::new(),
        events: BTreeMap::new(),
        native_events: BTreeMap::new(),
        children: Vec::new(),
        span: Some(Span::new(FileId(0), start, end)),
        ns: None,
        plain: false,
        forbidden: false,
        pre: false,
        once: false,
        has_bindings: false,
        if_exp: None,
        if_span: None,
        elseif: None,
        elseif_span: None,
        else_branch: false,
        else_span: None,
        if_conditions: Vec::new(),
        for_exp: None,
        for_span: None,
        alias: None,
        iterator1: None,
        iterator2: None,
        key: None,
        key_span: None,
        ref_name: None,
        ref_in_for: false,
        slot_name: None,
        slot_target: None,
        slot_target_dynamic: false,
        slot_scope: None,
        slot_new_syntax: false,
        scoped_slots: BTreeMap::new(),
        component: None,
        inline_template: false,
        static_class: None,
        class_binding: None,
        static_style: None,
        style_binding: None,
        model: None,
        wrap_data: None,
        wrap_listeners: None,
        validate: None,
        validators: Vec::new(),
        static_node: false,
        static_root: false,
        static_in_for: false,
        static_processed: false,
        once_processed: false,
        for_processed: false,
        if_processed: false,
    }
}

fn close_element(
    mut element: Vue2Element,
    stack: &mut [Vue2Element],
    root: &mut Option<Vue2Element>,
    diagnostics: &mut DiagnosticSink,
    options: &Vue2CompileOptions,
    in_v_pre: &mut bool,
) {
    if element.pre {
        *in_v_pre = false;
    }
    let in_pre_tag = element.tag == "pre" || stack.iter().any(|ancestor| ancestor.tag == "pre");
    if !in_pre_tag {
        trim_ending_whitespace(&mut element);
    }
    cleanup_scoped_slot_children(&mut element, in_pre_tag);
    element.plain = element_generates_empty_data(&element);

    let parent_in_pre_tag = stack.iter().any(|ancestor| ancestor.tag == "pre");
    if let Some(parent) = stack.last_mut() {
        if element.elseif.is_some() || element.else_branch {
            process_if_conditions(element, parent, diagnostics);
        } else {
            let mut element = element;
            if element.if_exp.is_some() {
                element.if_conditions = vec![Vue2IfCondition {
                    exp: element.if_exp.clone(),
                    block: Box::new(element.clone_without_conditions()),
                }];
            }
            if let Some(slot_scope) = element.slot_scope.clone() {
                let name = element
                    .slot_target
                    .clone()
                    .unwrap_or_else(|| "\"default\"".into());
                let mut scoped = element.clone();
                scoped.slot_scope = Some(slot_scope);
                parent.scoped_slots.insert(name, scoped);
            }
            parent.children.push(Vue2Node::Element(Box::new(element)));
        }
        if !parent_in_pre_tag {
            trim_ending_whitespace(parent);
        }
        return;
    }

    if let Some(existing) = root.as_mut() {
        if existing.if_exp.is_some() && (element.elseif.is_some() || element.else_branch) {
            if element.for_exp.is_some() {
                diagnostics.push(vue2_warning(
                    "W_VUE2_FOR_ROOT",
                    "Cannot use v-for on stateful component root element because it renders multiple elements.",
                    element.span,
                ));
            }
            existing.if_conditions.push(Vue2IfCondition {
                exp: element.elseif.clone(),
                block: Box::new(element),
            });
        } else if !is_ignorable_root_whitespace(&element) {
            diagnostics.push(vue2_warning(
                "W_VUE2_MULTIPLE_ROOTS",
                "Component template should contain exactly one root element. If you are using v-if on multiple elements, use v-else-if to chain them instead.",
                element.span,
            ));
        }
    } else {
        if matches!(element.tag.as_str(), "slot" | "template") {
            diagnostics.push(vue2_warning(
                "W_VUE2_INVALID_ROOT",
                format!(
                    "Cannot use <{}> as component root element because it may contain multiple nodes.",
                    element.tag
                ),
                element.span,
            ));
        }
        if element.for_exp.is_some() {
            diagnostics.push(vue2_warning(
                "W_VUE2_FOR_ROOT",
                "Cannot use v-for on stateful component root element because it renders multiple elements.",
                element.span,
            ));
        }
        let mut element = element;
        if element.if_exp.is_some() {
            element.if_conditions = vec![Vue2IfCondition {
                exp: element.if_exp.clone(),
                block: Box::new(element.clone_without_conditions()),
            }];
        }
        *root = Some(element);
    }

    if root.is_none() && options.warn {
        diagnostics.push(vue2_error(
            "E_VUE2_NO_ROOT",
            "Component template requires a root element, rather than just text.",
            None,
        ));
    }
}

fn is_ignorable_root_whitespace(_element: &Vue2Element) -> bool {
    false
}

fn cleanup_scoped_slot_children(element: &mut Vue2Element, in_pre_tag: bool) {
    element.children.retain(|child| {
        !matches!(
            child,
            Vue2Node::Element(child_element) if child_element.slot_scope.is_some()
        )
    });
    if !in_pre_tag {
        trim_ending_whitespace(element);
    }
}

fn collect_element_warnings(element: Option<&Vue2Element>, diagnostics: &mut DiagnosticSink) {
    let Some(element) = element else {
        return;
    };
    collect_element_warning_node(element, diagnostics);
}

fn collect_element_warning_node(element: &Vue2Element, diagnostics: &mut DiagnosticSink) {
    if element.inline_template && element.children.len() != 1 {
        diagnostics.push(vue2_warning(
            "W_VUE2_INLINE_TEMPLATE_CHILDREN",
            "Inline-template components must have exactly one child element.",
            element.span,
        ));
    }
    if element.tag == "transition-group" {
        for child in &element.children {
            let Vue2Node::Element(child) = child else {
                continue;
            };
            let Some(key) = child.key.as_deref() else {
                continue;
            };
            if child.for_exp.is_some()
                && (child.iterator1.as_deref() == Some(key)
                    || child.iterator2.as_deref() == Some(key))
            {
                diagnostics.push(vue2_warning(
                    "W_VUE2_TRANSITION_GROUP_INDEX_KEY",
                    "Do not use v-for index as key on <transition-group> children, this is the same as not using keys.",
                    child.span,
                ));
            }
        }
    }
    for child in &element.children {
        if let Vue2Node::Element(child) = child {
            collect_element_warning_node(child, diagnostics);
        }
    }
    for slot in element.scoped_slots.values() {
        collect_element_warning_node(slot, diagnostics);
    }
    for condition in element.if_conditions.iter().skip(1) {
        collect_element_warning_node(&condition.block, diagnostics);
    }
}

fn process_pre(element: &mut Vue2Element) {
    if remove_attr(element, "v-pre").is_some() {
        element.pre = true;
    }
}

fn process_raw_attrs(element: &mut Vue2Element) {
    element.attrs = element
        .attrs_list
        .iter()
        .map(|attr| Vue2Attribute {
            name: attr.name.clone(),
            value: js_string(&attr.value),
            span: attr.span,
            dynamic: false,
        })
        .collect();
}

fn process_structural_directives(element: &mut Vue2Element, diagnostics: &mut DiagnosticSink) {
    if let Some((value, span)) = remove_attr_with_span(element, "v-for") {
        element.for_span = span;
        if let Some(parsed) = parse_for(&value) {
            element.for_exp = Some(parsed.for_exp);
            element.alias = Some(parsed.alias);
            element.iterator1 = parsed.iterator1;
            element.iterator2 = parsed.iterator2;
        } else {
            diagnostics.push(vue2_warning(
                "W_VUE2_INVALID_FOR",
                format!("Invalid v-for expression: {value}"),
                span,
            ));
        }
    }
    if let Some((value, span)) = remove_attr_with_span(element, "v-if") {
        element.if_exp = Some(value);
        element.if_span = span;
    } else {
        if let Some((_, span)) = remove_attr_with_span(element, "v-else") {
            element.else_branch = true;
            element.else_span = span;
        }
        if let Some((value, span)) = remove_attr_with_span(element, "v-else-if") {
            element.elseif = Some(value);
            element.elseif_span = span;
        }
    }
    if remove_attr(element, "v-once").is_some() {
        element.once = true;
    }
}

fn process_element(
    element: &mut Vue2Element,
    diagnostics: &mut DiagnosticSink,
    options: &Vue2CompileOptions,
) {
    process_key(element, diagnostics);
    process_ref(element);
    process_slot_content(element, diagnostics);
    process_slot_outlet(element, diagnostics);
    process_component(element);
    process_platform_modules(element, diagnostics);
    process_attrs(element, diagnostics, options);
    process_sfc_asset_url_transform(element, options);
}

fn process_key(element: &mut Vue2Element, diagnostics: &mut DiagnosticSink) {
    if let Some((value, span)) = get_binding_attr_with_span(element, "key", true) {
        if element.tag == "template" {
            diagnostics.push(vue2_warning(
                "W_VUE2_TEMPLATE_KEY",
                "<template> cannot be keyed. Place the key on real elements instead.",
                span.or(element.span),
            ));
        }
        element.key = Some(value);
        element.key_span = span;
    }
}

fn process_ref(element: &mut Vue2Element) {
    if let Some(value) = get_binding_attr(element, "ref", true) {
        element.ref_name = Some(value);
        element.ref_in_for = element.for_exp.is_some();
    }
}

fn process_slot_content(element: &mut Vue2Element, diagnostics: &mut DiagnosticSink) {
    if element.tag == "template" {
        element.slot_scope =
            remove_attr(element, "scope").or_else(|| remove_attr(element, "slot-scope"));
    } else if let Some(slot_scope) = remove_attr(element, "slot-scope") {
        element.slot_scope = Some(slot_scope);
    }

    if let Some(slot_target) = get_binding_attr(element, "slot", true) {
        element.slot_target = Some(if slot_target == "\"\"" {
            "\"default\"".into()
        } else {
            slot_target.clone()
        });
        element.slot_target_dynamic = element.attrs_map.contains_key(":slot")
            || element.attrs_map.contains_key("v-bind:slot");
        if element.tag != "template" && element.slot_scope.is_none() {
            element.attrs.push(Vue2Attribute {
                name: "slot".into(),
                value: slot_target,
                span: element.span,
                dynamic: false,
            });
        }
    }

    if let Some((name, value, span)) = remove_slot_binding(element) {
        let (target, dynamic) = slot_name_from_binding(&name);
        let raw = name
            .strip_prefix("v-slot:")
            .or_else(|| name.strip_prefix('#'))
            .unwrap_or("default");
        if raw.starts_with('[') {
            warn_invalid_dynamic_arg(
                raw.trim_start_matches('[').trim_end_matches(']'),
                span,
                diagnostics,
            );
        }
        element.slot_target = Some(target);
        element.slot_target_dynamic = dynamic;
        element.slot_new_syntax = true;
        element.slot_scope = Some(if value.is_empty() {
            "_empty_".into()
        } else {
            value
        });
    }
}

fn process_slot_outlet(element: &mut Vue2Element, diagnostics: &mut DiagnosticSink) {
    if element.tag == "slot" {
        element.slot_name = get_binding_attr(element, "name", true);
        if element.key.is_some() {
            diagnostics.push(vue2_warning(
                "W_VUE2_SLOT_KEY",
                "`key` does not work on <slot> because slots are abstract outlets and can possibly expand into multiple elements.",
                element.key_span.or(element.span),
            ));
        }
    }
}

fn process_component(element: &mut Vue2Element) {
    if let Some(value) = get_binding_attr(element, "is", true) {
        element.component = Some(value);
    }
    if remove_attr(element, "inline-template").is_some() {
        element.inline_template = true;
    }
}

fn process_platform_modules(element: &mut Vue2Element, diagnostics: &mut DiagnosticSink) {
    if let Some(value) = remove_attr(element, "class") {
        if value.contains("{{") {
            diagnostics.push(vue2_warning(
                "W_VUE2_ATTR_INTERPOLATION",
                "Interpolation inside attributes has been removed. Use v-bind or the colon shorthand instead.",
                element.span,
            ));
        }
        element.static_class = Some(js_string(&value));
    }
    if let Some(value) = get_binding_attr(element, "class", false) {
        element.class_binding = Some(value);
    }
    if let Some(value) = remove_attr(element, "style") {
        element.static_style = Some(vue2_static_style_expression(&value));
    }
    if let Some(value) = get_binding_attr(element, "style", false) {
        element.style_binding = Some(value);
    }
    for name in [
        "required",
        "min",
        "max",
        "pattern",
        "maxlength",
        "minlength",
    ] {
        if let Some(rule) = remove_attr(element, name) {
            element.validators.push(Vue2Validator {
                name: name.into(),
                rule,
            });
        }
    }
}

fn process_attrs(
    element: &mut Vue2Element,
    diagnostics: &mut DiagnosticSink,
    options: &Vue2CompileOptions,
) {
    let list = element.attrs_list.clone();
    for attr in list {
        if !element
            .attrs_list
            .iter()
            .any(|current| current.name == attr.name)
        {
            continue;
        }
        let raw_name = attr.name.clone();
        let value = attr.value.clone();
        if is_directive_name(&raw_name) {
            element.has_bindings = true;
            let (name_no_modifiers, modifiers, modifier_order) = split_modifiers(&raw_name);
            if is_bind_name(&name_no_modifiers) {
                let mut name = bind_arg_name(&name_no_modifiers);
                let is_dynamic = is_dynamic_arg(&name);
                if name.starts_with('[') {
                    warn_invalid_dynamic_arg(
                        name.trim_start_matches('[').trim_end_matches(']'),
                        attr.span,
                        diagnostics,
                    );
                }
                if is_dynamic {
                    name = name
                        .trim_start_matches('[')
                        .trim_end_matches(']')
                        .to_string();
                }
                if value.trim().is_empty() {
                    diagnostics.push(vue2_warning(
                        "W_VUE2_EMPTY_BIND",
                        format!(
                            "The value for a v-bind expression cannot be empty. Found in \"v-bind:{name}\""
                        ),
                        attr.span,
                    ));
                }
                let parsed_value = parse_filters(&value);
                if name.is_empty() {
                    let prop = modifiers.get("prop").copied().unwrap_or(false);
                    let sync = modifiers.get("sync").copied().unwrap_or(false);
                    element.wrap_data = Some(Vue2DataWrap::Bind {
                        value: parsed_value,
                        prop,
                        sync,
                    });
                } else {
                    let target_attr = Vue2Attribute {
                        name: normalize_bound_name(&name, &modifiers, is_dynamic),
                        value: parsed_value.clone(),
                        span: attr.span,
                        dynamic: is_dynamic,
                    };
                    if should_use_prop(element, &target_attr.name, &modifiers, options) {
                        element.props.push(target_attr);
                    } else if is_dynamic {
                        element.dynamic_attrs.push(target_attr);
                    } else {
                        element.attrs.push(target_attr);
                    }
                    if modifiers.get("sync").copied().unwrap_or(false) {
                        let sync_code = gen_assignment_code(&parsed_value, "$event");
                        let camel_name = camelize(&name);
                        add_handler(
                            &mut element.events,
                            format!("update:{camel_name}"),
                            sync_code.clone(),
                            BTreeMap::new(),
                            Vec::new(),
                            false,
                            attr.span,
                        );
                        let hyphen_name = hyphenate(&name);
                        if hyphen_name != camel_name {
                            add_handler(
                                &mut element.events,
                                format!("update:{hyphen_name}"),
                                sync_code,
                                BTreeMap::new(),
                                Vec::new(),
                                false,
                                attr.span,
                            );
                        }
                    }
                }
            } else if is_on_name(&name_no_modifiers) {
                let name = on_arg_name(&name_no_modifiers);
                if name.starts_with('[') {
                    warn_invalid_dynamic_arg(
                        name.trim_start_matches('[').trim_end_matches(']'),
                        attr.span,
                        diagnostics,
                    );
                }
                add_handler(
                    &mut element.events,
                    name,
                    value,
                    modifiers,
                    modifier_order,
                    false,
                    attr.span,
                );
            } else {
                let (name, arg, is_dynamic_arg) = directive_name_and_arg(&name_no_modifiers);
                if is_dynamic_arg || arg.as_ref().is_some_and(|arg| arg.starts_with('[')) {
                    if let Some(arg) = arg.as_ref() {
                        warn_invalid_dynamic_arg(
                            arg.trim_start_matches('[').trim_end_matches(']'),
                            attr.span,
                            diagnostics,
                        );
                    }
                }
                if name == "model" {
                    if is_component(element, options) {
                        gen_component_model(element, &value, &modifiers);
                    } else {
                        gen_dom_model(element, &raw_name, &value, &modifiers);
                    }
                }
                if name == "validate" {
                    element.validate = Some(Vue2Validation {
                        field: arg.clone().unwrap_or_default(),
                        groups: modifiers.keys().cloned().collect(),
                    });
                }
                if name == "bind" && arg.is_none() {
                    element.wrap_data = Some(Vue2DataWrap::Bind {
                        value: value.clone(),
                        prop: modifiers.get("prop").copied().unwrap_or(false),
                        sync: modifiers.get("sync").copied().unwrap_or(false),
                    });
                } else if name == "on" && arg.is_none() {
                    element.wrap_listeners = Some(value.clone());
                } else if !matches!(name.as_str(), "model") {
                    element.directives.push(Vue2Directive {
                        name,
                        raw_name,
                        value: if value.is_empty() { None } else { Some(value) },
                        arg,
                        is_dynamic_arg,
                        modifiers,
                        span: attr.span,
                    });
                }
            }
            remove_attr(element, &attr.name);
        } else {
            if value.contains("{{") {
                diagnostics.push(vue2_warning(
                    "W_VUE2_ATTR_INTERPOLATION",
                    format!("{raw_name}=\"{value}\": Interpolation inside attributes has been removed. Use v-bind or the colon shorthand instead."),
                    attr.span,
                ));
            }
            element.attrs.push(Vue2Attribute {
                name: raw_name.clone(),
                value: js_string(&value),
                span: attr.span,
                dynamic: false,
            });
            if raw_name == "muted" && element.tag == "video" {
                element.props.push(Vue2Attribute {
                    name: raw_name,
                    value: "true".into(),
                    span: attr.span,
                    dynamic: false,
                });
            }
            remove_attr(element, &attr.name);
        }
    }

    if options.warn && has_duplicate_attr(&element.raw_attrs_list) {
        diagnostics.push(vue2_warning(
            "W_VUE2_DUPLICATE_ATTR",
            "duplicate attribute",
            element.span,
        ));
    }
}

fn process_sfc_asset_url_transform(element: &mut Vue2Element, options: &Vue2CompileOptions) {
    let Some(transform) = options.sfc_asset_url_transform.as_ref() else {
        return;
    };
    let asset_attrs = vue2_sfc_asset_attrs_for_tag(&element.tag, transform);
    let has_srcset_transform = matches!(element.tag.as_str(), "img" | "source");
    if asset_attrs.is_empty() && !has_srcset_transform {
        return;
    }
    for attr in &mut element.attrs {
        let should_rewrite_asset = asset_attrs.iter().any(|candidate| candidate == &attr.name);
        if should_rewrite_asset {
            if let Some(raw) = static_attr_raw_value(&attr.value) {
                attr.value = vue2_sfc_url_to_require(&raw, transform);
            }
        }
        if has_srcset_transform && attr.name == "srcset" {
            if let Some(raw) = static_attr_raw_value(&attr.value) {
                attr.value = vue2_sfc_srcset_to_require(&raw, transform);
            }
        }
    }
}

fn vue2_sfc_asset_attrs_for_tag(
    tag: &str,
    options: &Vue2SfcAssetUrlTransformOptions,
) -> Vec<String> {
    let mut attrs = options.tags.get(tag).cloned().unwrap_or_default();
    if let Some(wildcard) = options.tags.get("*") {
        attrs.extend(wildcard.iter().cloned());
    }
    attrs
}

fn static_attr_raw_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() < 2 || !value.starts_with('"') || !value.ends_with('"') {
        return None;
    }
    serde_json::from_str::<String>(value).ok()
}

fn vue2_sfc_srcset_to_require(value: &str, options: &Vue2SfcAssetUrlTransformOptions) -> String {
    let candidates = parse_vue2_sfc_srcset_candidates(value);
    if candidates.is_empty() {
        return js_string(value);
    }
    let mut code = String::new();
    for (url, descriptor) in candidates {
        code.push_str(&vue2_sfc_url_to_require(&url, options));
        code.push_str(" + ");
        code.push_str(&js_string(&format!(
            "{}{}, ",
            if descriptor.is_empty() { "" } else { " " },
            descriptor
        )));
        code.push_str(" + ");
    }
    code.truncate(code.len().saturating_sub(6));
    code.push('"');
    if code.ends_with(" + \"\"") {
        code.truncate(code.len() - " + \"\"".len());
    }
    code
}

fn parse_vue2_sfc_srcset_candidates(value: &str) -> Vec<(String, String)> {
    value
        .split(',')
        .filter_map(|candidate| {
            let normalized = candidate
                .replace(['\t', '\n', '\u{000C}', '\r'], " ")
                .trim()
                .to_string();
            if normalized.is_empty() {
                return None;
            }
            let (url, descriptor) = normalized.split_once(' ').map_or_else(
                || (normalized.clone(), String::new()),
                |(url, descriptor)| (url.to_string(), descriptor.trim().to_string()),
            );
            Some((url, descriptor))
        })
        .collect()
}

fn vue2_sfc_url_to_require(url: &str, options: &Vue2SfcAssetUrlTransformOptions) -> String {
    let first_char = url.chars().next();
    let mut normalized = url.to_string();
    if first_char == Some('~') {
        normalized = if url.chars().nth(1) == Some('/') {
            url.chars().skip(2).collect()
        } else {
            url.chars().skip(1).collect()
        };
    }

    if is_vue2_sfc_external_url(&normalized)
        || is_vue2_sfc_data_url(&normalized)
        || first_char == Some('#')
    {
        return js_string(url);
    }

    let (path, hash) = vue2_sfc_parse_url_parts(&normalized);
    if let Some(base) = options.base.as_deref().filter(|base| !base.is_empty()) {
        if first_char == Some('.') || first_char == Some('~') {
            return js_string(&vue2_sfc_join_base(base, &path, &hash));
        }
    }

    if options.include_absolute || matches!(first_char, Some('.' | '~' | '@')) {
        if hash.is_empty() {
            format!("require({})", js_string(&normalized))
        } else {
            format!("require({}) + {}", js_string(&path), js_string(&hash))
        }
    } else {
        js_string(url)
    }
}

fn vue2_sfc_parse_url_parts(url: &str) -> (String, String) {
    if url.is_empty() {
        return (String::new(), String::new());
    }
    if let Some(hash) = url.find('#') {
        (url[..hash].to_string(), url[hash..].to_string())
    } else {
        (url.to_string(), String::new())
    }
}

fn vue2_sfc_join_base(base: &str, path: &str, hash: &str) -> String {
    let (host, base_path) = split_vue2_sfc_base(base);
    let path = strip_vue2_sfc_leading_dot_segments(path);
    let mut joined = join_vue2_sfc_paths(base_path, &path);
    if joined.is_empty() {
        joined.push('/');
    }
    format!("{host}{joined}{hash}")
}

fn split_vue2_sfc_base(base: &str) -> (&str, &str) {
    if let Some(protocol) = base.find("://") {
        let after_protocol = protocol + 3;
        let rest = &base[after_protocol..];
        if let Some(slash) = rest.find('/') {
            let split = after_protocol + slash;
            return (&base[..split], &base[split..]);
        }
        return (base, "/");
    }
    if let Some(rest) = base.strip_prefix("//") {
        if let Some(slash) = rest.find('/') {
            let split = 2 + slash;
            return (&base[..split], &base[split..]);
        }
        return (base, "/");
    }
    ("", base)
}

fn strip_vue2_sfc_leading_dot_segments(path: &str) -> String {
    let mut rest = path;
    while let Some(stripped) = rest.strip_prefix("./") {
        rest = stripped;
    }
    rest.to_string()
}

fn join_vue2_sfc_paths(base: &str, path: &str) -> String {
    let absolute = base.starts_with('/');
    let mut parts = Vec::<&str>::new();
    for part in base.split('/').chain(path.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let mut joined = parts.join("/");
    if absolute {
        joined.insert(0, '/');
    }
    joined
}

fn is_vue2_sfc_external_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//")
}

fn is_vue2_sfc_data_url(url: &str) -> bool {
    url.trim_start().to_ascii_lowercase().starts_with("data:")
}

fn process_if_conditions(
    element: Vue2Element,
    parent: &mut Vue2Element,
    diagnostics: &mut DiagnosticSink,
) {
    let mut index = parent.children.len();
    while index > 0 {
        index -= 1;
        match &mut parent.children[index] {
            Vue2Node::Element(prev) => {
                if prev.if_exp.is_some() {
                    prev.if_conditions.push(Vue2IfCondition {
                        exp: element.elseif.clone(),
                        block: Box::new(element),
                    });
                } else {
                    diagnostics.push(vue2_warning(
                        "W_VUE2_ELSE_WITHOUT_IF",
                        format!(
                            "v-{} used on element <{}> without corresponding v-if.",
                            if let Some(exp) = &element.elseif {
                                format!("else-if=\"{exp}\"")
                            } else {
                                "else".into()
                            },
                            element.tag
                        ),
                        element.span,
                    ));
                }
                return;
            }
            Vue2Node::Text(text) => {
                if text.text != " " {
                    diagnostics.push(vue2_warning(
                        "W_VUE2_TEXT_BETWEEN_IF",
                        format!(
                            "text \"{}\" between v-if and v-else(-if) will be ignored.",
                            text.text.trim()
                        ),
                        text.span,
                    ));
                }
                parent.children.pop();
            }
        }
    }
    diagnostics.push(vue2_warning(
        "W_VUE2_ELSE_WITHOUT_IF",
        format!(
            "v-{} used on element <{}> without corresponding v-if.",
            if let Some(exp) = &element.elseif {
                format!("else-if=\"{exp}\"")
            } else {
                "else".into()
            },
            element.tag
        ),
        element.span,
    ));
}

fn element_generates_empty_data(element: &Vue2Element) -> bool {
    element.key.is_none()
        && element.ref_name.is_none()
        && !element.ref_in_for
        && !element.pre
        && element.component.is_none()
        && element.static_class.is_none()
        && element.class_binding.is_none()
        && element.static_style.is_none()
        && element.style_binding.is_none()
        && element.attrs.is_empty()
        && element.props.is_empty()
        && element.dynamic_attrs.is_empty()
        && element.directives.is_empty()
        && element.events.is_empty()
        && element.native_events.is_empty()
        && element.slot_target.is_none()
        && element.slot_scope.is_none()
        && element.scoped_slots.is_empty()
        && element.model.is_none()
        && element.wrap_data.is_none()
        && element.wrap_listeners.is_none()
        && element.validate.is_none()
        && element.validators.is_empty()
        && !element.inline_template
        && !element.has_bindings
}

fn push_text_node(
    parent: &mut Vue2Element,
    text: &str,
    start: usize,
    end: usize,
    options: &Vue2CompileOptions,
    in_v_pre: bool,
    in_pre_tag: bool,
) {
    let mut text = if is_text_tag(&parent.tag) {
        text.to_string()
    } else {
        decode_basic_entities(text)
    };
    if matches!(parent.tag.as_str(), "pre" | "textarea")
        && text.starts_with('\n')
        && parent.children.is_empty()
    {
        text.remove(0);
    }
    if text_is_collapsible_whitespace(&text) {
        if !in_pre_tag {
            if options.whitespace.as_deref() == Some("condense") {
                if text.contains('\n') {
                    return;
                }
            } else if parent.children.is_empty() || !options.preserve_whitespace {
                return;
            }
            text = if options.whitespace.as_deref() == Some("condense") {
                condense_whitespace(&text)
            } else {
                " ".into()
            };
            if parent
                .children
                .last()
                .is_some_and(|child| matches!(child, Vue2Node::Text(t) if t.text == " "))
            {
                return;
            }
        }
    } else if options.whitespace.as_deref() == Some("condense") && !in_pre_tag {
        text = condense_whitespace(&text);
    }
    let expression = if parent.pre || in_v_pre {
        None
    } else {
        parse_text(&text, options.delimiters.as_ref())
    };
    parent.children.push(Vue2Node::Text(Vue2Text {
        text,
        expression,
        is_comment: false,
        span: Some(Span::new(FileId(0), start, end)),
        static_node: false,
    }));
}

fn text_is_collapsible_whitespace(value: &str) -> bool {
    value.chars().all(|ch| ch.is_ascii_whitespace())
}

fn mark_static_node(node: &mut Vue2Node, options: &Vue2CompileOptions) -> bool {
    match node {
        Vue2Node::Text(text) => {
            text.static_node = text.expression.is_none();
            text.static_node
        }
        Vue2Node::Element(element) => mark_static_element(element, options),
    }
}

fn mark_static_element(element: &mut Vue2Element, options: &Vue2CompileOptions) -> bool {
    let mut static_node = element.pre
        || (!element.has_bindings
            && element.if_exp.is_none()
            && element.elseif.is_none()
            && !element.else_branch
            && element.for_exp.is_none()
            && !is_built_in_tag(&element.tag)
            && is_reserved_tag_with_options(&element.tag, options)
            && element.key.is_none()
            && element.ref_name.is_none()
            && element.slot_target.is_none()
            && element.component.is_none()
            && element.directives.is_empty()
            && element.events.is_empty()
            && element.dynamic_attrs.is_empty()
            && element.class_binding.is_none()
            && element.style_binding.is_none()
            && element.model.is_none());
    if !is_reserved_tag_with_options(&element.tag, options)
        && element.tag != "slot"
        && !element.inline_template
    {
        element.static_node = false;
        return false;
    }
    for child in &mut element.children {
        if !mark_static_node(child, options) {
            static_node = false;
        }
    }
    for (index, condition) in element.if_conditions.iter_mut().enumerate() {
        let condition_static = mark_static_element(&mut condition.block, options);
        if index == 0 && element.if_exp.is_some() {
            condition.block.static_node = false;
        } else if !condition_static {
            static_node = false;
        }
    }
    element.static_node = static_node;
    static_node
}

fn mark_static_roots(element: &mut Vue2Element, in_for: bool, options: &Vue2CompileOptions) {
    if element.static_node || element.once {
        element.static_in_for = in_for;
    }
    if element.static_node
        && !element.children.is_empty()
        && !(element.children.len() == 1
            && matches!(element.children.first(), Some(Vue2Node::Text(text)) if text.expression.is_none()))
    {
        element.static_root = true;
        return;
    }
    element.static_root = false;
    for child in &mut element.children {
        if let Vue2Node::Element(child) = child {
            mark_static_roots(child, in_for || element.for_exp.is_some(), options);
        }
    }
    for condition in &mut element.if_conditions {
        mark_static_roots(&mut condition.block, in_for, options);
    }
}

fn generate_render_mir(
    mir: &Vue2Mir,
    js: &JsAstStore,
    options: &Vue2CompileOptions,
    static_render_fns: &mut Vec<String>,
) -> String {
    let mut state = Vue2MirCodegenState {
        mir,
        js,
        options,
        static_render_fns,
        pre: false,
    };
    let code = mir
        .root_node()
        .and_then(|root| root.children.first().copied())
        .map(|root| {
            if mir_node_tag(mir, root).as_deref() == Some("script") {
                "null".into()
            } else {
                gen_mir_node(root, &mut state)
            }
        })
        .unwrap_or_else(|| "_c(\"div\")".into());
    format!("with(this){{return {code}}}")
}

struct Vue2MirCodegenState<'a> {
    mir: &'a Vue2Mir,
    js: &'a JsAstStore,
    options: &'a Vue2CompileOptions,
    static_render_fns: &'a mut Vec<String>,
    pre: bool,
}

fn gen_mir_node(id: NodeId, state: &mut Vue2MirCodegenState<'_>) -> String {
    let Some(node) = state.mir.node(id) else {
        return "_e()".into();
    };
    match &node.kind {
        Vue2MirKind::Root(_) => node
            .children
            .first()
            .map(|child| gen_mir_node(*child, state))
            .unwrap_or_else(|| "_c(\"div\")".into()),
        Vue2MirKind::CreateElement(create) => gen_mir_create_element(id, create, state),
        Vue2MirKind::Text(text) => gen_mir_text(id, text, state),
        Vue2MirKind::Comment { value } => format!("_e({})", js_string(value)),
        Vue2MirKind::If(if_node) => gen_mir_if_conditions(&if_node.branches, state),
        Vue2MirKind::For(for_node) => gen_mir_for(for_node, state),
        Vue2MirKind::RenderStatic(render_static) => gen_mir_static(render_static, state),
        Vue2MirKind::Once(once) => {
            let code = gen_mir_node(once.body, state);
            let key = once
                .key
                .as_ref()
                .map(|key| render_mir_expr(key, state))
                .unwrap_or_else(|| "null".into());
            format!("_o({code},{},{key})", once.once_id)
        }
        Vue2MirKind::SlotOutlet(slot) => gen_mir_slot_outlet(id, slot, state),
        Vue2MirKind::ScopedSlot(slot) => gen_mir_scoped_slot(slot, state),
        Vue2MirKind::FilterCall { .. } | Vue2MirKind::Directive(_) => "_e()".into(),
    }
}

fn gen_mir_create_element(
    id: NodeId,
    create: &Vue2CreateElement,
    state: &mut Vue2MirCodegenState<'_>,
) -> String {
    if create.is_template && !state.pre {
        return gen_mir_children(id, state, false).unwrap_or_else(|| "void 0".into());
    }

    let data = create
        .data
        .as_ref()
        .map(|data| gen_mir_data(data, mir_create_tag_literal(create), state));
    let children = if create
        .data
        .as_ref()
        .and_then(|data| data.inline_template.as_ref())
        .is_some()
    {
        None
    } else {
        gen_mir_children(id, state, true)
    };
    let tag = gen_mir_create_tag(create, state);
    let code = match (data, children) {
        (Some(data), Some(children)) => format!("_c({tag},{data},{children})"),
        (Some(data), None) => format!("_c({tag},{data})"),
        (None, Some(children)) => format!("_c({tag},{children})"),
        (None, None) => format!("_c({tag})"),
    };
    create
        .validation
        .as_ref()
        .map(|validation| wrap_validation_mir(validation, &code))
        .unwrap_or(code)
}

fn gen_mir_create_tag(create: &Vue2CreateElement, state: &Vue2MirCodegenState<'_>) -> String {
    match &create.tag {
        MirExpr::String(tag) => {
            let maybe_component =
                create.is_component || !is_reserved_tag_with_options(tag, state.options);
            if maybe_component {
                if let Some(binding) = binding_component_tag_name(tag, state.options) {
                    return binding;
                }
            }
            js_string_single(tag)
        }
        _ => render_mir_expr(&create.tag, state),
    }
}

fn mir_create_tag_literal(create: &Vue2CreateElement) -> Option<&str> {
    create
        .data
        .as_ref()
        .and_then(|data| data.tag.as_deref())
        .or_else(|| match &create.tag {
            MirExpr::String(tag) => Some(tag.as_str()),
            _ => None,
        })
}

fn gen_mir_text(id: NodeId, text: &Vue2TextCall, state: &Vue2MirCodegenState<'_>) -> String {
    let mut expression = render_mir_expr(&text.value, state);
    let mut filtered = false;
    if let Some(node) = state.mir.node(id) {
        for child in &node.children {
            if let Some(Vue2MirKind::FilterCall { name, args }) =
                state.mir.node(*child).map(|node| &node.kind)
            {
                let args = args
                    .iter()
                    .map(|arg| render_js_expr(state.js, *arg))
                    .collect::<Vec<_>>();
                expression = if args.is_empty() {
                    format!("_f({})({expression})", js_string(name))
                } else {
                    format!("_f({})({expression},{})", js_string(name), args.join(","))
                };
                filtered = true;
            }
        }
    }
    if filtered {
        format!("_v(_s({expression}))")
    } else {
        format!("_v({expression})")
    }
}

fn gen_mir_static(render_static: &Vue2RenderStatic, state: &mut Vue2MirCodegenState<'_>) -> String {
    let Some(body) = render_static.body else {
        return format!("_m({})", render_static.index);
    };
    let original_pre = state.pre;
    if mir_node_pre(state.mir, body) {
        state.pre = true;
    }
    let code = gen_mir_node(body, state);
    state.pre = original_pre;
    let index = render_static.index as usize;
    while state.static_render_fns.len() <= index {
        state.static_render_fns.push(String::new());
    }
    state.static_render_fns[index] = format!("with(this){{return {code}}}");
    if render_static.in_for {
        format!("_m({},true)", render_static.index)
    } else {
        format!("_m({})", render_static.index)
    }
}

fn gen_mir_if_conditions(
    conditions: &[Vue2IfMirBranch],
    state: &mut Vue2MirCodegenState<'_>,
) -> String {
    let Some((condition, rest)) = conditions.split_first() else {
        return "_e()".into();
    };
    if let Some(exp) = condition.condition {
        format!(
            "({})?{}:{}",
            render_js_expr(state.js, exp),
            gen_mir_node(condition.body, state),
            gen_mir_if_conditions(rest, state)
        )
    } else {
        gen_mir_node(condition.body, state)
    }
}

fn gen_mir_for(for_node: &Vue2ForMir, state: &mut Vue2MirCodegenState<'_>) -> String {
    let source = render_js_expr(state.js, for_node.source);
    let alias = render_js_pattern(state.js, for_node.alias);
    let iterator1 = for_node
        .iterator1
        .map(|value| format!(",{}", render_js_pattern(state.js, value)))
        .unwrap_or_default();
    let iterator2 = for_node
        .iterator2
        .map(|value| format!(",{}", render_js_pattern(state.js, value)))
        .unwrap_or_default();
    let body = gen_mir_node(for_node.body, state);
    format!("_l(({source}),function({alias}{iterator1}{iterator2}){{return {body}}})")
}

fn gen_mir_data(
    data: &Vue2DataObject,
    tag_literal: Option<&str>,
    state: &mut Vue2MirCodegenState<'_>,
) -> String {
    let mut parts = Vec::new();
    if let Some(dirs) = gen_mir_directives(&data.directives, state) {
        parts.push(dirs);
    }
    if let Some(key) = &data.key {
        parts.push(format!("key:{}", render_mir_expr(key, state)));
    }
    if let Some(ref_name) = &data.ref_name {
        parts.push(format!("ref:{}", render_mir_expr(ref_name, state)));
    }
    if data.ref_in_for {
        parts.push("refInFor:true".into());
    }
    if data.pre {
        parts.push("pre:true".into());
    }
    if let Some(tag) = &data.tag {
        parts.push(format!("tag:{}", js_string(tag)));
    }
    if let Some(static_class) = &data.static_class {
        parts.push(format!(
            "staticClass:{}",
            render_mir_expr(static_class, state)
        ));
    }
    if let Some(class_binding) = &data.class_binding {
        parts.push(format!("class:{}", render_mir_expr(class_binding, state)));
    }
    if let Some(static_style) = &data.static_style {
        parts.push(format!(
            "staticStyle:{}",
            render_mir_expr(static_style, state)
        ));
    }
    if let Some(style_binding) = &data.style_binding {
        parts.push(format!("style:({})", render_mir_expr(style_binding, state)));
    }
    if !data.attrs.is_empty() {
        parts.push(format!(
            "attrs:{}",
            gen_mir_props(
                &data.attrs,
                state.options,
                PropValueKind::StaticAttribute,
                state
            )
        ));
    }
    if !data.dom_props.is_empty() {
        parts.push(format!(
            "domProps:{}",
            gen_mir_props(
                &data.dom_props,
                state.options,
                PropValueKind::Expression,
                state
            )
        ));
    }
    if !data.events.is_empty() {
        parts.push(gen_mir_handlers(&data.events, false, state));
    }
    if !data.native_events.is_empty() {
        parts.push(gen_mir_handlers(&data.native_events, true, state));
    }
    if let Some(slot) = &data.slot {
        parts.push(format!("slot:{}", render_mir_expr(slot, state)));
    }
    if !data.scoped_slots.is_empty() {
        parts.push(gen_mir_scoped_slots(&data.scoped_slots, state));
    }
    if let Some(model) = &data.model {
        parts.push(format!(
            "model:{{value:{},callback:{},expression:{}}}",
            render_mir_expr(&model.value, state),
            render_js_stmt(state.js, model.callback),
            model.expression
        ));
    }
    if let Some(inline) = &data.inline_template {
        if let Some(inline) = gen_mir_inline_template(inline, state) {
            parts.push(inline);
        }
    }
    if let Some(validate) = &data.validate {
        parts.push(format!(
            "validate:{{\"field\":{},\"groups\":{}}}",
            js_string(&validate.field),
            json_string_array(&validate.groups)
        ));
    }
    if !data.validators.is_empty() {
        parts.push(format!(
            "validators:{}",
            ast_validators_json(&data.validators)
        ));
    }

    let mut rendered = format!("{{{}}}", parts.join(","));
    let tag = tag_literal.unwrap_or("");
    if !data.dynamic_attrs.is_empty() {
        rendered = format!(
            "_b({rendered},{},{} )",
            js_string(tag),
            gen_mir_props(
                &data.dynamic_attrs,
                state.options,
                PropValueKind::Expression,
                state
            )
        )
        .replace("} )", "})");
    }
    if let Some(wrap) = &data.wrap_data {
        rendered = format!(
            "_b({rendered},{},{},{prop}{sync})",
            js_string_single(tag),
            render_mir_expr(&wrap.value, state),
            prop = wrap.prop,
            sync = if wrap.sync { ",true" } else { "" }
        );
    }
    if let Some(listeners) = &data.wrap_listeners {
        rendered = format!("_g({rendered},{})", render_mir_expr(listeners, state));
    }
    rendered
}

fn gen_mir_directives(
    directives: &[Vue2DirectiveRuntime],
    state: &Vue2MirCodegenState<'_>,
) -> Option<String> {
    if directives.is_empty() {
        return None;
    }
    let rendered = directives
        .iter()
        .map(|directive| {
            let mut fields = vec![
                format!("name:{}", js_string(&directive.name)),
                format!("rawName:{}", js_string(&directive.raw_name)),
            ];
            if let Some(value) = &directive.value {
                let value = render_mir_expr(value, state);
                fields.push(format!("value:({value})"));
                fields.push(format!("expression:{}", js_string(&value)));
            }
            if let Some(arg) = &directive.arg {
                if directive.is_dynamic_arg {
                    fields.push(format!("arg:{}", render_mir_expr(arg, state)));
                } else {
                    fields.push(format!(
                        "arg:{}",
                        js_string(&render_mir_string_arg(arg, state))
                    ));
                }
            }
            if !directive.modifiers.is_empty() {
                fields.push(format!(
                    "modifiers:{}",
                    modifiers_json(&directive.modifiers)
                ));
            }
            format!("{{{}}}", fields.join(","))
        })
        .collect::<Vec<_>>();
    Some(format!("directives:[{}]", rendered.join(",")))
}

fn gen_mir_props(
    attrs: &[Vue2DataProp],
    options: &Vue2CompileOptions,
    value_kind: PropValueKind,
    state: &Vue2MirCodegenState<'_>,
) -> String {
    let static_props = attrs
        .iter()
        .filter(|attr| !attr.dynamic)
        .map(|attr| {
            let value = render_mir_expr(&attr.value, state);
            let value = match value_kind {
                PropValueKind::StaticAttribute => {
                    let value = decode_newline_entities_for_attr(&attr.name, &value, options);
                    transform_special_newlines(&value)
                }
                PropValueKind::Expression => value,
            };
            format!("{}:{value}", js_string(&attr.name))
        })
        .collect::<Vec<_>>()
        .join(",");
    let dynamic_props = attrs
        .iter()
        .filter(|attr| attr.dynamic)
        .flat_map(|attr| [attr.name.clone(), render_mir_expr(&attr.value, state)])
        .collect::<Vec<_>>();
    if dynamic_props.is_empty() {
        format!("{{{static_props}}}")
    } else {
        format!("_d({{{static_props}}},[{}])", dynamic_props.join(","))
    }
}

fn gen_mir_handlers(
    events: &BTreeMap<String, Vec<vuec_ast::Vue2EventHandler>>,
    native: bool,
    state: &Vue2MirCodegenState<'_>,
) -> String {
    let prefix = if native { "nativeOn" } else { "on" };
    let mut entries = events.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(name, handlers)| vue2_event_order_key(name, handlers));
    let handlers = entries
        .into_iter()
        .map(|(name, handlers)| {
            let code = if handlers.is_empty() {
                "function(){}".into()
            } else if handlers.len() == 1 {
                gen_mir_handler(&handlers[0], state)
            } else {
                format!(
                    "[{}]",
                    handlers
                        .iter()
                        .map(|handler| gen_mir_handler(handler, state))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            };
            format!("{}:{code}", js_string(name))
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{prefix}:{{{handlers}}}")
}

fn vue2_event_order_key<'a>(
    name: &'a str,
    handlers: &[vuec_ast::Vue2EventHandler],
) -> (usize, usize, usize, &'a str) {
    let span = handlers
        .first()
        .and_then(|handler| handler.span)
        .map(|span| (span.start.0, span.end.0))
        .unwrap_or((usize::MAX, usize::MAX));
    (span.0, span.1, vue2_same_attr_event_rank(name), name)
}

fn vue2_same_attr_event_rank(name: &str) -> usize {
    if name
        .strip_prefix("update:")
        .is_some_and(|event| event.contains('-'))
    {
        1
    } else {
        0
    }
}

fn gen_mir_handler(
    handler: &vuec_ast::Vue2EventHandler,
    state: &Vue2MirCodegenState<'_>,
) -> String {
    let value = render_js_stmt(state.js, handler.value);
    let is_method_path = is_simple_path(&value);
    let is_function_expression = is_function_expression(&value);
    let is_function_invocation = is_function_invocation(&value);
    let has_modifier_object = handler.has_modifier_object || !handler.modifiers.is_empty();
    if !has_modifier_object {
        if is_method_path || is_function_expression {
            return value;
        }
        if is_function_invocation {
            return format!("function($event){{return {value}}}");
        }
        return format!("function($event){{{value}}}");
    }

    let mut code = String::new();
    let mut modifier_code = String::new();
    let mut keys = Vec::new();
    let modifier_order = if handler.modifier_order.is_empty() {
        handler.modifiers.keys().cloned().collect::<Vec<_>>()
    } else {
        handler
            .modifier_order
            .iter()
            .filter(|key| handler.modifiers.contains_key(*key))
            .cloned()
            .collect::<Vec<_>>()
    };
    for key in &modifier_order {
        match key.as_str() {
            "stop" => modifier_code.push_str("$event.stopPropagation();"),
            "prevent" => modifier_code.push_str("$event.preventDefault();"),
            "self" => {
                modifier_code.push_str("if($event.target !== $event.currentTarget)return null;")
            }
            "ctrl" => modifier_code.push_str("if(!$event.ctrlKey)return null;"),
            "shift" => modifier_code.push_str("if(!$event.shiftKey)return null;"),
            "alt" => modifier_code.push_str("if(!$event.altKey)return null;"),
            "meta" => modifier_code.push_str("if(!$event.metaKey)return null;"),
            "left" => {
                modifier_code.push_str("if('button' in $event && $event.button !== 0)return null;");
                keys.push(key.clone());
            }
            "middle" => {
                modifier_code.push_str("if('button' in $event && $event.button !== 1)return null;")
            }
            "right" => {
                modifier_code.push_str("if('button' in $event && $event.button !== 2)return null;");
                keys.push(key.clone());
            }
            "exact" => {
                let guards = ["ctrl", "shift", "alt", "meta"]
                    .into_iter()
                    .filter(|modifier| !handler.modifiers.contains_key(*modifier))
                    .map(|modifier| format!("$event.{modifier}Key"))
                    .collect::<Vec<_>>()
                    .join("||");
                if !guards.is_empty() {
                    modifier_code.push_str(&format!("if({guards})return null;"));
                }
            }
            _ => keys.push(key.clone()),
        }
    }
    if !keys.is_empty() {
        code.push_str(&gen_key_filter(&keys));
    }
    code.push_str(&modifier_code);
    let handler_code = if is_method_path {
        format!("return {value}.apply(null, arguments)")
    } else if is_function_expression {
        format!("return ({value}).apply(null, arguments)")
    } else if is_function_invocation {
        format!("return {value}")
    } else {
        value
    };
    format!("function($event){{{code}{handler_code}}}")
}

fn gen_mir_children(
    parent: NodeId,
    state: &mut Vue2MirCodegenState<'_>,
    check_skip: bool,
) -> Option<String> {
    let children = renderable_mir_children(parent, state.mir);
    gen_mir_children_from_ids(&children, state, check_skip)
}

fn gen_mir_children_from_ids(
    children: &[NodeId],
    state: &mut Vue2MirCodegenState<'_>,
    check_skip: bool,
) -> Option<String> {
    if children.is_empty() {
        return None;
    }
    if children.len() == 1 && mir_for_child_can_skip_array(state.mir, children[0]) {
        let normalization = if check_skip {
            if mir_node_is_component(
                state.mir,
                mir_for_body_node(state.mir, children[0]).unwrap_or(children[0]),
                state.options,
            ) {
                ",1"
            } else {
                ",0"
            }
        } else {
            ""
        };
        let generated = gen_mir_node(children[0], state);
        return Some(format!("{generated}{normalization}"));
    }
    let nodes = children
        .iter()
        .map(|child| gen_mir_node(*child, state))
        .collect::<Vec<_>>();
    let normalization = if check_skip {
        get_mir_normalization_type(children, state.mir, state.options)
    } else {
        0
    };
    if normalization > 0 {
        Some(format!("[{}],{}", nodes.join(","), normalization))
    } else {
        Some(format!("[{}]", nodes.join(",")))
    }
}

fn gen_mir_slot_outlet(
    id: NodeId,
    slot: &Vue2SlotOutlet,
    state: &mut Vue2MirCodegenState<'_>,
) -> String {
    let name = render_mir_expr(&slot.name, state);
    let children = gen_mir_children(id, state, false);
    let props = (!slot.props.is_empty())
        .then(|| gen_mir_props(&slot.props, state.options, PropValueKind::Expression, state));
    let mut code = format!("_t({name}");
    if let Some(children) = children {
        code.push_str(&format!(",function(){{return {children}}}"));
    } else if props.is_some() || slot.bind.is_some() {
        code.push_str(",null");
    }
    if let Some(props) = props {
        code.push_str(&format!(",{props}"));
    }
    if let Some(bind) = &slot.bind {
        if slot.props.is_empty() {
            code.push_str(",null");
        }
        code.push_str(&format!(",{}", render_mir_expr(bind, state)));
    }
    code.push(')');
    code
}

fn gen_mir_inline_template(
    inline: &Vue2InlineTemplate,
    state: &Vue2MirCodegenState<'_>,
) -> Option<String> {
    let body = inline.body?;
    let mut static_render_fns = Vec::new();
    let mut nested = Vue2MirCodegenState {
        mir: state.mir,
        js: state.js,
        options: state.options,
        static_render_fns: &mut static_render_fns,
        pre: false,
    };
    let code = gen_mir_node(body, &mut nested);
    let render = format!("with(this){{return {code}}}");
    let static_render_fns = static_render_fns
        .into_iter()
        .map(|code| format!("function(){{{code}}}"))
        .collect::<Vec<_>>()
        .join(",");
    Some(format!(
        "inlineTemplate:{{render:function(){{{render}}},staticRenderFns:[{static_render_fns}]}}"
    ))
}

fn gen_mir_scoped_slots(slots: &[Vue2ScopedSlot], state: &mut Vue2MirCodegenState<'_>) -> String {
    let rendered = slots
        .iter()
        .map(|slot| gen_mir_scoped_slot(slot, state))
        .collect::<Vec<_>>()
        .join(",");
    if slots.iter().any(|slot| slot.force_update) {
        format!("scopedSlots:_u([{rendered}],null,true)")
    } else {
        format!("scopedSlots:_u([{rendered}])")
    }
}

fn gen_mir_scoped_slot(slot: &Vue2ScopedSlot, state: &mut Vue2MirCodegenState<'_>) -> String {
    if let Some(condition) = slot.condition {
        return format!(
            "({})?{}:null",
            render_js_expr(state.js, condition),
            gen_mir_scoped_slot_object(slot, state)
        );
    }
    if let Some(source) = slot.for_source {
        let alias = slot
            .for_alias
            .map(|alias| render_js_pattern(state.js, alias))
            .unwrap_or_else(|| "item".into());
        let iterator1 = slot
            .for_iterator1
            .map(|value| format!(",{}", render_js_pattern(state.js, value)))
            .unwrap_or_default();
        let iterator2 = slot
            .for_iterator2
            .map(|value| format!(",{}", render_js_pattern(state.js, value)))
            .unwrap_or_default();
        let body = gen_mir_scoped_slot_object(slot, state);
        return format!(
            "_l(({}),function({alias}{iterator1}{iterator2}){{return {body}}})",
            render_js_expr(state.js, source)
        );
    }
    gen_mir_scoped_slot_object(slot, state)
}

fn gen_mir_scoped_slot_object(
    slot: &Vue2ScopedSlot,
    state: &mut Vue2MirCodegenState<'_>,
) -> String {
    let scope = slot
        .params
        .map(|params| render_js_pattern(state.js, params))
        .unwrap_or_default();
    let scope = if scope == "_empty_" {
        ""
    } else {
        scope.as_str()
    };
    let body = gen_mir_scoped_slot_body(slot, state);
    let proxy = if scope.is_empty() { ",proxy:true" } else { "" };
    format!(
        "{{key:{},fn:function({scope}){{return {body}}}{proxy}}}",
        render_mir_expr(&slot.name, state)
    )
}

fn gen_mir_scoped_slot_body(slot: &Vue2ScopedSlot, state: &mut Vue2MirCodegenState<'_>) -> String {
    if slot.body_is_fragment {
        let children = gen_mir_children_from_ids(&slot.body, state, false)
            .unwrap_or_else(|| "undefined".into());
        if let Some(condition) = slot.legacy_condition {
            format!(
                "({})?{children}:undefined",
                render_js_expr(state.js, condition)
            )
        } else {
            children
        }
    } else {
        slot.body
            .first()
            .map(|body| gen_mir_node(*body, state))
            .unwrap_or_else(|| "undefined".into())
    }
}

fn renderable_mir_children(parent: NodeId, mir: &Vue2Mir) -> Vec<NodeId> {
    mir.node(parent)
        .map(|node| {
            node.children
                .iter()
                .copied()
                .filter(|child| {
                    !matches!(
                        mir.node(*child).map(|node| &node.kind),
                        Some(Vue2MirKind::ScopedSlot(_))
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn mir_for_child_can_skip_array(mir: &Vue2Mir, child: NodeId) -> bool {
    match mir.node(child).map(|node| &node.kind) {
        Some(Vue2MirKind::For(for_node)) => {
            mir_for_body_node(mir, for_node.body).is_some_and(|body| {
                !mir_node_is_template(mir, body) && !mir_node_is_slot_outlet(mir, body)
            })
        }
        _ => false,
    }
}

fn mir_for_body_node(mir: &Vue2Mir, child: NodeId) -> Option<NodeId> {
    match mir.node(child).map(|node| &node.kind) {
        Some(Vue2MirKind::For(for_node)) => Some(for_node.body),
        Some(Vue2MirKind::RenderStatic(render_static)) => render_static.body,
        Some(Vue2MirKind::Once(once)) => Some(once.body),
        _ => Some(child),
    }
}

fn get_mir_normalization_type(
    children: &[NodeId],
    mir: &Vue2Mir,
    options: &Vue2CompileOptions,
) -> u8 {
    let mut result = 0;
    for child in children {
        if mir_node_contains_for(mir, *child)
            || mir_node_is_template(mir, *child)
            || mir_node_is_slot_outlet(mir, *child)
        {
            return 2;
        }
        if mir_node_is_component(mir, *child, options) {
            result = 1;
        }
    }
    result
}

fn mir_node_contains_for(mir: &Vue2Mir, id: NodeId) -> bool {
    match mir.node(id).map(|node| &node.kind) {
        Some(Vue2MirKind::For(_)) => true,
        Some(Vue2MirKind::If(if_node)) => if_node
            .branches
            .iter()
            .any(|branch| mir_node_contains_for(mir, branch.body)),
        Some(Vue2MirKind::RenderStatic(render_static)) => render_static
            .body
            .is_some_and(|body| mir_node_contains_for(mir, body)),
        Some(Vue2MirKind::Once(once)) => mir_node_contains_for(mir, once.body),
        _ => false,
    }
}

fn mir_node_tag(mir: &Vue2Mir, id: NodeId) -> Option<String> {
    match mir.node(id).map(|node| &node.kind)? {
        Vue2MirKind::CreateElement(create) => match &create.tag {
            MirExpr::String(tag) => Some(tag.clone()),
            _ => None,
        },
        Vue2MirKind::RenderStatic(render_static) => {
            render_static.body.and_then(|body| mir_node_tag(mir, body))
        }
        Vue2MirKind::Once(once) => mir_node_tag(mir, once.body),
        Vue2MirKind::For(for_node) => mir_node_tag(mir, for_node.body),
        _ => None,
    }
}

fn mir_node_pre(mir: &Vue2Mir, id: NodeId) -> bool {
    matches!(
        mir.node(id).map(|node| &node.kind),
        Some(Vue2MirKind::CreateElement(Vue2CreateElement {
            data: Some(Vue2DataObject { pre: true, .. }),
            ..
        }))
    )
}

fn mir_node_is_template(mir: &Vue2Mir, id: NodeId) -> bool {
    match mir.node(id).map(|node| &node.kind) {
        Some(Vue2MirKind::CreateElement(create)) => create.is_template,
        Some(Vue2MirKind::If(if_node)) => if_node
            .branches
            .iter()
            .any(|branch| mir_node_is_template(mir, branch.body)),
        Some(Vue2MirKind::For(for_node)) => mir_node_is_template(mir, for_node.body),
        Some(Vue2MirKind::RenderStatic(render_static)) => render_static
            .body
            .is_some_and(|body| mir_node_is_template(mir, body)),
        Some(Vue2MirKind::Once(once)) => mir_node_is_template(mir, once.body),
        _ => false,
    }
}

fn mir_node_is_slot_outlet(mir: &Vue2Mir, id: NodeId) -> bool {
    match mir.node(id).map(|node| &node.kind) {
        Some(Vue2MirKind::SlotOutlet(_)) => true,
        Some(Vue2MirKind::If(if_node)) => if_node
            .branches
            .iter()
            .any(|branch| mir_node_is_slot_outlet(mir, branch.body)),
        Some(Vue2MirKind::For(for_node)) => mir_node_is_slot_outlet(mir, for_node.body),
        Some(Vue2MirKind::RenderStatic(render_static)) => render_static
            .body
            .is_some_and(|body| mir_node_is_slot_outlet(mir, body)),
        Some(Vue2MirKind::Once(once)) => mir_node_is_slot_outlet(mir, once.body),
        _ => false,
    }
}

fn mir_node_is_component(mir: &Vue2Mir, id: NodeId, options: &Vue2CompileOptions) -> bool {
    match mir.node(id).map(|node| &node.kind) {
        Some(Vue2MirKind::CreateElement(create)) => {
            create.is_component
                || matches!(
                    &create.tag,
                    MirExpr::String(tag) if !is_reserved_tag_with_options(tag, options)
                )
        }
        Some(Vue2MirKind::If(if_node)) => if_node
            .branches
            .iter()
            .any(|branch| mir_node_is_component(mir, branch.body, options)),
        Some(Vue2MirKind::For(for_node)) => mir_node_is_component(mir, for_node.body, options),
        Some(Vue2MirKind::RenderStatic(render_static)) => render_static
            .body
            .is_some_and(|body| mir_node_is_component(mir, body, options)),
        Some(Vue2MirKind::Once(once)) => mir_node_is_component(mir, once.body, options),
        _ => false,
    }
}

fn render_mir_expr(expr: &MirExpr, state: &Vue2MirCodegenState<'_>) -> String {
    match expr {
        MirExpr::String(value) => js_string(value),
        MirExpr::Bool(value) => value.to_string(),
        MirExpr::Null => "null".into(),
        MirExpr::JsExpr(id) => render_js_expr(state.js, *id),
        MirExpr::Helper(helper) => format!("{helper:?}"),
    }
}

fn render_mir_string_arg(expr: &MirExpr, state: &Vue2MirCodegenState<'_>) -> String {
    match expr {
        MirExpr::String(value) => value.clone(),
        _ => render_mir_expr(expr, state),
    }
}

fn render_js_expr(js: &JsAstStore, id: JsExprId) -> String {
    js.expr_entry(id)
        .map(|entry| entry.source.as_str().to_string())
        .unwrap_or_default()
}

fn render_js_stmt(js: &JsAstStore, id: JsStmtId) -> String {
    js.stmt_entry(id)
        .map(|entry| entry.source.as_str().to_string())
        .unwrap_or_default()
}

fn render_js_pattern(js: &JsAstStore, id: JsPatternId) -> String {
    js.pattern_entry(id)
        .map(|entry| entry.source.as_str().to_string())
        .unwrap_or_default()
}

fn binding_component_tag_name(tag: &str, options: &Vue2CompileOptions) -> Option<String> {
    if !options.bindings_is_script_setup || options.bindings.is_empty() {
        return None;
    }
    check_binding_type(&options.bindings, tag)
}

fn wrap_validation_mir(validation: &Vue2ValidationData, child_code: &str) -> String {
    let field = validation
        .validate
        .as_ref()
        .map(|validate| validate.field.clone())
        .unwrap_or_default();
    let groups = validation
        .validate
        .as_ref()
        .map(|validate| validate.groups.clone())
        .unwrap_or_default();
    format!(
        "_c('validate',{{props:{{field:{},groups:{},validators:{},result:{},child:{child_code}}}}})",
        js_string(&field),
        json_string_array(&groups),
        ast_validators_json(&validation.validators),
        ast_validation_result_json(&validation.validators)
    )
}

#[derive(Clone, Copy)]
enum PropValueKind {
    StaticAttribute,
    Expression,
}

fn gen_key_filter(keys: &[String]) -> String {
    format!(
        "if(!$event.type.indexOf('key')&&{})return null;",
        keys.iter()
            .map(|key| {
                key.parse::<u32>().map_or_else(
                    |_| match key.as_str() {
                        "tab" => "_k($event.keyCode,\"tab\",9,$event.key,\"Tab\")".into(),
                        "enter" => "_k($event.keyCode,\"enter\",13,$event.key,\"Enter\")".into(),
                        "delete" => "_k($event.keyCode,\"delete\",[8,46],$event.key,[\"Backspace\",\"Delete\",\"Del\"])".into(),
                        "esc" => "_k($event.keyCode,\"esc\",27,$event.key,[\"Esc\",\"Escape\"])".into(),
                        "space" => "_k($event.keyCode,\"space\",32,$event.key,[\" \",\"Spacebar\"])".into(),
                        "up" => "_k($event.keyCode,\"up\",38,$event.key,[\"Up\",\"ArrowUp\"])".into(),
                        "left" => "_k($event.keyCode,\"left\",37,$event.key,[\"Left\",\"ArrowLeft\"])".into(),
                        "right" => "_k($event.keyCode,\"right\",39,$event.key,[\"Right\",\"ArrowRight\"])".into(),
                        "down" => "_k($event.keyCode,\"down\",40,$event.key,[\"Down\",\"ArrowDown\"])".into(),
                        _ => format!("_k($event.keyCode,{},{},$event.key,{})", js_string(key), "undefined", "undefined"),
                    },
                    |code| format!("$event.keyCode!=={code}"),
                )
            })
            .collect::<Vec<_>>()
            .join("&&")
    )
}

fn validate_expressions(root: Option<&Vue2Element>, diagnostics: &mut DiagnosticSink) {
    let Some(root) = root else {
        return;
    };
    validate_element_expressions(root, diagnostics);
}

fn validate_element_expressions(element: &Vue2Element, diagnostics: &mut DiagnosticSink) {
    for (raw, expr, span) in [
        ("v-if", element.if_exp.as_deref(), element.if_span),
        ("v-for", element.for_exp.as_deref(), element.for_span),
    ] {
        if let Some(expr) = expr {
            if is_invalid_js_expression(expr) {
                diagnostics.push(vue2_error(
                    "E_VUE2_INVALID_EXPRESSION",
                    format!("Raw expression: {raw}=\"{expr}\""),
                    span.or(element.span),
                ));
            }
        }
    }
    for attr in element
        .attrs
        .iter()
        .chain(element.props.iter())
        .chain(element.dynamic_attrs.iter())
    {
        if attr.value.trim().is_empty() || attr.value.starts_with('"') {
            continue;
        }
        if is_invalid_js_expression(&attr.value) {
            diagnostics.push(vue2_error(
                "E_VUE2_INVALID_EXPRESSION",
                format!("Raw expression: {}=\"{}\"", attr.name, attr.value),
                attr.span,
            ));
        }
    }
    for child in &element.children {
        match child {
            Vue2Node::Element(child) => validate_element_expressions(child, diagnostics),
            Vue2Node::Text(text) => {
                if let Some(expression) = &text.expression {
                    if is_invalid_js_expression(expression) {
                        diagnostics.push(vue2_error(
                            "E_VUE2_INVALID_EXPRESSION",
                            format!("Raw expression: {}", text.text),
                            text.span,
                        ));
                    }
                }
            }
        }
    }
    for condition in element.if_conditions.iter().skip(1) {
        validate_element_expressions(&condition.block, diagnostics);
    }
}

fn is_invalid_js_expression(expr: &str) -> bool {
    let expr = expr.trim();
    expr.contains("----") || expr.contains("++++")
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParsedFor {
    for_exp: String,
    alias: String,
    iterator1: Option<String>,
    iterator2: Option<String>,
}

fn parse_for(exp: &str) -> Option<ParsedFor> {
    let (alias, for_exp) = split_for_expression(exp)?;
    let alias = strip_parens(alias.trim());
    let parts = split_top_level(alias, ',');
    Some(ParsedFor {
        for_exp: for_exp.trim().to_string(),
        alias: parts.first().copied().unwrap_or(alias).trim().to_string(),
        iterator1: parts.get(1).map(|part| part.trim().to_string()),
        iterator2: parts.get(2).map(|part| part.trim().to_string()),
    })
}

fn split_for_expression(source: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 => {
                let rest = &source[index..];
                if rest.starts_with(" in ") {
                    return Some((&source[..index], &source[index + 4..]));
                }
                if rest.starts_with(" of ") {
                    return Some((&source[..index], &source[index + 4..]));
                }
            }
            _ => {}
        }
    }
    None
}

fn strip_parens(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        trimmed[1..trimmed.len() - 1].trim()
    } else {
        trimmed
    }
}

fn parse_text(text: &str, delimiters: Option<&[String; 2]>) -> Option<String> {
    let (open, close) =
        delimiters.map_or(("{{", "}}"), |items| (items[0].as_str(), items[1].as_str()));
    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    while let Some(open_offset) = text[cursor..].find(open) {
        let open_index = cursor + open_offset;
        if open_index > cursor {
            tokens.push(js_string(&text[cursor..open_index]));
        }
        let expression_start = open_index + open.len();
        let Some(close_offset) = text[expression_start..].find(close) else {
            return None;
        };
        let close_index = expression_start + close_offset;
        let expression = parse_filters(text[expression_start..close_index].trim());
        tokens.push(format!("_s({expression})"));
        cursor = close_index + close.len();
    }
    if cursor == 0 {
        return None;
    }
    if cursor < text.len() {
        tokens.push(js_string(&text[cursor..]));
    }
    Some(tokens.join("+"))
}

fn parse_filters(exp: &str) -> String {
    rewrite_vue2_filter_expression(exp)
}

fn project_public_ast(
    template: &str,
    element_ast: Option<&Vue2Element>,
) -> Vue2AstProjectionResult {
    let mut projection = Vue2AstProjection {
        ast: Vue2Ast::with_capacity(
            Vue2NodeKind::root(),
            Some(Span::new(FileId(0), 0, template.len())),
            vuec_ast::template_node_capacity_hint(template),
        ),
        js: JsAstStore::new(),
        source_type: SourceType::script(),
    };
    let root = projection.ast.root;
    if let Some(element) = element_ast {
        projection.project_element(root, element);
    }
    Vue2AstProjectionResult {
        ast: projection.ast,
        js: projection.js,
    }
}

struct Vue2AstProjection {
    ast: Vue2Ast,
    js: JsAstStore,
    source_type: SourceType,
}

impl Vue2AstProjection {
    fn project_element(&mut self, parent: NodeId, element: &Vue2Element) -> NodeId {
        let payload = self.project_element_payload(element);
        let id = self.ast.push(Vue2AstKind::Element(payload), element.span);
        self.ast.attach_child(parent, id);

        for child in &element.children {
            match child {
                Vue2Node::Element(element) => {
                    self.project_element(id, element);
                }
                Vue2Node::Text(text) => {
                    self.project_text(id, text);
                }
            }
        }

        let scoped_slots = element
            .scoped_slots
            .iter()
            .map(|(name, slot)| (name.clone(), self.project_element(id, slot)))
            .collect::<BTreeMap<_, _>>();
        let if_conditions = self.project_if_conditions(id, element);
        if let Some(node) = self.ast.node_mut(id) {
            if let Vue2AstKind::Element(projected) = &mut node.kind {
                projected.scoped_slots = scoped_slots;
                projected.if_conditions = if_conditions;
            }
        }
        id
    }

    fn project_text(&mut self, parent: NodeId, text: &Vue2Text) -> NodeId {
        let kind = if text.is_comment {
            Vue2AstKind::Comment(vuec_ast::Vue2Comment {
                value: text.text.clone(),
            })
        } else if let Some(expression) = text.expression.as_ref() {
            Vue2AstKind::ExpressionText(vuec_ast::Vue2ExpressionText {
                raw: text.text.clone(),
                expr: Some(self.register_expr(expression, text.span)),
                filter_expr: self.project_filter_expression(&text.text, text.span),
            })
        } else {
            Vue2AstKind::Text(vuec_ast::Vue2Text {
                value: text.text.clone(),
                static_node: text.static_node,
            })
        };
        self.ast.push_child(parent, kind, text.span)
    }

    fn project_if_conditions(
        &mut self,
        primary_id: NodeId,
        element: &Vue2Element,
    ) -> Vec<vuec_ast::Vue2IfCondition> {
        if element.if_conditions.is_empty() {
            return element
                .if_exp
                .as_ref()
                .map(|condition| {
                    vec![vuec_ast::Vue2IfCondition {
                        exp: Some(self.register_expr(condition, element.if_span.or(element.span))),
                        block: primary_id,
                        span: element.if_span,
                    }]
                })
                .unwrap_or_default();
        }

        element
            .if_conditions
            .iter()
            .enumerate()
            .map(|(index, condition)| {
                let block = if index == 0 {
                    primary_id
                } else {
                    self.project_element(primary_id, &condition.block)
                };
                vuec_ast::Vue2IfCondition {
                    exp: condition.exp.as_ref().map(|exp| {
                        self.register_expr(
                            exp,
                            condition
                                .block
                                .if_span
                                .or(condition.block.elseif_span)
                                .or(element.span),
                        )
                    }),
                    block,
                    span: condition
                        .block
                        .if_span
                        .or(condition.block.elseif_span)
                        .or(condition.block.else_span),
                }
            })
            .collect()
    }

    fn project_element_payload(&mut self, element: &Vue2Element) -> vuec_ast::Vue2Element {
        let mut payload = vuec_ast::Vue2Element::new(element.tag.clone());
        payload.attrs_list = element.attrs_list.iter().map(project_attribute).collect();
        payload.attrs_map = element.attrs_map.clone();
        payload.raw_attrs_map = element
            .raw_attrs_map
            .iter()
            .map(|(name, attr)| (name.clone(), project_attribute(attr)))
            .collect();
        payload.attrs = element.attrs.iter().map(project_attribute).collect();
        payload.props = element.props.iter().map(project_attribute).collect();
        payload.dynamic_attrs = element
            .dynamic_attrs
            .iter()
            .map(project_attribute)
            .collect();
        payload.directives = element
            .directives
            .iter()
            .map(|directive| self.project_directive(directive))
            .collect();
        payload.events = self.project_events(&element.events);
        payload.native_events = self.project_events(&element.native_events);
        payload.ns = element.ns.clone();
        payload.plain = element.plain;
        payload.forbidden = element.forbidden;
        payload.pre = element.pre;
        payload.once = element.once;
        payload.has_bindings = element.has_bindings;
        payload.if_exp = element
            .if_exp
            .as_ref()
            .map(|exp| self.register_expr(exp, element.if_span.or(element.span)));
        payload.if_span = element.if_span;
        payload.elseif = element
            .elseif
            .as_ref()
            .map(|exp| self.register_expr(exp, element.elseif_span.or(element.span)));
        payload.elseif_span = element.elseif_span;
        payload.else_branch = element.else_branch;
        payload.else_span = element.else_span;
        payload.for_exp = element
            .for_exp
            .as_ref()
            .map(|exp| self.register_expr(exp, element.for_span.or(element.span)));
        payload.for_span = element.for_span;
        payload.alias = element
            .alias
            .as_ref()
            .map(|alias| self.register_pattern(alias, element.for_span.or(element.span)));
        payload.iterator1 = element
            .iterator1
            .as_ref()
            .map(|alias| self.register_pattern(alias, element.for_span.or(element.span)));
        payload.iterator2 = element
            .iterator2
            .as_ref()
            .map(|alias| self.register_pattern(alias, element.for_span.or(element.span)));
        payload.key = element
            .key
            .as_ref()
            .map(|key| self.register_expr(key, element.key_span.or(element.span)));
        payload.key_span = element.key_span;
        payload.ref_name = element.ref_name.clone();
        payload.ref_in_for = element.ref_in_for;
        payload.slot_name = element.slot_name.clone();
        payload.slot_target = element.slot_target.clone();
        payload.slot_target_dynamic = element.slot_target_dynamic;
        payload.slot_scope = element
            .slot_scope
            .as_ref()
            .map(|scope| self.register_pattern(scope, element.span));
        payload.slot_new_syntax = element.slot_new_syntax;
        payload.component = element.component.clone();
        payload.inline_template = element.inline_template;
        payload.static_class = element.static_class.clone();
        payload.class_binding = element
            .class_binding
            .as_ref()
            .map(|binding| self.register_expr(binding, element.span));
        payload.static_style = element.static_style.clone();
        payload.style_binding = element
            .style_binding
            .as_ref()
            .map(|binding| self.register_expr(binding, element.span));
        payload.model = element
            .model
            .as_ref()
            .map(|model| vuec_ast::Vue2ComponentModel {
                value: self.register_expr(&model.value, element.span),
                callback: self.register_stmt(&model.callback, element.span),
                expression: model.expression.clone(),
            });
        payload.wrap_data = element.wrap_data.as_ref().map(|wrap| match wrap {
            Vue2DataWrap::Bind { value, prop, sync } => vuec_ast::Vue2DataWrap::Bind {
                value: self.register_expr(value, element.span),
                prop: *prop,
                sync: *sync,
            },
        });
        payload.wrap_listeners = element.wrap_listeners.clone();
        payload.validate = element
            .validate
            .as_ref()
            .map(|validate| vuec_ast::Vue2Validation {
                field: validate.field.clone(),
                groups: validate.groups.clone(),
            });
        payload.validators = element
            .validators
            .iter()
            .map(|validator| vuec_ast::Vue2Validator {
                name: validator.name.clone(),
                rule: validator.rule.clone(),
            })
            .collect();
        payload.static_node = element.static_node;
        payload.static_root = element.static_root;
        payload.static_in_for = element.static_in_for;
        payload
    }

    fn project_directive(&mut self, directive: &Vue2Directive) -> vuec_ast::Vue2Directive {
        vuec_ast::Vue2Directive {
            name: directive.name.clone(),
            raw_name: directive.raw_name.clone(),
            value: directive
                .value
                .as_ref()
                .map(|value| self.register_expr(value, directive.span)),
            arg: directive.arg.clone(),
            is_dynamic_arg: directive.is_dynamic_arg,
            modifiers: directive.modifiers.clone(),
        }
    }

    fn project_events(
        &mut self,
        events: &BTreeMap<String, Vec<Vue2EventHandler>>,
    ) -> BTreeMap<String, Vec<vuec_ast::Vue2EventHandler>> {
        events
            .iter()
            .map(|(name, handlers)| {
                (
                    name.clone(),
                    handlers
                        .iter()
                        .map(|handler| vuec_ast::Vue2EventHandler {
                            value: self.register_stmt(&handler.value, handler.span),
                            modifiers: handler.modifiers.clone(),
                            modifier_order: handler.modifier_order.clone(),
                            has_modifier_object: handler.has_modifier_object,
                            dynamic: handler.dynamic,
                            span: handler.span,
                        })
                        .collect(),
                )
            })
            .collect()
    }

    fn project_filter_expression(
        &mut self,
        raw_text: &str,
        span: Option<Span>,
    ) -> Option<vuec_ast::Vue2FilterExpr> {
        let source = single_default_interpolation(raw_text)?;
        let parsed = parse_vue2_filter_expression(source);
        if parsed.filters.is_empty() {
            return None;
        }
        Some(vuec_ast::Vue2FilterExpr {
            raw: source.to_string(),
            base: self.register_expr(parsed.base, span),
            filters: parsed
                .filters
                .into_iter()
                .map(|filter| vuec_ast::Vue2FilterCall {
                    name: filter.name.to_string(),
                    args: filter
                        .args
                        .into_iter()
                        .map(|arg| self.register_expr(arg, span))
                        .collect(),
                })
                .collect(),
        })
    }

    fn register_expr(&mut self, source: &str, span: Option<Span>) -> JsExprId {
        self.js
            .register_expr(source, js_span(span), self.source_type)
    }

    fn register_stmt(&mut self, source: &str, span: Option<Span>) -> JsStmtId {
        self.js
            .register_stmt(source, js_span(span), self.source_type)
    }

    fn register_pattern(&mut self, source: &str, span: Option<Span>) -> JsPatternId {
        self.js
            .register_pattern(source, js_span(span), self.source_type)
    }
}

fn project_attribute(attr: &Vue2Attribute) -> vuec_ast::Vue2Attribute {
    vuec_ast::Vue2Attribute {
        name: attr.name.clone(),
        value: attr.value.clone(),
        span: attr.span,
        dynamic: attr.dynamic,
    }
}

struct Vue2LoweringState {
    hir: Hir,
    mir: Vue2Mir,
    map: LoweringMap,
    js: JsAstStore,
    static_render_index: u32,
    once_id: u32,
}

fn lower_vue2_child_sequence(
    children: &[NodeId],
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Vec<(NodeId, NodeId, NodeId)> {
    let mut lowered = Vec::new();
    for child in children {
        if let Some((hir, mir)) =
            lower_vue2_ast_node_to_mir(*child, ast, hir_parent, mir_parent, state)
        {
            lowered.push((*child, hir, mir));
        }
    }
    lowered
}

fn lower_vue2_ast_node_to_mir(
    ast_id: NodeId,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Option<(NodeId, NodeId)> {
    lower_vue2_ast_node_to_mir_inner(ast_id, ast, hir_parent, mir_parent, state, true)
}

fn lower_vue2_ast_node_to_mir_inner(
    ast_id: NodeId,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
    allow_for: bool,
) -> Option<(NodeId, NodeId)> {
    let ast_node = ast.node(ast_id)?;
    match &ast_node.kind {
        Vue2AstKind::Root(_) => None,
        Vue2AstKind::Element(element) => {
            if allow_for && element.for_exp.is_some() {
                return lower_vue2_for_to_mir(
                    ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
                );
            }
            if element.if_exp.is_some() && !element.if_conditions.is_empty() {
                return lower_vue2_if_to_mir(
                    ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
                );
            }
            if element.elseif.is_some() || element.else_branch {
                return None;
            }
            lower_vue2_plain_element_to_mir(
                ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
            )
        }
        Vue2AstKind::Text(text) => {
            let hir_id = state.hir.push_child(
                hir_parent,
                HirNodeKind::Text(vuec_ast::HirText {
                    value: text.value.clone(),
                }),
                ast_node.span.clone(),
            );
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue2MirKind::Text(Vue2TextCall {
                    value: MirExpr::String(text.value.clone()),
                }),
                ast_node.span.clone(),
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some((hir_id, mir_id))
        }
        Vue2AstKind::ExpressionText(text) => {
            let expression = text
                .filter_expr
                .as_ref()
                .map(|filter| HirExpr::Vue2Filter(filter.clone()))
                .or_else(|| text.expr.map(HirExpr::Js));
            let hir_id = state.hir.push_child(
                hir_parent,
                HirNodeKind::Interpolation(HirInterpolation {
                    expression: expression.unwrap_or(HirExpr::Js(JsExprId(0))),
                }),
                ast_node.span.clone(),
            );
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue2MirKind::Text(Vue2TextCall {
                    value: text
                        .filter_expr
                        .as_ref()
                        .map(|filter| MirExpr::JsExpr(filter.base))
                        .or_else(|| text.expr.map(MirExpr::JsExpr))
                        .unwrap_or_else(|| MirExpr::String(text.raw.clone())),
                }),
                ast_node.span.clone(),
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            if let Some(filter) = &text.filter_expr {
                for call in &filter.filters {
                    let filter_id = state.mir.push_child(
                        mir_id,
                        Vue2MirKind::FilterCall {
                            name: call.name.clone(),
                            args: call.args.clone(),
                        },
                        NodeSpan::generated(ast_node.span.source(), GeneratedReason::Lowering),
                    );
                    state.map.record_hir_to_mir(hir_id, filter_id);
                }
            }
            Some((hir_id, mir_id))
        }
        Vue2AstKind::Comment(comment) => {
            let span = NodeSpan::generated(ast_node.span.source(), GeneratedReason::Lowering);
            let hir_id =
                state
                    .hir
                    .push_child(hir_parent, HirNodeKind::Fragment(HirFragment), span.clone());
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue2MirKind::Comment {
                    value: comment.value.clone(),
                },
                span,
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some((hir_id, mir_id))
        }
    }
}

fn lower_vue2_for_to_mir(
    ast_id: NodeId,
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Option<(NodeId, NodeId)> {
    let source = element.for_exp?;
    let alias = element.alias.unwrap_or_else(|| {
        state
            .js
            .register_pattern("item", ast_node_span(ast_node), SourceType::script())
    });
    let hir_id = state.hir.push_child(
        hir_parent,
        HirNodeKind::For(HirFor {
            source,
            value_alias: alias,
            key_alias: element.iterator1,
            index_alias: element.iterator2,
            body: NodeId(0),
        }),
        ast_node.span.clone(),
    );
    let mir_id = state.mir.push_child(
        mir_parent,
        Vue2MirKind::For(Vue2ForMir {
            source,
            alias,
            iterator1: element.iterator1,
            iterator2: element.iterator2,
            body: NodeId(0),
        }),
        ast_node.span.clone(),
    );
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);
    let (body_hir, body_mir) =
        lower_vue2_ast_node_to_mir_inner(ast_id, ast, hir_id, mir_id, state, false)?;
    if let Some(node) = state.hir.node_mut(hir_id) {
        if let HirNodeKind::For(for_node) = &mut node.kind {
            for_node.body = body_hir;
        }
    }
    if let Some(node) = state.mir.node_mut(mir_id) {
        if let Vue2MirKind::For(for_node) = &mut node.kind {
            for_node.body = body_mir;
        }
    }
    Some((hir_id, mir_id))
}

fn lower_vue2_if_to_mir(
    ast_id: NodeId,
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Option<(NodeId, NodeId)> {
    let hir_id = state.hir.push_child(
        hir_parent,
        HirNodeKind::If(HirIf {
            branches: Vec::new(),
        }),
        ast_node.span.clone(),
    );
    let mir_id = state.mir.push_child(
        mir_parent,
        Vue2MirKind::If(Vue2IfMir {
            branches: Vec::new(),
        }),
        ast_node.span.clone(),
    );
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);

    let mut branches = Vec::new();
    let mut mir_branches = Vec::new();
    for condition in &element.if_conditions {
        let block = condition.block;
        let (body_hir, body_mir) = if block == ast_id {
            lower_vue2_plain_element_to_mir(ast_id, element, ast, ast_node, hir_id, mir_id, state)?
        } else {
            lower_vue2_branch_block_to_mir(block, ast, hir_id, mir_id, state)?
        };
        branches.push(HirIfBranch {
            condition: condition.exp,
            body: body_hir,
        });
        mir_branches.push(Vue2IfMirBranch {
            condition: condition.exp,
            body: body_mir,
        });
    }
    if let Some(node) = state.hir.node_mut(hir_id) {
        if let HirNodeKind::If(if_node) = &mut node.kind {
            if_node.branches = branches;
        }
    }
    if let Some(node) = state.mir.node_mut(mir_id) {
        if let Vue2MirKind::If(if_node) = &mut node.kind {
            if_node.branches = mir_branches;
        }
    }
    Some((hir_id, mir_id))
}

fn lower_vue2_branch_block_to_mir(
    block: NodeId,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Option<(NodeId, NodeId)> {
    let ast_node = ast.node(block)?;
    match &ast_node.kind {
        Vue2AstKind::Element(element) => lower_vue2_plain_element_to_mir(
            block, element, ast, ast_node, hir_parent, mir_parent, state,
        ),
        _ => lower_vue2_ast_node_to_mir_inner(block, ast, hir_parent, mir_parent, state, false),
    }
}

fn lower_vue2_plain_element_to_mir(
    ast_id: NodeId,
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Option<(NodeId, NodeId)> {
    let hir_kind = lower_vue2_element_to_hir_kind(element, ast_node, state);
    let render_static = if element.static_root || (element.once && !element.static_in_for) {
        let index = state.static_render_index;
        state.static_render_index += 1;
        Some(Vue2RenderStatic {
            index,
            body: None,
            in_for: element.static_in_for,
        })
    } else {
        None
    };
    let hir_id = state
        .hir
        .push_child(hir_parent, hir_kind, ast_node.span.clone());
    state.map.record_ast_to_hir(ast_id, hir_id);

    let wrapper_mir = if let Some(render_static) = render_static {
        Some(state.mir.push_child(
            mir_parent,
            Vue2MirKind::RenderStatic(render_static),
            ast_node.span.clone(),
        ))
    } else if element.once && element.static_in_for {
        let once_id = state.once_id;
        state.once_id += 1;
        Some(state.mir.push_child(
            mir_parent,
            Vue2MirKind::Once(Vue2Once {
                body: NodeId(0),
                once_id,
                key: element.key.map(MirExpr::JsExpr),
            }),
            ast_node.span.clone(),
        ))
    } else {
        None
    };
    let content_parent = wrapper_mir.unwrap_or(mir_parent);
    if let Some(wrapper) = wrapper_mir {
        state.map.record_hir_to_mir(hir_id, wrapper);
    }

    let mir_kind = lower_vue2_element_to_mir_kind(element, ast_node, state);
    let mir_id = state
        .mir
        .push_child(content_parent, mir_kind, ast_node.span.clone());
    state.map.record_hir_to_mir(hir_id, mir_id);

    let branch_blocks = element
        .if_conditions
        .iter()
        .skip(1)
        .map(|condition| condition.block)
        .collect::<Vec<_>>();
    let children = if element.inline_template {
        Vec::new()
    } else {
        ast_node
            .children
            .iter()
            .copied()
            .filter(|child| {
                !branch_blocks.contains(child)
                    && !element.scoped_slots.values().any(|slot| slot == child)
            })
            .collect::<Vec<_>>()
    };
    lower_vue2_child_sequence(&children, ast, hir_id, mir_id, state);
    lower_vue2_inline_template_to_mir(element, ast, ast_node, hir_id, mir_id, state);
    lower_vue2_scoped_slots_to_mir(element, ast, hir_id, mir_id, state);

    if let Some(wrapper) = wrapper_mir {
        if let Some(node) = state.mir.node_mut(wrapper) {
            match &mut node.kind {
                Vue2MirKind::RenderStatic(render_static) => {
                    render_static.body = Some(mir_id);
                }
                Vue2MirKind::Once(once) => {
                    once.body = mir_id;
                }
                _ => {}
            }
        }
    }
    Some((hir_id, wrapper_mir.unwrap_or(mir_id)))
}

fn lower_vue2_element_to_hir_kind(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> HirNodeKind {
    if element.tag == "slot" {
        return HirNodeKind::SlotOutlet(HirSlotOutlet {
            name: element.slot_name.clone(),
            props: lower_vue2_props_to_hir(element, ast_node, state),
        });
    }

    let props = lower_vue2_props_to_hir(element, ast_node, state);
    if let Some(component) = &element.component {
        return HirNodeKind::Component(vuec_ast::HirComponent {
            name: component.clone(),
            props,
        });
    }

    HirNodeKind::Element(HirElement {
        tag: HirTag::Native(element.tag.clone()),
        namespace: vue2_namespace(element.ns.as_deref()),
        props,
        directives: lower_vue2_directives_to_hir(element, ast_node, state),
        constness: if element.static_root {
            HirConstness::Constant
        } else if element.static_node {
            HirConstness::Static
        } else {
            HirConstness::Dynamic
        },
    })
}

fn lower_vue2_element_to_mir_kind(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Vue2MirKind {
    if element.tag == "slot" {
        return Vue2MirKind::SlotOutlet(Vue2SlotOutlet {
            name: element
                .slot_name
                .as_ref()
                .map(|name| {
                    MirExpr::JsExpr(state.js.register_expr(
                        name,
                        ast_node_span(ast_node),
                        SourceType::script(),
                    ))
                })
                .unwrap_or_else(|| MirExpr::String("default".into())),
            props: lower_vue2_slot_outlet_props(element, ast_node, state),
            bind: lower_vue2_slot_outlet_bind(element, ast_node, state),
        });
    }

    let explicit_component = element.component.is_some();
    let tag = element
        .component
        .as_ref()
        .map(|component| {
            MirExpr::JsExpr(state.js.register_expr(
                component,
                ast_node_span(ast_node),
                SourceType::script(),
            ))
        })
        .unwrap_or_else(|| MirExpr::String(element.tag.clone()));

    Vue2MirKind::CreateElement(Vue2CreateElement {
        tag,
        data: lower_vue2_data_object(element, ast_node, state),
        is_component: explicit_component,
        is_template: element.tag == "template" && element.slot_target.is_none(),
        validation: (!element.validators.is_empty() || element.validate.is_some()).then(|| {
            Vue2ValidationData {
                validate: element.validate.clone(),
                validators: element.validators.clone(),
            }
        }),
        normalization_type: Vue2NormalizationType::None,
    })
}

fn lower_vue2_data_object(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Option<Vue2DataObject> {
    let mut data = Vue2DataObject::default();
    data.directives = element
        .directives
        .iter()
        .map(|directive| Vue2DirectiveRuntime {
            name: directive.name.clone(),
            raw_name: directive.raw_name.clone(),
            value: directive.value.map(MirExpr::JsExpr),
            arg: directive.arg.as_ref().map(|arg| {
                if directive.is_dynamic_arg {
                    MirExpr::JsExpr(state.js.register_expr(
                        arg,
                        ast_node_span(ast_node),
                        SourceType::script(),
                    ))
                } else {
                    MirExpr::String(arg.clone())
                }
            }),
            is_dynamic_arg: directive.is_dynamic_arg,
            modifiers: directive.modifiers.clone(),
        })
        .collect();
    data.key = element.key.map(MirExpr::JsExpr);
    data.ref_name = element.ref_name.as_ref().map(|name| {
        MirExpr::JsExpr(
            state
                .js
                .register_expr(name, ast_node_span(ast_node), SourceType::script()),
        )
    });
    data.ref_in_for = element.ref_in_for;
    data.pre = element.pre;
    data.tag = element.component.as_ref().map(|_| element.tag.clone());
    data.static_class = element.static_class.as_ref().map(|value| {
        MirExpr::JsExpr(state.js.register_expr(
            value,
            ast_node_span(ast_node),
            SourceType::script(),
        ))
    });
    data.class_binding = element.class_binding.map(MirExpr::JsExpr);
    data.static_style = element.static_style.as_ref().map(|value| {
        MirExpr::JsExpr(state.js.register_expr(
            value,
            ast_node_span(ast_node),
            SourceType::script(),
        ))
    });
    data.style_binding = element.style_binding.map(MirExpr::JsExpr);
    data.attrs = lower_vue2_data_props(&element.attrs, ast_node, state);
    data.dom_props = lower_vue2_data_props(&element.props, ast_node, state);
    data.dynamic_attrs = lower_vue2_data_props(&element.dynamic_attrs, ast_node, state);
    data.events = element.events.clone();
    data.native_events = element.native_events.clone();
    if element.slot_scope.is_none() {
        data.slot = element.slot_target.as_ref().map(|slot| {
            MirExpr::JsExpr(state.js.register_expr(
                slot,
                ast_node_span(ast_node),
                SourceType::script(),
            ))
        });
    }
    data.model = element.model.as_ref().map(|model| Vue2ComponentModelMir {
        value: MirExpr::JsExpr(model.value),
        callback: model.callback,
        expression: model.expression.clone(),
    });
    data.validate = element.validate.clone();
    data.validators = element.validators.clone();
    data.wrap_data = element.wrap_data.as_ref().map(|wrap| match wrap {
        vuec_ast::Vue2DataWrap::Bind { value, prop, sync } => Vue2BindWrap {
            value: MirExpr::JsExpr(*value),
            prop: *prop,
            sync: *sync,
        },
    });
    data.wrap_listeners = element.wrap_listeners.as_ref().map(|listeners| {
        MirExpr::JsExpr(state.js.register_expr(
            listeners,
            ast_node_span(ast_node),
            SourceType::script(),
        ))
    });

    (!data.directives.is_empty()
        || data.key.is_some()
        || data.ref_name.is_some()
        || data.ref_in_for
        || data.pre
        || data.tag.is_some()
        || data.static_class.is_some()
        || data.class_binding.is_some()
        || data.static_style.is_some()
        || data.style_binding.is_some()
        || !data.attrs.is_empty()
        || !data.dom_props.is_empty()
        || !data.dynamic_attrs.is_empty()
        || !data.events.is_empty()
        || !data.native_events.is_empty()
        || data.slot.is_some()
        || !data.scoped_slots.is_empty()
        || data.model.is_some()
        || data.inline_template.is_some()
        || data.validate.is_some()
        || !data.validators.is_empty()
        || data.wrap_data.is_some()
        || data.wrap_listeners.is_some()
        || element.slot_scope.is_some()
        || element.inline_template)
        .then_some(data)
}

fn lower_vue2_data_props(
    attrs: &[vuec_ast::Vue2Attribute],
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Vec<Vue2DataProp> {
    attrs
        .iter()
        .map(|attr| Vue2DataProp {
            name: attr.name.clone(),
            value: MirExpr::JsExpr(state.js.register_expr(
                attr.value.clone(),
                attr.span.unwrap_or_else(|| ast_node_span(ast_node)),
                SourceType::script(),
            )),
            dynamic: attr.dynamic,
            static_attribute: !attr.dynamic && attr.value.trim_start().starts_with('"'),
        })
        .collect()
}

fn lower_vue2_slot_outlet_props(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Vec<Vue2DataProp> {
    element
        .attrs
        .iter()
        .chain(element.dynamic_attrs.iter())
        .map(|attr| Vue2DataProp {
            name: camelize(&attr.name),
            value: MirExpr::JsExpr(state.js.register_expr(
                attr.value.clone(),
                attr.span.unwrap_or_else(|| ast_node_span(ast_node)),
                SourceType::script(),
            )),
            dynamic: attr.dynamic,
            static_attribute: !attr.dynamic && attr.value.trim_start().starts_with('"'),
        })
        .collect()
}

fn lower_vue2_slot_outlet_bind(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Option<MirExpr> {
    let value = element.attrs_map.get("v-bind")?;
    let span = element
        .raw_attrs_map
        .get("v-bind")
        .and_then(|attr| attr.span)
        .unwrap_or_else(|| ast_node_span(ast_node));
    Some(MirExpr::JsExpr(state.js.register_expr(
        value.clone(),
        span,
        SourceType::script(),
    )))
}

fn lower_vue2_inline_template_to_mir(
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    hir_parent: NodeId,
    mir_id: NodeId,
    state: &mut Vue2LoweringState,
) {
    if !element.inline_template {
        return;
    }
    let body = ast_node.children.iter().copied().find(|child| {
        ast.node(*child)
            .is_some_and(|node| matches!(node.kind, Vue2AstKind::Element(_)))
    });
    let lowered_body = body.and_then(|body| {
        lower_vue2_ast_node_to_mir_inner(body, ast, hir_parent, mir_id, state, true)
            .map(|(_, mir)| mir)
    });
    if let Some(Vue2MirKind::CreateElement(create)) =
        state.mir.node_mut(mir_id).map(|node| &mut node.kind)
    {
        let data = create.data.get_or_insert_with(Vue2DataObject::default);
        data.inline_template = Some(Vue2InlineTemplate { body: lowered_body });
    }
}

fn lower_vue2_scoped_slots_to_mir(
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_id: NodeId,
    state: &mut Vue2LoweringState,
) {
    if element.scoped_slots.is_empty() {
        return;
    }

    let mut slots = Vec::new();
    for (key, slot_id) in &element.scoped_slots {
        let Some(slot_node) = ast.node(*slot_id) else {
            continue;
        };
        let Vue2AstKind::Element(slot) = &slot_node.kind else {
            continue;
        };
        let slot_mir_id = state.mir.push_child(
            mir_id,
            Vue2MirKind::ScopedSlot(Vue2ScopedSlot {
                name: slot
                    .slot_target
                    .as_ref()
                    .map(|target| {
                        MirExpr::JsExpr(state.js.register_expr(
                            target,
                            ast_node_span(slot_node),
                            SourceType::script(),
                        ))
                    })
                    .unwrap_or_else(|| {
                        MirExpr::JsExpr(state.js.register_expr(
                            key,
                            ast_node_span(slot_node),
                            SourceType::script(),
                        ))
                    }),
                params: slot.slot_scope,
                body: Vec::new(),
                proxy: slot
                    .slot_scope
                    .and_then(|scope| state.js.pattern_entry(scope))
                    .is_none_or(|entry| entry.source.as_str() == "_empty_"),
                new_syntax: slot.slot_new_syntax,
                body_is_fragment: slot.tag == "template",
                condition: slot.if_exp.filter(|_| slot.slot_new_syntax),
                legacy_condition: slot
                    .if_exp
                    .filter(|_| !slot.slot_new_syntax && slot.tag == "template"),
                for_source: slot.for_exp,
                for_alias: slot.alias,
                for_iterator1: slot.iterator1,
                for_iterator2: slot.iterator2,
                force_update: false,
            }),
            slot_node.span.clone(),
        );

        let body =
            lower_vue2_scoped_slot_body_to_mir(*slot_id, slot, ast, hir_parent, slot_mir_id, state);
        if let Some(node) = state.mir.node_mut(slot_mir_id) {
            if let Vue2MirKind::ScopedSlot(slot_payload) = &mut node.kind {
                slot_payload.body = body;
                slot_payload.force_update = element.for_exp.is_some()
                    || slot.slot_target_dynamic
                    || slot.if_exp.is_some()
                    || slot.for_exp.is_some()
                    || vue2_ast_contains_slot_child(*slot_id, ast);
                slots.push(slot_payload.clone());
            }
        }
    }

    let force_update = slots.iter().any(|slot| slot.force_update);
    for slot in &mut slots {
        slot.force_update = force_update;
    }
    if let Some(Vue2MirKind::CreateElement(create)) =
        state.mir.node_mut(mir_id).map(|node| &mut node.kind)
    {
        let data = create.data.get_or_insert_with(Vue2DataObject::default);
        data.scoped_slots = slots;
    }
}

fn lower_vue2_scoped_slot_body_to_mir(
    slot_id: NodeId,
    slot: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Vec<NodeId> {
    if slot.tag == "template" {
        let branch_blocks = slot
            .if_conditions
            .iter()
            .skip(1)
            .map(|condition| condition.block)
            .collect::<Vec<_>>();
        let children = ast
            .node(slot_id)
            .map(|node| {
                node.children
                    .iter()
                    .copied()
                    .filter(|child| !branch_blocks.contains(child))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        return lower_vue2_child_sequence(&children, ast, hir_parent, mir_parent, state)
            .into_iter()
            .map(|(_, _, mir)| mir)
            .collect();
    }

    lower_vue2_ast_node_to_mir_inner(slot_id, ast, hir_parent, mir_parent, state, false)
        .map(|(_, mir)| vec![mir])
        .unwrap_or_default()
}

fn vue2_ast_contains_slot_child(ast_id: NodeId, ast: &Vue2Ast) -> bool {
    let Some(node) = ast.node(ast_id) else {
        return false;
    };
    match &node.kind {
        Vue2AstKind::Element(element) if element.tag == "slot" => true,
        Vue2AstKind::Element(_) | Vue2AstKind::Root(_) => node
            .children
            .iter()
            .any(|child| vue2_ast_contains_slot_child(*child, ast)),
        Vue2AstKind::Text(_) | Vue2AstKind::ExpressionText(_) | Vue2AstKind::Comment(_) => false,
    }
}

fn lower_vue2_props_to_hir(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> HirProps {
    let mut props = HirProps {
        key: element.key,
        ref_name: element.ref_name.as_ref().map(|name| HirRef {
            name: name.clone(),
            in_for: element.ref_in_for,
        }),
        ..HirProps::default()
    };

    for attr in &element.attrs {
        lower_vue2_attr_to_hir(attr, ast_node, &mut props, state);
    }
    for attr in &element.props {
        lower_vue2_binding_attr_to_hir(attr, ast_node, &mut props, state);
    }
    for attr in &element.dynamic_attrs {
        lower_vue2_binding_attr_to_hir(attr, ast_node, &mut props, state);
    }
    if let Some(class_binding) = element.class_binding {
        push_hir_binding(
            &mut props,
            HirBinding {
                name: "class".into(),
                dynamic_name: None,
                value: class_binding,
                dynamic_arg: false,
                modifiers: Vec::new(),
            },
        );
    }
    if let Some(style_binding) = element.style_binding {
        push_hir_binding(
            &mut props,
            HirBinding {
                name: "style".into(),
                dynamic_name: None,
                value: style_binding,
                dynamic_arg: false,
                modifiers: Vec::new(),
            },
        );
    }
    for (event, handlers) in &element.events {
        for handler in handlers {
            let lowered = HirEvent {
                name: event.clone(),
                dynamic_name: None,
                handler: handler.value,
                dynamic_arg: handler.dynamic,
                modifiers: handler.modifiers.keys().cloned().collect(),
            };
            props.segments.push(HirPropSegment::Event(lowered.clone()));
            props.events.push(lowered);
        }
    }
    for (event, handlers) in &element.native_events {
        for handler in handlers {
            let lowered = HirEvent {
                name: format!("native:{event}"),
                dynamic_name: None,
                handler: handler.value,
                dynamic_arg: handler.dynamic,
                modifiers: handler.modifiers.keys().cloned().collect(),
            };
            props.segments.push(HirPropSegment::Event(lowered.clone()));
            props.events.push(lowered);
        }
    }
    if let Some(vuec_ast::Vue2DataWrap::Bind { value, .. }) = element.wrap_data {
        let lowered = HirObjectBinding { value };
        props
            .segments
            .push(HirPropSegment::ObjectBinding(lowered.clone()));
        props.object_bindings.push(lowered);
    }
    if let Some(listeners) = &element.wrap_listeners {
        let lowered = HirObjectListeners {
            value: state
                .js
                .register_expr(listeners, ast_node_span(ast_node), SourceType::script()),
        };
        props
            .segments
            .push(HirPropSegment::ObjectListeners(lowered.clone()));
        props.object_listeners.push(lowered);
    }
    props
}

fn lower_vue2_attr_to_hir(
    attr: &vuec_ast::Vue2Attribute,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    props: &mut HirProps,
    state: &mut Vue2LoweringState,
) {
    if !attr.dynamic && attr.value.trim_start().starts_with('"') {
        let lowered = HirStaticAttr {
            name: attr.name.clone(),
            value: attr.value.clone(),
        };
        props
            .segments
            .push(HirPropSegment::StaticAttr(lowered.clone()));
        props.static_attrs.push(lowered);
    } else {
        lower_vue2_binding_attr_to_hir(attr, ast_node, props, state);
    }
}

fn lower_vue2_binding_attr_to_hir(
    attr: &vuec_ast::Vue2Attribute,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    props: &mut HirProps,
    state: &mut Vue2LoweringState,
) {
    let value = state.js.register_expr(
        attr.value.clone(),
        attr.span.unwrap_or_else(|| ast_node_span(ast_node)),
        SourceType::script(),
    );
    let lowered = HirBinding {
        name: attr.name.clone(),
        dynamic_name: attr.dynamic.then(|| {
            state.js.register_expr(
                attr.name.clone(),
                attr.span.unwrap_or_else(|| ast_node_span(ast_node)),
                SourceType::script(),
            )
        }),
        value,
        dynamic_arg: attr.dynamic,
        modifiers: Vec::new(),
    };
    push_hir_binding(props, lowered);
}

fn push_hir_binding(props: &mut HirProps, binding: HirBinding) {
    props
        .segments
        .push(HirPropSegment::DynamicBinding(binding.clone()));
    props.dynamic_bindings.push(binding);
}

fn lower_vue2_directives_to_hir(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Vec<HirDirectiveUse> {
    element
        .directives
        .iter()
        .map(|directive| HirDirectiveUse {
            name: directive.name.clone(),
            argument: (!directive.is_dynamic_arg)
                .then(|| directive.arg.clone())
                .flatten(),
            dynamic_argument: directive.arg.as_ref().and_then(|arg| {
                directive.is_dynamic_arg.then(|| {
                    state
                        .js
                        .register_expr(arg, ast_node_span(ast_node), SourceType::script())
                })
            }),
            expression: directive.value,
            modifiers: directive.modifiers.keys().cloned().collect(),
        })
        .collect()
}

fn vue2_namespace(ns: Option<&str>) -> HtmlNamespace {
    match ns {
        Some("svg") => HtmlNamespace::Svg,
        Some("math") | Some("mathml") => HtmlNamespace::MathMl,
        _ => HtmlNamespace::Html,
    }
}

fn ast_node_span(node: &vuec_ast::Node<Vue2NodeKind>) -> Span {
    node.span
        .source()
        .unwrap_or_else(|| Span::new(FileId(0), 0, 0))
}

fn js_span(span: Option<Span>) -> Span {
    span.unwrap_or_else(|| Span::new(FileId(0), 0, 0))
}

fn single_default_interpolation(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    let inner = trimmed.strip_prefix("{{")?.strip_suffix("}}")?.trim();
    (!inner.contains("{{") && !inner.contains("}}")).then_some(inner)
}

fn get_binding_attr(element: &mut Vue2Element, name: &str, get_static: bool) -> Option<String> {
    get_binding_attr_with_span(element, name, get_static).map(|(value, _)| value)
}

fn get_binding_attr_with_span(
    element: &mut Vue2Element,
    name: &str,
    get_static: bool,
) -> Option<(String, Option<Span>)> {
    let dynamic = remove_attr_with_span(element, &format!(":{name}"))
        .or_else(|| remove_attr_with_span(element, &format!("v-bind:{name}")));
    if let Some((value, span)) = dynamic {
        Some((parse_filters(&value), span))
    } else if get_static {
        remove_attr_with_span(element, name).map(|(value, span)| (js_string(&value), span))
    } else {
        None
    }
}

fn remove_attr(element: &mut Vue2Element, name: &str) -> Option<String> {
    remove_attr_with_span(element, name).map(|(value, _)| value)
}

fn remove_attr_with_span(element: &mut Vue2Element, name: &str) -> Option<(String, Option<Span>)> {
    let value = element.attrs_map.get(name).cloned()?;
    let span = element.raw_attrs_map.get(name).and_then(|attr| attr.span);
    if let Some(index) = element.attrs_list.iter().position(|attr| attr.name == name) {
        element.attrs_list.remove(index);
    }
    Some((value, span))
}

fn remove_slot_binding(element: &mut Vue2Element) -> Option<(String, String, Option<Span>)> {
    let index = element
        .attrs_list
        .iter()
        .position(|attr| attr.name.starts_with("v-slot") || attr.name.starts_with('#'))?;
    let attr = element.attrs_list.remove(index);
    Some((attr.name, attr.value, attr.span))
}

fn slot_name_from_binding(name: &str) -> (String, bool) {
    let raw = name
        .strip_prefix("v-slot:")
        .or_else(|| name.strip_prefix('#'))
        .unwrap_or("default");
    if raw.starts_with('[') && raw.ends_with(']') {
        (raw[1..raw.len() - 1].to_string(), true)
    } else if raw.is_empty() {
        ("\"default\"".into(), false)
    } else {
        (js_string(raw), false)
    }
}

fn is_directive_name(name: &str) -> bool {
    name.starts_with("v-")
        || name.starts_with('@')
        || name.starts_with(':')
        || name.starts_with('#')
}

fn is_bind_name(name: &str) -> bool {
    name.starts_with(':') || name.starts_with("v-bind:")
}

fn is_on_name(name: &str) -> bool {
    name.starts_with('@') || name.starts_with("v-on:")
}

fn bind_arg_name(name: &str) -> String {
    name.strip_prefix(':')
        .or_else(|| name.strip_prefix("v-bind:"))
        .unwrap_or("")
        .to_string()
}

fn is_dynamic_arg(name: &str) -> bool {
    name.starts_with('[') && name.ends_with(']')
}

fn warn_invalid_dynamic_arg(arg: &str, span: Option<Span>, diagnostics: &mut DiagnosticSink) {
    if arg.contains(char::is_whitespace)
        || arg.contains('\'')
        || arg.contains('"')
        || arg.contains('+')
    {
        diagnostics.push(vue2_warning(
            "W_VUE2_INVALID_DYNAMIC_ARG",
            "Invalid dynamic argument expression: attribute names cannot contain spaces, quotes, <, >, / or =.",
            span,
        ));
    }
}

fn on_arg_name(name: &str) -> String {
    name.strip_prefix('@')
        .or_else(|| name.strip_prefix("v-on:"))
        .unwrap_or("")
        .to_string()
}

fn directive_name_and_arg(raw: &str) -> (String, Option<String>, bool) {
    let raw = raw.strip_prefix("v-").unwrap_or(raw);
    let (name, arg) = raw
        .split_once(':')
        .map_or((raw, None), |(name, arg)| (name, Some(arg.to_string())));
    let is_dynamic = arg
        .as_ref()
        .is_some_and(|arg| arg.starts_with('[') && arg.ends_with(']'));
    let arg = arg.map(|arg| {
        if is_dynamic {
            arg[1..arg.len() - 1].to_string()
        } else {
            arg
        }
    });
    (name.to_string(), arg, is_dynamic)
}

fn split_modifiers(raw_name: &str) -> (String, BTreeMap<String, bool>, Vec<String>) {
    let mut base = String::new();
    let mut modifiers = BTreeMap::new();
    let mut modifier_order = Vec::new();
    let mut in_dynamic = false;
    let mut modifier = String::new();
    let mut reading_modifier = false;
    for ch in raw_name.chars() {
        match ch {
            '[' => {
                in_dynamic = true;
                if reading_modifier {
                    modifier.push(ch);
                } else {
                    base.push(ch);
                }
            }
            ']' => {
                in_dynamic = false;
                if reading_modifier {
                    modifier.push(ch);
                } else {
                    base.push(ch);
                }
            }
            '.' if !in_dynamic => {
                if reading_modifier && !modifier.is_empty() {
                    modifiers.insert(modifier.clone(), true);
                    modifier_order.push(modifier.clone());
                    modifier.clear();
                }
                reading_modifier = true;
            }
            _ if reading_modifier => modifier.push(ch),
            _ => base.push(ch),
        }
    }
    if reading_modifier && !modifier.is_empty() {
        modifiers.insert(modifier.clone(), true);
        modifier_order.push(modifier);
    }
    (base, modifiers, modifier_order)
}

fn normalize_bound_name(name: &str, modifiers: &BTreeMap<String, bool>, dynamic: bool) -> String {
    if dynamic {
        return name.to_string();
    }
    if modifiers.get("prop").copied().unwrap_or(false)
        || modifiers.get("camel").copied().unwrap_or(false)
    {
        let camelized = camelize(name);
        if camelized == "innerHtml" {
            "innerHTML".into()
        } else {
            camelized
        }
    } else {
        name.to_string()
    }
}

fn should_use_prop(
    element: &Vue2Element,
    name: &str,
    modifiers: &BTreeMap<String, bool>,
    options: &Vue2CompileOptions,
) -> bool {
    if modifiers.get("prop").copied().unwrap_or(false) {
        return true;
    }
    if options.disable_default_must_use_prop {
        return false;
    }
    if element.component.is_some() {
        return false;
    }
    if name == "value" && vue2_tag_accepts_value_prop(&element.tag) {
        return element.attrs_map.get("type").map(String::as_str) != Some("button");
    }
    matches!(
        (element.tag.as_str(), name),
        ("option", "selected") | ("input", "checked") | ("video", "muted")
    )
}

fn vue2_tag_accepts_value_prop(tag: &str) -> bool {
    matches!(tag, "input" | "textarea" | "option" | "select" | "progress")
}

fn add_handler(
    events: &mut BTreeMap<String, Vec<Vue2EventHandler>>,
    mut name: String,
    value: String,
    mut modifiers: BTreeMap<String, bool>,
    modifier_order: Vec<String>,
    dynamic: bool,
    span: Option<Span>,
) {
    let has_modifier_object = !modifiers.is_empty();
    if modifiers.get("right").copied().unwrap_or(false) && name == "click" {
        modifiers.remove("right");
        name = "contextmenu".into();
    } else if modifiers.get("middle").copied().unwrap_or(false) && name == "click" {
        name = "mouseup".into();
    }
    if modifiers.remove("capture").is_some() {
        name = format!("!{name}");
    }
    if modifiers.remove("once").is_some() {
        name = format!("~{name}");
    }
    if modifiers.remove("passive").is_some() {
        name = format!("&{name}");
    }
    events.entry(name).or_default().push(Vue2EventHandler {
        value: value.trim().to_string(),
        modifiers,
        modifier_order,
        has_modifier_object,
        dynamic,
        span,
    });
}

fn gen_component_model(element: &mut Vue2Element, value: &str, modifiers: &BTreeMap<String, bool>) {
    let mut value_expression = "$$v".to_string();
    if modifiers.get("trim").copied().unwrap_or(false) {
        value_expression = "(typeof $$v === 'string'? $$v.trim(): $$v)".into();
    }
    if modifiers.get("number").copied().unwrap_or(false) {
        value_expression = format!("_n({value_expression})");
    }
    let assignment = gen_assignment_code(value, &value_expression);
    element.model = Some(Vue2ComponentModel {
        value: format!("({value})"),
        expression: js_string(value),
        callback: format!("function ($$v) {{{assignment}}}"),
    });
}

fn gen_dom_model(
    element: &mut Vue2Element,
    raw_name: &str,
    value: &str,
    modifiers: &BTreeMap<String, bool>,
) {
    element.props.push(Vue2Attribute {
        name: "value".into(),
        value: format!("({value})"),
        span: element.span,
        dynamic: false,
    });
    let assignment_value = if modifiers.get("trim").copied().unwrap_or(false) {
        "$event.target.value.trim()"
    } else {
        "$event.target.value"
    };
    let assignment_value = if modifiers.get("number").copied().unwrap_or(false) {
        format!("_n({assignment_value})")
    } else {
        assignment_value.into()
    };
    let assignment = gen_assignment_code(value, &assignment_value);
    let mut handler = "if($event.target.composing)return;".to_string();
    handler.push_str(&assignment);
    add_handler(
        &mut element.events,
        "input".into(),
        handler,
        BTreeMap::new(),
        Vec::new(),
        false,
        element.span,
    );
    element.directives.push(Vue2Directive {
        name: "model".into(),
        raw_name: raw_name.into(),
        value: Some(value.into()),
        arg: None,
        is_dynamic_arg: false,
        modifiers: modifiers.clone(),
        span: element.span,
    });
}

fn gen_assignment_code(value: &str, assignment: &str) -> String {
    let parsed_value = value.trim();
    if let Some(dot) = parsed_value.rfind('.') {
        if !parsed_value[dot + 1..].contains(']') && !parsed_value[dot + 1..].contains('[') {
            return format!(
                "$set({}, \"{}\", {assignment})",
                &parsed_value[..dot],
                &parsed_value[dot + 1..]
            );
        }
    }
    if parsed_value.ends_with(']') {
        if let Some(open) = find_model_bracket(parsed_value) {
            return format!(
                "$set({}, {}, {assignment})",
                &parsed_value[..open],
                &parsed_value[open + 1..parsed_value.len() - 1]
            );
        }
    }
    format!("{value}={assignment}")
}

fn find_model_bracket(value: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in value.char_indices().rev() {
        match ch {
            ']' => depth += 1,
            '[' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level(source: &str, separator: char) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if ch == separator && depth == 0 => {
                let item = source[start..index].trim();
                if !item.is_empty() {
                    items.push(item);
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        items.push(tail);
    }
    items
}

fn trim_ending_whitespace(element: &mut Vue2Element) {
    while matches!(element.children.last(), Some(Vue2Node::Text(text)) if text.text == " ") {
        element.children.pop();
    }
}

fn is_text_tag(tag: &str) -> bool {
    matches!(tag, "script" | "style" | "textarea")
}

fn is_forbidden_tag(element: &Vue2Element) -> bool {
    element.tag == "style"
        || (element.tag == "script"
            && element
                .attrs_map
                .get("type")
                .map_or(true, |value| value == "text/javascript"))
}

fn is_unary_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "frame"
            | "hr"
            | "img"
            | "input"
            | "isindex"
            | "keygen"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn is_reserved_tag(tag: &str) -> bool {
    matches!(
        tag,
        "html"
            | "body"
            | "base"
            | "head"
            | "link"
            | "meta"
            | "style"
            | "title"
            | "address"
            | "article"
            | "aside"
            | "footer"
            | "header"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "nav"
            | "section"
            | "div"
            | "dd"
            | "dl"
            | "dt"
            | "figcaption"
            | "figure"
            | "picture"
            | "hr"
            | "img"
            | "li"
            | "main"
            | "ol"
            | "p"
            | "pre"
            | "ul"
            | "a"
            | "b"
            | "abbr"
            | "bdi"
            | "bdo"
            | "br"
            | "cite"
            | "code"
            | "data"
            | "dfn"
            | "em"
            | "i"
            | "kbd"
            | "mark"
            | "q"
            | "rp"
            | "rt"
            | "rtc"
            | "ruby"
            | "s"
            | "samp"
            | "small"
            | "span"
            | "strong"
            | "sub"
            | "sup"
            | "time"
            | "u"
            | "var"
            | "wbr"
            | "area"
            | "audio"
            | "map"
            | "track"
            | "video"
            | "embed"
            | "object"
            | "param"
            | "source"
            | "canvas"
            | "script"
            | "noscript"
            | "del"
            | "ins"
            | "caption"
            | "col"
            | "colgroup"
            | "table"
            | "thead"
            | "tbody"
            | "td"
            | "th"
            | "tr"
            | "button"
            | "datalist"
            | "fieldset"
            | "form"
            | "input"
            | "label"
            | "legend"
            | "meter"
            | "optgroup"
            | "option"
            | "output"
            | "progress"
            | "select"
            | "textarea"
            | "details"
            | "dialog"
            | "menu"
            | "menuitem"
            | "summary"
            | "content"
            | "element"
            | "shadow"
            | "template"
            | "blockquote"
            | "iframe"
            | "tfoot"
            | "svg"
            | "text"
            | "circle"
            | "path"
            | "g"
    )
}

fn is_built_in_tag(tag: &str) -> bool {
    matches!(tag, "slot" | "component")
}

fn namespace_for_tag(tag: &str, options: &Vue2CompileOptions) -> Option<String> {
    if let Some(namespace) = options.tag_namespaces.get(tag) {
        return Some(namespace.clone());
    }
    (options.use_default_tag_namespaces && tag == "svg").then(|| "svg".into())
}

fn is_reserved_tag_with_options(tag: &str, options: &Vue2CompileOptions) -> bool {
    if let Some(tags) = options.reserved_tags.as_ref() {
        return tags.iter().any(|candidate| candidate == tag);
    }
    options.use_default_reserved_tags && is_reserved_tag(tag)
}

fn is_component(element: &Vue2Element, options: &Vue2CompileOptions) -> bool {
    element.component.is_some() || !is_reserved_tag_with_options(&element.tag, options)
}

fn check_binding_type(bindings: &BTreeMap<String, String>, key: &str) -> Option<String> {
    let camel_name = camelize(key);
    let pascal_name = capitalize(&camel_name);
    let candidates = [key, camel_name.as_str(), pascal_name.as_str()];
    for binding_type in ["setup-const", "setup-reactive-const"] {
        if let Some(name) = check_binding_type_candidates(bindings, &candidates, binding_type) {
            return Some(name);
        }
    }
    for binding_type in ["setup-let", "setup-ref", "setup-maybe-ref"] {
        if let Some(name) = check_binding_type_candidates(bindings, &candidates, binding_type) {
            return Some(name);
        }
    }
    None
}

fn check_binding_type_candidates(
    bindings: &BTreeMap<String, String>,
    candidates: &[&str],
    binding_type: &str,
) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        (bindings.get(*candidate).map(String::as_str) == Some(binding_type))
            .then(|| (*candidate).to_string())
    })
}

fn is_simple_path(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || matches!(first, '_' | '$'))
        && chars.all(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(ch, '_' | '$' | '.' | '[' | ']' | '\'' | '"' | '0'..='9')
        })
}

fn is_function_expression(value: &str) -> bool {
    let value = value.trim_start();
    is_function_keyword_expression(value) || is_arrow_function_expression(value)
}

fn is_function_invocation(value: &str) -> bool {
    let value = value.trim();
    let value = value.trim_end_matches(';');
    if !value.ends_with(')') {
        return false;
    }
    let Some(open) = value.rfind('(') else {
        return false;
    };
    if value[open + 1..value.len() - 1].contains(')') {
        return false;
    }
    is_simple_path(value[..open].trim())
}

fn is_function_keyword_expression(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("function") else {
        return false;
    };
    if rest.starts_with('(') {
        return true;
    }
    if !rest.starts_with(char::is_whitespace) {
        return false;
    }
    let rest = rest.trim_start();
    if rest.starts_with('(') {
        return true;
    }
    let Some((ident, after_ident)) = split_identifier(rest) else {
        return false;
    };
    !ident.is_empty() && after_ident.trim_start().starts_with('(')
}

fn is_arrow_function_expression(value: &str) -> bool {
    let Some(arrow) = value.find("=>") else {
        return false;
    };
    let params = value[..arrow].trim_end();
    if is_simple_identifier(params.trim()) {
        return true;
    }
    params.starts_with('(') && params.ends_with(')') && !params[1..params.len() - 1].contains(')')
}

fn split_identifier(value: &str) -> Option<(&str, &str)> {
    let mut end = 0usize;
    for (index, ch) in value.char_indices() {
        if index == 0 {
            if !is_identifier_start(ch) {
                return None;
            }
        } else if !is_identifier_continue(ch) {
            break;
        }
        end = index + ch.len_utf8();
    }
    (end > 0).then(|| (&value[..end], &value[end..]))
}

fn is_simple_identifier(value: &str) -> bool {
    let Some((ident, rest)) = split_identifier(value) else {
        return false;
    };
    ident.len() == value.len() && rest.is_empty()
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || matches!(ch, '_' | '$')
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$')
}

fn decode_basic_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", "\u{00a0}")
}

fn condense_whitespace(value: &str) -> String {
    let mut out = String::new();
    let mut previous_ws = false;
    for ch in value.chars() {
        if ch.is_ascii_whitespace() {
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

fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn js_string_single(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn vue2_static_style_expression(value: &str) -> String {
    let fields = vue2_parse_static_style(value)
        .into_iter()
        .map(|(name, value)| format!("{}:{}", js_string(&name), js_string(&value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn vue2_parse_static_style(value: &str) -> Vec<(String, String)> {
    let mut style = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0usize;
    for ch in value.chars() {
        match ch {
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(ch);
            }
            ';' if paren_depth == 0 => {
                vue2_push_static_style_decl(&mut style, &current);
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    vue2_push_static_style_decl(&mut style, &current);
    style
}

fn vue2_push_static_style_decl(style: &mut Vec<(String, String)>, item: &str) {
    if item.is_empty() {
        return;
    }
    let Some(colon) = item.find(':') else {
        return;
    };
    if colon + 1 >= item.len() {
        return;
    }
    let name = item[..colon].trim();
    let value = item[colon + 1..].trim();
    if name.is_empty() || value.is_empty() {
        return;
    }
    if let Some((_, existing)) = style.iter_mut().find(|(key, _)| key == name) {
        *existing = value.to_string();
    } else {
        style.push((name.to_string(), value.to_string()));
    }
}

fn transform_special_newlines(value: &str) -> String {
    value
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

fn decode_newline_entities_for_attr(
    name: &str,
    value: &str,
    options: &Vue2CompileOptions,
) -> String {
    if (name == "href" && options.should_decode_newlines_for_href)
        || (name != "href" && options.should_decode_newlines)
    {
        value
            .replace("&#10;", "\n")
            .replace("&#x0A;", "\n")
            .replace("&#x0a;", "\n")
            .replace("&#9;", "\t")
            .replace("&#x09;", "\t")
            .replace("&#x9;", "\t")
    } else {
        value.to_string()
    }
}

fn modifiers_json(modifiers: &BTreeMap<String, bool>) -> String {
    let body = modifiers
        .keys()
        .map(|key| format!("{}:true", js_string(key)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{body}}}")
}

fn json_string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| js_string(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn ast_validators_json(validators: &[vuec_ast::Vue2Validator]) -> String {
    format!(
        "[{}]",
        validators
            .iter()
            .map(|validator| {
                format!(
                    "{{\"name\":{},\"rule\":{}}}",
                    js_string(&validator.name),
                    js_string(&validator.rule)
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn ast_validation_result_json(validators: &[vuec_ast::Vue2Validator]) -> String {
    let mut fields = vec!["\"dirty\":false".to_string()];
    fields.extend(
        validators
            .iter()
            .map(|validator| format!("{}:null", js_string(&validator.name))),
    );
    format!("{{{}}}", fields.join(","))
}

fn camelize(value: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = false;
    for ch in value.chars() {
        if ch == '-' {
            uppercase_next = true;
        } else if uppercase_next {
            out.extend(ch.to_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn hyphenate(value: &str) -> String {
    let mut out = String::new();
    for (index, ch) in value.char_indices() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('-');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = first.to_uppercase().collect::<String>();
    out.push_str(chars.as_str());
    out
}

fn has_duplicate_attr(attrs: &[Vue2Attribute]) -> bool {
    let mut seen = BTreeMap::new();
    for attr in attrs {
        if seen.insert(attr.name.clone(), true).is_some() {
            return true;
        }
    }
    false
}

fn vue2_warning(code: &str, message: impl Into<String>, span: Option<Span>) -> Diagnostic {
    Diagnostic::vue2_warning(code, message, span)
}

fn vue2_error(code: &str, message: impl Into<String>, span: Option<Span>) -> Diagnostic {
    Diagnostic::vue2_error(code, message, span)
}

fn split_compilation_issues(diagnostics: &DiagnosticSink) -> (Vec<Vue2Error>, Vec<Vue2Warning>) {
    let mut errors = Vec::new();
    let mut tips = Vec::new();
    for diagnostic in diagnostics.as_slice() {
        match diagnostic.severity {
            Severity::Error | Severity::Warning => errors.push(Vue2Error {
                msg: diagnostic.message.clone(),
                start: diagnostic.span.map(|span| span.start.0),
                end: vue2_issue_end(diagnostic),
            }),
            Severity::Tip | Severity::Note => tips.push(Vue2Warning {
                msg: diagnostic.message.clone(),
                start: diagnostic.span.map(|span| span.start.0),
                end: vue2_issue_end(diagnostic),
                tip: matches!(diagnostic.severity, Severity::Tip),
            }),
        }
    }
    (errors, tips)
}

fn vue2_issue_end(diagnostic: &Diagnostic) -> Option<usize> {
    if diagnostic.code == "W_VUE2_TEXT_OUTSIDE_ROOT"
        && diagnostic
            .message
            .contains("requires a root element, rather than just text")
    {
        return None;
    }
    diagnostic.span.map(|span| span.end.0)
}

fn render_diagnostic_message(diagnostic: &Diagnostic) -> String {
    match diagnostic.span {
        Some(span) => format!(
            "[{}] {} @ {}:{}-{}:{}",
            diagnostic.code,
            diagnostic.message,
            span.file_id.0,
            span.start.0,
            span.file_id.0,
            span.end.0
        ),
        None => format!("[{}] {}", diagnostic.code, diagnostic.message),
    }
}

impl Vue2Element {
    fn clone_without_conditions(&self) -> Self {
        let mut clone = self.clone();
        clone.if_exp = None;
        clone.if_span = None;
        clone.elseif = None;
        clone.elseif_span = None;
        clone.else_branch = false;
        clone.else_span = None;
        clone.if_conditions = Vec::new();
        clone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Vue2CompileOptions {
        Vue2CompileOptions {
            comments: true,
            warn: true,
            preserve_whitespace: true,
            optimize: true,
            ..Vue2CompileOptions::default()
        }
    }

    #[test]
    fn compile_returns_vue2_shapes() {
        let result = compile("<div>{{ msg }}</div>", options());
        assert!(result.render.contains("with(this)"));
        assert!(result.render.contains("_s(msg)"));
        assert!(result.ast.node(result.ast.root).is_some());
        assert!(result.element_ast.is_some());
    }

    #[test]
    fn compile_to_functions_wraps_render() {
        let result = compile_to_functions("<div/>", options());
        assert!(result.render.contains("with(this)"));
    }

    #[test]
    fn projects_vue2_public_ast_with_registered_js_ids() {
        let compiled = compile(
            r#"<div :id="item.id" @click.stop="save(item)">{{ item.name | upper }}</div>"#,
            options(),
        );
        let projected = project_vue2_public_ast("<div/>", compiled.element_ast.as_ref());
        assert!(projected.ast.validate_span_consistency().is_ok());
        assert!(projected.js.expressions().len() >= 2);
        assert_eq!(projected.js.statements().len(), 1);

        let root = projected.ast.root_node().unwrap();
        let element = projected.ast.node(root.children[0]).unwrap();
        let Vue2AstKind::Element(element) = &element.kind else {
            panic!("expected Vue2 element projection");
        };
        assert!(element.attrs.iter().any(|attr| attr.name == "id"));
        assert_eq!(
            element
                .events
                .get("click")
                .and_then(|handlers| handlers.first())
                .map(|handler| handler.modifiers.contains_key("stop")),
            Some(true)
        );

        let text_id = projected.ast.node(root.children[0]).unwrap().children[0];
        let Vue2AstKind::ExpressionText(text) = &projected.ast.node(text_id).unwrap().kind else {
            panic!("expected expression text projection");
        };
        let filter = text.filter_expr.as_ref().expect("Vue2 filter payload");
        assert_eq!(filter.filters[0].name, "upper");
    }

    #[test]
    fn lowers_vue2_ast_to_hir_and_target_split_mir() {
        let compiled = compile(
            r#"<ul><li v-for="(item, i) in items" :key="item.id">{{ item.name }}</li></ul>"#,
            options(),
        );
        let projected = project_vue2_public_ast("<ul/>", compiled.element_ast.as_ref());
        let lowered = lower_vue2_ast_to_mir(&projected.ast, projected.js);

        assert!(lowered.hir.validate_span_consistency().is_ok());
        assert!(lowered.mir.validate_span_consistency().is_ok());
        assert_eq!(
            lowered
                .map
                .hir_for_ast(projected.ast.root)
                .collect::<Vec<_>>(),
            vec![lowered.hir.root]
        );
        assert!(lowered.map.hir_to_mir.iter().any(|(_, mir)| matches!(
            lowered.mir.node(*mir).map(|node| &node.kind),
            Some(Vue2MirKind::For { .. })
        )));
        assert!(lowered.hir.nodes.iter().any(|node| matches!(
            node.kind,
            HirNodeKind::For(_) | HirNodeKind::Interpolation(_)
        )));
        assert!(lowered.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue2MirKind::CreateElement(_) | Vue2MirKind::Text(_)
        )));
    }

    #[test]
    fn lowers_vue2_if_chain_and_filters_without_hir_helpers() {
        let compiled = compile(
            r#"<div><p v-if="ok">{{ msg | upper }}</p><p v-else>fallback</p></div>"#,
            options(),
        );
        let projected = project_vue2_public_ast("<div/>", compiled.element_ast.as_ref());
        let lowered = lower_vue2_ast_to_mir(&projected.ast, projected.js);

        let if_nodes = lowered
            .hir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                HirNodeKind::If(if_node) => Some(if_node),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(if_nodes.len(), 1);
        assert_eq!(if_nodes[0].branches.len(), 2);
        assert!(lowered.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue2MirKind::If { .. } | Vue2MirKind::FilterCall { .. }
        )));
        assert!(lowered.hir.nodes.iter().all(|node| {
            !matches!(
                node.kind,
                HirNodeKind::Fragment(_) if matches!(lowered.mir.node(node.id), Some(_))
            )
        }));
    }

    #[test]
    fn lowers_vue2_codegen_payloads_into_mir() {
        let compiled = compile(
            r#"<section><button @click.stop="save" :id="foo">go</button><foo><template #default="slotProps">{{ slotProps.msg }}</template></foo></section>"#,
            options(),
        );
        let projected = project_vue2_public_ast("<section/>", compiled.element_ast.as_ref());
        let lowered = lower_vue2_ast_to_mir(&projected.ast, projected.js);

        let button_data = lowered.mir.nodes.iter().find_map(|node| match &node.kind {
            Vue2MirKind::CreateElement(Vue2CreateElement {
                tag: MirExpr::String(tag),
                data: Some(data),
                ..
            }) if tag == "button" => Some(data),
            _ => None,
        });
        let button_data = button_data.expect("button MIR data");
        assert!(button_data.attrs.iter().any(|attr| attr.name == "id"));
        assert!(button_data
            .events
            .get("click")
            .and_then(|handlers| handlers.first())
            .is_some_and(|handler| handler.modifiers.contains_key("stop")));

        let scoped = lowered.mir.nodes.iter().find_map(|node| match &node.kind {
            Vue2MirKind::CreateElement(Vue2CreateElement {
                tag: MirExpr::String(tag),
                data: Some(data),
                ..
            }) if tag == "foo" => data.scoped_slots.first(),
            _ => None,
        });
        let scoped = scoped.expect("scoped slot payload");
        assert!(scoped.new_syntax);
        assert!(scoped.body_is_fragment);
        assert!(!scoped.body.is_empty());

        let static_compiled = compile(r#"<div v-pre><p>{{ msg }}</p></div>"#, options());
        let static_projected =
            project_vue2_public_ast("<div/>", static_compiled.element_ast.as_ref());
        let static_lowered = lower_vue2_ast_to_mir(&static_projected.ast, static_projected.js);
        assert!(static_lowered.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue2MirKind::RenderStatic(Vue2RenderStatic { body: Some(_), .. })
        )));
    }

    #[test]
    fn normalizes_if_template_with_nested_for_children() {
        let result = compile(
            r#"<div><template v-if="ok"><foo v-for="i in 1" :key="i"></foo></template></div>"#,
            options(),
        );

        assert_eq!(
            result.render,
            "with(this){return _c('div',[(ok)?_l((1),function(i){return _c('foo',{key:i})}):_e()],2)}"
        );
    }

    #[test]
    fn parses_v_for_and_generates_list_render() {
        let result = compile(
            r#"<div><li v-for="(item, i) in items" :key="item.uid">{{ item }}</li></div>"#,
            options(),
        );
        assert!(result.render.contains("_l((items),function(item,i)"));
        assert!(result.render.contains("key:item.uid"));
    }

    #[test]
    fn parses_v_if_else_chain() {
        let result = compile(
            r#"<div><p v-if="show">hello</p><p v-else>world</p></div>"#,
            options(),
        );
        assert!(result.render.contains("(show)?_c('p'"), "{}", result.render);
        assert!(result.render.contains(":_c('p'"), "{}", result.render);
    }

    #[test]
    fn generates_filters_and_events() {
        let result = compile(
            r#"<div :id="a | b | c" @click.stop="save">{{ d | e }}</div>"#,
            options(),
        );
        assert!(result.render.contains("_f(\"c\")(_f(\"b\")(a))"));
        assert!(result.render.contains("$event.stopPropagation();"));
    }

    #[test]
    fn generates_ref_data_and_single_dom_model_directive() {
        let plain_ref = compile(r#"<p ref="component1"></p>"#, options());
        assert_eq!(
            plain_ref.render,
            r#"with(this){return _c('p',{ref:"component1"})}"#
        );

        let for_ref = compile(
            r#"<ul><li v-for="item in items" ref="component1"></li></ul>"#,
            options(),
        );
        assert_eq!(
            for_ref.render,
            r#"with(this){return _c('ul',_l((items),function(item){return _c('li',{ref:"component1",refInFor:true})}),0)}"#
        );

        let model = compile(r#"<input v-model="test">"#, options());
        assert_eq!(model.render.matches(r#"name:"model""#).count(), 1);
        assert!(model
            .render
            .contains(r#"domProps:{"value":(test)},on:{"input":function($event){if($event.target.composing)return;test=$event.target.value}}"#));

        let multiline_model = compile("<input v-model=\"\n test \n\">", options());
        let expected_value_prop = concat!("domProps:{\"value\":(\n", " test \n", ")}");
        assert!(multiline_model.render.contains(expected_value_prop));
        assert!(multiline_model
            .render
            .contains("if($event.target.composing)return;\n test \n=$event.target.value"));

        let component_model = compile("<my-component v-model=\"\n test \n\" />", options());
        assert!(component_model
            .render
            .contains("callback:function ($$v) {\n test \n=$$v}"));
    }

    #[test]
    fn generates_vue2_platform_must_use_props_like_official_codegen() {
        let option = compile(
            r#"<option :value="item.value" :selected="item.isRefined"></option>"#,
            options(),
        );
        assert_eq!(
            option.render,
            r#"with(this){return _c('option',{domProps:{"value":item.value,"selected":item.isRefined}})}"#
        );

        let checkbox = compile(
            r#"<input type="checkbox" :value="item.value" :checked="item.isRefined">"#,
            options(),
        );
        assert_eq!(
            checkbox.render,
            r#"with(this){return _c('input',{attrs:{"type":"checkbox"},domProps:{"value":item.value,"checked":item.isRefined}})}"#
        );

        let select = compile(r#"<select :value="selected"></select>"#, options());
        assert_eq!(
            select.render,
            r#"with(this){return _c('select',{domProps:{"value":selected}})}"#
        );

        let progress = compile(r#"<progress :value="n"></progress>"#, options());
        assert_eq!(
            progress.render,
            r#"with(this){return _c('progress',{domProps:{"value":n}})}"#
        );

        let button_value = compile(r#"<input type="button" :value="label">"#, options());
        assert_eq!(
            button_value.render,
            r#"with(this){return _c('input',{attrs:{"type":"button","value":label}})}"#
        );

        let dynamic_component = compile(
            r#"<component :is="tag" :value="label"></component>"#,
            options(),
        );
        assert_eq!(
            dynamic_component.render,
            r#"with(this){return _c(tag,{tag:"component",attrs:{"value":label}})}"#
        );
    }

    #[test]
    fn generates_vue2_event_handlers_like_official_codegen() {
        let method_call = compile(r#"<input @input="functionName()">"#, options());
        assert_eq!(
            method_call.render,
            r#"with(this){return _c('input',{on:{"input":function($event){return functionName()}}})}"#
        );

        let tricky_call = compile(r#"<input @input="onInput(');[\'());');">"#, options());
        assert_eq!(
            tricky_call.render,
            r#"with(this){return _c('input',{on:{"input":function($event){onInput(');[\'());');}}})}"#
        );

        let multiple_statements = compile(r#"<input @input="onInput1();onInput2()">"#, options());
        assert_eq!(
            multiple_statements.render,
            r#"with(this){return _c('input',{on:{"input":function($event){onInput1();onInput2()}}})}"#
        );

        let ordered_keys = compile(r#"<input @keydown.enter.delete="onInput">"#, options());
        assert_eq!(
            ordered_keys.render,
            r#"with(this){return _c('input',{on:{"keydown":function($event){if(!$event.type.indexOf('key')&&_k($event.keyCode,"enter",13,$event.key,"Enter")&&_k($event.keyCode,"delete",[8,46],$event.key,["Backspace","Delete","Del"]))return null;return onInput.apply(null, arguments)}}})}"#
        );

        let ordered_modifiers = compile(r#"<input @input.stop.prevent.self="onInput">"#, options());
        assert_eq!(
            ordered_modifiers.render,
            r#"with(this){return _c('input',{on:{"input":function($event){$event.stopPropagation();$event.preventDefault();if($event.target !== $event.currentTarget)return null;return onInput.apply(null, arguments)}}})}"#
        );

        let left_exact = compile(
            r#"<button @click.exact.left.prevent="go"></button>"#,
            options(),
        );
        assert_eq!(
            left_exact.render,
            r#"with(this){return _c('button',{on:{"click":function($event){if(!$event.type.indexOf('key')&&_k($event.keyCode,"left",37,$event.key,["Left","ArrowLeft"]))return null;if($event.ctrlKey||$event.shiftKey||$event.altKey||$event.metaKey)return null;if('button' in $event && $event.button !== 0)return null;$event.preventDefault();return go.apply(null, arguments)}}})}"#
        );

        let capture_once = compile(r#"<input @input.capture.once="onInput">"#, options());
        assert_eq!(
            capture_once.render,
            r#"with(this){return _c('input',{on:{"~!input":function($event){return onInput.apply(null, arguments)}}})}"#
        );
    }

    #[test]
    fn generates_vue2_static_style_sync_and_event_order_like_official_codegen() {
        let pagination = compile(
            r#"<el-pagination
  :page-size.sync="page.size"
  :total="page.total"
  :current-page.sync="page.page"
  style="margin-top: 8px;"
  layout="total, prev, pager, next, sizes"
  @size-change="crud.sizeChangeHandler($event)"
  @current-change="crud.pageChangeHandler"
/>"#,
            options(),
        );
        assert_eq!(
            pagination.render,
            r#"with(this){return _c('el-pagination',{staticStyle:{"margin-top":"8px"},attrs:{"page-size":page.size,"total":page.total,"current-page":page.page,"layout":"total, prev, pager, next, sizes"},on:{"update:pageSize":function($event){return $set(page, "size", $event)},"update:page-size":function($event){return $set(page, "size", $event)},"update:currentPage":function($event){return $set(page, "page", $event)},"update:current-page":function($event){return $set(page, "page", $event)},"size-change":function($event){return crud.sizeChangeHandler($event)},"current-change":crud.pageChangeHandler}})}"#
        );

        let popover = compile(
            r#"<div style="text-align: right; margin: 0" @show="onPopoverShow" @hide="onPopoverHide"></div>"#,
            options(),
        );
        assert_eq!(
            popover.render,
            r#"with(this){return _c('div',{staticStyle:{"text-align":"right","margin":"0"},on:{"show":onPopoverShow,"hide":onPopoverHide}})}"#
        );
    }

    #[test]
    fn generates_vue2_empty_event_handler_like_official_codegen() {
        let parsed = compile(r#"<input @input="current++">"#, options());
        let mut element = parsed.element_ast.unwrap();
        element.events.insert("input".into(), Vec::new());
        let generated = generate(Some(&element), &options());
        assert_eq!(
            generated.render,
            r#"with(this){return _c('input',{on:{"input":function(){}}})}"#
        );
        assert!(generated.static_render_fns.is_empty());
    }

    #[test]
    fn generates_vue2_scoped_slots_like_official_codegen() {
        let default_template = compile(
            r#"<foo><template slot-scope="bar">{{ bar }}</template></foo>"#,
            options(),
        );
        assert_eq!(
            default_template.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([{key:"default",fn:function(bar){return [_v(_s(bar))]}}])})}"#
        );

        let default_element = compile(
            r#"<foo><div slot-scope="bar">{{ bar }}</div></foo>"#,
            options(),
        );
        assert_eq!(
            default_element.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([{key:"default",fn:function(bar){return _c('div',{},[_v(_s(bar))])}}])})}"#
        );

        let dynamic_slot = compile(
            r#"<foo><template :slot="foo" slot-scope="bar">{{ bar }}</template></foo>"#,
            options(),
        );
        assert_eq!(
            dynamic_slot.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([{key:foo,fn:function(bar){return [_v(_s(bar))]}}],null,true)})}"#
        );

        let legacy_if = compile(
            "<foo><template v-if=\"\nshow\n\" slot-scope=\"bar\">{{ bar }}</template></foo>",
            options(),
        );
        assert_eq!(
            legacy_if.render,
            "with(this){return _c('foo',{scopedSlots:_u([{key:\"default\",fn:function(bar){return (\nshow\n)?[_v(_s(bar))]:undefined}}],null,true)})}"
        );

        let new_syntax_if = compile(
            r#"<foo><template v-if="show" #default="bar">{{ bar }}</template></foo>"#,
            options(),
        );
        assert_eq!(
            new_syntax_if.render,
            r#"with(this){return _c('foo',{scopedSlots:_u([(show)?{key:"default",fn:function(bar){return [_v(_s(bar))]}}:null],null,true)})}"#
        );
    }

    #[test]
    fn generates_vue2_slot_outlet_props_like_official_codegen() {
        let no_fallback = compile(
            r#"<div><slot :has-refinements="canRefine" :refine="state.refine" /></div>"#,
            options(),
        );
        assert_eq!(
            no_fallback.render,
            r#"with(this){return _c('div',[_t("default",null,{"hasRefinements":canRefine,"refine":state.refine})],2)}"#
        );

        let fallback = compile(
            r#"<div><slot foo="bar" :has-refinements="canRefine"><span>x</span></slot></div>"#,
            options(),
        );
        assert_eq!(
            fallback.render,
            r#"with(this){return _c('div',[_t("default",function(){return [_c('span',[_v("x")])]},{"foo":"bar","hasRefinements":canRefine})],2)}"#
        );

        let dynamic_prop = compile(r#"<div><slot v-bind:[foo]="bar | baz" /></div>"#, options());
        assert_eq!(
            dynamic_prop.render,
            r#"with(this){return _c('div',[_t("default",null,_d({},[foo,_f("baz")(bar)]))],2)}"#
        );

        let bind_object = compile(r#"<div><slot v-bind="slotProps" /></div>"#, options());
        assert_eq!(
            bind_object.render,
            r#"with(this){return _c('div',[_t("default",null,null,slotProps)],2)}"#
        );
    }

    #[test]
    fn generates_vue2_inline_template_like_official_codegen() {
        let single = compile(
            r#"<my-component inline-template><p><span>hello world</span></p></my-component>"#,
            options(),
        );
        assert_eq!(
            single.render,
            r#"with(this){return _c('my-component',{inlineTemplate:{render:function(){with(this){return _m(0)}},staticRenderFns:[function(){with(this){return _c('p',[_c('span',[_v("hello world")])])}}]}})}"#
        );

        let multiple = compile(
            r#"<my-component inline-template><hr><hr></my-component>"#,
            options(),
        );
        assert_eq!(
            multiple.render,
            r#"with(this){return _c('my-component',{inlineTemplate:{render:function(){with(this){return _c('hr')}},staticRenderFns:[]}})}"#
        );
        assert!(multiple
            .errors
            .iter()
            .any(|error| error.msg
                == "Inline-template components must have exactly one child element."));

        let empty = compile(
            r#"<my-component inline-template></my-component>"#,
            options(),
        );
        assert_eq!(empty.render, r#"with(this){return _c('my-component',{})}"#);
    }

    #[test]
    fn generates_vue27_setup_binding_component_tags_like_official_codegen() {
        let mut parsed = compile("<div><Foo/><foo-bar></foo-bar></div>", options())
            .element_ast
            .unwrap();
        optimize(&mut parsed, &options());
        let generated = generate(
            Some(&parsed),
            &Vue2CompileOptions {
                bindings: BTreeMap::from([
                    ("Foo".into(), "setup-const".into()),
                    ("FooBar".into(), "setup-const".into()),
                ]),
                ..options()
            },
        );
        assert_eq!(
            generated.render,
            r#"with(this){return _c('div',[_c(Foo),_c(FooBar)],1)}"#
        );
    }

    #[test]
    fn vue27_setup_bindings_do_not_resolve_native_tags() {
        let mut parsed = compile("<div><form>{{ n }}</form></div>", options())
            .element_ast
            .unwrap();
        optimize(&mut parsed, &options());
        let generated = generate(
            Some(&parsed),
            &Vue2CompileOptions {
                bindings: BTreeMap::from([("form".into(), "setup-const".into())]),
                ..options()
            },
        );
        assert_eq!(
            generated.render,
            r#"with(this){return _c('div',[_c('form',[_v(_s(n))])])}"#
        );
    }

    #[test]
    fn generates_vue2_v_pre_template_like_official_codegen() {
        let result = compile(
            r#"<div v-pre><template><p>{{msg}}</p></template></div>"#,
            options(),
        );
        assert_eq!(result.render, r#"with(this){return _m(0)}"#);
        assert_eq!(
            result.static_render_fns,
            vec![
                r#"with(this){return _c('div',{pre:true},[_c('template',[_c('p',[_v("{{msg}}")])])],2)}"#
                    .to_string()
            ]
        );
    }

    #[test]
    fn parses_vue2_raw_text_elements_like_official_parser() {
        let textarea = compile(
            "<textarea>\n        <p>Test 1</p>\n        test2\n      </textarea>",
            options(),
        );
        let textarea = textarea.element_ast.unwrap();
        assert_eq!(textarea.tag, "textarea");
        assert_eq!(textarea.children.len(), 1);
        match &textarea.children[0] {
            Vue2Node::Text(text) => {
                assert_eq!(text.text, "        <p>Test 1</p>\n        test2\n      ");
                assert!(text.expression.is_none());
            }
            Vue2Node::Element(_) => panic!("textarea content must stay raw text"),
        }

        let script = compile(
            r#"<script type="x/template">&gt;<foo>&lt;</script>"#,
            options(),
        );
        let script = script.element_ast.unwrap();
        assert_eq!(script.tag, "script");
        assert_eq!(script.children.len(), 1);
        match &script.children[0] {
            Vue2Node::Text(text) => assert_eq!(text.text, "&gt;<foo>&lt;"),
            Vue2Node::Element(_) => panic!("script template content must stay raw text"),
        }
    }

    #[test]
    fn parses_vue2_pre_children_as_normal_elements_with_preserved_whitespace() {
        let result = compile(
            "<pre><code>  \n<span>hi</span>\n  </code><span> </span></pre>",
            options(),
        );
        let root = result.element_ast.unwrap();
        assert_eq!(root.tag, "pre");
        assert_eq!(root.children.len(), 2);
        let code = match &root.children[0] {
            Vue2Node::Element(element) => element,
            Vue2Node::Text(_) => panic!("expected code child element"),
        };
        assert_eq!(code.children.len(), 3);
        match &code.children[0] {
            Vue2Node::Text(text) => assert_eq!(text.text, "  \n"),
            Vue2Node::Element(_) => panic!("expected preserved pre whitespace"),
        }
        match &code.children[2] {
            Vue2Node::Text(text) => assert_eq!(text.text, "\n  "),
            Vue2Node::Element(_) => panic!("expected preserved pre whitespace"),
        }
    }

    #[test]
    fn parses_vue2_condensed_whitespace_like_official_parser() {
        let mut options = options();
        options.whitespace = Some("condense".into());
        options.preserve_whitespace = false;
        let result = compile(
            "<p>\n  Welcome to <b>Vue.js</b>    <i>world</i>  \n  <span>.\n  Have fun!\n</span></p>",
            options.clone(),
        );
        let root = result.element_ast.unwrap();
        assert_eq!(root.children.len(), 5);
        match &root.children[2] {
            Vue2Node::Text(text) => assert_eq!(text.text, " "),
            Vue2Node::Element(_) => panic!("expected condensed inline space"),
        }

        let nbsp = compile("<span>&nbsp;</span>", options);
        let root = nbsp.element_ast.unwrap();
        assert_eq!(root.children.len(), 1);
        match &root.children[0] {
            Vue2Node::Text(text) => assert_eq!(text.text, "\u{00a0}"),
            Vue2Node::Element(_) => panic!("expected non-breaking space text"),
        }
    }

    #[test]
    fn vue27_sfc_asset_url_transform_rewrites_attrs_and_srcset() {
        let mut compile_options = options();
        compile_options.sfc_asset_url_transform = Some(Vue2SfcAssetUrlTransformOptions::default());
        let result = compile(
            r#"<div><img src="./logo.png" srcset="./logo.png 2x, @/icon.svg#heart 3x"><svg><use href="~@svg/file.svg#fragment"/></svg></div>"#,
            compile_options,
        );
        let code = format!("{}{}", result.render, result.static_render_fns.join(""));

        assert!(code.contains(r#""src":require("./logo.png")"#));
        assert!(code.contains(
            r##""srcset":require("./logo.png") + " 2x, " + require("@/icon.svg") + "#heart" + " 3x""##
        ));
        assert!(code.contains(r##""href":require("@svg/file.svg") + "#fragment""##));
    }

    #[test]
    fn vue27_sfc_asset_url_transform_honors_base_and_include_absolute() {
        let mut base_options = options();
        base_options.sfc_asset_url_transform = Some(Vue2SfcAssetUrlTransformOptions {
            base: Some("/base/".into()),
            ..Vue2SfcAssetUrlTransformOptions::default()
        });
        let base = compile(
            r#"<div><img src="./logo.png" srcset="./logo.png 2x, @/logo.png 3x"><img src="@/alias.png"></div>"#,
            base_options,
        );
        let base_code = format!("{}{}", base.render, base.static_render_fns.join(""));
        assert!(base_code.contains(r#""src":"/base/logo.png""#));
        assert!(base_code
            .contains(r#""srcset":"/base/logo.png" + " 2x, " + require("@/logo.png") + " 3x""#));
        assert!(base_code.contains(r#""src":require("@/alias.png")"#));

        let mut absolute_options = options();
        absolute_options.sfc_asset_url_transform = Some(Vue2SfcAssetUrlTransformOptions {
            include_absolute: true,
            ..Vue2SfcAssetUrlTransformOptions::default()
        });
        let absolute = compile(r#"<img src="/logo.png">"#, absolute_options);
        let absolute_code = format!("{}{}", absolute.render, absolute.static_render_fns.join(""));
        assert!(absolute_code.contains(r#""src":require("/logo.png")"#));
    }

    #[test]
    fn warns_for_vue2_duplicate_raw_attrs_and_invalid_dynamic_args() {
        let duplicate = compile(r#"<p class="one" class="two"></p>"#, options());
        assert!(duplicate
            .errors
            .iter()
            .any(|error| error.msg.contains("duplicate attribute")));

        for template in [
            r#"<div v-bind:['foo' + bar]="baz"/>"#,
            r#"<div :['foo' + bar]="baz"/>"#,
            r#"<div @['foo' + bar]="baz"/>"#,
            r#"<foo #['foo' + bar]="baz"/>"#,
            r#"<div :['foo' + bar].some.mod="baz"/>"#,
        ] {
            let result = compile(template, options());
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.msg.contains("Invalid dynamic argument expression")),
                "{template}"
            );
        }
    }

    #[test]
    fn collects_vue2_source_ranges_like_official_compiler() {
        let text_root = compile("hello", options());
        assert_eq!(text_root.errors.len(), 1);
        assert_eq!(text_root.errors[0].start, Some(0));
        assert_eq!(text_root.errors[0].end, None);

        let invalid_expr = compile(r#"<div v-if="a----">{{ b++++ }}</div>"#, options());
        assert_eq!(invalid_expr.errors.len(), 2);
        assert!(invalid_expr.errors[0]
            .msg
            .contains(r#"Raw expression: v-if="a----""#));
        assert_eq!(invalid_expr.errors[0].start, Some(5));
        assert_eq!(invalid_expr.errors[0].end, Some(17));
        assert!(invalid_expr.errors[1]
            .msg
            .contains("Raw expression: {{ b++++ }}"));
        assert_eq!(invalid_expr.errors[1].start, Some(18));
        assert_eq!(invalid_expr.errors[1].end, Some(29));

        let unclosed = compile("<div><span></div>", options());
        assert_eq!(unclosed.errors.len(), 1);
        assert_eq!(unclosed.errors[0].start, Some(5));
        assert_eq!(unclosed.errors[0].end, Some(11));

        let slot_key = compile(r#"<div><slot v-bind:key="key" /></div>"#, options());
        assert_eq!(slot_key.errors.len(), 1);
        assert_eq!(slot_key.errors[0].start, Some(11));
        assert_eq!(slot_key.errors[0].end, Some(27));
    }

    #[test]
    fn optimizer_marks_static_roots() {
        let result = compile("<h1 id=\"x\"><span>hello</span></h1>", options());
        let root = result.element_ast.as_ref().unwrap();
        assert!(root.static_node);
        assert!(root.static_root);
        assert!(result.render.contains("_m(0)"));
    }

    #[test]
    fn optimizer_honors_platform_reserved_tag_options() {
        let mut parsed = compile("<h1 id=\"x\">hello</h1>", options())
            .element_ast
            .unwrap();
        let mut optimizer_options = options();
        optimizer_options.reserved_tags = Some(Vec::new());
        optimizer_options.use_default_reserved_tags = false;
        optimize(&mut parsed, &optimizer_options);
        assert!(!parsed.static_node);
    }

    #[test]
    fn parser_honors_platform_namespace_options() {
        let mut parse_options = options();
        parse_options.tag_namespaces = BTreeMap::new();
        parse_options.use_default_tag_namespaces = false;
        let root = compile("<svg><text>hello</text></svg>", parse_options)
            .element_ast
            .unwrap();
        assert_eq!(root.ns, None);
    }

    #[test]
    fn code_frame_matches_vue2_shape() {
        let source = "<div>\n  <span key=\"one\"></span>\n</div>";
        let start = source.find("key").unwrap();
        let frame = generate_code_frame(source, start, start + 9);
        assert!(frame.contains("2  |    <span key=\"one\"></span>"));
        assert!(frame.contains("^"));

        let multiline = "<div attr=\"some\n  multiline\nattr\n\">\n</div>";
        let multiline_start = multiline.find("attr=").unwrap();
        let multiline_end = multiline.find("\">").unwrap() + 1;
        assert_eq!(
            generate_code_frame(multiline, multiline_start, multiline_end),
            "1  |  <div attr=\"some\n   |       ^^^^^^^^^^\n2  |    multiline\n   |  ^^^^^^^^^^^\n3  |  attr\n   |  ^^^^\n4  |  \">\n   |  ^"
        );
    }
}
