#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlNamespace {
    Html,
    Svg,
    MathMl,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlQuoteKind {
    Double,
    Single,
    Unquoted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HtmlAttribute {
    pub name: String,
    pub value: Option<String>,
    pub quote: Option<HtmlQuoteKind>,
    pub start: usize,
    pub end: usize,
    pub name_start: usize,
    pub name_end: usize,
    pub value_start: Option<usize>,
    pub value_end: Option<usize>,
    pub value_content_start: Option<usize>,
    pub value_content_end: Option<usize>,
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
    interpolation_open: String,
    interpolation_close: String,
}

impl<'a> HtmlTokenizer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            cursor: 0,
            interpolation_open: "{{".into(),
            interpolation_close: "}}".into(),
        }
    }

    pub fn with_interpolation_delimiters(
        mut self,
        open: impl Into<String>,
        close: impl Into<String>,
    ) -> Self {
        self.interpolation_open = open.into();
        self.interpolation_close = close.into();
        self
    }

    pub fn set_interpolation_delimiters(
        &mut self,
        open: impl Into<String>,
        close: impl Into<String>,
    ) {
        self.interpolation_open = open.into();
        self.interpolation_close = close.into();
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
        if self
            .remaining()
            .to_ascii_uppercase()
            .starts_with("<!DOCTYPE")
        {
            return self.consume_block("<!DOCTYPE", ">", |body| {
                HtmlTokenKind::Doctype(body.trim().to_string())
            });
        }
        if self.remaining().starts_with("</") {
            return self.consume_end_tag();
        }
        if self.remaining().starts_with('<') && self.starts_valid_tag_at(self.cursor) {
            return self.consume_start_tag();
        }

        self.consume_text()
    }

    fn consume_text(&mut self) -> HtmlToken {
        let start = self.cursor;
        let end = self.next_text_end();
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
            if self.remaining().starts_with("</") {
                break;
            }
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

            let attr_start = self.cursor;
            let (attr_name, name_start, name_end) = self.consume_attr_name();
            if attr_name.is_empty() {
                if let Some(ch) = self.remaining().chars().next() {
                    self.cursor += ch.len_utf8();
                }
                continue;
            }
            self.consume_whitespace();
            let mut value_start = None;
            let mut value_end = None;
            let mut value_content_start = None;
            let mut value_content_end = None;
            let mut quote = None;
            let value = if self.remaining().starts_with('=') {
                self.cursor += 1;
                self.consume_whitespace();
                let consumed = self.consume_attr_value();
                value_start = Some(consumed.start);
                value_end = Some(consumed.end);
                value_content_start = Some(consumed.content_start);
                value_content_end = Some(consumed.content_end);
                quote = Some(consumed.quote);
                Some(consumed.value)
            } else {
                None
            };
            let attr_end = value_end.unwrap_or(name_end);
            attributes.push(HtmlAttribute {
                name: attr_name,
                value,
                quote,
                start: attr_start,
                end: attr_end,
                name_start,
                name_end,
                value_start,
                value_end,
                value_content_start,
                value_content_end,
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
        let body_end = self
            .remaining()
            .find(close)
            .map(|offset| self.cursor + offset)
            .unwrap_or(self.source.len());
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

    fn consume_attr_name(&mut self) -> (String, usize, usize) {
        let start = self.cursor;
        while let Some(ch) = self.remaining().chars().next() {
            if ch.is_whitespace() || matches!(ch, '=' | '>' | '/') {
                break;
            }
            self.cursor += ch.len_utf8();
        }
        (
            self.source[start..self.cursor].to_string(),
            start,
            self.cursor,
        )
    }

    fn consume_attr_value(&mut self) -> ConsumedAttrValue {
        match self.remaining().chars().next() {
            Some('"') | Some('\'') => {
                let quote = self.remaining().chars().next().unwrap();
                let start = self.cursor;
                self.cursor += quote.len_utf8();
                let value_start = self.cursor;
                while let Some(ch) = self.remaining().chars().next() {
                    if ch == quote {
                        let value = self.source[value_start..self.cursor].to_string();
                        self.cursor += quote.len_utf8();
                        return ConsumedAttrValue {
                            value,
                            quote: if quote == '"' {
                                HtmlQuoteKind::Double
                            } else {
                                HtmlQuoteKind::Single
                            },
                            start,
                            end: self.cursor,
                            content_start: value_start,
                            content_end: self.cursor - quote.len_utf8(),
                        };
                    }
                    self.cursor += ch.len_utf8();
                }
                ConsumedAttrValue {
                    value: self.source[value_start..self.cursor].to_string(),
                    quote: if quote == '"' {
                        HtmlQuoteKind::Double
                    } else {
                        HtmlQuoteKind::Single
                    },
                    start,
                    end: self.cursor,
                    content_start: value_start,
                    content_end: self.cursor,
                }
            }
            _ => {
                let start = self.cursor;
                while let Some(ch) = self.remaining().chars().next() {
                    if ch.is_whitespace() || ch == '>' {
                        break;
                    }
                    self.cursor += ch.len_utf8();
                }
                ConsumedAttrValue {
                    value: self.source[start..self.cursor].to_string(),
                    quote: HtmlQuoteKind::Unquoted,
                    start,
                    end: self.cursor,
                    content_start: start,
                    content_end: self.cursor,
                }
            }
        }
    }

    fn next_text_end(&self) -> usize {
        let mut cursor = self.cursor;
        let mut interpolation_close_at = None;
        while cursor < self.source.len() {
            if let Some(close_at) = interpolation_close_at {
                if self.source[cursor..].starts_with(&self.interpolation_close) {
                    cursor += self.interpolation_close.len();
                    interpolation_close_at = None;
                    continue;
                }
                if cursor >= close_at {
                    interpolation_close_at = None;
                    continue;
                }
            } else if !self.interpolation_open.is_empty()
                && self.source[cursor..].starts_with(&self.interpolation_open)
            {
                let close_at = self.source[cursor + self.interpolation_open.len()..]
                    .find(&self.interpolation_close)
                    .map(|offset| cursor + self.interpolation_open.len() + offset);
                if let Some(close_at) = close_at {
                    interpolation_close_at = Some(close_at);
                    cursor += self.interpolation_open.len();
                    continue;
                }
                return self.source.len();
            } else if self.source[cursor..].starts_with('<') && self.starts_valid_tag_at(cursor) {
                return cursor;
            }

            let Some(ch) = self.source[cursor..].chars().next() else {
                break;
            };
            cursor += ch.len_utf8();
        }
        self.source.len()
    }

    fn starts_valid_tag_at(&self, offset: usize) -> bool {
        let rest = &self.source[offset..];
        if rest.starts_with("<!--")
            || rest.starts_with("<![CDATA[")
            || rest.to_ascii_uppercase().starts_with("<!DOCTYPE")
        {
            return true;
        }
        if let Some(after_slash) = rest.strip_prefix("</") {
            return after_slash
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic());
        }
        if let Some(after_lt) = rest.strip_prefix('<') {
            return after_lt
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic());
        }
        false
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConsumedAttrValue {
    value: String,
    quote: HtmlQuoteKind,
    start: usize,
    end: usize,
    content_start: usize,
    content_end: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_basic_html() {
        let tokens = HtmlTokenizer::new("<!--x--><div id=\"a\">hi</div>").tokenize();
        assert!(matches!(tokens[0].kind, HtmlTokenKind::Comment(ref s) if s == "x"));
        assert!(
            matches!(tokens[1].kind, HtmlTokenKind::StartTag { ref name, .. } if name == "div")
        );
        assert!(matches!(tokens[2].kind, HtmlTokenKind::Text(ref s) if s == "hi"));
        assert!(matches!(tokens[3].kind, HtmlTokenKind::EndTag { ref name } if name == "div"));
    }

    #[test]
    fn tokenizes_doctype_and_cdata() {
        let tokens = HtmlTokenizer::new("<!DOCTYPE html><![CDATA[x]]>").tokenize();
        assert!(matches!(tokens[0].kind, HtmlTokenKind::Doctype(ref s) if s == "html"));
        assert!(matches!(tokens[1].kind, HtmlTokenKind::Cdata(ref s) if s == "x"));
    }

    #[test]
    fn tokenizes_vue_directive_attribute_names() {
        let tokens =
            HtmlTokenizer::new(r#"<button @click.stop="save" #[item]="slot" :[id].prop="x">"#)
                .tokenize();
        let HtmlTokenKind::StartTag { attributes, .. } = &tokens[0].kind else {
            panic!("expected start tag");
        };
        assert_eq!(attributes[0].name, "@click.stop");
        assert_eq!(attributes[1].name, "#[item]");
        assert_eq!(attributes[2].name, ":[id].prop");
    }

    #[test]
    fn records_attribute_spans_and_quotes() {
        let tokens = HtmlTokenizer::new(r#"<div id=a class="c" inert style=''>"#).tokenize();
        let HtmlTokenKind::StartTag { attributes, .. } = &tokens[0].kind else {
            panic!("expected start tag");
        };
        assert_eq!(attributes[0].start, 5);
        assert_eq!(attributes[0].end, 9);
        assert_eq!(attributes[0].value_start, Some(8));
        assert_eq!(attributes[0].value_end, Some(9));
        assert_eq!(attributes[0].quote, Some(HtmlQuoteKind::Unquoted));
        assert_eq!(attributes[1].value_start, Some(16));
        assert_eq!(attributes[1].value_end, Some(19));
        assert_eq!(attributes[1].quote, Some(HtmlQuoteKind::Double));
        assert_eq!(attributes[3].value_start, Some(32));
        assert_eq!(attributes[3].value_end, Some(34));
        assert_eq!(attributes[3].quote, Some(HtmlQuoteKind::Single));
    }

    #[test]
    fn keeps_invalid_lt_and_lt_inside_interpolation_in_text() {
        let tokens = HtmlTokenizer::new("a < b {{ c<d }} <span>").tokenize();
        assert!(matches!(tokens[0].kind, HtmlTokenKind::Text(ref s) if s == "a < b {{ c<d }} "));
        assert!(
            matches!(tokens[1].kind, HtmlTokenKind::StartTag { ref name, .. } if name == "span")
        );
    }

    #[test]
    fn treats_slash_as_unquoted_attribute_value_until_whitespace_or_gt() {
        let tokens = HtmlTokenizer::new("<div id=a/></div>").tokenize();
        let HtmlTokenKind::StartTag {
            attributes,
            self_closing,
            ..
        } = &tokens[0].kind
        else {
            panic!("expected start tag");
        };
        assert!(!self_closing);
        assert_eq!(attributes[0].value.as_deref(), Some("a/"));
    }

    #[test]
    fn terminates_start_tag_before_end_tag_for_ide_recovery() {
        let tokens =
            HtmlTokenizer::new("<template><Hello\n</template><script>x</script>").tokenize();
        assert!(
            matches!(tokens[0].kind, HtmlTokenKind::StartTag { ref name, .. } if name == "template")
        );
        assert!(
            matches!(tokens[1].kind, HtmlTokenKind::StartTag { ref name, .. } if name == "Hello")
        );
        assert_eq!(tokens[1].end, 17);
        assert!(matches!(tokens[2].kind, HtmlTokenKind::EndTag { ref name } if name == "template"));
        assert!(
            matches!(tokens[3].kind, HtmlTokenKind::StartTag { ref name, .. } if name == "script")
        );
    }
}
