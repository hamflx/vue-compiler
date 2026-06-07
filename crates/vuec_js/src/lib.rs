//! JavaScript parsing and side-store support for Vue compiler ASTs.
//!
//! Vue AST/HIR/MIR nodes store JavaScript handles instead of embedding parser
//! trees directly. This crate owns the source registry, Oxc parser entry
//! points, and small summary helpers used by SFC and template compilation.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

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

/// Registry for JavaScript snippets referenced by AST/HIR/MIR nodes.
pub struct JsAstStore {
    allocator: Allocator,
    sources: BTreeMap<String, Arc<str>>,
    interner_hits: usize,
    interner_misses: usize,
    expressions: Vec<JsEntry>,
    statements: Vec<JsEntry>,
    patterns: Vec<JsEntry>,
    programs: Vec<JsEntry>,
}

impl JsAstStore {
    /// Creates an empty JavaScript AST store.
    pub fn new() -> Self {
        Self {
            allocator: Allocator::default(),
            sources: BTreeMap::new(),
            interner_hits: 0,
            interner_misses: 0,
            expressions: Vec::new(),
            statements: Vec::new(),
            patterns: Vec::new(),
            programs: Vec::new(),
        }
    }

    /// Registers a JavaScript expression and returns its stable expression id.
    pub fn register_expr(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsExprId {
        self.push_expr(source, span, JsParseMode::Expression, source_type)
    }

    /// Registers a Vue `v-for` expression and returns its stable expression id.
    pub fn register_for_expression(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsExprId {
        self.push_expr(source, span, JsParseMode::ForExpression, source_type)
    }

    /// Registers JavaScript statement source and returns its stable statement id.
    pub fn register_stmt(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsStmtId {
        let id = JsStmtId(self.statements.len() as u32);
        let source = self.intern_source(source);
        self.statements.push(JsEntry {
            source,
            span,
            mode: JsParseMode::Statements,
            source_type: JsSourceType::from_oxc(source_type),
        });
        id
    }

    /// Registers a parameter or binding pattern list and returns its stable id.
    pub fn register_pattern(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsPatternId {
        let id = JsPatternId(self.patterns.len() as u32);
        let source = self.intern_source(source);
        self.patterns.push(JsEntry {
            source,
            span,
            mode: JsParseMode::Params,
            source_type: JsSourceType::from_oxc(source_type),
        });
        id
    }

    /// Registers a full JavaScript or TypeScript program and returns its id.
    pub fn register_program(
        &mut self,
        source: impl Into<String>,
        span: Span,
        mode: JsParseMode,
        source_type: SourceType,
    ) -> JsProgramId {
        let id = JsProgramId(self.programs.len() as u32);
        let source = self.intern_source(source);
        self.programs.push(JsEntry {
            source,
            span,
            mode,
            source_type: JsSourceType::from_oxc(source_type),
        });
        id
    }

    fn push_expr(
        &mut self,
        source: impl Into<String>,
        span: Span,
        mode: JsParseMode,
        source_type: SourceType,
    ) -> JsExprId {
        let id = JsExprId(self.expressions.len() as u32);
        let source = self.intern_source(source);
        self.expressions.push(JsEntry {
            source,
            span,
            mode,
            source_type: JsSourceType::from_oxc(source_type),
        });
        id
    }

    /// Returns source interning statistics for registered entries.
    pub fn string_interner_stats(&self) -> JsStringInternerStats {
        JsStringInternerStats {
            hits: self.interner_hits,
            misses: self.interner_misses,
            entries: self.sources.len(),
        }
    }

    /// Returns whether two entries share the same interned source allocation.
    pub fn interned_source_ptr_eq(&self, left: &JsEntry, right: &JsEntry) -> bool {
        left.source.ptr_eq(&right.source)
    }

    fn intern_source(&mut self, source: impl Into<String>) -> JsSource {
        let source = source.into();
        if let Some(existing) = self.sources.get(source.as_str()) {
            self.interner_hits += 1;
            return JsSource::from(existing.clone());
        }
        let interned = Arc::<str>::from(source.as_str());
        self.sources.insert(source, interned.clone());
        self.interner_misses += 1;
        JsSource::from(interned)
    }

    /// Looks up a registered expression entry.
    pub fn expr_entry(&self, id: JsExprId) -> Option<&JsEntry> {
        self.expressions.get(id.0 as usize)
    }

    /// Looks up a registered statement entry.
    pub fn stmt_entry(&self, id: JsStmtId) -> Option<&JsEntry> {
        self.statements.get(id.0 as usize)
    }

    /// Looks up a registered pattern entry.
    pub fn pattern_entry(&self, id: JsPatternId) -> Option<&JsEntry> {
        self.patterns.get(id.0 as usize)
    }

    /// Looks up a registered program entry.
    pub fn program_entry(&self, id: JsProgramId) -> Option<&JsEntry> {
        self.programs.get(id.0 as usize)
    }

    /// Returns all registered expression entries.
    pub fn expressions(&self) -> &[JsEntry] {
        &self.expressions
    }

    /// Returns all registered statement entries.
    pub fn statements(&self) -> &[JsEntry] {
        &self.statements
    }

    /// Returns all registered pattern entries.
    pub fn patterns(&self) -> &[JsEntry] {
        &self.patterns
    }

    /// Returns all registered program entries.
    pub fn programs(&self) -> &[JsEntry] {
        &self.programs
    }

    /// Parses a full program with Oxc and returns the raw parser result.
    pub fn parse_program<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> ParserReturn<'a> {
        Parser::new(&self.allocator, source_text, source_type)
            .with_options(ParseOptions {
                parse_regular_expression: true,
                ..ParseOptions::default()
            })
            .parse()
    }

    /// Parses a single JavaScript expression with Oxc.
    pub fn parse_expression<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> Result<Expression<'a>, JsParseError> {
        Parser::new(&self.allocator, source_text, source_type)
            .with_options(ParseOptions {
                parse_regular_expression: true,
                ..ParseOptions::default()
            })
            .parse_expression()
            .map_err(|diagnostics: Vec<OxcDiagnostic>| JsParseError::from_diagnostics(diagnostics))
    }

    /// Validates source text as one complete JavaScript expression.
    pub fn validate_expression(
        &self,
        source_text: &str,
        source_type: SourceType,
    ) -> Result<(), JsParseError> {
        let wrapped = format!("({source_text});");
        self.parse_program_checked(&wrapped, source_type)
            .map(|_| ())
    }

    /// Parses a registered expression by id.
    pub fn parse_expr(&self, id: JsExprId) -> Result<Expression<'_>, JsParseError> {
        let entry = self
            .expr_entry(id)
            .ok_or_else(|| JsParseError::new(format!("unknown JS expression id {}", id.0)))?;
        match entry.mode {
            JsParseMode::Expression => {
                self.parse_expression(&entry.source, entry.source_type.to_oxc())
            }
            JsParseMode::ForExpression => {
                let parsed =
                    self.parse_for_expression(&entry.source, entry.source_type.to_oxc())?;
                self.parse_expression(parsed.iterable, entry.source_type.to_oxc())
            }
            _ => Err(JsParseError::new(format!(
                "JS expression id {} has incompatible mode {:?}",
                id.0, entry.mode
            ))),
        }
    }

    /// Parses registered statement source by id as a checked program.
    pub fn parse_stmt(&self, id: JsStmtId) -> Result<ParserReturn<'_>, JsParseError> {
        let entry = self
            .stmt_entry(id)
            .ok_or_else(|| JsParseError::new(format!("unknown JS statement id {}", id.0)))?;
        self.parse_program_checked(&entry.source, entry.source_type.to_oxc())
    }

    /// Parses registered statement source by id and returns its first statement.
    pub fn parse_single_stmt(&self, id: JsStmtId) -> Result<Statement<'_>, JsParseError> {
        let parsed = self.parse_stmt(id)?;
        let mut body = parsed.program.body;
        if body.len() != 1 {
            return Err(JsParseError::new(format!(
                "JS statement id {} parsed to {} statements",
                id.0,
                body.len()
            )));
        }
        Ok(body.pop().expect("checked one statement"))
    }

    /// Parses a registered parameter or binding pattern list by id.
    pub fn parse_pattern(&self, id: JsPatternId) -> Result<ParsedParams<'_>, JsParseError> {
        let entry = self
            .pattern_entry(id)
            .ok_or_else(|| JsParseError::new(format!("unknown JS pattern id {}", id.0)))?;
        self.parse_params(&entry.source)
    }

    /// Parses a registered program by id.
    pub fn parse_registered_program(
        &self,
        id: JsProgramId,
    ) -> Result<ParserReturn<'_>, JsParseError> {
        let entry = self
            .program_entry(id)
            .ok_or_else(|| JsParseError::new(format!("unknown JS program id {}", id.0)))?;
        self.parse_program_checked(&entry.source, entry.source_type.to_oxc())
    }

    fn parse_program_checked<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> Result<ParserReturn<'a>, JsParseError> {
        let ret = self.parse_program(source_text, source_type);
        if ret.panicked || !ret.errors.is_empty() {
            return Err(JsParseError::from_diagnostics(ret.errors));
        }
        Ok(ret)
    }

    /// Parses source text according to a Vue compiler parse mode.
    pub fn parse_mode<'a>(
        &'a self,
        source_text: &'a str,
        mode: JsParseMode,
        source_type: SourceType,
    ) -> Result<JsParseResult<'a>, JsParseError> {
        match mode {
            JsParseMode::Expression => self
                .parse_expression(source_text, source_type)
                .map(JsParseResult::Expression),
            JsParseMode::Statements
            | JsParseMode::ScriptModule
            | JsParseMode::ScriptClassic
            | JsParseMode::TypeScript => Ok(JsParseResult::Program(
                self.parse_program_checked(source_text, source_type)?,
            )),
            JsParseMode::Params => self.parse_params(source_text).map(JsParseResult::Params),
            JsParseMode::ForExpression => self
                .parse_for_expression(source_text, source_type)
                .map(JsParseResult::ForExpression),
        }
    }

    /// Parses a Vue 2 filter chain and validates the base and argument expressions with Oxc.
    pub fn parse_vue2_filter_expression<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> Result<Vue2FilterExpression<'a>, JsParseError> {
        let parsed = parse_vue2_filter_expression(source_text);
        self.validate_expression(parsed.base, source_type)?;
        for filter in &parsed.filters {
            self.validate_vue2_filter_call(filter, source_type)?;
            for arg in &filter.args {
                self.validate_expression(arg, source_type)?;
            }
        }
        Ok(parsed)
    }

    fn validate_vue2_filter_call(
        &self,
        filter: &Vue2FilterCall<'_>,
        source_type: SourceType,
    ) -> Result<(), JsParseError> {
        let Some(open) = filter_call_open_paren(filter.raw) else {
            return Ok(());
        };
        let wrapped = format!("__vuec_filter__({}", &filter.raw[open + 1..]);
        self.validate_expression(&wrapped, source_type)
    }

    /// Validates JavaScript source as a Vue event handler function body.
    pub fn validate_function_body(
        &self,
        source_text: &str,
        source_type: SourceType,
    ) -> Result<(), JsParseError> {
        let wrapped = format!("function __vuec__($event){{\n{source_text}\n}}");
        self.parse_program_checked(&wrapped, source_type)
            .map(|_| ())
    }

    /// Converts a Vue 2 filter chain into the official runtime helper shape.
    pub fn rewrite_vue2_filter_expression(&self, source_text: &str) -> String {
        rewrite_vue2_filter_expression(source_text)
    }

    /// Parses a parameter or binding pattern list.
    pub fn parse_params<'a>(
        &'a self,
        source_text: &'a str,
    ) -> Result<ParsedParams<'a>, JsParseError> {
        let wrapped = format!("function __vuec__({source_text}) {{}}");
        let ret = self.parse_program(&wrapped, SourceType::script());
        if ret.panicked || !ret.errors.is_empty() {
            return Err(JsParseError::from_diagnostics(ret.errors));
        }

        Ok(ParsedParams {
            raw: source_text,
            items: split_top_level(source_text, ','),
        })
    }

    /// Parses a Vue `v-for` expression and validates its iterable expression.
    pub fn parse_for_expression<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> Result<ParsedForExpression<'a>, JsParseError> {
        let (aliases, iterable) = split_for_expression(source_text)
            .ok_or_else(|| JsParseError::new("missing `in`/`of` in v-for expression"))?;
        self.validate_expression(iterable, source_type)?;
        Ok(ParsedForExpression {
            raw: source_text,
            aliases,
            iterable,
            items: split_top_level(aliases, ','),
        })
    }

    /// Summarizes top-level program bindings, imports, exports, and parse errors.
    pub fn summarize_program(
        &self,
        source_text: &str,
        source_type: SourceType,
    ) -> JsProgramSummary {
        let parsed = self.parse_program(source_text, source_type);
        let mut summary = JsProgramSummary {
            errors: parsed.errors.iter().map(ToString::to_string).collect(),
            panicked: parsed.panicked,
            ..JsProgramSummary::default()
        };
        for statement in &parsed.program.body {
            collect_statement_summary(statement, &mut summary);
        }
        summary.bindings.sort();
        summary.bindings.dedup();
        summary.imports.sort();
        summary.imports.dedup();
        summary.exports.sort();
        summary.exports.dedup();
        summary
    }
}

impl Default for JsAstStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Converts an Oxc parse error into Vue 3's invalid-expression diagnostic shape.
pub fn js_error_to_vue3_invalid_expression_diagnostic(
    error: &JsParseError,
    source_text: &str,
    span: Option<Span>,
) -> Diagnostic {
    Diagnostic::vue3_error(
        Vue3ErrorCode::XInvalidExpression,
        vue3_expression_parse_error_message(error.message()),
        js_parse_error_span(error, source_text, span),
    )
}

/// Converts checked program parser output into a Vue 3 invalid-expression diagnostic.
pub fn js_program_errors_to_vue3_invalid_expression_diagnostic(
    errors: &[OxcDiagnostic],
    source_text: &str,
    span: Option<Span>,
) -> Diagnostic {
    let error = JsParseError::from_diagnostics(errors.to_vec());
    js_error_to_vue3_invalid_expression_diagnostic(&error, source_text, span)
}

/// Creates the official Vue 3 invalid-expression message from Oxc text.
pub fn vue3_expression_parse_error_message(raw: &str) -> String {
    let detail = raw
        .lines()
        .find_map(|line| line.trim().strip_prefix("× ").map(str::trim))
        .or_else(|| raw.lines().next().map(str::trim))
        .unwrap_or("Unexpected token");
    let detail = if detail == "Unexpected token" {
        "Unexpected token (1:3)"
    } else {
        detail
    };
    format!("Error parsing JavaScript expression: {detail}")
}

/// Maps an Oxc parse error onto the original source span for a JavaScript snippet.
pub fn js_parse_error_span(
    error: &JsParseError,
    source_text: &str,
    span: Option<Span>,
) -> Option<Span> {
    let span = span?;
    let relative = error
        .diagnostics()
        .first()
        .and_then(primary_label_offset)
        .or_else(|| parse_oxc_line_column(error.message()).map(|(_line, column)| column))
        .map(|offset| offset.min(source_text.len()))
        .unwrap_or(source_text.len());
    let start = span.start.0.saturating_add(relative).min(span.end.0);
    Some(Span::new(span.file_id, start, start))
}

/// Prefixes identifiers in a JavaScript-like Vue expression using a caller-supplied mapper.
pub fn prefix_expression_identifiers(
    expression: &str,
    rewriter: impl IdentifierRewriter,
    locals: &[String],
) -> String {
    let mut output = String::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let chars = expression.char_indices().collect::<Vec<_>>();
    let mut index = 0usize;
    let mut last_end = 0usize;

    while index < chars.len() {
        let start = chars[index].0;
        let ch = chars[index].1;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if !is_identifier_start(ch) {
            index += 1;
            continue;
        }

        index += 1;
        while index < chars.len() && is_identifier_continue(chars[index].1) {
            index += 1;
        }
        let end = chars
            .get(index)
            .map_or(expression.len(), |(offset, _)| *offset);
        let ident = &expression[start..end];
        let prev = previous_non_ws(expression, start);
        let next = next_non_ws(expression, end);
        let property_key = next == Some(':') && prev != Some('?');
        let object_shorthand = prev.is_some_and(|prev| matches!(prev, '{' | ','))
            && next.is_some_and(|next| matches!(next, '}' | ','));
        let should_keep = is_keyword(ident)
            || is_global_or_literal(ident)
            || locals.iter().any(|local| local == ident)
            || prev == Some('.')
            || property_key;
        if should_keep {
            continue;
        }
        if let Some(replacement) = rewriter.rewrite_identifier(ident) {
            output.push_str(expression.get(last_end..start).unwrap_or_default());
            if object_shorthand {
                output.push_str(ident);
                output.push_str(": ");
            }
            output.push_str(&replacement);
            last_end = end;
        }
    }

    if last_end == 0 {
        return expression.to_string();
    }
    output.push_str(expression.get(last_end..).unwrap_or_default());
    output
}

/// Parses a Vue 2 filter chain without treating filter pipes as JavaScript syntax.
pub fn parse_vue2_filter_expression(source_text: &str) -> Vue2FilterExpression<'_> {
    let mut filters = Vec::new();
    let mut expression: Option<&str> = None;
    let mut last_filter_index = 0usize;
    let mut state = JsScanState::default();
    let mut prev = '\0';

    for (index, ch) in source_text.char_indices() {
        if state.consume(source_text, index, ch, prev) {
            prev = ch;
            continue;
        }
        if ch == '|'
            && source_text[index + ch.len_utf8()..].chars().next() != Some('|')
            && prev != '|'
            && state.depth_is_zero()
        {
            if expression.is_none() {
                expression = Some(source_text[..index].trim());
                last_filter_index = index + ch.len_utf8();
            } else {
                let raw = source_text[last_filter_index..index].trim();
                if !raw.is_empty() {
                    filters.push(parse_vue2_filter_call(raw));
                }
                last_filter_index = index + ch.len_utf8();
            }
        }
        prev = ch;
    }

    let base = if let Some(expression) = expression {
        let raw = source_text[last_filter_index..].trim();
        if !raw.is_empty() {
            filters.push(parse_vue2_filter_call(raw));
        }
        expression
    } else {
        source_text.trim()
    };

    Vue2FilterExpression {
        raw: source_text,
        base,
        filters,
    }
}

/// Converts a Vue 2 filter chain into nested `_f()` runtime helper calls.
pub fn rewrite_vue2_filter_expression(source_text: &str) -> String {
    let parsed = parse_vue2_filter_expression(source_text);
    let mut expression = parsed.base.to_string();
    for filter in parsed.filters {
        expression = wrap_vue2_filter(&expression, &filter);
    }
    expression
}

/// Result of parsing source through a selected [`JsParseMode`].
pub enum JsParseResult<'a> {
    /// Parsed program result.
    Program(ParserReturn<'a>),
    /// Parsed expression node.
    Expression(Expression<'a>),
    /// Parsed parameter list.
    Params(ParsedParams<'a>),
    /// Parsed Vue `v-for` expression.
    ForExpression(ParsedForExpression<'a>),
}

fn split_for_expression(source_text: &str) -> Option<(&str, &str)> {
    let mut index = 0usize;
    let mut state = JsScanState::default();
    let mut prev = '\0';
    while index < source_text.len() {
        let ch = source_text[index..].chars().next()?;
        if state.consume(source_text, index, ch, prev) {
            prev = ch;
            index += ch.len_utf8();
            continue;
        }
        if ch == ' ' && state.depth_is_zero() {
            let rest = &source_text[index..];
            if rest.starts_with(" in ") {
                let left = source_text[..index].trim();
                let right = source_text[index + 4..].trim();
                return Some((left, right));
            }
            if rest.starts_with(" of ") {
                let left = source_text[..index].trim();
                let right = source_text[index + 4..].trim();
                return Some((left, right));
            }
        }
        prev = ch;
        index += ch.len_utf8();
    }
    None
}

fn split_top_level(source_text: &str, separator: char) -> Vec<&str> {
    let mut items = Vec::new();
    let mut state = JsScanState::default();
    let mut prev = '\0';
    let mut start = 0usize;
    for (index, ch) in source_text.char_indices() {
        if state.consume(source_text, index, ch, prev) {
            prev = ch;
            continue;
        }
        if ch == separator && state.depth_is_zero() {
            let item = source_text[start..index].trim();
            if !item.is_empty() {
                items.push(item);
            }
            start = index + ch.len_utf8();
        }
        prev = ch;
    }
    let tail = source_text[start..].trim();
    if !tail.is_empty() {
        items.push(tail);
    }
    items
}

fn primary_label_offset(diagnostic: &OxcDiagnostic) -> Option<usize> {
    let labels = diagnostic.labels.as_ref()?;
    labels
        .iter()
        .find(|label| label.primary())
        .or_else(|| labels.first())
        .map(LabeledSpan::offset)
}

fn parse_oxc_line_column(message: &str) -> Option<(usize, usize)> {
    for line in message.lines() {
        let trimmed = line.trim();
        let Some(open) = trimmed.rfind('(') else {
            continue;
        };
        let Some(close) = trimmed[open + 1..].find(')') else {
            continue;
        };
        let location = &trimmed[open + 1..open + 1 + close];
        let Some((line, column)) = location.split_once(':') else {
            continue;
        };
        let line = line.parse::<usize>().ok()?;
        let column = column.parse::<usize>().ok()?;
        return Some((line, column));
    }
    None
}

fn parse_vue2_filter_call(raw: &str) -> Vue2FilterCall<'_> {
    if let Some(open) = filter_call_open_paren(raw) {
        let name = raw[..open].trim();
        let close = raw.rfind(')').unwrap_or(raw.len());
        let args_source = raw[open + 1..close].trim();
        Vue2FilterCall {
            name,
            args: split_top_level(args_source, ','),
            raw,
        }
    } else {
        Vue2FilterCall {
            name: raw.trim(),
            args: Vec::new(),
            raw,
        }
    }
}

fn filter_call_open_paren(raw: &str) -> Option<usize> {
    let mut state = JsScanState::default();
    let mut prev = '\0';
    for (index, ch) in raw.char_indices() {
        let top_level = state.depth_is_zero();
        if state.consume(raw, index, ch, prev) {
            prev = ch;
            continue;
        }
        if ch == '(' && top_level {
            return Some(index);
        }
        prev = ch;
    }
    None
}

fn wrap_vue2_filter(exp: &str, filter: &Vue2FilterCall<'_>) -> String {
    if let Some(open) = filter_call_open_paren(filter.raw) {
        let args = &filter.raw[open + 1..];
        if args == ")" {
            format!("_f(\"{}\")({exp})", filter.name)
        } else {
            format!("_f(\"{}\")({exp},{args}", filter.name)
        }
    } else {
        format!("_f(\"{}\")({exp})", filter.name)
    }
}

#[derive(Default)]
struct JsScanState {
    in_single: bool,
    in_double: bool,
    in_template: bool,
    in_regex: bool,
    curly: usize,
    square: usize,
    paren: usize,
}

impl JsScanState {
    fn consume(&mut self, source: &str, index: usize, ch: char, prev: char) -> bool {
        if self.in_single {
            if ch == '\'' && prev != '\\' {
                self.in_single = false;
            }
            return true;
        }
        if self.in_double {
            if ch == '"' && prev != '\\' {
                self.in_double = false;
            }
            return true;
        }
        if self.in_template {
            if ch == '`' && prev != '\\' {
                self.in_template = false;
            }
            return true;
        }
        if self.in_regex {
            if ch == '/' && prev != '\\' {
                self.in_regex = false;
            }
            return true;
        }

        match ch {
            '\'' => self.in_single = true,
            '"' => self.in_double = true,
            '`' => self.in_template = true,
            '(' => self.paren += 1,
            ')' => self.paren = self.paren.saturating_sub(1),
            '[' => self.square += 1,
            ']' => self.square = self.square.saturating_sub(1),
            '{' => self.curly += 1,
            '}' => self.curly = self.curly.saturating_sub(1),
            '/' if !valid_division_before(source, index) => self.in_regex = true,
            _ => {}
        }
        false
    }

    fn depth_is_zero(&self) -> bool {
        self.curly == 0 && self.square == 0 && self.paren == 0
    }
}

fn valid_division_before(source: &str, slash_index: usize) -> bool {
    let previous = source[..slash_index]
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace());
    previous.is_some_and(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, ')' | '.' | '+' | '-' | '_' | '$' | ']')
    })
}

fn previous_non_ws(source: &str, offset: usize) -> Option<char> {
    source
        .get(..offset)?
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace())
}

fn next_non_ws(source: &str, offset: usize) -> Option<char> {
    source.get(offset..)?.chars().find(|ch| !ch.is_whitespace())
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

fn is_keyword(value: &str) -> bool {
    matches!(
        value,
        "const"
            | "let"
            | "var"
            | "function"
            | "return"
            | "if"
            | "else"
            | "for"
            | "while"
            | "do"
            | "switch"
            | "case"
            | "default"
            | "break"
            | "continue"
            | "new"
            | "class"
            | "extends"
            | "super"
            | "import"
            | "export"
            | "from"
            | "as"
            | "async"
            | "await"
            | "yield"
            | "try"
            | "catch"
            | "finally"
            | "throw"
            | "typeof"
            | "void"
            | "delete"
            | "in"
            | "of"
            | "instanceof"
    )
}

fn is_global_or_literal(value: &str) -> bool {
    matches!(
        value,
        "true"
            | "false"
            | "null"
            | "undefined"
            | "NaN"
            | "Infinity"
            | "this"
            | "Math"
            | "Number"
            | "String"
            | "Boolean"
            | "Array"
            | "Object"
            | "Date"
            | "RegExp"
            | "JSON"
            | "Promise"
            | "Symbol"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "BigInt"
            | "console"
            | "Reflect"
            | "globalThis"
            | "Error"
    )
}

fn collect_statement_summary(statement: &Statement<'_>, summary: &mut JsProgramSummary) {
    match statement {
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                collect_binding_pattern(&declarator.id, &mut summary.bindings);
            }
        }
        Statement::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
                summary.bindings.push(id.name.to_string());
            }
        }
        Statement::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                summary.bindings.push(id.name.to_string());
            }
        }
        Statement::ImportDeclaration(declaration) => {
            summary.imports.push(declaration.source.value.to_string());
            if let Some(specifiers) = &declaration.specifiers {
                for specifier in specifiers {
                    match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                            summary.bindings.push(specifier.local.name.to_string());
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                            summary.bindings.push(specifier.local.name.to_string());
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                            summary.bindings.push(specifier.local.name.to_string());
                        }
                    }
                }
            }
        }
        Statement::ExportDefaultDeclaration(_) => {
            summary.exports.push("default".into());
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                collect_declaration_summary(declaration, summary);
            }
            for specifier in &declaration.specifiers {
                summary.exports.push(specifier.local.name().to_string());
            }
        }
        Statement::ExportAllDeclaration(declaration) => {
            summary
                .exports
                .push(format!("* from {}", declaration.source.value));
        }
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_statement_summary(statement, summary);
            }
        }
        _ => {}
    }
}

fn collect_declaration_summary(declaration: &Declaration<'_>, summary: &mut JsProgramSummary) {
    match declaration {
        Declaration::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                collect_binding_pattern(&declarator.id, &mut summary.bindings);
            }
        }
        Declaration::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
                summary.bindings.push(id.name.to_string());
                summary.exports.push(id.name.to_string());
            }
        }
        Declaration::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                summary.bindings.push(id.name.to_string());
                summary.exports.push(id.name.to_string());
            }
        }
        _ => {}
    }
}

fn collect_binding_pattern(pattern: &BindingPattern<'_>, bindings: &mut Vec<String>) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            bindings.push(identifier.name.to_string());
        }
        BindingPattern::ObjectPattern(object) => {
            for property in &object.properties {
                collect_binding_pattern(&property.value, bindings);
            }
            if let Some(rest) = &object.rest {
                collect_binding_pattern(&rest.argument, bindings);
            }
        }
        BindingPattern::ArrayPattern(array) => {
            for element in array.elements.iter().flatten() {
                collect_binding_pattern(element, bindings);
            }
            if let Some(rest) = &array.rest {
                collect_binding_pattern(&rest.argument, bindings);
            }
        }
        BindingPattern::AssignmentPattern(assignment) => {
            collect_binding_pattern(&assignment.left, bindings);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vuec_source::{FileId, Span};

    #[test]
    fn parses_expression() {
        let store = JsAstStore::new();
        let expr = store
            .parse_expression("foo + bar", SourceType::script())
            .expect("expression");
        assert!(matches!(expr, Expression::BinaryExpression(_)));
    }

    #[test]
    fn parses_program() {
        let store = JsAstStore::new();
        let ret = store.parse_program("let x = 1 + 2;", SourceType::script());
        assert!(ret.errors.is_empty());
        assert!(!ret.program.body.is_empty());
    }

    #[test]
    fn validates_complete_expression_source() {
        let store = JsAstStore::new();
        assert!(store
            .validate_expression("{ foo: bar }", SourceType::script())
            .is_ok());
        assert!(store
            .validate_expression("a----", SourceType::script())
            .is_err());
        assert!(store
            .validate_expression("foo(", SourceType::script())
            .is_err());
        assert!(store
            .validate_expression("foo(); bar()", SourceType::script())
            .is_err());
        assert!(store
            .validate_function_body("foo(); bar()", SourceType::script())
            .is_ok());
    }

    #[test]
    fn parses_v_for_shape() {
        let store = JsAstStore::new();
        let parsed = store
            .parse_for_expression("(item, index) in list", SourceType::script())
            .expect("v-for");
        assert_eq!(parsed.aliases, "(item, index)");
        assert_eq!(parsed.iterable, "list");
        assert_eq!(parsed.items, vec!["(item, index)"]);
    }

    #[test]
    fn parses_v_for_aliases_without_splitting_literal_commas() {
        let store = JsAstStore::new();
        let parsed = store
            .parse_for_expression(
                "item, label = 'a,b', matcher = /x,y/g in rows",
                SourceType::script(),
            )
            .expect("v-for");

        assert_eq!(parsed.aliases, "item, label = 'a,b', matcher = /x,y/g");
        assert_eq!(
            parsed.items,
            vec!["item", "label = 'a,b'", "matcher = /x,y/g"]
        );
    }

    #[test]
    fn registers_and_parses_expression_by_id() {
        let mut store = JsAstStore::new();
        let id = store.register_expr(
            "foo + bar",
            Span::new(FileId(0), 10, 19),
            SourceType::script(),
        );
        let entry = store.expr_entry(id).expect("entry");
        assert_eq!(entry.source, "foo + bar");
        assert_eq!(entry.span, Span::new(FileId(0), 10, 19));
        assert_eq!(entry.mode, JsParseMode::Expression);
        assert_eq!(entry.source_type, JsSourceType::Script);

        let expr = store.parse_expr(id).expect("registered expression");
        assert!(matches!(expr, Expression::BinaryExpression(_)));
    }

    #[test]
    fn repeated_js_sources_are_interned_without_changing_serialized_shape() {
        let mut store = JsAstStore::new();
        let first = store.register_expr(
            "item.count",
            Span::new(FileId(0), 0, 10),
            SourceType::script(),
        );
        let second = store.register_stmt(
            "item.count",
            Span::new(FileId(0), 20, 30),
            SourceType::script(),
        );
        let distinct =
            store.register_pattern("item", Span::new(FileId(0), 40, 44), SourceType::script());

        let first_entry = store.expr_entry(first).unwrap();
        let second_entry = store.stmt_entry(second).unwrap();
        let distinct_entry = store.pattern_entry(distinct).unwrap();
        assert!(store.interned_source_ptr_eq(first_entry, second_entry));
        assert!(!store.interned_source_ptr_eq(first_entry, distinct_entry));
        assert_eq!(
            store.string_interner_stats(),
            JsStringInternerStats {
                hits: 1,
                misses: 2,
                entries: 2,
            }
        );

        let serialized = serde_json::to_value(first_entry).unwrap();
        assert_eq!(serialized["source"], "item.count");
    }

    #[test]
    fn registers_statements_patterns_and_programs_by_id() {
        let mut store = JsAstStore::new();
        let stmt_id =
            store.register_stmt("foo();", Span::new(FileId(0), 0, 6), SourceType::script());
        let pattern_id = store.register_pattern(
            "{ item, index }",
            Span::new(FileId(0), 7, 22),
            SourceType::script(),
        );
        let program_id = store.register_program(
            "export const x = 1;",
            Span::new(FileId(0), 23, 42),
            JsParseMode::ScriptModule,
            SourceType::mjs(),
        );

        let stmt = store
            .parse_single_stmt(stmt_id)
            .expect("registered statement");
        assert!(matches!(stmt, Statement::ExpressionStatement(_)));

        let pattern = store.parse_pattern(pattern_id).expect("registered pattern");
        assert_eq!(pattern.items, vec!["{ item, index }"]);

        let program = store
            .parse_registered_program(program_id)
            .expect("registered program");
        assert!(program.errors.is_empty());
        assert_eq!(
            store.program_entry(program_id).unwrap().source_type,
            JsSourceType::Module
        );
    }

    #[test]
    fn parses_statement_lists_params_for_script_modes_by_id() {
        let mut store = JsAstStore::new();
        let stmt_id = store.register_stmt(
            "foo(); bar();",
            Span::new(FileId(0), 0, 12),
            SourceType::script(),
        );
        let params_id = store.register_pattern(
            "item, i",
            Span::new(FileId(0), 13, 20),
            SourceType::script(),
        );
        let for_id = store.register_for_expression(
            "(item, i) in list",
            Span::new(FileId(0), 21, 38),
            SourceType::script(),
        );
        let classic_id = store.register_program(
            "var x = 1;",
            Span::new(FileId(0), 39, 49),
            JsParseMode::ScriptClassic,
            SourceType::script(),
        );
        let ts_id = store.register_program(
            "const x: number = 1;",
            Span::new(FileId(0), 50, 70),
            JsParseMode::TypeScript,
            SourceType::ts(),
        );

        let statements = store.parse_stmt(stmt_id).expect("statement list");
        assert_eq!(statements.program.body.len(), 2);
        assert!(store.parse_single_stmt(stmt_id).is_err());

        let params = store.parse_pattern(params_id).expect("params");
        assert_eq!(params.items, vec!["item", "i"]);

        let parsed_for = match store
            .parse_mode(
                "(item, i) in list",
                JsParseMode::ForExpression,
                SourceType::script(),
            )
            .expect("v-for mode")
        {
            JsParseResult::ForExpression(parsed) => parsed,
            _ => panic!("expected v-for result"),
        };
        assert_eq!(parsed_for.iterable, "list");
        assert!(store.parse_expr(for_id).is_ok());

        assert!(store
            .parse_registered_program(classic_id)
            .expect("classic script")
            .errors
            .is_empty());
        assert!(store
            .parse_registered_program(ts_id)
            .expect("typescript")
            .errors
            .is_empty());
    }

    #[test]
    fn parses_params_without_splitting_literal_commas() {
        let store = JsAstStore::new();
        let parsed = store
            .parse_params("first = ',', second = /a,b/g, third = `x,y`, fourth")
            .expect("params");

        assert_eq!(
            parsed.items,
            vec!["first = ','", "second = /a,b/g", "third = `x,y`", "fourth"]
        );
    }

    #[test]
    fn parse_mode_checks_program_errors() {
        let store = JsAstStore::new();
        assert!(store
            .parse_mode("const =", JsParseMode::Statements, SourceType::script())
            .is_err());
    }

    #[test]
    fn maps_template_local_oxc_spans_to_absolute_source_spans() {
        let mapper = TemplateJsSource::new(FileId(7), 42, 20);
        assert_eq!(mapper.full_span(), Span::new(FileId(7), 42, 62));
        assert_eq!(mapper.span(3, 9), Some(Span::new(FileId(7), 45, 51)));
        assert_eq!(
            mapper.oxc_span(oxc_span::Span::new(4, 8)),
            Some(Span::new(FileId(7), 46, 50))
        );
        assert_eq!(mapper.point(20), Some(Span::new(FileId(7), 62, 62)));
        assert_eq!(mapper.span(3, 21), None);
    }

    #[test]
    fn maps_parse_errors_to_vue3_diagnostics_with_source_span() {
        let store = JsAstStore::new();
        let err = store
            .parse_expression("(a[)", SourceType::script())
            .expect_err("parse error");
        let diagnostic =
            err.to_vue3_invalid_expression_diagnostic("a[", Some(Span::new(FileId(3), 100, 102)));
        assert_eq!(diagnostic.code, "46");
        assert!(diagnostic
            .message
            .contains("Error parsing JavaScript expression"));
        assert_eq!(diagnostic.span, Some(Span::new(FileId(3), 102, 102)));
    }

    #[test]
    fn parses_vue2_filters_before_validating_base_expression() {
        let store = JsAstStore::new();
        let parsed = store
            .parse_vue2_filter_expression(
                "message | capitalize | append('!')",
                SourceType::script(),
            )
            .expect("filter chain");

        assert_eq!(parsed.base, "message");
        assert_eq!(parsed.filters.len(), 2);
        assert_eq!(parsed.filters[0].name, "capitalize");
        assert_eq!(parsed.filters[1].name, "append");
        assert_eq!(parsed.filters[1].args, vec!["'!'"]);
        assert_eq!(
            rewrite_vue2_filter_expression("message | capitalize | append('!')"),
            "_f(\"append\")(_f(\"capitalize\")(message),'!')"
        );

        let logical_or = parse_vue2_filter_expression("a || b | c");
        assert_eq!(logical_or.base, "a || b");
        assert_eq!(logical_or.filters[0].name, "c");

        assert_eq!(
            rewrite_vue2_filter_expression("message | append('!', count + 1)"),
            "_f(\"append\")(message,'!', count + 1)"
        );
    }

    #[test]
    fn parses_vue2_filter_args_without_splitting_literal_commas() {
        let store = JsAstStore::new();
        let parsed = store
            .parse_vue2_filter_expression(
                "message | append(',', `x,y`, /a,b/g, count)",
                SourceType::script(),
            )
            .expect("filter chain");

        assert_eq!(parsed.filters.len(), 1);
        assert_eq!(
            parsed.filters[0].args,
            vec!["','", "`x,y`", "/a,b/g", "count"]
        );
        assert_eq!(
            rewrite_vue2_filter_expression("message | append(',', `x,y`, /a,b/g, count)"),
            "_f(\"append\")(message,',', `x,y`, /a,b/g, count)"
        );
    }

    #[test]
    fn rejects_invalid_vue2_filter_arguments() {
        let store = JsAstStore::new();
        assert!(store
            .parse_vue2_filter_expression("message | append(foo()", SourceType::script())
            .is_err());
        assert!(store
            .parse_vue2_filter_expression("message | append(ok, nope }", SourceType::script())
            .is_err());
    }

    #[test]
    fn prefixes_expression_identifiers_with_locals_and_property_keys() {
        let rewritten = prefix_expression_identifiers(
            "{ foo, bar: baz, nested: local + Math.max(count, item.value) }",
            |ident: &str| Some(format!("_ctx.{ident}")),
            &["local".into()],
        );
        assert_eq!(
            rewritten,
            "{ foo: _ctx.foo, bar: _ctx.baz, nested: local + Math.max(_ctx.count, _ctx.item.value) }"
        );
    }
}
