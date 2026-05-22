#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_ast::{NodeId, Vue2Ast, Vue2NodeKind};
use vuec_diagnostics::{Diagnostic, DiagnosticSink, Severity};
use vuec_html::{HtmlTokenKind, HtmlTokenizer};
use vuec_js::JsAstStore;
use vuec_source::{FileId, Span};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2CompileOptions {
    pub modules: Vec<String>,
    pub directives: Vec<String>,
    pub warn: bool,
    pub output_source_range: bool,
    pub comments: bool,
    pub delimiters: Option<[String; 2]>,
    pub whitespace: Option<String>,
    pub preserve_whitespace: bool,
    pub should_decode_newlines: bool,
    pub should_decode_newlines_for_href: bool,
    pub optimize: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Warning {
    pub msg: String,
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub tip: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Error {
    pub msg: String,
    pub start: Option<usize>,
    pub end: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2CompiledResult {
    pub ast: Vue2Ast,
    pub render: String,
    pub static_render_fns: Vec<String>,
    pub errors: Vec<Vue2Error>,
    pub tips: Vec<Vue2Warning>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2FunctionResult {
    pub render: String,
    pub static_render_fns: Vec<String>,
    pub warnings: Vec<Vue2Warning>,
    pub errors: Vec<String>,
}

pub struct Vue2Compiler {
    js: JsAstStore,
}

impl Vue2Compiler {
    pub fn new() -> Self {
        Self {
            js: JsAstStore::new(),
        }
    }

    pub fn compile(&self, template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
        let mut diagnostics = DiagnosticSink::default();
        let ast = parse_template(&mut diagnostics, template.trim(), &options);
        let mut static_render_fns = Vec::new();
        let render = generate_render(&ast, &options, &mut static_render_fns);
        let diagnostics_messages = diagnostics
            .as_slice()
            .iter()
            .map(render_diagnostic_message)
            .collect();
        let (errors, tips) = split_compilation_issues(&diagnostics);
        Vue2CompiledResult {
            ast,
            render,
            static_render_fns,
            errors,
            tips,
            diagnostics: diagnostics_messages,
        }
    }

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

    pub fn compile_ssr(&self, template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
        let mut compiled = self.compile(template, options);
        if !compiled.render.contains("_ssr") {
            compiled.render = format!(
                "function ssrRender(_ctx, _push, _parent, _attrs){{return {}}}",
                compiled.render
            );
        }
        compiled
    }

    pub fn js(&self) -> &JsAstStore {
        &self.js
    }
}

impl Default for Vue2Compiler {
    fn default() -> Self {
        Self::new()
    }
}

pub fn compile(template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
    Vue2Compiler::new().compile(template, options)
}

pub fn compile_to_functions(template: &str, options: Vue2CompileOptions) -> Vue2FunctionResult {
    Vue2Compiler::new().compile_to_functions(template, options)
}

pub fn compile_ssr(template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
    Vue2Compiler::new().compile_ssr(template, options)
}

fn parse_template(
    diagnostics: &mut DiagnosticSink,
    template: &str,
    options: &Vue2CompileOptions,
) -> Vue2Ast {
    let mut ast = Vue2Ast::new();
    let root = ast.push(
        Vue2NodeKind::Root,
        Some(Span::new(FileId(0), 0, template.len())),
    );
    ast.set_root(root);
    let tokens = HtmlTokenizer::new(template).tokenize();
    for token in tokens {
        match token.kind {
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing: _,
            } => {
                let id = ast.push(
                    Vue2NodeKind::Element { tag: name },
                    Some(Span::new(FileId(0), token.start, token.end)),
                );
                if !attributes.is_empty() && options.warn {
                    diagnostics.push(Diagnostic {
                        code: "W_VUE2_ATTRS".into(),
                        severity: Severity::Warning,
                        message: "element has attributes".into(),
                        span: Some(Span::new(FileId(0), token.start, token.end)),
                        notes: Vec::new(),
                    });
                }
                ast.node_mut(root).unwrap().children.push(id);
            }
            HtmlTokenKind::Text(text) => {
                if let Some(id) = push_text_node(&mut ast, root, &text, token.start) {
                    ast.node_mut(root).unwrap().children.push(id);
                }
            }
            HtmlTokenKind::Comment(text) if options.comments => {
                let id = ast.push(
                    Vue2NodeKind::Comment { value: text },
                    Some(Span::new(FileId(0), token.start, token.end)),
                );
                ast.node_mut(root).unwrap().children.push(id);
            }
            HtmlTokenKind::Comment(_) => {}
            HtmlTokenKind::EndTag { .. }
            | HtmlTokenKind::Cdata(_)
            | HtmlTokenKind::Doctype(_)
            | HtmlTokenKind::Eof => {}
        }
    }
    if ast.root.is_none() {
        diagnostics.push(Diagnostic {
            code: "E_VUE2_NO_ROOT".into(),
            severity: Severity::Error,
            message: "template requires a root element".into(),
            span: None,
            notes: Vec::new(),
        });
    }
    ast
}

fn push_text_node(ast: &mut Vue2Ast, parent: NodeId, text: &str, start: usize) -> Option<NodeId> {
    if text.trim().is_empty() {
        return None;
    }
    let id = ast.push(
        Vue2NodeKind::Text {
            value: text.to_string(),
        },
        Some(Span::new(FileId(0), start, start + text.len())),
    );
    ast.node_mut(parent).unwrap().children.push(id);
    Some(id)
}

fn generate_render(
    ast: &Vue2Ast,
    options: &Vue2CompileOptions,
    static_render_fns: &mut Vec<String>,
) -> String {
    let Some(root_id) = ast.root else {
        return "with(this){return _c('div')}".into();
    };
    let code = gen_element(ast, root_id, options, static_render_fns);
    format!("with(this){{return {code}}}")
}

fn gen_element(
    ast: &Vue2Ast,
    node_id: NodeId,
    options: &Vue2CompileOptions,
    static_render_fns: &mut Vec<String>,
) -> String {
    let Some(node) = ast.node(node_id) else {
        return "_e()".into();
    };
    match &node.kind {
        Vue2NodeKind::Root => gen_children(ast, &node.children, options, static_render_fns),
        Vue2NodeKind::Element { tag } => {
            let children = gen_children(ast, &node.children, options, static_render_fns);
            if children.is_empty() {
                format!("_c('{tag}')")
            } else {
                format!("_c('{tag}',{children})")
            }
        }
        Vue2NodeKind::Text { value } => format!("_v({:?})", value),
        Vue2NodeKind::Interpolation { expression } => format!("_v(_s({expression}))"),
        Vue2NodeKind::Comment { value } => format!("_e({:?})", value),
        Vue2NodeKind::Directive { name, expression } => {
            format!("/* directive:{name}:{:?} */", expression)
        }
    }
}

fn gen_children(
    ast: &Vue2Ast,
    children: &[NodeId],
    options: &Vue2CompileOptions,
    static_render_fns: &mut Vec<String>,
) -> String {
    let rendered: Vec<String> = children
        .iter()
        .filter_map(|child_id| {
            ast.node(*child_id)
                .map(|_| gen_element(ast, *child_id, options, static_render_fns))
        })
        .collect();
    if rendered.is_empty() {
        String::new()
    } else if rendered.len() == 1 {
        rendered[0].clone()
    } else {
        format!("[{}]", rendered.join(","))
    }
}

fn split_compilation_issues(diagnostics: &DiagnosticSink) -> (Vec<Vue2Error>, Vec<Vue2Warning>) {
    let mut errors = Vec::new();
    let mut tips = Vec::new();
    for diagnostic in diagnostics.as_slice() {
        match diagnostic.severity {
            Severity::Error => errors.push(Vue2Error {
                msg: diagnostic.message.clone(),
                start: diagnostic.span.map(|span| span.start.0),
                end: diagnostic.span.map(|span| span.end.0),
            }),
            Severity::Warning | Severity::Tip | Severity::Note => tips.push(Vue2Warning {
                msg: diagnostic.message.clone(),
                start: diagnostic.span.map(|span| span.start.0),
                end: diagnostic.span.map(|span| span.end.0),
                tip: matches!(diagnostic.severity, Severity::Tip),
            }),
        }
    }
    (errors, tips)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_returns_vue2_shapes() {
        let result = compile(
            "<div>{{ msg }}</div>",
            Vue2CompileOptions {
                comments: true,
                warn: true,
                ..Vue2CompileOptions::default()
            },
        );
        assert!(result.render.contains("with(this)"));
        assert!(result.static_render_fns.is_empty());
        assert!(result.ast.root.is_some());
    }

    #[test]
    fn compile_to_functions_wraps_render() {
        let result = compile_to_functions("<div/>", Vue2CompileOptions::default());
        assert!(result.render.contains("with(this)"));
    }
}
