#[derive(Clone, Debug, Default)]
pub(crate) struct Scope {
    pub(crate) locals: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Identifier,
    Keyword,
    OpenParen,
    Comma,
    Dot,
    Other,
}

pub(crate) fn is_local(scopes: &[Scope], ident: &str) -> bool {
    scopes
        .iter()
        .rev()
        .any(|scope| scope.locals.iter().any(|local| local == ident))
}

pub(crate) fn next_non_ws(source: &str, offset: usize) -> Option<char> {
    source.get(offset..)?.chars().find(|ch| !ch.is_whitespace())
}

pub(crate) fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

pub(crate) fn is_identifier_continue(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

pub(crate) fn is_keyword(value: &str) -> bool {
    matches!(
        value,
        "async"
            | "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "export"
            | "extends"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "let"
            | "new"
            | "of"
            | "return"
            | "super"
            | "switch"
            | "throw"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

pub(crate) fn is_global_or_literal(value: &str) -> bool {
    matches!(
        value,
        "true"
            | "false"
            | "null"
            | "undefined"
            | "this"
            | "Infinity"
            | "NaN"
            | "Math"
            | "Number"
            | "Date"
            | "Array"
            | "Object"
            | "Boolean"
            | "String"
            | "RegExp"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "JSON"
            | "Intl"
            | "BigInt"
            | "console"
            | "Error"
            | "TypeError"
            | "Symbol"
            | "Promise"
            | "Reflect"
            | "globalThis"
    )
}

pub(crate) fn is_generated_asset_import_ident(value: &str) -> bool {
    value
        .strip_prefix("_imports_")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
}

pub(crate) fn rewrite_identifier(ident: &str, options: &Vue3CompilerOptions) -> String {
    match options.binding_metadata.get(ident).map(String::as_str) {
        Some("setup-ref") if options.inline => format!("{ident}.value"),
        Some("setup-maybe-ref") if options.inline => format!("_unref({ident})"),
        Some("setup-let") if options.inline => format!("_unref({ident})"),
        Some("setup-const" | "literal-const" | "setup-reactive-const") if options.inline => {
            ident.to_string()
        }
        Some("props") if options.inline => format!("__props.{ident}"),
        Some("props-aliased") if options.inline => {
            let source = options
                .props_aliases
                .get(ident)
                .map_or(ident, String::as_str);
            render_props_access("__props", source)
        }
        Some("props-aliased") => {
            let source = options
                .props_aliases
                .get(ident)
                .map_or(ident, String::as_str);
            render_props_access("$props", source)
        }
        Some("data" | "options") if options.inline => format!("_ctx.{ident}"),
        Some(kind) if kind.starts_with("setup") || kind == "literal-const" => {
            format!("$setup.{ident}")
        }
        Some(kind) => format!("${kind}.{ident}"),
        None => format!("_ctx.{ident}"),
    }
}

pub(crate) fn render_props_access(base: &str, key: &str) -> String {
    if is_simple_identifier_ascii(key) {
        format!("{base}.{key}")
    } else {
        format!("{base}[{}]", quote_string(key))
    }
}
