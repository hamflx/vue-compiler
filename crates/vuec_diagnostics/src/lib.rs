#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Shared diagnostic data structures and rendering helpers.
//!
//! Parser, transform, codegen, CLI, NAPI, and WASM layers exchange diagnostics
//! through these stable structs so compiler errors can be serialized while still
//! retaining optional source spans for human-readable code frames.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use vuec_source::{Loc, SourceMap, Span};

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

/// Stable Vue 3 compiler-core / compiler-dom error code values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum Vue3ErrorCode {
    /// Illegal empty comment close.
    AbruptClosingOfEmptyComment = 0,
    /// CDATA appeared in HTML content.
    CdataInHtmlContent = 1,
    /// Duplicate attribute.
    DuplicateAttribute = 2,
    /// End tag has attributes.
    EndTagWithAttributes = 3,
    /// End tag has a trailing solidus.
    EndTagWithTrailingSolidus = 4,
    /// EOF before tag name.
    EofBeforeTagName = 5,
    /// EOF in CDATA.
    EofInCdata = 6,
    /// EOF in comment.
    EofInComment = 7,
    /// EOF in script HTML-comment-like text.
    EofInScriptHtmlCommentLikeText = 8,
    /// EOF in tag.
    EofInTag = 9,
    /// Incorrectly closed comment.
    IncorrectlyClosedComment = 10,
    /// Incorrectly opened comment.
    IncorrectlyOpenedComment = 11,
    /// Invalid first character of tag name.
    InvalidFirstCharacterOfTagName = 12,
    /// Missing attribute value.
    MissingAttributeValue = 13,
    /// Missing end tag name.
    MissingEndTagName = 14,
    /// Missing whitespace between attributes.
    MissingWhitespaceBetweenAttributes = 15,
    /// Nested comment.
    NestedComment = 16,
    /// Unexpected character in attribute name.
    UnexpectedCharacterInAttributeName = 17,
    /// Unexpected character in unquoted attribute value.
    UnexpectedCharacterInUnquotedAttributeValue = 18,
    /// Unexpected equals sign before attribute name.
    UnexpectedEqualsSignBeforeAttributeName = 19,
    /// Unexpected null character.
    UnexpectedNullCharacter = 20,
    /// Unexpected question mark instead of tag name.
    UnexpectedQuestionMarkInsteadOfTagName = 21,
    /// Unexpected solidus in tag.
    UnexpectedSolidusInTag = 22,
    /// Invalid end tag.
    XInvalidEndTag = 23,
    /// Missing end tag.
    XMissingEndTag = 24,
    /// Missing interpolation end delimiter.
    XMissingInterpolationEnd = 25,
    /// Missing directive name.
    XMissingDirectiveName = 26,
    /// Missing dynamic directive argument end.
    XMissingDynamicDirectiveArgumentEnd = 27,
    /// `v-if` is missing expression.
    XVIfNoExpression = 28,
    /// `v-if` branches use the same key.
    XVIfSameKey = 29,
    /// `v-else` has no adjacent `v-if`.
    XVElseNoAdjacentIf = 30,
    /// `v-for` is missing expression.
    XVForNoExpression = 31,
    /// `v-for` expression is malformed.
    XVForMalformedExpression = 32,
    /// `<template v-for>` key placement is invalid.
    XVForTemplateKeyPlacement = 33,
    /// `v-bind` is missing expression.
    XVBindNoExpression = 34,
    /// `v-on` is missing expression.
    XVOnNoExpression = 35,
    /// Unexpected custom directive on slot outlet.
    XVSlotUnexpectedDirectiveOnSlotOutlet = 36,
    /// Mixed slot usage.
    XVSlotMixedSlotUsage = 37,
    /// Duplicate slot names.
    XVSlotDuplicateSlotNames = 38,
    /// Extraneous default slot children.
    XVSlotExtraneousDefaultSlotChildren = 39,
    /// Misplaced `v-slot`.
    XVSlotMisplaced = 40,
    /// `v-model` is missing expression.
    XVModelNoExpression = 41,
    /// `v-model` expression is malformed.
    XVModelMalformedExpression = 42,
    /// `v-model` on scope variable.
    XVModelOnScopeVariable = 43,
    /// `v-model` on props.
    XVModelOnProps = 44,
    /// `v-model` on const binding.
    XVModelOnConst = 45,
    /// Invalid JavaScript expression.
    XInvalidExpression = 46,
    /// Invalid KeepAlive children.
    XKeepAliveInvalidChildren = 47,
    /// Prefix identifiers unsupported.
    XPrefixIdNotSupported = 48,
    /// Module mode unsupported.
    XModuleModeNotSupported = 49,
    /// Cache handler unsupported.
    XCacheHandlerNotSupported = 50,
    /// Scope id unsupported.
    XScopeIdNotSupported = 51,
    /// Removed vnode hook syntax.
    XVnodeHooks = 52,
    /// Invalid same-name v-bind shorthand argument.
    XVBindInvalidSameNameArgument = 53,
    /// `v-html` is missing expression.
    XVHtmlNoExpression = 54,
    /// `v-html` has children.
    XVHtmlWithChildren = 55,
    /// `v-text` is missing expression.
    XVTextNoExpression = 56,
    /// `v-text` has children.
    XVTextWithChildren = 57,
    /// `v-model` on invalid native element.
    XVModelOnInvalidElement = 58,
    /// `v-model` argument on native element.
    XVModelArgOnElement = 59,
    /// `v-model` on file input.
    XVModelOnFileInputElement = 60,
    /// Unnecessary value binding with `v-model`.
    XVModelUnnecessaryValue = 61,
    /// `v-show` is missing expression.
    XVShowNoExpression = 62,
    /// Invalid Transition children.
    XTransitionInvalidChildren = 63,
    /// Ignored side effect tag.
    XIgnoredSideEffectTag = 64,
    /// Vue 3 compiler-dom extension point.
    DomExtendPoint = 65,
}

impl Vue3ErrorCode {
    /// Official compiler-core extension point used by compiler-dom.
    pub const CORE_EXTEND_POINT: u16 = 54;
    /// Official compiler-dom extension point.
    pub const DOM_EXTEND_POINT: u16 = 65;

    /// Returns the official numeric compiler code.
    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    /// Returns the official numeric compiler code as a string.
    pub fn as_code_string(self) -> String {
        self.as_u16().to_string()
    }
}

/// Related diagnostic information with an optional source span.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedInformation {
    /// Human-readable related message.
    pub message: String,
    /// Optional related source span.
    pub span: Option<Span>,
}

/// A machine-readable fix suggestion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSuggestion {
    /// Human-readable suggestion message.
    pub message: String,
    /// Optional span to replace.
    pub span: Option<Span>,
    /// Optional replacement text.
    pub replacement: Option<String>,
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
    /// Related source locations.
    #[serde(default)]
    pub related: Vec<RelatedInformation>,
    /// Suggested source edits or human-readable fixes.
    #[serde(default)]
    pub suggestions: Vec<DiagnosticSuggestion>,
}

impl Diagnostic {
    /// Creates a diagnostic without a source span.
    pub fn new(code: impl Into<String>, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity,
            message: message.into(),
            span: None,
            notes: Vec::new(),
            related: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    /// Creates an error diagnostic without a source span.
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Error, message)
    }

    /// Creates a warning diagnostic without a source span.
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Warning, message)
    }

    /// Creates a tip diagnostic without a source span.
    pub fn tip(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Tip, message)
    }

    /// Creates a Vue 2 warning with an optional output-source-range span.
    pub fn vue2_warning(
        code: impl Into<String>,
        message: impl Into<String>,
        span: Option<Span>,
    ) -> Self {
        Self::warning(code, message).with_span(span)
    }

    /// Creates a Vue 2 error with an optional output-source-range span.
    pub fn vue2_error(
        code: impl Into<String>,
        message: impl Into<String>,
        span: Option<Span>,
    ) -> Self {
        Self::error(code, message).with_span(span)
    }

    /// Creates a Vue 2 tip with an optional output-source-range span.
    pub fn vue2_tip(
        code: impl Into<String>,
        message: impl Into<String>,
        span: Option<Span>,
    ) -> Self {
        Self::tip(code, message).with_span(span)
    }

    /// Creates a Vue 3 compiler error using the official numeric error code.
    pub fn vue3_error(code: Vue3ErrorCode, message: impl Into<String>, span: Option<Span>) -> Self {
        Self::error(code.as_code_string(), message).with_span(span)
    }

    /// Sets the diagnostic source span.
    pub fn with_span(mut self, span: Option<Span>) -> Self {
        self.span = span;
        self
    }

    /// Adds one note to the diagnostic.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Adds related source information.
    pub fn with_related(mut self, message: impl Into<String>, span: Option<Span>) -> Self {
        self.related.push(RelatedInformation {
            message: message.into(),
            span,
        });
        self
    }

    /// Adds a source edit or human-readable suggestion.
    pub fn with_suggestion(
        mut self,
        message: impl Into<String>,
        span: Option<Span>,
        replacement: Option<impl Into<String>>,
    ) -> Self {
        self.suggestions.push(DiagnosticSuggestion {
            message: message.into(),
            span,
            replacement: replacement.map(Into::into),
        });
        self
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

    /// Returns deterministic diagnostic snapshots for compatibility assertions.
    pub fn snapshots(&self, sources: &SourceMap) -> Result<Vec<DiagnosticSnapshot>, RenderError> {
        diagnostic_snapshots(self.as_slice(), sources)
    }
}

/// Location-aware diagnostic snapshot used by compatibility tests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSnapshot {
    /// Stable diagnostic code.
    pub code: String,
    /// Diagnostic severity.
    pub severity: Severity,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Optional primary source span.
    pub span: Option<DiagnosticSpanSnapshot>,
    /// Additional explanatory notes.
    pub notes: Vec<String>,
    /// Related source-location snapshots.
    pub related: Vec<RelatedSnapshot>,
    /// Suggestion snapshots.
    pub suggestions: Vec<SuggestionSnapshot>,
}

/// Span snapshot with byte offsets and one-based source locations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSpanSnapshot {
    /// Source file id.
    pub file_id: FileIdSnapshot,
    /// Optional display path.
    pub path: Option<String>,
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
    /// One-based start location.
    pub start_loc: Loc,
    /// One-based end location.
    pub end_loc: Loc,
}

/// Serializable source file id.
pub type FileIdSnapshot = u32;

/// Related source information snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedSnapshot {
    /// Related message.
    pub message: String,
    /// Optional related source span.
    pub span: Option<DiagnosticSpanSnapshot>,
}

/// Suggestion snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionSnapshot {
    /// Suggestion message.
    pub message: String,
    /// Optional edit span.
    pub span: Option<DiagnosticSpanSnapshot>,
    /// Optional replacement text.
    pub replacement: Option<String>,
}

/// Error returned while rendering a diagnostic.
#[derive(Debug, Error)]
pub enum RenderError {
    /// The diagnostic span refers to a missing or invalid source file.
    #[error("missing source file for span")]
    MissingSource,
}

/// Returns deterministic diagnostic snapshots for compatibility assertions.
pub fn diagnostic_snapshots(
    diagnostics: &[Diagnostic],
    sources: &SourceMap,
) -> Result<Vec<DiagnosticSnapshot>, RenderError> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic_snapshot(diagnostic, sources))
        .collect()
}

fn diagnostic_snapshot(
    diagnostic: &Diagnostic,
    sources: &SourceMap,
) -> Result<DiagnosticSnapshot, RenderError> {
    Ok(DiagnosticSnapshot {
        code: diagnostic.code.clone(),
        severity: diagnostic.severity,
        message: diagnostic.message.clone(),
        span: snapshot_optional_span(diagnostic.span, sources)?,
        notes: diagnostic.notes.clone(),
        related: diagnostic
            .related
            .iter()
            .map(|related| {
                Ok(RelatedSnapshot {
                    message: related.message.clone(),
                    span: snapshot_optional_span(related.span, sources)?,
                })
            })
            .collect::<Result<Vec<_>, RenderError>>()?,
        suggestions: diagnostic
            .suggestions
            .iter()
            .map(|suggestion| {
                Ok(SuggestionSnapshot {
                    message: suggestion.message.clone(),
                    span: snapshot_optional_span(suggestion.span, sources)?,
                    replacement: suggestion.replacement.clone(),
                })
            })
            .collect::<Result<Vec<_>, RenderError>>()?,
    })
}

fn snapshot_optional_span(
    span: Option<Span>,
    sources: &SourceMap,
) -> Result<Option<DiagnosticSpanSnapshot>, RenderError> {
    span.map(|span| snapshot_span(span, sources)).transpose()
}

fn snapshot_span(span: Span, sources: &SourceMap) -> Result<DiagnosticSpanSnapshot, RenderError> {
    let file = sources
        .file(span.file_id)
        .ok_or(RenderError::MissingSource)?;
    let locs = file.span_locs(span).ok_or(RenderError::MissingSource)?;
    Ok(DiagnosticSpanSnapshot {
        file_id: span.file_id.0,
        path: file
            .path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        start: span.start.0,
        end: span.end.0,
        start_loc: locs.start,
        end_loc: locs.end,
    })
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
    for related in &diagnostic.related {
        rendered.push('\n');
        rendered.push_str("related: ");
        rendered.push_str(&related.message);
        if let Some(span) = related.span {
            let frame = sources
                .code_frame(span.file_id, span.start, span.end, None)
                .ok_or(RenderError::MissingSource)?;
            rendered.push('\n');
            rendered.push_str(&frame);
        }
    }
    for suggestion in &diagnostic.suggestions {
        rendered.push('\n');
        rendered.push_str("suggestion: ");
        rendered.push_str(&suggestion.message);
        if let Some(replacement) = &suggestion.replacement {
            rendered.push_str(" -> ");
            rendered.push_str(replacement);
        }
        if let Some(span) = suggestion.span {
            let frame = sources
                .code_frame(span.file_id, span.start, span.end, None)
                .ok_or(RenderError::MissingSource)?;
            rendered.push('\n');
            rendered.push_str(&frame);
        }
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
        let diagnostic = Diagnostic::error("E001", "boom")
            .with_span(Some(Span::new(id, 6, 11)))
            .with_note("check template syntax");
        let rendered = render_diagnostic(&diagnostic, &sources).expect("render");
        assert!(rendered.contains("[error] E001: boom"));
        assert!(rendered.contains("world"));
        assert!(rendered.contains("note: check template syntax"));
    }

    #[test]
    fn vue3_error_codes_match_official_numeric_values() {
        assert_eq!(Vue3ErrorCode::CORE_EXTEND_POINT, 54);
        assert_eq!(Vue3ErrorCode::DOM_EXTEND_POINT, 65);
        assert_eq!(Vue3ErrorCode::XInvalidExpression.as_u16(), 46);
        assert_eq!(
            Vue3ErrorCode::XVModelMalformedExpression.as_code_string(),
            "42"
        );
        assert_eq!(Vue3ErrorCode::XVModelOnInvalidElement.as_u16(), 58);
        assert_eq!(Vue3ErrorCode::XIgnoredSideEffectTag.as_u16(), 64);
    }

    #[test]
    fn snapshots_include_locations_related_info_and_suggestions() {
        let mut sources = SourceMap::default();
        let id = sources.add_file(Some("App.vue".into()), "<template>\n  <div/>\n</template>");
        let diagnostic = Diagnostic::vue3_error(
            Vue3ErrorCode::XInvalidEndTag,
            "Invalid end tag.",
            Some(Span::new(id, 13, 19)),
        )
        .with_related("opening tag is here", Some(Span::new(id, 13, 18)))
        .with_suggestion("remove end tag", Some(Span::new(id, 13, 19)), Some(""));
        let snapshots = diagnostic_snapshots(&[diagnostic], &sources).expect("snapshots");
        let snapshot = &snapshots[0];
        assert_eq!(snapshot.code, "23");
        assert_eq!(snapshot.span.as_ref().map(|span| span.start), Some(13));
        assert_eq!(
            snapshot.span.as_ref().map(|span| span.start_loc),
            Some(Loc { line: 2, column: 3 })
        );
        assert_eq!(snapshot.related.len(), 1);
        assert_eq!(snapshot.suggestions[0].replacement.as_deref(), Some(""));
    }

    #[test]
    fn vue2_output_source_range_is_snapshot_expressible() {
        let mut sources = SourceMap::default();
        let id = sources.add_file(None, "<div><span></div>");
        let diagnostic = Diagnostic::vue2_warning(
            "W_VUE2_MISSING_END_TAG",
            "tag <span> has no matching end tag.",
            Some(Span::new(id, 5, 11)),
        );
        let snapshots = DiagnosticSink {
            diagnostics: vec![diagnostic],
        }
        .snapshots(&sources)
        .expect("snapshots");
        assert_eq!(snapshots[0].severity, Severity::Warning);
        assert_eq!(snapshots[0].span.as_ref().map(|span| span.end), Some(11));
    }
}
