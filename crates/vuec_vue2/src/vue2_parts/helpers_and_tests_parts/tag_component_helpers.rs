fn trim_ending_whitespace(element: &mut Vue2Element) {
    while matches!(element.children.last(), Some(Vue2Node::Text(text)) if text.text == " ") {
        element.children.pop();
    }
}

fn is_text_tag(tag: &str) -> bool {
    matches!(tag, "script" | "style" | "textarea")
}

fn is_raw_text_tag(tag: &str) -> bool {
    matches!(tag, "script" | "style")
}

fn is_forbidden_tag(element: &Vue2Element) -> bool {
    element.tag == "style"
        || (element.tag == "script"
            && element
                .attrs_map
                .get("type")
                .map_or(true, |value| value == "text/javascript"))
}

fn is_unary_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "frame"
            | "hr"
            | "img"
            | "input"
            | "isindex"
            | "keygen"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn is_reserved_tag(tag: &str) -> bool {
    if is_vue2_svg_tag(tag) {
        return true;
    }
    matches!(
        tag,
        "html"
            | "body"
            | "base"
            | "head"
            | "link"
            | "meta"
            | "style"
            | "title"
            | "address"
            | "article"
            | "aside"
            | "footer"
            | "header"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "nav"
            | "section"
            | "div"
            | "dd"
            | "dl"
            | "dt"
            | "figcaption"
            | "figure"
            | "picture"
            | "hr"
            | "img"
            | "li"
            | "main"
            | "ol"
            | "p"
            | "pre"
            | "ul"
            | "a"
            | "b"
            | "abbr"
            | "bdi"
            | "bdo"
            | "br"
            | "cite"
            | "code"
            | "data"
            | "dfn"
            | "em"
            | "i"
            | "kbd"
            | "mark"
            | "q"
            | "rp"
            | "rt"
            | "rtc"
            | "ruby"
            | "s"
            | "samp"
            | "small"
            | "span"
            | "strong"
            | "sub"
            | "sup"
            | "time"
            | "u"
            | "var"
            | "wbr"
            | "area"
            | "audio"
            | "map"
            | "track"
            | "video"
            | "embed"
            | "object"
            | "param"
            | "source"
            | "canvas"
            | "script"
            | "noscript"
            | "del"
            | "ins"
            | "caption"
            | "col"
            | "colgroup"
            | "table"
            | "thead"
            | "tbody"
            | "td"
            | "th"
            | "tr"
            | "button"
            | "datalist"
            | "fieldset"
            | "form"
            | "input"
            | "label"
            | "legend"
            | "meter"
            | "optgroup"
            | "option"
            | "output"
            | "progress"
            | "select"
            | "textarea"
            | "details"
            | "dialog"
            | "menu"
            | "menuitem"
            | "summary"
            | "content"
            | "element"
            | "shadow"
            | "template"
            | "blockquote"
            | "iframe"
            | "tfoot"
    )
}

fn is_vue2_svg_tag(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "svg"
            | "animate"
            | "circle"
            | "clippath"
            | "cursor"
            | "defs"
            | "desc"
            | "ellipse"
            | "filter"
            | "font-face"
            | "foreignobject"
            | "g"
            | "glyph"
            | "image"
            | "line"
            | "marker"
            | "mask"
            | "missing-glyph"
            | "path"
            | "pattern"
            | "polygon"
            | "polyline"
            | "rect"
            | "switch"
            | "symbol"
            | "text"
            | "textpath"
            | "tspan"
            | "use"
            | "view"
    )
}

fn is_built_in_tag(tag: &str) -> bool {
    matches!(tag, "slot" | "component")
}

fn namespace_for_tag(tag: &str, options: &Vue2CompileOptions) -> Option<String> {
    if let Some(namespace) = options.tag_namespaces.get(tag) {
        return Some(namespace.clone());
    }
    (options.use_default_tag_namespaces && is_vue2_svg_tag(tag)).then(|| "svg".into())
}

fn is_reserved_tag_with_options(tag: &str, options: &Vue2CompileOptions) -> bool {
    if let Some(tags) = options.reserved_tags.as_ref() {
        return tags.iter().any(|candidate| candidate == tag);
    }
    options.use_default_reserved_tags && is_reserved_tag(tag)
}

fn is_component(element: &Vue2Element, options: &Vue2CompileOptions) -> bool {
    element.component.is_some() || !is_reserved_tag_with_options(&element.tag, options)
}

fn check_binding_type(bindings: &BTreeMap<String, String>, key: &str) -> Option<String> {
    let camel_name = camelize(key);
    let pascal_name = capitalize(&camel_name);
    let candidates = [key, camel_name.as_str(), pascal_name.as_str()];
    for binding_type in ["setup-const", "setup-reactive-const"] {
        if let Some(name) = check_binding_type_candidates(bindings, &candidates, binding_type) {
            return Some(name);
        }
    }
    for binding_type in ["setup-let", "setup-ref", "setup-maybe-ref"] {
        if let Some(name) = check_binding_type_candidates(bindings, &candidates, binding_type) {
            return Some(name);
        }
    }
    None
}

fn check_binding_type_candidates(
    bindings: &BTreeMap<String, String>,
    candidates: &[&str],
    binding_type: &str,
) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        (bindings.get(*candidate).map(String::as_str) == Some(binding_type))
            .then(|| (*candidate).to_string())
    })
}
