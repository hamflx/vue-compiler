#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use thiserror::Error;
use vuec_source::{Span, SourceMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Tip,
    Note,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub notes: Vec<String>,
}

impl Diagnostic {
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

#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSink {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticSink {
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn extend<I>(&mut self, diagnostics: I)
    where
        I: IntoIterator<Item = Diagnostic>,
    {
        self.diagnostics.extend(diagnostics);
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("missing source file for span")]
    MissingSource,
}

pub fn render_diagnostic(diagnostic: &Diagnostic, sources: &SourceMap) -> Result<String, RenderError> {
    let mut rendered = format!("[{}] {}: {}", diagnostic.severity.as_str(), diagnostic.code, diagnostic.message);
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
