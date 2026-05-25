#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use vuec_codegen::SourceMapArtifact;
use vuec_js::{JsAstStore, JsParseMode};
use vuec_source::{FileId, SourceMap, Span};
use vuec_style::{compile_style, StyleCompileOptions};
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions};
use vuec_vue3_dom::{compile as compile_dom, AssetUrlOptions, DomCompilerOptions};
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
        let core = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            scope_id: options.scope_id.clone(),
            slotted: options.slotted,
            source_map: true,
            ..Vue3CompilerOptions::default()
        };
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
                },
            );
            let ast_summary = result.ast_summary;
            return SfcTemplateCompileResult {
                code: result.code,
                map: result.map,
                errors: Vec::new(),
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
                errors: Vec::new(),
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
        let core = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            scope_id: options.scope_id.clone(),
            slotted: options.slotted,
            source_map: true,
            ..Vue3CompilerOptions::default()
        };
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
                },
            );
            return SfcTemplateCompileResult {
                code: result.code,
                map: result.map,
                errors: side_effect_errors,
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
            errors: side_effect_errors,
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
    if let Some(src) = attr_value(head, "src") {
        attrs.src = Some(src.to_string());
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

fn attr_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    let start = head.find(name)?;
    let tail = &head[start + name.len()..];
    let tail = tail.trim_start();
    let tail = tail.strip_prefix('=')?.trim_start();
    let quote = tail.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value_start = quote.len_utf8();
    let value_tail = &tail[value_start..];
    let value_end = value_tail.find(quote)?;
    Some(&value_tail[..value_end])
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
        assert!(template.code.contains("_ssrInterpolate(msg)"));
    }

    #[test]
    fn compile_template_passes_asset_url_base_to_dom_backend() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><img src="./logo.png"><img src="~logo.png"></template>"#,
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
        assert!(template.code.contains(r#"src: "~logo.png""#));
    }
}
