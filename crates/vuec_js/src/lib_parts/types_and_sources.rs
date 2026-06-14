use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingPattern, Declaration, Expression, ImportDeclarationSpecifier, Statement,
};
use oxc_diagnostics::{LabeledSpan, OxcDiagnostic};
use oxc_parser::{ParseOptions, Parser, ParserReturn};
use oxc_span::SourceType;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;
use thiserror::Error;
use vuec_ast::{JsExprId, JsPatternId, JsProgramId, JsStmtId};
use vuec_diagnostics::{Diagnostic, Vue3ErrorCode};
use vuec_source::{FileId, SourceAnchor, Span};

/// Parsing modes used for registered JavaScript snippets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsParseMode {
    /// Parse a single JavaScript expression.
    Expression,
    /// Parse one or more JavaScript statements.
    Statements,
    /// Parse a comma-separated parameter or binding pattern list.
    Params,
    /// Parse a Vue `v-for` expression with aliases and iterable.
    ForExpression,
    /// Parse a JavaScript module program.
    ScriptModule,
    /// Parse a classic script program.
    ScriptClassic,
    /// Parse a TypeScript program.
    TypeScript,
}

/// Serializable representation of an Oxc [`SourceType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsSourceType {
    /// Classic JavaScript script source.
    Script,
    /// ECMAScript module source.
    Module,
    /// CommonJS source.
    CommonJs,
    /// Oxc unambiguous source detection.
    Unambiguous,
    /// JavaScript source with JSX enabled.
    Jsx,
    /// TypeScript source.
    TypeScript,
    /// TypeScript source with JSX enabled.
    Tsx,
}

impl JsSourceType {
    /// Converts an Oxc source type into the serializable Vue compiler form.
    pub fn from_oxc(source_type: SourceType) -> Self {
        if source_type.is_typescript() {
            if source_type.is_jsx() {
                Self::Tsx
            } else {
                Self::TypeScript
            }
        } else if source_type.is_jsx() {
            Self::Jsx
        } else if source_type.is_commonjs() {
            Self::CommonJs
        } else if source_type.is_script() {
            Self::Script
        } else if source_type.is_module() {
            Self::Module
        } else {
            Self::Unambiguous
        }
    }

    /// Converts this value back to an Oxc source type.
    pub fn to_oxc(self) -> SourceType {
        match self {
            Self::Script => SourceType::script(),
            Self::Module => SourceType::mjs(),
            Self::CommonJs => SourceType::cjs(),
            Self::Unambiguous => SourceType::unambiguous(),
            Self::Jsx => SourceType::jsx(),
            Self::TypeScript => SourceType::ts(),
            Self::Tsx => SourceType::tsx(),
        }
    }
}

/// Registered JavaScript source plus span and parse metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsEntry {
    /// Interned source text.
    pub source: JsSource,
    /// Source span in the owning Vue file.
    pub span: Span,
    /// Parse mode used for this entry.
    pub mode: JsParseMode,
    /// Source type used for Oxc parsing.
    pub source_type: JsSourceType,
}

/// Interned JavaScript source text.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd)]
pub struct JsSource(Arc<str>);

impl JsSource {
    /// Returns the source text as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether two sources share the same interned allocation.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Returns the current strong reference count for the interned source.
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }
}

impl Default for JsSource {
    fn default() -> Self {
        Self(Arc::from(""))
    }
}

impl From<Arc<str>> for JsSource {
    fn from(value: Arc<str>) -> Self {
        Self(value)
    }
}

impl From<&str> for JsSource {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl From<String> for JsSource {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl AsRef<str> for JsSource {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for JsSource {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for JsSource {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for JsSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq for JsSource {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<str> for JsSource {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<JsSource> for str {
    fn eq(&self, other: &JsSource) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<&str> for JsSource {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<JsSource> for &str {
    fn eq(&self, other: &JsSource) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<String> for JsSource {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<JsSource> for String {
    fn eq(&self, other: &JsSource) -> bool {
        self == other.as_str()
    }
}

impl Serialize for JsSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for JsSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self::from)
    }
}

/// Statistics for the JavaScript source interner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsStringInternerStats {
    /// Number of source interning hits.
    pub hits: usize,
    /// Number of source interning misses.
    pub misses: usize,
    /// Number of unique interned source strings.
    pub entries: usize,
}

/// Parsed parameter list used by Vue directive aliases and slot params.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedParams<'a> {
    /// Original parameter-list source text.
    pub raw: &'a str,
    /// Top-level comma-separated items.
    pub items: Vec<&'a str>,
}

/// Parsed Vue `v-for` expression.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedForExpression<'a> {
    /// Original `v-for` expression source text.
    pub raw: &'a str,
    /// Alias side before the `in` or `of` separator.
    pub aliases: &'a str,
    /// Iterable expression after the `in` or `of` separator.
    pub iterable: &'a str,
    /// Top-level comma-separated alias items.
    pub items: Vec<&'a str>,
}

/// Filter call parsed from a Vue 2 filter-chain expression.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2FilterCall<'a> {
    /// Filter function name before the optional argument list.
    pub name: &'a str,
    /// Additional call arguments, excluding the piped base expression.
    pub args: Vec<&'a str>,
    /// Original filter segment after the pipe.
    pub raw: &'a str,
}

/// Vue 2 filter-chain parse result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2FilterExpression<'a> {
    /// Original source expression.
    pub raw: &'a str,
    /// Base expression before the first top-level filter pipe.
    pub base: &'a str,
    /// Parsed filters applied from left to right.
    pub filters: Vec<Vue2FilterCall<'a>>,
}

/// Absolute source context for JavaScript snippets inside a Vue template.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateJsSource {
    anchor: SourceAnchor,
}

impl TemplateJsSource {
    /// Creates a template JavaScript source mapper from file metadata.
    pub const fn new(file_id: FileId, base_offset: usize, len: usize) -> Self {
        Self {
            anchor: SourceAnchor::new(file_id, base_offset, len),
        }
    }

    /// Creates a mapper from an existing source anchor.
    pub const fn from_anchor(anchor: SourceAnchor) -> Self {
        Self { anchor }
    }

    /// Returns the source anchor used by this mapper.
    pub const fn anchor(self) -> SourceAnchor {
        self.anchor
    }

    /// Returns the full template span in the original file.
    pub fn full_span(self) -> Span {
        self.anchor.full_span()
    }

    /// Maps local template offsets to an absolute source span.
    pub fn span(self, start: usize, end: usize) -> Option<Span> {
        self.anchor.span(start, end)
    }

    /// Maps an Oxc local span to an absolute source span.
    pub fn oxc_span(self, span: oxc_span::Span) -> Option<Span> {
        self.span(span.start as usize, span.end as usize)
    }

    /// Returns a zero-length span at a local template offset.
    pub fn point(self, offset: usize) -> Option<Span> {
        self.span(offset, offset)
    }
}

/// Identifier rewrite callback used by [`prefix_expression_identifiers`].
pub trait IdentifierRewriter {
    /// Returns the replacement for one identifier, or `None` to keep it unchanged.
    fn rewrite_identifier(&self, ident: &str) -> Option<String>;
}

impl<F> IdentifierRewriter for F
where
    F: Fn(&str) -> Option<String>,
{
    fn rewrite_identifier(&self, ident: &str) -> Option<String> {
        self(ident)
    }
}

/// Error returned by JavaScript parsing helpers.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct JsParseError {
    message: String,
    diagnostics: Vec<OxcDiagnostic>,
}

impl JsParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            diagnostics: Vec::new(),
        }
    }

    fn from_diagnostics(diagnostics: Vec<OxcDiagnostic>) -> Self {
        let message = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            message,
            diagnostics,
        }
    }

    /// Returns the formatted parser error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the original Oxc diagnostics when this error came from Oxc.
    pub fn diagnostics(&self) -> &[OxcDiagnostic] {
        &self.diagnostics
    }

    /// Converts this parse error to a Vue 3 `X_INVALID_EXPRESSION` diagnostic.
    pub fn to_vue3_invalid_expression_diagnostic(
        &self,
        source_text: &str,
        span: Option<Span>,
    ) -> Diagnostic {
        js_error_to_vue3_invalid_expression_diagnostic(self, source_text, span)
    }
}

/// Lightweight summary of declarations, imports, exports, and parser errors.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsProgramSummary {
    /// Top-level binding names discovered in the program.
    pub bindings: Vec<String>,
    /// Import source strings discovered in the program.
    pub imports: Vec<String>,
    /// Export names discovered in the program.
    pub exports: Vec<String>,
    /// Oxc parser errors rendered as strings.
    pub errors: Vec<String>,
    /// Whether Oxc reported a parser panic.
    pub panicked: bool,
}
