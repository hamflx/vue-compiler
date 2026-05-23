#![forbid(unsafe_code)]

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingPattern, Declaration, Expression, ImportDeclarationSpecifier, Statement,
};
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::{ParseOptions, Parser, ParserReturn};
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use vuec_ast::{JsExprId, JsPatternId, JsProgramId, JsStmtId};
use vuec_source::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsParseMode {
    Expression,
    Statements,
    Params,
    ForExpression,
    ScriptModule,
    ScriptClassic,
    TypeScript,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsSourceType {
    Script,
    Module,
    CommonJs,
    Unambiguous,
    Jsx,
    TypeScript,
    Tsx,
}

impl JsSourceType {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsEntry {
    pub source: String,
    pub span: Span,
    pub mode: JsParseMode,
    pub source_type: JsSourceType,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedParams<'a> {
    pub raw: &'a str,
    pub items: Vec<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedForExpression<'a> {
    pub raw: &'a str,
    pub aliases: &'a str,
    pub iterable: &'a str,
    pub items: Vec<&'a str>,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct JsParseError {
    message: String,
}

impl JsParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn from_diagnostics(diagnostics: Vec<String>) -> Self {
        Self {
            message: diagnostics.join("\n"),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsProgramSummary {
    pub bindings: Vec<String>,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub errors: Vec<String>,
    pub panicked: bool,
}

pub struct JsAstStore {
    allocator: Allocator,
    expressions: Vec<JsEntry>,
    statements: Vec<JsEntry>,
    patterns: Vec<JsEntry>,
    programs: Vec<JsEntry>,
}

impl JsAstStore {
    pub fn new() -> Self {
        Self {
            allocator: Allocator::default(),
            expressions: Vec::new(),
            statements: Vec::new(),
            patterns: Vec::new(),
            programs: Vec::new(),
        }
    }

    pub fn register_expr(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsExprId {
        self.push_expr(source, span, JsParseMode::Expression, source_type)
    }

    pub fn register_for_expression(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsExprId {
        self.push_expr(source, span, JsParseMode::ForExpression, source_type)
    }

    pub fn register_stmt(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsStmtId {
        let id = JsStmtId(self.statements.len() as u32);
        self.statements.push(JsEntry {
            source: source.into(),
            span,
            mode: JsParseMode::Statements,
            source_type: JsSourceType::from_oxc(source_type),
        });
        id
    }

    pub fn register_pattern(
        &mut self,
        source: impl Into<String>,
        span: Span,
        source_type: SourceType,
    ) -> JsPatternId {
        let id = JsPatternId(self.patterns.len() as u32);
        self.patterns.push(JsEntry {
            source: source.into(),
            span,
            mode: JsParseMode::Params,
            source_type: JsSourceType::from_oxc(source_type),
        });
        id
    }

    pub fn register_program(
        &mut self,
        source: impl Into<String>,
        span: Span,
        mode: JsParseMode,
        source_type: SourceType,
    ) -> JsProgramId {
        let id = JsProgramId(self.programs.len() as u32);
        self.programs.push(JsEntry {
            source: source.into(),
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
        self.expressions.push(JsEntry {
            source: source.into(),
            span,
            mode,
            source_type: JsSourceType::from_oxc(source_type),
        });
        id
    }

    pub fn expr_entry(&self, id: JsExprId) -> Option<&JsEntry> {
        self.expressions.get(id.0 as usize)
    }

    pub fn stmt_entry(&self, id: JsStmtId) -> Option<&JsEntry> {
        self.statements.get(id.0 as usize)
    }

    pub fn pattern_entry(&self, id: JsPatternId) -> Option<&JsEntry> {
        self.patterns.get(id.0 as usize)
    }

    pub fn program_entry(&self, id: JsProgramId) -> Option<&JsEntry> {
        self.programs.get(id.0 as usize)
    }

    pub fn expressions(&self) -> &[JsEntry] {
        &self.expressions
    }

    pub fn statements(&self) -> &[JsEntry] {
        &self.statements
    }

    pub fn patterns(&self) -> &[JsEntry] {
        &self.patterns
    }

    pub fn programs(&self) -> &[JsEntry] {
        &self.programs
    }

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
            .map_err(|diagnostics: Vec<OxcDiagnostic>| {
                JsParseError::from_diagnostics(
                    diagnostics.into_iter().map(|d| d.to_string()).collect(),
                )
            })
    }

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

    pub fn parse_stmt(&self, id: JsStmtId) -> Result<Statement<'_>, JsParseError> {
        let entry = self
            .stmt_entry(id)
            .ok_or_else(|| JsParseError::new(format!("unknown JS statement id {}", id.0)))?;
        let parsed = self.parse_program_checked(&entry.source, entry.source_type.to_oxc())?;
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

    pub fn parse_pattern(&self, id: JsPatternId) -> Result<ParsedParams<'_>, JsParseError> {
        let entry = self
            .pattern_entry(id)
            .ok_or_else(|| JsParseError::new(format!("unknown JS pattern id {}", id.0)))?;
        self.parse_params(&entry.source)
    }

    pub fn parse_registered_program(
        &self,
        id: JsProgramId,
    ) -> Result<ParserReturn<'_>, JsParseError> {
        let entry = self
            .program_entry(id)
            .ok_or_else(|| JsParseError::new(format!("unknown JS program id {}", id.0)))?;
        Ok(self.parse_program(&entry.source, entry.source_type.to_oxc()))
    }

    fn parse_program_checked<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> Result<ParserReturn<'a>, JsParseError> {
        let ret = self.parse_program(source_text, source_type);
        if ret.panicked || !ret.errors.is_empty() {
            return Err(JsParseError::from_diagnostics(
                ret.errors.into_iter().map(|d| d.to_string()).collect(),
            ));
        }
        Ok(ret)
    }

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
                self.parse_program(source_text, source_type),
            )),
            JsParseMode::Params => self.parse_params(source_text).map(JsParseResult::Params),
            JsParseMode::ForExpression => self
                .parse_for_expression(source_text, source_type)
                .map(JsParseResult::ForExpression),
        }
    }

    pub fn parse_params<'a>(
        &'a self,
        source_text: &'a str,
    ) -> Result<ParsedParams<'a>, JsParseError> {
        let wrapped = format!("function __vuec__({source_text}) {{}}");
        let ret = self.parse_program(&wrapped, SourceType::script());
        if ret.panicked || !ret.errors.is_empty() {
            return Err(JsParseError::from_diagnostics(
                ret.errors.into_iter().map(|d| d.to_string()).collect(),
            ));
        }

        Ok(ParsedParams {
            raw: source_text,
            items: split_top_level(source_text, ','),
        })
    }

    pub fn parse_for_expression<'a>(
        &'a self,
        source_text: &'a str,
        source_type: SourceType,
    ) -> Result<ParsedForExpression<'a>, JsParseError> {
        let (aliases, iterable) =
            split_for_expression(source_text).ok_or_else(|| JsParseError {
                message: "missing `in`/`of` in v-for expression".into(),
            })?;
        let _iterable = self.parse_expression(iterable, source_type)?;
        Ok(ParsedForExpression {
            raw: source_text,
            aliases,
            iterable,
            items: split_top_level(aliases, ','),
        })
    }

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

pub enum JsParseResult<'a> {
    Program(ParserReturn<'a>),
    Expression(Expression<'a>),
    Params(ParsedParams<'a>),
    ForExpression(ParsedForExpression<'a>),
}

fn split_for_expression(source_text: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < source_text.len() {
        let ch = source_text[index..].chars().next()?;
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 => {
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
            _ => {}
        }
        index += ch.len_utf8();
    }
    None
}

fn split_top_level(source_text: &str, separator: char) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in source_text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if ch == separator && depth == 0 => {
                let item = source_text[start..index].trim();
                if !item.is_empty() {
                    items.push(item);
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = source_text[start..].trim();
    if !tail.is_empty() {
        items.push(tail);
    }
    items
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

        let stmt = store.parse_stmt(stmt_id).expect("registered statement");
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
}
