//! HTML tokenization primitives shared by Vue parser frontends.
//!
//! The tokenizer keeps byte offsets for tags and attributes so higher layers can
//! build deterministic public AST locations without re-scanning the source.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// HTML integration namespace for parsed elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlNamespace {
    /// The normal HTML namespace.
    Html,
    /// The SVG namespace.
    Svg,
    /// The MathML namespace.
    MathMl,
}

/// HTML text parsing mode for element children.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlTextMode {
    /// Normal data mode where markup and interpolation are recognized.
    Data,
    /// RCDATA mode, used by tags such as `textarea` and `title`.
    RcData,
    /// Raw text mode, used by tags such as `script` and `style`.
    RawText,
}

/// HTML entity decoding context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlEntityDecodeMode {
    /// Text-node entity decoding.
    Text,
    /// Attribute-value entity decoding.
    Attribute,
}

/// The quote style used by an HTML attribute value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlQuoteKind {
    /// A double-quoted value such as `id="app"`.
    Double,
    /// A single-quoted value such as `id='app'`.
    Single,
    /// An unquoted value such as `id=app`.
    Unquoted,
}

/// A tokenized HTML attribute with source offsets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HtmlAttribute {
    /// The raw attribute name as it appears in source.
    pub name: String,
    /// The decoded attribute value text, if the attribute has an assignment.
    pub value: Option<String>,
    /// The quote kind for assigned values.
    pub quote: Option<HtmlQuoteKind>,
    /// Byte offset where the attribute starts.
    pub start: usize,
    /// Byte offset where the attribute ends.
    pub end: usize,
    /// Byte offset where the attribute name starts.
    pub name_start: usize,
    /// Byte offset where the attribute name ends.
    pub name_end: usize,
    /// Byte offset where the full value token starts, including quotes.
    pub value_start: Option<usize>,
    /// Byte offset where the full value token ends, including quotes.
    pub value_end: Option<usize>,
    /// Byte offset where the value content starts, excluding quotes.
    pub value_content_start: Option<usize>,
    /// Byte offset where the value content ends, excluding quotes.
    pub value_content_end: Option<usize>,
}

/// A token emitted by [`HtmlTokenizer`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlTokenKind {
    /// Plain text content.
    Text(String),
    /// An HTML comment body without `<!--` and `-->`.
    Comment(String),
    /// A CDATA body without the CDATA delimiters.
    Cdata(String),
    /// A doctype declaration body.
    Doctype(String),
    /// A recoverable bogus `<?...>` tag.
    BogusQuestionTag,
    /// An opening tag.
    StartTag {
        /// The lower-layer raw tag name.
        name: String,
        /// Attributes parsed from the tag.
        attributes: Vec<HtmlAttribute>,
        /// Whether the tag ended with `/>`.
        self_closing: bool,
    },
    /// A closing tag.
    EndTag {
        /// The closing tag name, or an empty string for invalid recovery tags.
        name: String,
    },
    /// End-of-file marker.
    Eof,
}

/// A token plus its byte span in the source template.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HtmlToken {
    /// The token payload.
    pub kind: HtmlTokenKind,
    /// Byte offset where the token starts.
    pub start: usize,
    /// Byte offset where the token ends.
    pub end: usize,
}

/// Decodes HTML entities in text-node context.
pub fn decode_html_text_entities(text: &str) -> String {
    decode_html_entities(text, HtmlEntityDecodeMode::Text)
}

/// Decodes HTML entities in attribute-value context.
pub fn decode_html_attr_entities(text: &str) -> String {
    decode_html_entities(text, HtmlEntityDecodeMode::Attribute)
}

/// Decodes HTML entities in the requested context.
pub fn decode_html_entities(text: &str, mode: HtmlEntityDecodeMode) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while cursor < text.len() {
        let Some(offset) = text[cursor..].find('&') else {
            output.push_str(&text[cursor..]);
            break;
        };
        let amp = cursor + offset;
        output.push_str(&text[cursor..amp]);
        if let Some((decoded, consumed)) = decode_html_entity_at(&text[amp..], mode) {
            output.push(decoded);
            cursor = amp + consumed;
        } else {
            output.push('&');
            cursor = amp + 1;
        }
    }
    output
}

/// Returns whether `tag` is a void / unary HTML element.
pub fn is_html_void_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

/// Returns whether `tag` can be implicitly closed by following content.
pub fn can_be_left_open_tag(tag: &str) -> bool {
    matches!(
        tag,
        "colgroup"
            | "dd"
            | "dt"
            | "li"
            | "options"
            | "p"
            | "td"
            | "tfoot"
            | "th"
            | "thead"
            | "tr"
            | "source"
    )
}

/// Returns the raw-text mode used by an HTML tag in the given namespace.
pub fn raw_text_mode_for_tag(
    tag: &str,
    namespace: HtmlNamespace,
    preserve_raw_text: bool,
) -> HtmlTextMode {
    if preserve_raw_text || namespace != HtmlNamespace::Html {
        return HtmlTextMode::Data;
    }
    match tag {
        "textarea" | "title" => HtmlTextMode::RcData,
        "script" | "style" => HtmlTextMode::RawText,
        _ => HtmlTextMode::Data,
    }
}

/// Finds the matching raw-text end tag and returns `(text_end, end_tag_end)`.
pub fn find_matching_raw_text_end(
    source: &str,
    content_start: usize,
    tag: &str,
) -> Option<(usize, usize)> {
    let mut cursor = content_start;
    while cursor < source.len() {
        let offset = source.get(cursor..)?.find("</")?;
        let candidate = cursor + offset;
        if let Some(end_tag_end) = matching_raw_text_end_tag_end(source, candidate, tag) {
            return Some((candidate, end_tag_end));
        }
        cursor = candidate + "</".len();
    }
    None
}

/// Resolves an element namespace using HTML integration point rules.
pub fn resolve_html_namespace(
    tag: &str,
    parent_namespace: HtmlNamespace,
    parent_tag: Option<&str>,
    parent_has_annotation_xml_html_encoding: bool,
    dom_namespaces: bool,
) -> HtmlNamespace {
    if !dom_namespaces {
        return parent_namespace;
    }
    let mut namespace = parent_namespace;
    if let Some(parent_tag) = parent_tag {
        if namespace == HtmlNamespace::MathMl {
            if parent_tag == "annotation-xml" {
                if tag == "svg" {
                    return HtmlNamespace::Svg;
                }
                if parent_has_annotation_xml_html_encoding {
                    namespace = HtmlNamespace::Html;
                }
            } else if mathml_text_integration_point(parent_tag)
                && tag != "mglyph"
                && tag != "malignmark"
            {
                namespace = HtmlNamespace::Html;
            }
        } else if namespace == HtmlNamespace::Svg
            && matches!(parent_tag, "foreignObject" | "desc" | "title")
        {
            namespace = HtmlNamespace::Html;
        }
    }
    if namespace == HtmlNamespace::Html {
        if tag == "svg" {
            return HtmlNamespace::Svg;
        }
        if tag == "math" {
            return HtmlNamespace::MathMl;
        }
    }
    namespace
}

/// Returns whether a MathML tag is a text integration point.
pub fn mathml_text_integration_point(tag: &str) -> bool {
    matches!(tag, "mi" | "mo" | "mn" | "ms" | "mtext")
}

/// Streaming HTML tokenizer used by Vue template parsers.
#[derive(Clone, Debug)]
pub struct HtmlTokenizer<'a> {
    source: &'a str,
    cursor: usize,
    interpolation_open: String,
    interpolation_close: String,
}

impl<'a> HtmlTokenizer<'a> {
    /// Creates a tokenizer for `source` with Vue's default `{{` / `}}`
    /// interpolation delimiters.
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            cursor: 0,
            interpolation_open: "{{".into(),
            interpolation_close: "}}".into(),
        }
    }

    /// Returns a tokenizer using custom interpolation delimiters.
    pub fn with_interpolation_delimiters(
        mut self,
        open: impl Into<String>,
        close: impl Into<String>,
    ) -> Self {
        self.interpolation_open = open.into();
        self.interpolation_close = close.into();
        self
    }

    /// Updates the interpolation delimiters used when finding text boundaries.
    pub fn set_interpolation_delimiters(
        &mut self,
        open: impl Into<String>,
        close: impl Into<String>,
    ) {
        self.interpolation_open = open.into();
        self.interpolation_close = close.into();
    }

    /// Moves the tokenizer cursor to `cursor`, clamped to the source length and
    /// the nearest previous UTF-8 character boundary.
    pub fn set_cursor(&mut self, cursor: usize) {
        let mut cursor = cursor.min(self.source.len());
        while cursor > 0 && !self.source.is_char_boundary(cursor) {
            cursor -= 1;
        }
        self.cursor = cursor;
    }

    /// Returns the current byte cursor.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Tokenizes the full remaining source, including the final EOF token.
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

    /// Reads and returns the next token from the current cursor.
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
        if self.starts_bogus_question_tag_at(self.cursor) {
            return self.consume_bogus_question_tag();
        }
        if self.remaining().starts_with("</") && self.starts_invalid_space_end_tag_at(self.cursor) {
            return self.consume_invalid_space_end_tag();
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

    fn consume_invalid_space_end_tag(&mut self) -> HtmlToken {
        let start = self.cursor;
        self.cursor += 2;
        self.consume_whitespace();
        self.consume_until('>');
        HtmlToken {
            kind: HtmlTokenKind::EndTag {
                name: String::new(),
            },
            start,
            end: self.cursor,
        }
    }

    fn consume_bogus_question_tag(&mut self) -> HtmlToken {
        let start = self.cursor;
        self.cursor += 2;
        self.consume_until('>');
        HtmlToken {
            kind: HtmlTokenKind::BogusQuestionTag,
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
        if self.remaining().starts_with('=') {
            self.cursor += 1;
            while let Some(ch) = self.remaining().chars().next() {
                if ch.is_whitespace() || matches!(ch, '=' | '>' | '/') {
                    break;
                }
                self.cursor += ch.len_utf8();
            }
            return (
                self.source[start..self.cursor].to_string(),
                start,
                self.cursor,
            );
        }
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
            } else if self.source[cursor..].starts_with('<')
                && self.starts_markup_boundary_at(cursor)
            {
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

    fn starts_markup_boundary_at(&self, offset: usize) -> bool {
        self.starts_valid_tag_at(offset)
            || self.starts_empty_end_tag_at(offset)
            || self.starts_invalid_space_end_tag_at(offset)
            || self.starts_bogus_question_tag_at(offset)
    }

    fn starts_invalid_space_end_tag_at(&self, offset: usize) -> bool {
        self.source
            .get(offset..)
            .and_then(|rest| rest.strip_prefix("</"))
            .is_some_and(|after_slash| after_slash.chars().next().is_some_and(char::is_whitespace))
    }

    fn starts_empty_end_tag_at(&self, offset: usize) -> bool {
        self.source
            .get(offset..)
            .and_then(|rest| rest.strip_prefix("</"))
            .is_some_and(|after_slash| after_slash.starts_with('>'))
    }

    fn starts_bogus_question_tag_at(&self, offset: usize) -> bool {
        self.source
            .get(offset..)
            .and_then(|rest| rest.strip_prefix("<?"))
            .is_some_and(|after_question| after_question.contains('>'))
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

fn decode_html_entity_at(value: &str, mode: HtmlEntityDecodeMode) -> Option<(char, usize)> {
    if let Some(decoded) = decode_numeric_html_entity_at(value) {
        return Some(decoded);
    }
    const NAMED: [(&str, char); 28] = [
        ("amp", '&'),
        ("lt", '<'),
        ("gt", '>'),
        ("nbsp", '\u{00a0}'),
        ("apos", '\''),
        ("quot", '"'),
        ("plus", '+'),
        ("times", '\u{00d7}'),
        ("copy", '\u{00a9}'),
        ("reg", '\u{00ae}'),
        ("trade", '\u{2122}'),
        ("ndash", '\u{2013}'),
        ("mdash", '\u{2014}'),
        ("lsquo", '\u{2018}'),
        ("rsquo", '\u{2019}'),
        ("ldquo", '\u{201c}'),
        ("rdquo", '\u{201d}'),
        ("hellip", '\u{2026}'),
        ("bull", '\u{2022}'),
        ("laquo", '\u{00ab}'),
        ("raquo", '\u{00bb}'),
        ("lsaquo", '\u{2039}'),
        ("rsaquo", '\u{203a}'),
        ("larr", '\u{2190}'),
        ("uarr", '\u{2191}'),
        ("rarr", '\u{2192}'),
        ("darr", '\u{2193}'),
        ("Eacute", '\u{00c9}'),
    ];
    for (name, decoded) in NAMED {
        let prefix = format!("&{name}");
        if !value.starts_with(&prefix) {
            continue;
        }
        let after_name = prefix.len();
        if value.as_bytes().get(after_name) == Some(&b';') {
            return Some((decoded, after_name + 1));
        }
        if matches!(mode, HtmlEntityDecodeMode::Text) && matches!(name, "amp" | "lt" | "gt") {
            return Some((decoded, after_name));
        }
        if matches!(mode, HtmlEntityDecodeMode::Attribute)
            && name == "amp"
            && value
                .as_bytes()
                .get(after_name)
                .is_some_and(|byte| !byte.is_ascii_alphanumeric() && *byte != b'=')
        {
            return Some((decoded, after_name));
        }
    }
    None
}

fn decode_numeric_html_entity_at(value: &str) -> Option<(char, usize)> {
    let rest = value.strip_prefix("&#")?;
    let (radix, digits_start) = match rest.as_bytes().first().copied() {
        Some(b'x' | b'X') => (16, "&#x".len()),
        _ => (10, "&#".len()),
    };
    let mut digits_end = digits_start;
    while let Some(byte) = value.as_bytes().get(digits_end).copied() {
        let is_digit = if radix == 16 {
            byte.is_ascii_hexdigit()
        } else {
            byte.is_ascii_digit()
        };
        if !is_digit {
            break;
        }
        digits_end += 1;
    }
    if digits_end == digits_start {
        return None;
    }
    let raw = u32::from_str_radix(&value[digits_start..digits_end], radix).ok()?;
    let consumed = digits_end + usize::from(value.as_bytes().get(digits_end) == Some(&b';'));
    Some((html_numeric_entity_char(raw), consumed))
}

fn html_numeric_entity_char(value: u32) -> char {
    match value {
        0x00 => '\u{fffd}',
        0x80 => '\u{20ac}',
        0x82 => '\u{201a}',
        0x83 => '\u{0192}',
        0x84 => '\u{201e}',
        0x85 => '\u{2026}',
        0x86 => '\u{2020}',
        0x87 => '\u{2021}',
        0x88 => '\u{02c6}',
        0x89 => '\u{2030}',
        0x8a => '\u{0160}',
        0x8b => '\u{2039}',
        0x8c => '\u{0152}',
        0x8e => '\u{017d}',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201c}',
        0x94 => '\u{201d}',
        0x95 => '\u{2022}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x98 => '\u{02dc}',
        0x99 => '\u{2122}',
        0x9a => '\u{0161}',
        0x9b => '\u{203a}',
        0x9c => '\u{0153}',
        0x9e => '\u{017e}',
        0x9f => '\u{0178}',
        value => char::from_u32(value).unwrap_or('\u{fffd}'),
    }
}

fn matching_raw_text_end_tag_end(source: &str, start: usize, tag: &str) -> Option<usize> {
    let after_slash = start.checked_add("</".len())?;
    let tag_end = after_slash.checked_add(tag.len())?;
    let raw_tag = source.get(after_slash..tag_end)?;
    if !raw_tag.eq_ignore_ascii_case(tag) {
        return None;
    }
    let mut cursor = tag_end;
    loop {
        let Some(ch) = source.get(cursor..).and_then(|rest| rest.chars().next()) else {
            return None;
        };
        if ch == '>' {
            return Some(cursor + ch.len_utf8());
        }
        if !ch.is_whitespace() {
            return None;
        }
        cursor += ch.len_utf8();
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
    fn keeps_equals_as_invalid_attribute_name_prefix() {
        let tokens = HtmlTokenizer::new("<div =foo=bar =>").tokenize();
        let HtmlTokenKind::StartTag { attributes, .. } = &tokens[0].kind else {
            panic!("expected start tag");
        };
        assert_eq!(attributes[0].name, "=foo");
        assert_eq!(attributes[0].name_start, 5);
        assert_eq!(attributes[0].name_end, 9);
        assert_eq!(attributes[0].value.as_deref(), Some("bar"));
        assert_eq!(attributes[1].name, "=");
        assert_eq!(attributes[1].start, 14);
        assert_eq!(attributes[1].end, 15);
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

    #[test]
    fn tokenizes_invalid_space_end_tag_as_markup_boundary() {
        let tokens = HtmlTokenizer::new("<template>a </ b</template>").tokenize();
        assert!(
            matches!(tokens[0].kind, HtmlTokenKind::StartTag { ref name, .. } if name == "template")
        );
        assert!(matches!(tokens[1].kind, HtmlTokenKind::Text(ref s) if s == "a "));
        assert!(matches!(tokens[2].kind, HtmlTokenKind::EndTag { ref name } if name.is_empty()));
        assert_eq!(tokens[2].end, "<template>a </ b</template>".len());
        assert!(matches!(tokens[3].kind, HtmlTokenKind::Eof));
    }

    #[test]
    fn set_cursor_clamps_to_utf8_boundary() {
        let mut tokenizer = HtmlTokenizer::new("aé<b>");
        tokenizer.set_cursor(2);

        assert_eq!(tokenizer.cursor(), 1);
        assert!(matches!(tokenizer.next_token().kind, HtmlTokenKind::Text(text) if text == "é"));
    }

    #[test]
    fn tokenizes_bogus_question_tag_as_markup_boundary() {
        let tokens = HtmlTokenizer::new("<template><?xml?></template>").tokenize();
        assert!(
            matches!(tokens[0].kind, HtmlTokenKind::StartTag { ref name, .. } if name == "template")
        );
        assert!(matches!(tokens[1].kind, HtmlTokenKind::BogusQuestionTag));
        assert!(matches!(tokens[2].kind, HtmlTokenKind::EndTag { ref name } if name == "template"));
    }

    #[test]
    fn decodes_text_and_attribute_entities_like_vue_modes() {
        assert_eq!(
            decode_html_text_entities("&ampersand;&Eacute;&#x80;&#0;"),
            "&ersand;É€�"
        );
        assert_eq!(
            decode_html_text_entities(
                "&larr;&uarr;&rarr;&darr;&mdash;&ndash;&copy;&reg;&trade;&lsaquo;&rsaquo;"
            ),
            "←↑→↓—–©®™‹›"
        );
        assert_eq!(
            decode_html_text_entities("&foo;&rarrx;&ampersand;"),
            "&foo;&rarrx;&ersand;"
        );
        assert_eq!(decode_html_attr_entities("&amp;&amp=&amp!"), "&&amp=&!");
        assert_eq!(decode_html_attr_entities("&lt;"), "<");
    }

    #[test]
    fn classifies_void_left_open_and_raw_text_tags() {
        assert!(is_html_void_tag("img"));
        assert!(is_html_void_tag("source"));
        assert!(can_be_left_open_tag("p"));
        assert!(can_be_left_open_tag("li"));
        assert_eq!(
            raw_text_mode_for_tag("textarea", HtmlNamespace::Html, false),
            HtmlTextMode::RcData
        );
        assert_eq!(
            raw_text_mode_for_tag("script", HtmlNamespace::Html, false),
            HtmlTextMode::RawText
        );
        assert_eq!(
            raw_text_mode_for_tag("script", HtmlNamespace::Svg, false),
            HtmlTextMode::Data
        );
    }

    #[test]
    fn finds_matching_raw_text_end_tag_case_insensitively() {
        assert_eq!(
            find_matching_raw_text_end("<textarea>a</TEXTAREA> tail", 10, "textarea"),
            Some((11, 22))
        );
        assert_eq!(
            find_matching_raw_text_end("<script>x</script type=x>", 8, "script"),
            None
        );
    }

    #[test]
    fn resolves_dom_namespace_integration_points() {
        assert_eq!(
            resolve_html_namespace("svg", HtmlNamespace::Html, None, false, true),
            HtmlNamespace::Svg
        );
        assert_eq!(
            resolve_html_namespace(
                "foreignObject",
                HtmlNamespace::Svg,
                Some("svg"),
                false,
                true
            ),
            HtmlNamespace::Svg
        );
        assert_eq!(
            resolve_html_namespace(
                "div",
                HtmlNamespace::Svg,
                Some("foreignObject"),
                false,
                true
            ),
            HtmlNamespace::Html
        );
        assert_eq!(
            resolve_html_namespace(
                "span",
                HtmlNamespace::MathMl,
                Some("annotation-xml"),
                true,
                true
            ),
            HtmlNamespace::Html
        );
        assert_eq!(
            resolve_html_namespace(
                "div",
                HtmlNamespace::Svg,
                Some("foreignObject"),
                false,
                false
            ),
            HtmlNamespace::Svg
        );
    }
}
