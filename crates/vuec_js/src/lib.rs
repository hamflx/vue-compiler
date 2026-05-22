#![forbid(unsafe_code)]

use oxc_allocator::Allocator;
use oxc_ast::ast::Expression;
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::{ParseOptions, Parser, ParserReturn};
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsParseMode {
    Expression,
    Statements,
    Params,
    ForExpression,
    ScriptModule,
    ScriptClassic,
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
    pub items: Vec<&'a str>,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct JsParseError {
    message: String,
}

impl JsParseError {
    fn from_diagnostics(diagnostics: Vec<String>) -> Self {
        Self {
            message: diagnostics.join("\n"),
        }
    }
}

pub struct JsAstStore {
    allocator: Allocator,
}

impl JsAstStore {
    pub fn new() -> Self {
        Self {
            allocator: Allocator::default(),
        }
    }

    pub fn parse_program<'a>(&'a self, source_text: &'a str, source_type: SourceType) -> ParserReturn<'a> {
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
                JsParseError::from_diagnostics(diagnostics.into_iter().map(|d| d.to_string()).collect())
            })
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
            JsParseMode::Statements | JsParseMode::ScriptModule | JsParseMode::ScriptClassic => {
                Ok(JsParseResult::Program(self.parse_program(source_text, source_type)))
            }
            JsParseMode::Params => self.parse_params(source_text).map(JsParseResult::Params),
            JsParseMode::ForExpression => self
                .parse_for_expression(source_text, source_type)
                .map(JsParseResult::ForExpression),
        }
    }

    pub fn parse_params<'a>(&'a self, source_text: &'a str) -> Result<ParsedParams<'a>, JsParseError> {
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
        let (aliases, iterable) = split_for_expression(source_text)
            .ok_or_else(|| JsParseError { message: "missing `in`/`of` in v-for expression".into() })?;
        let _iterable = self.parse_expression(iterable, source_type)?;
        Ok(ParsedForExpression {
            raw: source_text,
            aliases,
            items: split_top_level(aliases, ','),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(parsed.items, vec!["(item, index)"]);
    }
}
