#![forbid(unsafe_code)]

use oxc_ast::ast::{
    ExportDefaultDeclaration, ExportDefaultDeclarationKind, ExportNamedDeclaration,
    ExportSpecifier, ModuleExportName, Statement,
};
use oxc_span::GetSpan;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use vuec_codegen::SourceMapArtifact;
use vuec_diagnostics::{Diagnostic, Severity};
use vuec_html::{HtmlAttribute, HtmlTokenKind, HtmlTokenizer};
use vuec_js::{JsAstStore, JsParseMode};
use vuec_source::{FileId, SourceMap, Span};
use vuec_style::{compile_style, StyleCompileOptions};
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions};
use vuec_vue3_dom::{
    apply_dom_parser_defaults, compile as compile_dom, AssetUrlOptions, DomCompilerOptions,
};
use vuec_vue3_ssr::{compile as compile_ssr, SsrCompilerOptions};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcBlock {
    pub type_name: String,
    pub content: String,
    pub attrs: SfcBlockAttrs,
    pub loc: SfcBlockLocation,
    #[serde(skip)]
    pub content_start: usize,
    #[serde(skip)]
    pub content_end: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcBlockAttrs {
    pub lang: Option<String>,
    pub src: Option<String>,
    pub scoped: bool,
    pub module: Option<String>,
    pub setup: bool,
    pub generic: Option<String>,
    pub raw: BTreeMap<String, SfcAttrValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SfcAttrValue {
    Bool(bool),
    String(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcBlockLocation {
    pub start: usize,
    pub end: usize,
    pub source_file: FileId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcDescriptor {
    pub filename: String,
    pub source: String,
    pub source_file: FileId,
    pub template: Option<SfcBlock>,
    pub script: Option<SfcBlock>,
    pub script_setup: Option<SfcBlock>,
    pub styles: Vec<SfcBlock>,
    pub custom_blocks: Vec<SfcBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue27ParseComponentOptions {
    pub pad: Vue27SfcPad,
    pub deindent: Option<bool>,
    pub output_source_range: bool,
}

impl Default for Vue27ParseComponentOptions {
    fn default() -> Self {
        Self {
            pad: Vue27SfcPad::False,
            deindent: None,
            output_source_range: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue27SfcPad {
    #[default]
    False,
    True,
    Line,
    Space,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue27ParseComponentResult {
    pub descriptor: SfcDescriptor,
    pub errors: Vec<Vue27SfcParseError>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue27SfcParseError {
    pub msg: String,
    pub start: Option<usize>,
    pub end: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcTemplateCompileOptions {
    pub id: Option<String>,
    pub ssr: bool,
    pub scope_id: Option<String>,
    pub slotted: bool,
    pub is_prod: bool,
    pub transform_asset_urls: bool,
    pub asset_url_options: AssetUrlOptions,
}

impl Default for SfcTemplateCompileOptions {
    fn default() -> Self {
        Self {
            id: None,
            ssr: false,
            scope_id: None,
            slotted: false,
            is_prod: false,
            transform_asset_urls: true,
            asset_url_options: AssetUrlOptions::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcScriptCompileOptions {
    pub id: Option<String>,
    pub inline_template: bool,
    pub ref_sugar: bool,
}

impl Default for SfcScriptCompileOptions {
    fn default() -> Self {
        Self {
            id: None,
            inline_template: false,
            ref_sugar: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcStyleCompileOptions {
    pub id: Option<String>,
    pub scoped: bool,
    pub vars: Vec<String>,
}

impl Default for SfcStyleCompileOptions {
    fn default() -> Self {
        Self {
            id: None,
            scoped: false,
            vars: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcTemplateCompileResult {
    pub code: String,
    pub map: Option<SourceMapArtifact>,
    pub errors: Vec<SfcTemplateError>,
    pub bindings: Vec<String>,
    pub ast_summary: String,
    pub ast: String,
    pub preamble: String,
    pub source: String,
    pub tips: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcTemplateError {
    pub code: u32,
    pub loc: SfcSourceLocation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcSourceLocation {
    pub start: SfcPosition,
    pub end: SfcPosition,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcPosition {
    pub column: usize,
    pub line: usize,
    pub offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcScriptBlock {
    #[serde(rename = "type")]
    pub type_name: String,
    pub content: String,
    pub loc: Option<SfcBlockLocation>,
    pub attrs: SfcBlockAttrs,
    pub setup: bool,
    pub lang: Option<String>,
    pub bindings: BTreeMap<String, String>,
    pub imports: Vec<String>,
    pub errors: Vec<String>,
    pub map: Option<SourceMapArtifact>,
    #[serde(rename = "scriptAst")]
    pub script_ast: Vec<String>,
    #[serde(rename = "scriptSetupAst")]
    pub script_setup_ast: Vec<String>,
    pub deps: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcStyleCompileResult {
    pub code: String,
    pub map: Option<SourceMapArtifact>,
    pub errors: Vec<String>,
    pub dependencies: Vec<String>,
    #[serde(rename = "rawResult")]
    pub raw_result: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue27RewriteDefaultOptions {
    pub typescript: bool,
    #[serde(default)]
    pub decorators: bool,
}

pub struct SfcCompiler {
    sources: SourceMap,
    js: JsAstStore,
}

impl SfcCompiler {
    pub fn new() -> Self {
        Self {
            sources: SourceMap::default(),
            js: JsAstStore::new(),
        }
    }

    pub fn parse(&mut self, filename: impl Into<String>, source: &str) -> SfcDescriptor {
        let filename = filename.into();
        let source_file = self.sources.add_file(
            Some(std::path::PathBuf::from(&filename)),
            source.to_string(),
        );
        descriptor_from_blocks(
            filename,
            source,
            source_file,
            extract_sfc_blocks(source, source_file, SfcBlockContentMode::Raw).blocks,
        )
    }

    pub fn parse_vue27_component(
        &mut self,
        source: &str,
        options: Vue27ParseComponentOptions,
    ) -> Vue27ParseComponentResult {
        let filename = "anonymous.vue".to_string();
        let source_file = self.sources.add_file(
            Some(std::path::PathBuf::from(&filename)),
            source.to_string(),
        );
        let extracted = extract_sfc_blocks(
            source,
            source_file,
            SfcBlockContentMode::Vue27 { options: &options },
        );
        let descriptor = descriptor_from_blocks(filename, source, source_file, extracted.blocks);

        Vue27ParseComponentResult {
            descriptor,
            errors: if options.output_source_range {
                extracted.errors
            } else {
                extracted
                    .errors
                    .into_iter()
                    .map(|error| Vue27SfcParseError {
                        msg: error.msg,
                        start: None,
                        end: None,
                    })
                    .collect()
            },
        }
    }

    pub fn compile_template(
        &self,
        descriptor: &SfcDescriptor,
        options: SfcTemplateCompileOptions,
    ) -> SfcTemplateCompileResult {
        let Some(template) = descriptor.template.as_ref() else {
            return SfcTemplateCompileResult {
                code: String::new(),
                map: None,
                errors: vec![SfcTemplateError {
                    code: 0,
                    loc: SfcSourceLocation {
                        start: SfcPosition {
                            column: 1,
                            line: 1,
                            offset: 0,
                        },
                        end: SfcPosition {
                            column: 1,
                            line: 1,
                            offset: 0,
                        },
                        source: String::new(),
                    },
                }],
                bindings: Vec::new(),
                ast_summary: "missing-template".into(),
                ast: String::new(),
                preamble: String::new(),
                source: String::new(),
                tips: Vec::new(),
            };
        };
        let mut core = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            hoist_static: true,
            cache_handlers: true,
            scope_id: options.scope_id.clone(),
            slotted: options.slotted,
            source_map: true,
            ..Vue3CompilerOptions::default()
        };
        apply_dom_parser_defaults(&mut core);
        let source = TemplateSource {
            filename: descriptor.filename.clone(),
            source: template.content.clone(),
            file_id: descriptor.source_file,
            base_offset: template.loc.start,
        };
        if options.ssr {
            let result = compile_ssr(
                source,
                SsrCompilerOptions {
                    core,
                    scope_id: options.scope_id.clone(),
                    slotted: options.slotted,
                    transform_asset_urls: options.transform_asset_urls,
                    asset_url_options: options.asset_url_options.clone(),
                },
            );
            let ast_summary = result.ast_summary;
            return SfcTemplateCompileResult {
                code: result.code,
                map: result.map,
                errors: sfc_template_errors_from_diagnostics(
                    &result.diagnostics,
                    &template.content,
                ),
                bindings: Vec::new(),
                ast_summary: ast_summary.clone(),
                ast: format!("ast:{ast_summary}"),
                preamble: result.preamble,
                source: template.content.clone(),
                tips: Vec::new(),
            };
        } else {
            let result = compile_dom(
                source,
                DomCompilerOptions {
                    core,
                    transform_asset_urls: options.transform_asset_urls,
                    asset_url_options: options.asset_url_options.clone(),
                    ..DomCompilerOptions::default()
                },
            );
            let ast_summary = result.ast_summary;
            return SfcTemplateCompileResult {
                code: result.code,
                map: result.map,
                errors: sfc_template_errors_from_diagnostics(
                    &result.diagnostics,
                    &template.content,
                ),
                bindings: Vec::new(),
                ast_summary: ast_summary.clone(),
                ast: format!("ast:{ast_summary}"),
                preamble: result.preamble,
                source: template.content.clone(),
                tips: Vec::new(),
            };
        }
    }

    pub fn compile_template_source(
        &self,
        filename: impl Into<String>,
        source: &str,
        options: SfcTemplateCompileOptions,
    ) -> SfcTemplateCompileResult {
        let filename = filename.into();
        let raw_source = source.to_string();
        let side_effect_errors = side_effect_tag_errors(source);
        let mut core = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            hoist_static: true,
            cache_handlers: true,
            scope_id: options.scope_id.clone(),
            slotted: options.slotted,
            source_map: true,
            ..Vue3CompilerOptions::default()
        };
        apply_dom_parser_defaults(&mut core);
        let template_source = TemplateSource {
            filename: filename.clone(),
            source: raw_source.clone(),
            file_id: FileId(0),
            base_offset: 0,
        };
        if options.ssr {
            let result = compile_ssr(
                template_source,
                SsrCompilerOptions {
                    core,
                    scope_id: options.scope_id.clone(),
                    slotted: options.slotted,
                    transform_asset_urls: options.transform_asset_urls,
                    asset_url_options: options.asset_url_options.clone(),
                },
            );
            return SfcTemplateCompileResult {
                code: result.code,
                map: result.map,
                errors: merge_template_errors(
                    side_effect_errors,
                    sfc_template_errors_from_diagnostics(&result.diagnostics, &raw_source),
                ),
                bindings: Vec::new(),
                ast_summary: result.ast_summary.clone(),
                ast: json!({
                    "type": 0,
                    "source": raw_source,
                    "transformed": true,
                })
                .to_string(),
                preamble: result.preamble,
                source: raw_source,
                tips: Vec::new(),
            };
        }
        let result = compile_dom(
            template_source,
            DomCompilerOptions {
                core,
                transform_asset_urls: options.transform_asset_urls,
                asset_url_options: options.asset_url_options,
                ..DomCompilerOptions::default()
            },
        );
        SfcTemplateCompileResult {
            code: result.code,
            map: result.map,
            errors: merge_template_errors(
                side_effect_errors,
                sfc_template_errors_from_diagnostics(&result.diagnostics, &raw_source),
            ),
            bindings: Vec::new(),
            ast_summary: result.ast_summary.clone(),
            ast: json!({
                "type": 0,
                "source": raw_source.clone(),
                "transformed": true,
            })
            .to_string(),
            preamble: result.preamble,
            source: raw_source,
            tips: Vec::new(),
        }
    }

    pub fn compile_script(
        &mut self,
        descriptor: &SfcDescriptor,
        _options: SfcScriptCompileOptions,
    ) -> SfcScriptBlock {
        let mut raw_content = String::new();
        let mut script_ast = Vec::new();
        let mut script_setup_ast = Vec::new();
        let source_type = script_source_type(descriptor);
        if let Some(script) = descriptor.script.as_ref() {
            raw_content.push_str(&script.content);
            let id = self.js.register_program(
                script.content.clone(),
                Span::new(descriptor.source_file, script.loc.start, script.loc.end),
                script_mode(&script.attrs),
                source_type,
            );
            script_ast.push(format!("JsProgramId({})", id.0));
        }
        if let Some(script_setup) = descriptor.script_setup.as_ref() {
            if !raw_content.is_empty() {
                raw_content.push('\n');
            }
            raw_content.push_str(&script_setup.content);
            let id = self.js.register_program(
                script_setup.content.clone(),
                Span::new(
                    descriptor.source_file,
                    script_setup.loc.start,
                    script_setup.loc.end,
                ),
                script_mode(&script_setup.attrs),
                source_type,
            );
            script_setup_ast.push(format!("JsProgramId({})", id.0));
        }
        let summary = self.js.summarize_program(&raw_content, source_type);
        let bindings = script_bindings(&summary.bindings);
        let imports = summary.imports;
        let attrs = descriptor
            .script
            .as_ref()
            .or(descriptor.script_setup.as_ref())
            .map(|block| block.attrs.clone())
            .unwrap_or_default();
        SfcScriptBlock {
            type_name: "script".into(),
            content: script_content(
                descriptor,
                &raw_content,
                &summary.bindings,
                descriptor.filename.as_str(),
            ),
            loc: descriptor
                .script
                .as_ref()
                .or(descriptor.script_setup.as_ref())
                .map(|block| block.loc.clone()),
            attrs,
            setup: descriptor.script_setup.is_some(),
            lang: descriptor
                .script_setup
                .as_ref()
                .or(descriptor.script.as_ref())
                .and_then(|block| block.attrs.lang.clone()),
            bindings,
            imports,
            errors: summary.errors,
            map: None,
            script_ast,
            script_setup_ast,
            deps: Vec::new(),
        }
    }

    pub fn compile_style(
        &self,
        descriptor: &SfcDescriptor,
        options: SfcStyleCompileOptions,
    ) -> SfcStyleCompileResult {
        let mut code = String::new();
        let mut errors = Vec::new();
        let mut dependencies = Vec::new();
        let mut raw_result = Vec::new();
        let mut map = None;
        for style in &descriptor.styles {
            let result = compile_style(
                &style.content,
                StyleCompileOptions {
                    id: options.id.clone(),
                    scoped: options.scoped || style.attrs.scoped,
                    modules: style.attrs.module.is_some(),
                    vars: scoped_style_vars(options.id.as_deref(), &options.vars),
                    filename: Some(descriptor.filename.clone()),
                    source_map: false,
                },
            );
            if !code.is_empty() && !result.code.is_empty() {
                code.push('\n');
            }
            code.push_str(&result.code);
            errors.extend(result.errors);
            if map.is_none() {
                map = result.map;
            }
            dependencies.extend(style_dependencies(style));
            raw_result.push("postcss-result".to_string());
        }
        dependencies.sort();
        dependencies.dedup();
        SfcStyleCompileResult {
            code,
            map,
            errors,
            dependencies,
            raw_result,
        }
    }

    pub fn rewrite_vue27_default(
        &self,
        input: &str,
        variable: &str,
        options: Vue27RewriteDefaultOptions,
    ) -> String {
        rewrite_vue27_default(input, variable, options)
    }

    pub fn js(&self) -> &JsAstStore {
        &self.js
    }
}

impl Default for SfcCompiler {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
struct ExtractedSfcBlocks {
    blocks: Vec<SfcBlock>,
    errors: Vec<Vue27SfcParseError>,
}

#[derive(Clone, Copy)]
enum SfcBlockContentMode<'a> {
    Raw,
    Vue27 {
        options: &'a Vue27ParseComponentOptions,
    },
}

struct OpenSfcBlock {
    type_name: String,
    attrs: SfcBlockAttrs,
    start: usize,
    open_end: usize,
    self_closing: bool,
}

fn descriptor_from_blocks(
    filename: String,
    source: &str,
    source_file: FileId,
    blocks: Vec<SfcBlock>,
) -> SfcDescriptor {
    let mut descriptor = SfcDescriptor {
        filename,
        source: source.to_string(),
        source_file,
        template: None,
        script: None,
        script_setup: None,
        styles: Vec::new(),
        custom_blocks: Vec::new(),
    };

    for block in blocks {
        match block.type_name.as_str() {
            "template" => descriptor.template = Some(block),
            "script" => {
                if block.attrs.setup {
                    descriptor.script_setup = Some(block);
                } else {
                    descriptor.script = Some(block);
                }
            }
            "style" => descriptor.styles.push(block),
            _ => descriptor.custom_blocks.push(block),
        }
    }

    descriptor
}

fn extract_sfc_blocks(
    source: &str,
    source_file: FileId,
    mode: SfcBlockContentMode<'_>,
) -> ExtractedSfcBlocks {
    let mut blocks = Vec::new();
    let mut errors = Vec::new();
    let mut stack: Vec<(String, usize, usize)> = Vec::new();
    let mut current_block: Option<OpenSfcBlock> = None;
    let mut depth = 0usize;
    let mut malformed_tail_start = None;
    let mut tokenizer = HtmlTokenizer::new(source);

    loop {
        let token = tokenizer.next_token();
        match token.kind {
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if depth == 0 {
                    current_block = Some(OpenSfcBlock {
                        type_name: name.clone(),
                        attrs: attrs_from_html(&attributes),
                        start: token.start,
                        open_end: token.end,
                        self_closing,
                    });
                }

                if !self_closing {
                    if depth == 0 && is_plain_text_sfc_tag(&name) {
                        consume_plain_text_element(
                            source,
                            source_file,
                            mode,
                            &mut tokenizer,
                            &mut blocks,
                            &mut current_block,
                            token.end,
                        );
                        depth = 0;
                    } else {
                        stack.push((name, token.start, token.end));
                        depth += 1;
                    }
                } else if depth == 0 {
                    if let Some(open) = current_block.take() {
                        blocks.push(finish_sfc_block(
                            source,
                            source_file,
                            mode,
                            open,
                            0,
                            token.end,
                        ));
                    }
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if depth == 0 {
                    continue;
                }
                let Some(pos) = matching_open_pos(&stack, &name) else {
                    if name.is_empty() {
                        malformed_tail_start.get_or_insert(token.start);
                    } else if name.eq_ignore_ascii_case("br") && depth == 0 {
                        current_block = Some(OpenSfcBlock {
                            type_name: name,
                            attrs: SfcBlockAttrs::default(),
                            start: token.start,
                            open_end: token.end,
                            self_closing: true,
                        });
                    }
                    continue;
                };
                while stack.len() > pos + 1 {
                    if let Some((tag, start, end)) = stack.pop() {
                        errors.push(Vue27SfcParseError {
                            msg: format!("tag <{tag}> has no matching end tag."),
                            start: Some(start),
                            end: Some(end),
                        });
                        depth = depth.saturating_sub(1);
                    }
                }
                stack.pop();
                if depth == 1 {
                    if let Some(open) = current_block.take() {
                        blocks.push(finish_sfc_block(
                            source,
                            source_file,
                            mode,
                            open,
                            token.start,
                            token.end,
                        ));
                    }
                }
                depth = depth.saturating_sub(1);
            }
            HtmlTokenKind::Eof => {
                while let Some((tag, start, end)) = stack.pop() {
                    errors.push(Vue27SfcParseError {
                        msg: format!("tag <{tag}> has no matching end tag."),
                        start: Some(start),
                        end: Some(end),
                    });
                    if stack.is_empty() {
                        if let Some(open) = current_block.take() {
                            let fallback_end = malformed_tail_start.unwrap_or_else(|| {
                                malformed_tail_content_end(source, &open, token.start)
                            });
                            blocks.push(finish_sfc_block(
                                source,
                                source_file,
                                mode,
                                open,
                                fallback_end,
                                token.end,
                            ));
                        }
                    }
                }
                break;
            }
            _ => {}
        }
    }

    blocks.sort_by_key(|block| block.loc.start);
    ExtractedSfcBlocks { blocks, errors }
}

fn consume_plain_text_element(
    source: &str,
    source_file: FileId,
    mode: SfcBlockContentMode<'_>,
    tokenizer: &mut HtmlTokenizer<'_>,
    blocks: &mut Vec<SfcBlock>,
    current_block: &mut Option<OpenSfcBlock>,
    content_start: usize,
) {
    let Some(open) = current_block.take() else {
        return;
    };
    let lower_name = open.type_name.to_ascii_lowercase();
    let rest = &source[content_start..];
    let needle = format!("</{lower_name}");
    if let Some(close_offset) = find_ascii_case_insensitive(rest, &needle) {
        let close_start = content_start + close_offset;
        let close_end = source[close_start..]
            .find('>')
            .map(|offset| close_start + offset + 1)
            .unwrap_or(source.len());
        tokenizer.set_cursor(close_end);
        blocks.push(finish_sfc_block(
            source,
            source_file,
            mode,
            open,
            close_start,
            close_end,
        ));
    } else {
        tokenizer.set_cursor(source.len());
        blocks.push(finish_sfc_block(
            source,
            source_file,
            mode,
            open,
            source.len(),
            source.len(),
        ));
    }
}

fn finish_sfc_block(
    source: &str,
    source_file: FileId,
    mode: SfcBlockContentMode<'_>,
    open: OpenSfcBlock,
    content_end: usize,
    close_end: usize,
) -> SfcBlock {
    let content_start = open.open_end.min(source.len());
    let raw_end = content_end.min(source.len()).max(content_start);
    let mut content = source[content_start..raw_end].to_string();
    if let SfcBlockContentMode::Vue27 { options } = mode {
        if should_vue27_deindent(&open, options) {
            content = deindent(&content);
        }
        if open.type_name != "template" && options.pad.is_enabled() {
            content = vue27_pad_content(source, &open, &options.pad) + &content;
        }
    }

    SfcBlock {
        type_name: open.type_name,
        content,
        attrs: open.attrs,
        loc: SfcBlockLocation {
            start: open.start,
            end: if open.self_closing { 0 } else { close_end },
            source_file,
        },
        content_start,
        content_end: raw_end,
    }
}

fn matching_open_pos(stack: &[(String, usize, usize)], name: &str) -> Option<usize> {
    let lower_name = name.to_ascii_lowercase();
    stack
        .iter()
        .rposition(|(tag, _, _)| tag.to_ascii_lowercase() == lower_name)
}

fn malformed_tail_content_end(source: &str, open: &OpenSfcBlock, fallback: usize) -> usize {
    let fallback = fallback.min(source.len());
    let tail = &source[open.open_end.min(source.len())..fallback];
    let Some(last_lt) = tail.rfind('<') else {
        return fallback;
    };
    let absolute = open.open_end + last_lt;
    if source[absolute..fallback].contains('>') {
        return fallback;
    }
    absolute
}

fn attrs_from_html(attributes: &[HtmlAttribute]) -> SfcBlockAttrs {
    let mut attrs = SfcBlockAttrs::default();
    for attribute in attributes {
        let value = attribute
            .value
            .as_ref()
            .map(|value| SfcAttrValue::String(value.clone()))
            .unwrap_or(SfcAttrValue::Bool(true));
        attrs.raw.insert(attribute.name.clone(), value.clone());
        match attribute.name.as_str() {
            "lang" => {
                if let SfcAttrValue::String(value) = value {
                    attrs.lang = Some(value);
                }
            }
            "src" => {
                if let SfcAttrValue::String(value) = value {
                    attrs.src = Some(value);
                }
            }
            "scoped" => {
                attrs.scoped = true;
            }
            "setup" => {
                attrs.setup = true;
            }
            "generic" => {
                if let SfcAttrValue::String(value) = value {
                    attrs.generic = Some(value);
                }
            }
            "module" => {
                attrs.module = Some(match value {
                    SfcAttrValue::String(value) => value,
                    SfcAttrValue::Bool(_) => String::new(),
                });
            }
            _ => {}
        }
    }
    attrs
}

fn is_plain_text_sfc_tag(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "script" | "style")
}

fn should_vue27_deindent(block: &OpenSfcBlock, options: &Vue27ParseComponentOptions) -> bool {
    if options.deindent == Some(true) {
        return true;
    }
    if options.deindent == Some(false) {
        return false;
    }
    !(block.type_name == "script"
        && block
            .attrs
            .lang
            .as_deref()
            .is_none_or(|lang| matches!(lang, "js" | "jsx" | "ts" | "tsx")))
}

fn deindent(source: &str) -> String {
    if !source
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, '\r' | '\n' | ' ' | '\t'))
    {
        return source.to_string();
    }
    let mut indent_char = None;
    let mut min_indent = usize::MAX;
    let lines = split_preserving_no_cr(source);
    for line in &lines {
        if line.chars().all(char::is_whitespace) {
            continue;
        }
        match indent_char {
            None => {
                let Some(ch) = line.chars().next() else {
                    continue;
                };
                if ch != ' ' && ch != '\t' {
                    return source.to_string();
                }
                indent_char = Some(ch);
                min_indent = min_indent.min(line.chars().take_while(|value| *value == ch).count());
            }
            Some(ch) => {
                min_indent = min_indent.min(line.chars().take_while(|value| *value == ch).count());
            }
        }
    }
    if min_indent == usize::MAX || min_indent == 0 {
        return source.to_string();
    }
    lines
        .iter()
        .map(|line| strip_chars(line, min_indent))
        .collect::<Vec<_>>()
        .join("\n")
}

fn split_preserving_no_cr(source: &str) -> Vec<String> {
    source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect()
}

fn strip_chars(source: &str, count: usize) -> String {
    let mut cursor = 0usize;
    for _ in 0..count {
        let Some(ch) = source[cursor..].chars().next() else {
            return String::new();
        };
        cursor += ch.len_utf8();
    }
    source[cursor..].to_string()
}

impl Vue27SfcPad {
    fn is_enabled(&self) -> bool {
        !matches!(self, Vue27SfcPad::False)
    }
}

fn vue27_pad_content(source: &str, block: &OpenSfcBlock, pad: &Vue27SfcPad) -> String {
    if matches!(pad, Vue27SfcPad::Space) {
        return source[..block.open_end]
            .chars()
            .map(|ch| if matches!(ch, '\n' | '\r') { ch } else { ' ' })
            .collect();
    }
    let offset = source[..block.open_end].split('\n').count();
    let pad_char = if block.type_name == "script" && block.attrs.lang.is_none() {
        "//\n"
    } else {
        "\n"
    };
    pad_char.repeat(offset.saturating_sub(1))
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}

fn rewrite_vue27_default(
    input: &str,
    variable: &str,
    options: Vue27RewriteDefaultOptions,
) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let source_type = if options.typescript {
        oxc_span::SourceType::ts()
    } else {
        oxc_span::SourceType::mjs()
    };
    let parsed = oxc_parser::Parser::new(&allocator, input, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        if !options.typescript {
            let ts_parsed = oxc_parser::Parser::new(&allocator, input, oxc_span::SourceType::ts())
                .with_options(oxc_parser::ParseOptions {
                    parse_regular_expression: true,
                    ..oxc_parser::ParseOptions::default()
                })
                .parse();
            if !ts_parsed.panicked && ts_parsed.errors.is_empty() {
                return rewrite_vue27_default_from_program(
                    input,
                    variable,
                    &ts_parsed.program.body,
                );
            }
        }
        return rewrite_vue27_default_lexical(input, variable);
    }

    rewrite_vue27_default_from_program(input, variable, &parsed.program.body)
}

fn rewrite_vue27_default_from_program(
    input: &str,
    variable: &str,
    body: &[Statement<'_>],
) -> String {
    let mut edits = SourceEdits::new(input);
    let mut found_default = false;
    for statement in body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                found_default = true;
                rewrite_export_default(input, variable, declaration, &mut edits);
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if rewrite_named_default_exports(input, variable, declaration, &mut edits) {
                    found_default = true;
                }
            }
            _ => {}
        }
    }
    if !found_default {
        edits.append(format!("\nconst {variable} = {{}}"));
    }
    edits.apply()
}

fn rewrite_export_default(
    input: &str,
    variable: &str,
    declaration: &ExportDefaultDeclaration<'_>,
    edits: &mut SourceEdits,
) {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                let fast_candidate = source_with_overwrite(
                    input,
                    declaration.span.start as usize,
                    id.span.start as usize,
                    "class ",
                );
                if has_vue27_default_export_like(input)
                    && has_vue27_default_export_like(&fast_candidate)
                {
                    let replace_start = class
                        .decorators
                        .last()
                        .map(|decorator| decorator.span.end as usize)
                        .unwrap_or(declaration.span.start as usize);
                    edits.overwrite(replace_start, id.span.start as usize, " class ");
                } else {
                    edits.overwrite(
                        declaration.span.start as usize,
                        id.span.start as usize,
                        "class ",
                    );
                }
                edits.append(format!("\nconst {variable} = {}", id.name));
                return;
            }
        }
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
                edits.overwrite(
                    declaration.span.start as usize,
                    function.span.start as usize,
                    "",
                );
                edits.append(format!("\nconst {variable} = {}", id.name));
                return;
            }
        }
        _ => {}
    }

    edits.overwrite(
        declaration.span.start as usize,
        export_default_declaration_value_start(input, declaration),
        format!("const {variable} ="),
    );
}

fn export_default_declaration_value_start(
    input: &str,
    declaration: &ExportDefaultDeclaration<'_>,
) -> usize {
    let start = declaration.span.start as usize;
    let end = declaration.declaration.span().start as usize;
    let segment = &input[start..end.min(input.len())];
    segment
        .find("default")
        .map(|offset| start + offset + "default".len())
        .unwrap_or(end)
}

fn rewrite_named_default_exports(
    input: &str,
    variable: &str,
    declaration: &ExportNamedDeclaration<'_>,
    edits: &mut SourceEdits,
) -> bool {
    let mut found = false;
    for specifier in &declaration.specifiers {
        if module_export_name(specifier.exported()) != Some("default") {
            continue;
        }
        found = true;
        let local_name = module_export_name(specifier.local()).unwrap_or("default");
        if let Some(source) = declaration.source.as_ref() {
            let source_value = source.value.to_string();
            if local_name == "default" {
                let end = specifier_end(
                    input,
                    specifier.local().span().end as usize,
                    declaration.span.end as usize,
                );
                edits.prepend(format!(
                    "import {{ default as __VUE_DEFAULT__ }} from '{}'\n",
                    source_value
                ));
                edits.overwrite(specifier.span.start as usize, end, "");
                edits.append(format!("\nconst {variable} = __VUE_DEFAULT__"));
            } else {
                let end = specifier_end(
                    input,
                    specifier.exported().span().end as usize,
                    declaration.span.end as usize,
                );
                edits.prepend(format!("import {{ {local_name} }} from '{source_value}'\n"));
                edits.overwrite(specifier.span.start as usize, end, "");
                edits.append(format!("\nconst {variable} = {local_name}"));
            }
        } else {
            let end = specifier_end(
                input,
                specifier.span.end as usize,
                declaration.span.end as usize,
            );
            edits.overwrite(specifier.span.start as usize, end, "");
            edits.append(format!("\nconst {variable} = {local_name}"));
        }
    }
    found
}

trait ExportSpecifierAccess<'a> {
    fn local(&self) -> &ModuleExportName<'a>;
    fn exported(&self) -> &ModuleExportName<'a>;
}

impl<'a> ExportSpecifierAccess<'a> for ExportSpecifier<'a> {
    fn local(&self) -> &ModuleExportName<'a> {
        &self.local
    }

    fn exported(&self) -> &ModuleExportName<'a> {
        &self.exported
    }
}

fn module_export_name<'a>(name: &'a ModuleExportName<'a>) -> Option<&'a str> {
    match name {
        ModuleExportName::IdentifierName(identifier) => Some(identifier.name.as_str()),
        ModuleExportName::IdentifierReference(identifier) => Some(identifier.name.as_str()),
        ModuleExportName::StringLiteral(literal) => Some(literal.value.as_str()),
    }
}

fn specifier_end(input: &str, mut end: usize, node_end: usize) -> usize {
    let node_end = node_end.min(input.len());
    let old_end = end;
    let mut has_comma = false;
    while end < node_end {
        let Some(ch) = input[end..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            end += ch.len_utf8();
        } else if ch == ',' {
            end += ch.len_utf8();
            has_comma = true;
            break;
        } else if ch == '}' {
            break;
        } else {
            break;
        }
    }
    if has_comma {
        end
    } else {
        old_end
    }
}

fn rewrite_vue27_default_lexical(input: &str, variable: &str) -> String {
    let Some(default_start) = find_export_default_keyword(input) else {
        return format!("{input}\nconst {variable} = {{}}");
    };
    let value_start = default_start + "default".len();
    let export_start = input[..default_start]
        .rfind("export")
        .unwrap_or(default_start);
    let mut output = String::new();
    output.push_str(&input[..export_start]);
    output.push_str(&format!("const {variable} ="));
    output.push_str(&input[value_start..]);
    output
}

fn find_export_default_keyword(input: &str) -> Option<usize> {
    let mut index = 0usize;
    while index < input.len() {
        let next = input[index..].find("export")? + index;
        if is_word_boundary(input, next, "export")
            && input[next + "export".len()..]
                .trim_start()
                .starts_with("default")
        {
            let default_start = next
                + "export".len()
                + input[next + "export".len()..]
                    .len()
                    .saturating_sub(input[next + "export".len()..].trim_start().len());
            if is_word_boundary(input, default_start, "default") {
                return Some(default_start);
            }
        }
        index = next + "export".len();
    }
    None
}

fn is_word_boundary(input: &str, start: usize, word: &str) -> bool {
    let before = input[..start].chars().next_back();
    let after = input[start + word.len()..].chars().next();
    !before.is_some_and(is_identifier_continue) && !after.is_some_and(is_identifier_continue)
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
}

fn source_with_overwrite(input: &str, start: usize, end: usize, replacement: &str) -> String {
    let start = start.min(input.len());
    let end = end.min(input.len()).max(start);
    let mut output = String::new();
    output.push_str(&input[..start]);
    output.push_str(replacement);
    output.push_str(&input[end..]);
    output
}

fn has_vue27_default_export_like(input: &str) -> bool {
    let mut index = 0usize;
    while let Some(offset) = input[index..].find("export") {
        let export_start = index + offset;
        if is_vue27_export_boundary(input, export_start)
            && input[export_start..].contains("default")
        {
            return true;
        }
        index = export_start + "export".len();
    }
    false
}

fn is_vue27_export_boundary(input: &str, export_start: usize) -> bool {
    let prefix = &input[..export_start];
    let Some(non_space) = prefix
        .char_indices()
        .rev()
        .find(|(_, ch)| !matches!(ch, ' ' | '\t' | '\r'))
    else {
        return true;
    };
    matches!(non_space.1, '\n' | ';')
}

#[derive(Debug)]
struct SourceEdits<'a> {
    input: &'a str,
    edits: Vec<SourceEdit>,
    prepend: String,
    append: String,
}

#[derive(Debug)]
struct SourceEdit {
    start: usize,
    end: usize,
    replacement: String,
}

impl<'a> SourceEdits<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            edits: Vec::new(),
            prepend: String::new(),
            append: String::new(),
        }
    }

    fn overwrite(&mut self, start: usize, end: usize, replacement: impl Into<String>) {
        self.edits.push(SourceEdit {
            start,
            end,
            replacement: replacement.into(),
        });
    }

    fn prepend(&mut self, value: impl AsRef<str>) {
        self.prepend.push_str(value.as_ref());
    }

    fn append(&mut self, value: impl AsRef<str>) {
        self.append.push_str(value.as_ref());
    }

    fn apply(mut self) -> String {
        self.edits.sort_by_key(|edit| (edit.start, edit.end));
        let mut output = String::new();
        output.push_str(&self.prepend);
        let mut cursor = 0usize;
        for edit in self.edits {
            if edit.start < cursor {
                continue;
            }
            output.push_str(&self.input[cursor..edit.start.min(self.input.len())]);
            output.push_str(&edit.replacement);
            cursor = edit.end.min(self.input.len());
        }
        output.push_str(&self.input[cursor..]);
        output.push_str(&self.append);
        output
    }
}

fn style_dependencies(style: &SfcBlock) -> Vec<String> {
    let mut dependencies = Vec::new();
    if let Some(src) = style.attrs.src.as_ref() {
        dependencies.push(src.clone());
    }
    for line in style.content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("@import") {
            continue;
        }
        if let Some(dep) = quoted_import_path(trimmed) {
            dependencies.push(dep.to_string());
        }
    }
    dependencies
}

fn scoped_style_vars(id: Option<&str>, vars: &[String]) -> Vec<String> {
    let Some(id) = id else {
        return vars.to_vec();
    };
    let prefix = id
        .strip_prefix("data-v-")
        .unwrap_or(id)
        .trim_matches('_')
        .trim_matches('-');
    if prefix.is_empty() {
        return vars.to_vec();
    }
    vars.iter()
        .map(|var| {
            if var.starts_with(&format!("{prefix}-")) {
                var.clone()
            } else {
                format!("{prefix}-{var}")
            }
        })
        .collect()
}

fn merge_template_errors(
    mut first: Vec<SfcTemplateError>,
    second: Vec<SfcTemplateError>,
) -> Vec<SfcTemplateError> {
    for error in second {
        if !first.iter().any(|existing| {
            existing.code == error.code
                && existing.loc.start.offset == error.loc.start.offset
                && existing.loc.end.offset == error.loc.end.offset
        }) {
            first.push(error);
        }
    }
    first
}

fn sfc_template_errors_from_diagnostics(
    diagnostics: &[Diagnostic],
    source: &str,
) -> Vec<SfcTemplateError> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .filter_map(|diagnostic| sfc_template_error_from_diagnostic(diagnostic, source))
        .collect()
}

fn sfc_template_error_from_diagnostic(
    diagnostic: &Diagnostic,
    source: &str,
) -> Option<SfcTemplateError> {
    let span = diagnostic.span?;
    let start = span.start.0.min(source.len());
    let end = span.end.0.min(source.len()).max(start);
    Some(SfcTemplateError {
        code: diagnostic.code.parse().unwrap_or(0),
        loc: SfcSourceLocation {
            start: position_at(source, start)?,
            end: position_at(source, end)?,
            source: source.get(start..end).unwrap_or_default().to_string(),
        },
    })
}

fn script_bindings(names: &[String]) -> BTreeMap<String, String> {
    names
        .iter()
        .filter(|name| !name.starts_with("import:") && !name.starts_with("export:"))
        .map(|name| (name.clone(), "literal-const".to_string()))
        .collect()
}

fn script_content(
    descriptor: &SfcDescriptor,
    raw_content: &str,
    bindings: &[String],
    filename: &str,
) -> String {
    let Some(script_setup) = descriptor.script_setup.as_ref() else {
        return raw_content.to_string();
    };
    let component_name = script_component_name(filename);
    let returned = bindings
        .iter()
        .filter(|name| !name.starts_with("import:") && !name.starts_with("export:"))
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let returned = if returned.is_empty() {
        "{}".to_string()
    } else {
        format!("{{ {returned} }}")
    };
    format!(
        "import {{ defineComponent as _defineComponent }} from 'vue'\n{}\nexport default /*@__PURE__*/_defineComponent({{\n  __name: '{}',\n  setup(__props, {{ expose: __expose }}) {{\n  __expose();\n\nconst __returned__ = {}\nObject.defineProperty(__returned__, '__isScriptSetup', {{ enumerable: false, value: true }})\nreturn __returned__\n}}\n\n}})",
        script_setup.content, component_name, returned
    )
}

fn script_component_name(filename: &str) -> String {
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("anonymous");
    stem.to_string()
}

fn quoted_import_path(source: &str) -> Option<&str> {
    let start = source.find(['"', '\''])?;
    let quote = source[start..].chars().next()?;
    let rest = &source[start + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(&rest[..end])
}

fn side_effect_tag_errors(source: &str) -> Vec<SfcTemplateError> {
    side_effect_tag_ranges(source)
        .into_iter()
        .filter_map(|(start, end, _)| {
            let start_pos = position_at(source, start)?;
            let end_pos = position_at(source, end)?;
            Some(SfcTemplateError {
                code: 64,
                loc: SfcSourceLocation {
                    start: start_pos,
                    end: end_pos,
                    source: source[start..end].to_string(),
                },
            })
        })
        .collect()
}

fn side_effect_tag_ranges(source: &str) -> Vec<(usize, usize, &'static str)> {
    let mut ranges = Vec::new();
    for tag in ["script", "style"] {
        let mut cursor = 0usize;
        while let Some(start_offset) = source[cursor..].find(&format!("<{tag}")) {
            let start = cursor + start_offset;
            let Some(after_open_offset) = source[start..].find('>') else {
                break;
            };
            let after_open = start + after_open_offset + 1;
            let close_tag = format!("</{tag}>");
            let Some(close_offset) = source[after_open..].find(&close_tag) else {
                break;
            };
            let end = after_open + close_offset + close_tag.len();
            ranges.push((start, end, tag));
            cursor = end;
        }
    }
    ranges.sort_by_key(|(start, _, _)| *start);
    ranges
}

fn position_at(source: &str, offset: usize) -> Option<SfcPosition> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let mut line = 1usize;
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
    Some(SfcPosition {
        column: source[line_start..offset].encode_utf16().count() + 1,
        line,
        offset,
    })
}

fn script_source_type(descriptor: &SfcDescriptor) -> oxc_span::SourceType {
    let lang = descriptor
        .script_setup
        .as_ref()
        .or(descriptor.script.as_ref())
        .and_then(|block| block.attrs.lang.as_deref());
    match lang {
        Some("tsx") => oxc_span::SourceType::tsx(),
        Some("ts") => oxc_span::SourceType::ts(),
        _ => oxc_span::SourceType::mjs(),
    }
}

fn script_mode(attrs: &SfcBlockAttrs) -> JsParseMode {
    if matches!(attrs.lang.as_deref(), Some("ts" | "tsx")) {
        JsParseMode::TypeScript
    } else {
        JsParseMode::ScriptModule
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blocks() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><div/></template><script setup lang="ts">const x = 1</script><style scoped>.a{}</style>"#,
        );
        assert!(descriptor.template.is_some());
        assert!(descriptor.script_setup.is_some());
        assert_eq!(descriptor.styles.len(), 1);
    }

    #[test]
    fn vue27_parse_component_preserves_top_level_blocks_and_attrs() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue27_component(
            r#"
<template><div><style>nested</style></div></template>
<style bool-attr val-attr="test" module></style>
<example name="simple"><my-button>Hello</my-button></example>
<div><style>ignored</style></div>
"#,
            Vue27ParseComponentOptions::default(),
        );

        let descriptor = result.descriptor;
        assert_eq!(
            descriptor.template.as_ref().unwrap().content.trim(),
            "<div><style>nested</style></div>"
        );
        assert_eq!(descriptor.styles.len(), 1);
        assert_eq!(
            descriptor.styles[0].attrs.raw.get("bool-attr"),
            Some(&SfcAttrValue::Bool(true))
        );
        assert_eq!(
            descriptor.styles[0].attrs.raw.get("val-attr"),
            Some(&SfcAttrValue::String("test".into()))
        );
        assert_eq!(descriptor.styles[0].attrs.module.as_deref(), Some(""));
        assert_eq!(descriptor.custom_blocks.len(), 2);
        assert_eq!(descriptor.custom_blocks[0].type_name, "example");
        assert_eq!(
            descriptor.custom_blocks[0].content.trim(),
            "<my-button>Hello</my-button>"
        );
        assert_eq!(descriptor.custom_blocks[1].type_name, "div");
    }

    #[test]
    fn vue27_parse_component_deindents_like_official_parser() {
        let content = r#"<template>
        <div></div>
      </template>
      <script>
        export default {}
      </script>
      <style>
        h1 { color: red }
      </style>"#;
        let mut compiler = SfcCompiler::new();
        let default = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                pad: Vue27SfcPad::False,
                ..Vue27ParseComponentOptions::default()
            },
        );
        assert_eq!(
            default.descriptor.template.unwrap().content,
            "\n<div></div>\n"
        );
        assert_eq!(
            default.descriptor.script.unwrap().content,
            "\n        export default {}\n      "
        );
        assert_eq!(
            default.descriptor.styles[0].content,
            "\nh1 { color: red }\n"
        );

        let enabled = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                deindent: Some(true),
                ..Vue27ParseComponentOptions::default()
            },
        );
        assert_eq!(
            enabled.descriptor.script.unwrap().content,
            "\nexport default {}\n"
        );
    }

    #[test]
    fn vue27_parse_component_pads_non_template_content() {
        let content = r#"<template>
        <div></div>
      </template>
      <script>
        export default {}
      </script>
      <style>
        h1 { color: red }
      </style>"#;
        let mut compiler = SfcCompiler::new();
        let line = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                pad: Vue27SfcPad::Line,
                deindent: Some(true),
                ..Vue27ParseComponentOptions::default()
            },
        );
        assert_eq!(
            line.descriptor.script.unwrap().content,
            format!("{}\nexport default {{}}\n", "//\n".repeat(3))
        );
        assert_eq!(
            line.descriptor.styles[0].content,
            "\n\n\n\n\n\n\nh1 { color: red }\n"
        );

        let space = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                pad: Vue27SfcPad::Space,
                deindent: Some(true),
                ..Vue27ParseComponentOptions::default()
            },
        );
        let script_pad = content[..space.descriptor.script.as_ref().unwrap().content_start]
            .chars()
            .map(|ch| if matches!(ch, '\n' | '\r') { ch } else { ' ' })
            .collect::<String>();
        assert_eq!(
            space.descriptor.script.unwrap().content,
            script_pad + "\nexport default {}\n"
        );
    }

    #[test]
    fn vue27_parse_component_recovers_unclosed_template_with_source_range() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue27_component(
            "<template>hi</",
            Vue27ParseComponentOptions {
                output_source_range: true,
                ..Vue27ParseComponentOptions::default()
            },
        );

        assert_eq!(result.descriptor.template.unwrap().content, "hi");
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].start, Some(0));
        assert_eq!(result.errors[0].end, Some(10));
    }

    #[test]
    fn vue27_rewrite_default_handles_default_declarations() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler.rewrite_vue27_default(
                "export  default {}",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "const script = {}"
        );
        assert_eq!(
            compiler.rewrite_vue27_default(
                "// export default\nexport default class Foo {}",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "// export default\nclass Foo {}\nconst script = Foo"
        );
    }

    #[test]
    fn vue27_rewrite_default_handles_named_default_exports() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler.rewrite_vue27_default(
                "const a = 1 \n export { a as b, a as default, a as c}",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "const a = 1 \n export { a as b,  a as c}\nconst script = a"
        );
        assert_eq!(
            compiler.rewrite_vue27_default(
                "export { default, foo } from './index.js'",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "import { default as __VUE_DEFAULT__ } from './index.js'\nexport {  foo } from './index.js'\nconst script = __VUE_DEFAULT__"
        );
        assert_eq!(
            compiler.rewrite_vue27_default(
                "export { foo as default, bar } from './index.js'",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "import { foo } from './index.js'\nexport {  bar } from './index.js'\nconst script = foo"
        );
    }

    #[test]
    fn vue27_rewrite_default_handles_typescript_decorated_classes() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler.rewrite_vue27_default(
                "@Component({})\nexport default class HelloWorld extends Vue {\n  test = \"\";\n}",
                "script",
                Vue27RewriteDefaultOptions {
                    typescript: true,
                    decorators: true,
                },
            ),
            "@Component({})\nclass HelloWorld extends Vue {\n  test = \"\";\n}\nconst script = HelloWorld"
        );
    }

    #[test]
    fn compile_wrappers_return_shapes() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><div/></template><script>export default {}</script><script setup lang="ts">const x = 1</script><style scoped src="./base.css">@import "./dep.css"; .a{ color: v-bind(color); }</style>"#,
        );
        let template = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());
        assert!(template.code.contains("render"));
        assert!(template.ast_summary.starts_with("dom:"));
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
        assert_eq!(script.errors.len(), 0);
        assert!(script.setup);
        assert_eq!(script.lang.as_deref(), Some("ts"));
        assert_eq!(
            script.bindings.get("x").map(String::as_str),
            Some("literal-const")
        );
        assert!(script.content.contains("_defineComponent"));
        assert!(script.content.contains("__returned__ = { x }"));
        assert_eq!(script.script_ast, vec!["JsProgramId(0)"]);
        assert_eq!(script.script_setup_ast, vec!["JsProgramId(1)"]);
        let script_json = serde_json::to_value(&script).expect("script json");
        assert!(script_json.get("scriptAst").is_some());
        assert!(script_json.get("scriptSetupAst").is_some());
        assert_eq!(
            script_json.get("type").and_then(|value| value.as_str()),
            Some("script")
        );
        let style = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        assert!(style.errors.is_empty());
        assert!(style.map.is_none());
        assert!(style.code.contains("var(--color)"));
        assert_eq!(style.dependencies, vec!["./base.css", "./dep.css"]);
        assert_eq!(style.raw_result.len(), 1);
        let style_json = serde_json::to_value(&style).expect("style json");
        assert!(style_json.get("rawResult").is_some());
    }

    #[test]
    fn compile_template_uses_ssr_backend() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("foo.vue", r#"<template><div>{{ msg }}</div></template>"#);
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                ssr: true,
                ..SfcTemplateCompileOptions::default()
            },
        );
        assert!(template.code.contains("ssrRender"));
        assert!(template.code.contains("_ssrInterpolate(_ctx.msg)"));
    }

    #[test]
    fn compile_template_passes_asset_url_base_to_dom_backend() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><img src="./logo.png"><img src="~logo.png"><img srcset="@/logo.png 1x, ./logo.png 2x"></template>"#,
        );
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    ..AssetUrlOptions::default()
                },
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template.code.contains(r#"src: "/foo/logo.png""#));
        assert!(template.code.contains("import _imports_0 from 'logo.png'"));
        assert!(template
            .code
            .contains("import _imports_1 from '@/logo.png'"));
        assert!(template.code.contains("src: _imports_0"));
        assert!(template
            .code
            .contains(r#"srcset: _imports_1 + ' 1x, ' + "/foo/logo.png" + ' 2x'"#));
        assert!(!template.code.contains(r#"src: "~logo.png""#));
    }

    #[test]
    fn compile_template_supports_custom_asset_url_tags() {
        let mut compiler = SfcCompiler::new();
        let descriptor =
            compiler.parse("foo.vue", r#"<template><foo bar="~baz"></foo></template>"#);
        let mut tags = BTreeMap::new();
        tags.insert("foo".into(), vec!["bar".into()]);
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                asset_url_options: AssetUrlOptions {
                    tags,
                    ..AssetUrlOptions::default()
                },
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template.code.contains("import _imports_0 from 'baz'"));
        assert!(template.code.contains("bar: _imports_0"));
    }

    #[test]
    fn compile_template_transforms_asset_urls_to_imports() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><img src="./logo.png" srcset="./logo.png 2x"><img src="@theme/logo.png"></template>"#,
        );
        let template = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());

        assert!(template
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(template
            .code
            .contains("import _imports_1 from '@theme/logo.png'"));
        assert!(template.code.contains("src: _imports_0"));
        assert!(template.code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(!template.code.contains("_ctx._imports_"));
        assert!(!template.code.contains("PROPS"));
    }

    #[test]
    fn compile_template_uses_official_cache_handler_default() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><input @blur="onBlur" @[validateEvent]="onValidateEvent"></template>"#,
        );
        let template = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());

        assert!(template.code.contains("toHandlerKey as _toHandlerKey"));
        assert!(template.code.contains("mergeProps as _mergeProps"));
        assert!(template.code.contains(
            "_cache[0] || (_cache[0] = (...args) => (_ctx.onBlur && _ctx.onBlur(...args)))"
        ));
        assert!(template.code.contains("_cache[1] || (_cache[1] = (...args) => (_ctx.onValidateEvent && _ctx.onValidateEvent(...args)))"));
        assert!(!template.code.contains("data-vuec-dom"));
    }

    #[test]
    fn compile_template_source_does_not_cache_dynamic_interpolation_subtrees() {
        let compiler = SfcCompiler::new();
        let template = compiler.compile_template_source(
            "contract.vue",
            r#"<template><div>{{ msg }}</div></template><script setup lang="ts">const msg = 'x'</script><style scoped>.a{ color: v-bind(color); }</style>"#,
            SfcTemplateCompileOptions {
                scope_id: Some("data-v-contract".into()),
                slotted: false,
                ssr: false,
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template.code.contains("_toDisplayString(_ctx.msg)"));
        assert!(template.code.contains("1 /* TEXT */"));
        assert!(!template.code.contains("-1 /* CACHED */"));
        assert!(!template.code.contains("[...(_cache[0]"));
        assert_eq!(template.errors.len(), 2);
        assert_eq!(template.errors[0].code, 64);
        assert_eq!(template.errors[1].code, 64);
    }

    #[test]
    fn compile_template_source_returns_dom_compile_errors() {
        let compiler = SfcCompiler::new();
        let template = compiler.compile_template_source(
            "x.vue",
            r#"<div :bar="a[" v-model="baz"/>"#,
            SfcTemplateCompileOptions::default(),
        );

        assert_eq!(template.errors.len(), 2);
        assert_eq!(template.errors[0].code, 46);
        assert_eq!(template.errors[0].loc.start.offset, 13);
        assert_eq!(template.errors[1].code, 58);
        assert_eq!(template.errors[1].loc.source, r#"v-model="baz""#);
    }

    #[test]
    fn compile_template_ssr_transforms_asset_urls_to_imports() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><img src="./logo.png" srcset="./logo.png 2x"></template>"#,
        );
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                ssr: true,
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(template.code.contains("src: _imports_0"));
        assert!(template.code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(template.code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(!template.code.contains("</img>"));
        assert!(!template.code.contains("_ctx._imports_"));
    }

    #[test]
    fn compile_template_source_ssr_respects_disabled_asset_url_transform() {
        let compiler = SfcCompiler::new();
        let template = compiler.compile_template_source(
            "foo.vue",
            r#"<img src="./logo.png">"#,
            SfcTemplateCompileOptions {
                ssr: true,
                transform_asset_urls: false,
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(!template.code.contains("import _imports_0"));
        assert!(template.code.contains(r#"src: "./logo.png""#));
        assert!(template.code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(!template.code.contains("</img>"));
    }
}
