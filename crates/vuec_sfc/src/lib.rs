#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_js::JsAstStore;
use vuec_codegen::SourceMapArtifact;
use vuec_source::{FileId, SourceMap};
use vuec_style::{compile_style, StyleCompileOptions};
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions};
use vuec_vue3_dom::{compile as compile_dom, DomCompilerOptions};
use vuec_vue3_ssr::{compile as compile_ssr, SsrCompilerOptions};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcBlock {
    pub type_name: String,
    pub content: String,
    pub attrs: SfcBlockAttrs,
    pub loc: SfcBlockLocation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcBlockAttrs {
    pub lang: Option<String>,
    pub src: Option<String>,
    pub scoped: bool,
    pub module: Option<String>,
    pub setup: bool,
    pub generic: Option<String>,
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
pub struct SfcTemplateCompileOptions {
    pub id: Option<String>,
    pub ssr: bool,
    pub scope_id: Option<String>,
    pub slotted: bool,
    pub is_prod: bool,
}

impl Default for SfcTemplateCompileOptions {
    fn default() -> Self {
        Self {
            id: None,
            ssr: false,
            scope_id: None,
            slotted: false,
            is_prod: false,
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
    pub errors: Vec<String>,
    pub bindings: Vec<String>,
    pub ast_summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcScriptBlock {
    pub content: String,
    pub bindings: Vec<String>,
    pub errors: Vec<String>,
    pub loc: Option<SfcBlockLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcStyleCompileResult {
    pub code: String,
    pub map: Option<SourceMapArtifact>,
    pub errors: Vec<String>,
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

        for block in extract_basic_blocks(source, source_file) {
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

    pub fn compile_template(
        &self,
        descriptor: &SfcDescriptor,
        options: SfcTemplateCompileOptions,
    ) -> SfcTemplateCompileResult {
        let Some(template) = descriptor.template.as_ref() else {
            return SfcTemplateCompileResult {
                code: String::new(),
                map: None,
                errors: vec!["missing template block".to_string()],
                bindings: Vec::new(),
                ast_summary: "missing-template".into(),
            };
        };
        let core = Vue3CompilerOptions {
            scope_id: options.scope_id.clone(),
            slotted: options.slotted,
            ..Vue3CompilerOptions::default()
        };
        let source = TemplateSource {
            filename: descriptor.filename.clone(),
            source: template.content.clone(),
            file_id: descriptor.source_file,
        };
        let result = if options.ssr {
            compile_ssr(
                source,
                SsrCompilerOptions {
                    core,
                    scope_id: options.scope_id.clone(),
                    slotted: options.slotted,
                },
            )
        } else {
            compile_dom(
                source,
                DomCompilerOptions {
                    core,
                    ..DomCompilerOptions::default()
                },
            )
        };
        SfcTemplateCompileResult {
            code: result.code,
            map: result.map,
            errors: result.diagnostics,
            bindings: Vec::new(),
            ast_summary: result.ast_summary,
        }
    }

    pub fn compile_template_source(
        &self,
        filename: impl Into<String>,
        source: &str,
        options: SfcTemplateCompileOptions,
    ) -> SfcTemplateCompileResult {
        let filename = filename.into();
        let descriptor = SfcDescriptor {
            filename,
            source: source.to_string(),
            source_file: FileId(0),
            template: Some(SfcBlock {
                type_name: "template".into(),
                content: source.to_string(),
                attrs: SfcBlockAttrs::default(),
                loc: SfcBlockLocation {
                    start: 0,
                    end: source.len(),
                    source_file: FileId(0),
                },
            }),
            script: None,
            script_setup: None,
            styles: Vec::new(),
            custom_blocks: Vec::new(),
        };
        self.compile_template(&descriptor, options)
    }

    pub fn compile_script(
        &self,
        descriptor: &SfcDescriptor,
        _options: SfcScriptCompileOptions,
    ) -> SfcScriptBlock {
        let mut content = String::new();
        let mut bindings = Vec::new();
        if let Some(script) = descriptor.script.as_ref() {
            content.push_str(&script.content);
        }
        if let Some(script_setup) = descriptor.script_setup.as_ref() {
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(&script_setup.content);
        }
        let source_type = script_source_type(descriptor);
        let summary = self.js.summarize_program(&content, source_type);
        bindings.extend(summary.bindings);
        bindings.extend(
            summary
                .imports
                .into_iter()
                .map(|import| format!("import:{import}")),
        );
        bindings.extend(
            summary
                .exports
                .into_iter()
                .map(|export| format!("export:{export}")),
        );
        SfcScriptBlock {
            content,
            bindings,
            errors: summary.errors,
            loc: descriptor
                .script
                .as_ref()
                .or(descriptor.script_setup.as_ref())
                .map(|block| block.loc.clone()),
        }
    }

    pub fn compile_style(
        &self,
        descriptor: &SfcDescriptor,
        options: SfcStyleCompileOptions,
    ) -> SfcStyleCompileResult {
        let mut code = String::new();
        let mut errors = Vec::new();
        for style in &descriptor.styles {
            let result = compile_style(
                &style.content,
                StyleCompileOptions {
                    id: options.id.clone(),
                    scoped: options.scoped || style.attrs.scoped,
                    modules: style.attrs.module.is_some(),
                    vars: options.vars.clone(),
                    filename: Some(descriptor.filename.clone()),
                    source_map: true,
                },
            );
            if !code.is_empty() && !result.code.is_empty() {
                code.push('\n');
            }
            code.push_str(&result.code);
            errors.extend(result.errors);
        }
        SfcStyleCompileResult {
            code,
            map: descriptor.styles.first().and_then(|_| {
                compile_style(
                    "",
                    StyleCompileOptions {
                        id: options.id.clone(),
                        scoped: options.scoped,
                        modules: false,
                        vars: Vec::new(),
                        filename: Some(descriptor.filename.clone()),
                        source_map: true,
                    },
                )
                .map
            }),
            errors,
        }
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

fn extract_basic_blocks(source: &str, source_file: FileId) -> Vec<SfcBlock> {
    let mut blocks = Vec::new();
    for tag in ["template", "script", "style"] {
        let mut cursor = 0usize;
        while let Some(start) = source[cursor..].find(&format!("<{tag}")) {
            let start = cursor + start;
            let after_start = match source[start..].find('>') {
                Some(offset) => start + offset + 1,
                None => break,
            };
            let end_tag = format!("</{tag}>");
            let end = match source[after_start..].find(&end_tag) {
                Some(offset) => after_start + offset,
                None => break,
            };
            let head = &source[start..after_start];
            let attrs = parse_attrs(head);
            let content = source[after_start..end].to_string();
            blocks.push(SfcBlock {
                type_name: tag.to_string(),
                content,
                attrs,
                loc: SfcBlockLocation {
                    start,
                    end: end + end_tag.len(),
                    source_file,
                },
            });
            cursor = end + end_tag.len();
        }
    }
    blocks
}

fn parse_attrs(head: &str) -> SfcBlockAttrs {
    let mut attrs = SfcBlockAttrs::default();
    if head.contains("lang=\"ts\"") || head.contains("lang='ts'") {
        attrs.lang = Some("ts".into());
    }
    if head.contains("scoped") {
        attrs.scoped = true;
    }
    if head.contains("setup") {
        attrs.setup = true;
    }
    if let Some(module_pos) = head.find("module") {
        let tail = &head[module_pos..];
        if let Some(eq_pos) = tail.find('=') {
            let value = tail[eq_pos + 1..].trim().trim_matches(['"', '\'', '>']);
            attrs.module = if value.is_empty() {
                Some(String::new())
            } else {
                Some(value.to_string())
            };
        } else {
            attrs.module = Some(String::new());
        }
    }
    attrs
}

fn script_source_type(descriptor: &SfcDescriptor) -> oxc_span::SourceType {
    let lang = descriptor
        .script_setup
        .as_ref()
        .or(descriptor.script.as_ref())
        .and_then(|block| block.attrs.lang.as_deref());
    match lang {
        Some("ts" | "tsx") => oxc_span::SourceType::ts(),
        _ => oxc_span::SourceType::mjs(),
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
    fn compile_wrappers_return_shapes() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("foo.vue", r#"<template><div/></template>"#);
        let template = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());
        assert!(template.code.contains("render"));
        assert!(template.ast_summary.starts_with("dom:"));
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
        assert_eq!(script.errors.len(), 0);
        let style = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        assert!(style.errors.is_empty());
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
        assert!(template.code.contains("_ssrInterpolate(msg)"));
    }
}
