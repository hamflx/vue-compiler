#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlNamespace {
    Html,
    Svg,
    MathMl,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HtmlAttribute {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlTokenKind {
    Text(String),
    Comment(String),
    Cdata(String),
    Doctype(String),
    StartTag {
        name: String,
        attributes: Vec<HtmlAttribute>,
        self_closing: bool,
    },
    EndTag {
        name: String,
    },
    Eof,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HtmlToken {
    pub kind: HtmlTokenKind,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug)]
pub struct HtmlTokenizer<'a> {
    source: &'a str,
    cursor: usize,
}

impl<'a> HtmlTokenizer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source, cursor: 0 }
    }

    pub fn tokenize(mut self) -> Vec<HtmlToken> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token();
            let eof = matches!(token.kind, HtmlTokenKind::Eof);
            tokens.push(token);
            if eof {
                break;
            }
        }
        tokens
    }

    pub fn next_token(&mut self) -> HtmlToken {
        if self.cursor >= self.source.len() {
            return HtmlToken {
                kind: HtmlTokenKind::Eof,
                start: self.cursor,
                end: self.cursor,
            };
        }

        if self.remaining().starts_with("<!--") {
            return self.consume_block("<!--", "-->", |body| HtmlTokenKind::Comment(body));
        }
        if self.remaining().starts_with("<![CDATA[") {
            return self.consume_block("<![CDATA[", "]]>", |body| HtmlTokenKind::Cdata(body));
        }
        if self.remaining().to_ascii_uppercase().starts_with("<!DOCTYPE") {
            return self.consume_block("<!DOCTYPE", ">", |body| HtmlTokenKind::Doctype(body.trim().to_string()));
        }
        if self.remaining().starts_with("</") {
            return self.consume_end_tag();
        }
        if self.remaining().starts_with('<') {
            return self.consume_start_tag();
        }

        self.consume_text()
    }

    fn consume_text(&mut self) -> HtmlToken {
        let start = self.cursor;
        let end = self.remaining().find('<').map(|offset| self.cursor + offset).unwrap_or(self.source.len());
        self.cursor = end;
        HtmlToken {
            kind: HtmlTokenKind::Text(self.source[start..end].to_string()),
            start,
            end,
        }
    }

    fn consume_end_tag(&mut self) -> HtmlToken {
        let start = self.cursor;
        self.cursor += 2;
        self.consume_whitespace();
        let name = self.consume_name();
        self.consume_until('>');
        HtmlToken {
            kind: HtmlTokenKind::EndTag { name },
            start,
            end: self.cursor,
        }
    }

    fn consume_start_tag(&mut self) -> HtmlToken {
        let start = self.cursor;
        self.cursor += 1;
        let name = self.consume_name();
        let mut attributes = Vec::new();
        let mut self_closing = false;

        loop {
            self.consume_whitespace();
            if self.remaining().starts_with("/>") {
                self_closing = true;
                self.cursor += 2;
                break;
            }
            if self.remaining().starts_with('>') {
                self.cursor += 1;
                break;
            }
            if self.cursor >= self.source.len() {
                break;
            }

            let attr_name = self.consume_name();
            self.consume_whitespace();
            let value = if self.remaining().starts_with('=') {
                self.cursor += 1;
                self.consume_whitespace();
                Some(self.consume_attr_value())
            } else {
                None
            };
            attributes.push(HtmlAttribute {
                name: attr_name,
                value,
            });
        }

        HtmlToken {
            kind: HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            },
            start,
            end: self.cursor,
        }
    }

    fn consume_block<F>(&mut self, open: &str, close: &str, map: F) -> HtmlToken
    where
        F: FnOnce(String) -> HtmlTokenKind,
    {
        let start = self.cursor;
        self.cursor += open.len();
        let body_start = self.cursor;
        let body_end = self.remaining().find(close).map(|offset| self.cursor + offset).unwrap_or(self.source.len());
        let body = self.source[body_start..body_end].to_string();
        self.cursor = if body_end < self.source.len() {
            body_end + close.len()
        } else {
            body_end
        };
        HtmlToken {
            kind: map(body),
            start,
            end: self.cursor,
        }
    }

    fn consume_name(&mut self) -> String {
        let start = self.cursor;
        while let Some(ch) = self.remaining().chars().next() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | ':' | '_' | '.') {
                self.cursor += ch.len_utf8();
            } else {
                break;
            }
        }
        self.source[start..self.cursor].to_string()
    }

    fn consume_attr_value(&mut self) -> String {
        match self.remaining().chars().next() {
            Some('"') | Some('\'') => {
                let quote = self.remaining().chars().next().unwrap();
                self.cursor += quote.len_utf8();
                let value_start = self.cursor;
                while let Some(ch) = self.remaining().chars().next() {
                    if ch == quote {
                        let value = self.source[value_start..self.cursor].to_string();
                        self.cursor += quote.len_utf8();
                        return value;
                    }
                    self.cursor += ch.len_utf8();
                }
                self.source[value_start..self.cursor].to_string()
            }
            _ => {
                let start = self.cursor;
                while let Some(ch) = self.remaining().chars().next() {
                    if ch.is_whitespace() || ch == '>' || ch == '/' {
                        break;
                    }
                    self.cursor += ch.len_utf8();
                }
                self.source[start..self.cursor].to_string()
            }
        }
    }

    fn consume_whitespace(&mut self) {
        while let Some(ch) = self.remaining().chars().next() {
            if ch.is_whitespace() {
                self.cursor += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn consume_until(&mut self, needle: char) {
        while let Some(ch) = self.remaining().chars().next() {
            self.cursor += ch.len_utf8();
            if ch == needle {
                break;
            }
        }
    }

    fn remaining(&self) -> &'a str {
        &self.source[self.cursor..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_basic_html() {
        let tokens = HtmlTokenizer::new("<!--x--><div id=\"a\">hi</div>").tokenize();
        assert!(matches!(tokens[0].kind, HtmlTokenKind::Comment(ref s) if s == "x"));
        assert!(matches!(tokens[1].kind, HtmlTokenKind::StartTag { ref name, .. } if name == "div"));
        assert!(matches!(tokens[2].kind, HtmlTokenKind::Text(ref s) if s == "hi"));
        assert!(matches!(tokens[3].kind, HtmlTokenKind::EndTag { ref name } if name == "div"));
    }

    #[test]
    fn tokenizes_doctype_and_cdata() {
        let tokens = HtmlTokenizer::new("<!DOCTYPE html><![CDATA[x]]>").tokenize();
        assert!(matches!(tokens[0].kind, HtmlTokenKind::Doctype(ref s) if s == "html"));
        assert!(matches!(tokens[1].kind, HtmlTokenKind::Cdata(ref s) if s == "x"));
    }
}
