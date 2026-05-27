#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Shared code emission and source-map result types.
//!
//! Compiler backends use this crate for deterministic string emission and for
//! serializable source-map artifacts passed through Rust, CLI, NAPI, and WASM
//! package boundaries.

use serde::{Deserialize, Serialize};
use vuec_source::Span;

/// Small indentation-aware code writer used by codegen backends.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodeWriter {
    code: String,
    indent: usize,
    at_line_start: bool,
    indent_unit: &'static str,
}

impl CodeWriter {
    /// Creates an empty writer using two spaces per indentation level.
    pub fn new() -> Self {
        Self {
            code: String::new(),
            indent: 0,
            at_line_start: true,
            indent_unit: "  ",
        }
    }

    /// Appends text while applying indentation at line starts.
    pub fn push_str(&mut self, text: &str) {
        for segment in text.split_inclusive('\n') {
            if self.at_line_start {
                for _ in 0..self.indent {
                    self.code.push_str(self.indent_unit);
                }
            }
            self.code.push_str(segment);
            self.at_line_start = segment.ends_with('\n');
        }
    }

    /// Appends text without inserting indentation.
    pub fn push_raw(&mut self, text: &str) {
        self.code.push_str(text);
        self.at_line_start = text.ends_with('\n');
    }

    /// Appends one line and then writes a newline.
    pub fn push_line(&mut self, text: &str) {
        self.push_str(text);
        self.newline();
    }

    /// Writes a newline and marks the next write as line-start text.
    pub fn newline(&mut self) {
        self.code.push('\n');
        self.at_line_start = true;
    }

    /// Increases indentation for subsequent line-start writes.
    pub fn indent(&mut self) {
        self.indent += 1;
    }

    /// Decreases indentation without underflow.
    pub fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    /// Consumes the writer and returns generated code.
    pub fn finish(self) -> String {
        self.code
    }
}

/// One source-map mapping before VLQ encoding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapMapping {
    /// One-based generated line.
    pub generated_line: usize,
    /// Zero-based generated column.
    pub generated_column: usize,
    /// Optional original source span.
    pub original: Option<Span>,
    /// Optional original source file name.
    pub source_name: Option<String>,
}

/// Serializable source-map artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMapArtifact {
    /// Source-map format version.
    pub version: u8,
    /// Optional generated file name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Original source file names.
    pub sources: Vec<String>,
    /// Source-map symbol names.
    pub names: Vec<String>,
    /// Encoded VLQ mappings string.
    pub mappings: String,
    /// Optional source contents aligned with `sources`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources_content: Option<Vec<Option<String>>>,
}

/// Builder for `SourceMapArtifact`.
#[derive(Clone, Debug, Default)]
pub struct SourceMapBuilder {
    file: Option<String>,
    sources: Vec<String>,
    names: Vec<String>,
    mappings: Vec<SourceMapMapping>,
}

impl SourceMapBuilder {
    /// Creates an empty source-map builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the generated file name.
    pub fn file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Adds a generated-to-original mapping.
    pub fn add_mapping(
        &mut self,
        generated_line: usize,
        generated_column: usize,
        original: Option<Span>,
        source_name: Option<String>,
    ) {
        if let Some(name) = source_name.as_ref() {
            if !self.sources.iter().any(|existing| existing == name) {
                self.sources.push(name.clone());
            }
        }
        self.mappings.push(SourceMapMapping {
            generated_line,
            generated_column,
            original,
            source_name,
        });
    }

    /// Adds a source-map symbol name if it is not already present.
    pub fn add_name(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !self.names.iter().any(|existing| existing == &name) {
            self.names.push(name);
        }
    }

    /// Merges another builder, offsetting generated lines from the merged map.
    pub fn merge(mut self, mut other: SourceMapBuilder, line_offset: usize) -> Self {
        for mapping in other.mappings.drain(..) {
            self.mappings.push(SourceMapMapping {
                generated_line: mapping.generated_line + line_offset,
                ..mapping
            });
        }
        self.sources.append(&mut other.sources);
        self.names.append(&mut other.names);
        self
    }

    /// Builds an encoded source-map artifact.
    pub fn build(self) -> SourceMapArtifact {
        let mut encoded = oxc_sourcemap::SourceMapBuilder::default();
        if let Some(file) = self.file.as_deref() {
            encoded.set_file(file);
        }
        let source_ids = self
            .sources
            .iter()
            .map(|source| encoded.add_source_and_content(source, ""))
            .collect::<Vec<_>>();
        let name_ids = self
            .names
            .iter()
            .map(|name| encoded.add_name(name))
            .collect::<Vec<_>>();
        for mapping in &self.mappings {
            let source_id = mapping
                .source_name
                .as_ref()
                .and_then(|name| self.sources.iter().position(|source| source == name))
                .and_then(|index| source_ids.get(index).copied());
            let name_id = mapping
                .source_name
                .as_ref()
                .and_then(|name| self.names.iter().position(|existing| existing == name))
                .and_then(|index| name_ids.get(index).copied());
            encoded.add_token(
                mapping.generated_line.saturating_sub(1) as u32,
                mapping.generated_column as u32,
                mapping
                    .original
                    .map(|span| span.start.0)
                    .unwrap_or_default() as u32,
                0,
                source_id,
                name_id,
            );
        }
        let json = encoded.into_sourcemap().to_json();
        SourceMapArtifact {
            version: 3,
            file: self.file,
            sources: self.sources,
            names: self.names,
            mappings: json.mappings,
            sources_content: json.sources_content,
        }
    }
}

impl SourceMapArtifact {
    /// Builds a source map directly from already-normalized segments.
    pub fn from_segments(
        file: Option<String>,
        source: String,
        source_content: String,
        names: Vec<String>,
        segments: Vec<SourceMapSegment>,
    ) -> Self {
        let mut builder = oxc_sourcemap::SourceMapBuilder::default();
        if let Some(file) = file.as_deref() {
            builder.set_file(file);
        }
        let source_id = builder.set_source_and_content(&source, &source_content);
        let name_ids = names
            .iter()
            .map(|name| builder.add_name(name))
            .collect::<Vec<_>>();
        for segment in segments {
            let name_id = segment
                .name_index
                .and_then(|index| name_ids.get(index).copied());
            builder.add_token(
                segment.generated_line,
                segment.generated_column,
                segment.original_line,
                segment.original_column,
                Some(source_id),
                name_id,
            );
        }
        let json = builder.into_sourcemap().to_json();
        SourceMapArtifact {
            version: 3,
            file,
            sources: vec![source],
            names,
            mappings: json.mappings,
            sources_content: Some(vec![Some(source_content)]),
        }
    }
}

/// Normalized source-map segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceMapSegment {
    /// Zero-based generated line.
    pub generated_line: u32,
    /// Zero-based generated column.
    pub generated_column: u32,
    /// Zero-based original line.
    pub original_line: u32,
    /// Zero-based original column.
    pub original_column: u32,
    /// Optional index into the source-map names table.
    pub name_index: Option<usize>,
}

/// Code emission result with an optional source map.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmitResult {
    /// Generated JavaScript source.
    pub code: String,
    /// Optional source map for the generated code.
    pub map: Option<SourceMapArtifact>,
}

impl EmitResult {
    /// Creates an emission result without a source map.
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            map: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vuec_source::FileId;

    #[test]
    fn writer_handles_indent() {
        let mut writer = CodeWriter::new();
        writer.push_line("function test() {");
        writer.indent();
        writer.push_line("return 1;");
        writer.dedent();
        writer.push_str("}");
        let code = writer.finish();
        assert!(code.contains("  return 1;"));
    }

    #[test]
    fn source_map_builder_serializes() {
        let mut builder = SourceMapBuilder::new().file("test.js");
        builder.add_name("foo");
        builder.add_mapping(
            1,
            0,
            Some(Span::new(FileId(0), 0, 3)),
            Some("src.vue".into()),
        );
        let map = builder.build();
        let json = serde_json::to_string(&map).unwrap();
        assert!(json.contains("\"version\":3"));
        assert!(json.contains("src.vue"));
    }
}
