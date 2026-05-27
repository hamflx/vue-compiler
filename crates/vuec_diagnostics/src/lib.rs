#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Shared diagnostic data structures and rendering helpers.
//!
//! Parser, transform, codegen, CLI, NAPI, and WASM layers exchange diagnostics
//! through these stable structs so compiler errors can be serialized while still
//! retaining optional source spans for human-readable code frames.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use vuec_source::{SourceMap, Span};

/// Diagnostic severity level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Compilation cannot continue without reporting an error.
    Error,
    /// Compilation can continue, but output may not match user intent.
    Warning,
    /// Non-fatal user guidance, matching Vue compiler tip output.
    Tip,
    /// Additional contextual information attached to another diagnostic.
    Note,
}

/// Structured compiler diagnostic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable diagnostic code.
    pub code: String,
    /// Severity used by CLI/package renderers.
    pub severity: Severity,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Optional source span for code-frame rendering.
    pub span: Option<Span>,
    /// Additional explanatory notes.
    pub notes: Vec<String>,
}

impl Diagnostic {
    /// Creates an error diagnostic without a source span.
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            message: message.into(),
            span: None,
            notes: Vec::new(),
        }
    }
}

/// Accumulator for diagnostics produced by a compiler phase.
#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSink {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticSink {
    /// Adds one diagnostic to the sink.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Adds diagnostics from an iterator.
    pub fn extend<I>(&mut self, diagnostics: I)
    where
        I: IntoIterator<Item = Diagnostic>,
    {
        self.diagnostics.extend(diagnostics);
    }

    /// Returns `true` when no diagnostics have been collected.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Returns the collected diagnostics without consuming the sink.
    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Consumes the sink and returns collected diagnostics.
    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// Error returned while rendering a diagnostic.
#[derive(Debug, Error)]
pub enum RenderError {
    /// The diagnostic span refers to a missing or invalid source file.
    #[error("missing source file for span")]
    MissingSource,
}

/// Renders a diagnostic with an optional source code frame.
pub fn render_diagnostic(
    diagnostic: &Diagnostic,
    sources: &SourceMap,
) -> Result<String, RenderError> {
    let mut rendered = format!(
        "[{}] {}: {}",
        diagnostic.severity.as_str(),
        diagnostic.code,
        diagnostic.message
    );
    if let Some(span) = diagnostic.span {
        let frame = sources
            .code_frame(span.file_id, span.start, span.end, None)
            .ok_or(RenderError::MissingSource)?;
        rendered.push('\n');
        rendered.push_str(&frame);
    }
    for note in &diagnostic.notes {
        rendered.push('\n');
        rendered.push_str("note: ");
        rendered.push_str(note);
    }
    Ok(rendered)
}

impl Severity {
    /// Returns the lower-case display name for the severity.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Tip => "tip",
            Severity::Note => "note",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sink_collects_diagnostics() {
        let mut sink = DiagnosticSink::default();
        sink.push(Diagnostic::error("E001", "boom"));
        assert!(!sink.is_empty());
        assert_eq!(sink.as_slice().len(), 1);
    }

    #[test]
    fn render_uses_source_frames() {
        let mut sources = SourceMap::default();
        let id = sources.add_file(None, "hello\nworld");
        let diagnostic = Diagnostic {
            code: "E001".into(),
            severity: Severity::Error,
            message: "boom".into(),
            span: Some(Span::new(id, 6, 11)),
            notes: vec!["check template syntax".into()],
        };
        let rendered = render_diagnostic(&diagnostic, &sources).expect("render");
        assert!(rendered.contains("[error] E001: boom"));
        assert!(rendered.contains("world"));
        assert!(rendered.contains("note: check template syntax"));
    }
}
