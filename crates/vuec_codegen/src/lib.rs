#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_source::Span;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodeWriter {
    code: String,
    indent: usize,
    at_line_start: bool,
    indent_unit: &'static str,
}

impl CodeWriter {
    pub fn new() -> Self {
        Self {
            code: String::new(),
            indent: 0,
            at_line_start: true,
            indent_unit: "  ",
        }
    }

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

    pub fn push_line(&mut self, text: &str) {
        self.push_str(text);
        self.newline();
    }

    pub fn newline(&mut self) {
        self.code.push('\n');
        self.at_line_start = true;
    }

    pub fn indent(&mut self) {
        self.indent += 1;
    }

    pub fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    pub fn finish(self) -> String {
        self.code
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapMapping {
    pub generated_line: usize,
    pub generated_column: usize,
    pub original: Option<Span>,
    pub source_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapArtifact {
    pub version: u8,
    pub file: Option<String>,
    pub sources: Vec<String>,
    pub names: Vec<String>,
    pub mappings: Vec<SourceMapMapping>,
}

#[derive(Clone, Debug, Default)]
pub struct SourceMapBuilder {
    file: Option<String>,
    sources: Vec<String>,
    names: Vec<String>,
    mappings: Vec<SourceMapMapping>,
}

impl SourceMapBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

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

    pub fn add_name(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !self.names.iter().any(|existing| existing == &name) {
            self.names.push(name);
        }
    }

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

    pub fn build(self) -> SourceMapArtifact {
        SourceMapArtifact {
            version: 3,
            file: self.file,
            sources: self.sources,
            names: self.names,
            mappings: self.mappings,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmitResult {
    pub code: String,
    pub map: Option<SourceMapArtifact>,
}

impl EmitResult {
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
