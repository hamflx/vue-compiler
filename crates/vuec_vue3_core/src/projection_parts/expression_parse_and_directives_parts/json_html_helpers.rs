pub(crate) fn camelize(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase_next = false;
    for ch in value.chars() {
        if uppercase_next {
            output.extend(ch.to_uppercase());
            uppercase_next = false;
        } else if ch == '-' {
            uppercase_next = true;
        } else {
            output.push(ch);
        }
    }
    output
}

pub(crate) fn setup_reference_name_for_tag(
    tag: &str,
    options: &Vue3CompilerOptions,
) -> Option<String> {
    setup_reference_name(tag, options)
}

pub(crate) fn setup_reference_name(name: &str, options: &Vue3CompilerOptions) -> Option<String> {
    let camel_name = camelize(name);
    let pascal_name = capitalize(&camel_name);
    [name.to_string(), camel_name, pascal_name]
        .into_iter()
        .find(|candidate| {
            options
            .binding_metadata
            .get(candidate)
            .is_some_and(|kind| {
                matches!(
                    kind.as_str(),
                    "setup-const"
                        | "setup-reactive-const"
                        | "literal-const"
                        | "setup-let"
                        | "setup-ref"
                        | "setup-maybe-ref"
                        | "props"
                )
            })
        })
}

pub(crate) fn to_handler_key(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!("on{}", capitalize(value))
    }
}

pub(crate) fn is_simple_identifier_ascii(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

pub(crate) fn json_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(crate) fn json_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub(crate) fn vue3_element_kind(
    tag: String,
    attributes: Vec<vuec_html::HtmlAttribute>,
    self_closing: bool,
    options: &Vue3CompilerOptions,
    file_id: FileId,
    base_offset: usize,
    in_v_pre: bool,
    namespace: vuec_ast::HtmlNamespace,
) -> Vue3NodeKind {
    let props = attributes
        .into_iter()
        .filter(|attr| !(in_v_pre && attr.name == "v-pre"))
        .map(|attr| {
            if in_v_pre {
                vue3_attribute_from_attr(attr, file_id, base_offset)
            } else {
                vue3_prop_from_attr(attr, file_id, base_offset)
            }
        })
        .collect::<Vec<_>>();
    let tag_type = if in_v_pre {
        Vue3ElementType::Element
    } else {
        vue3_tag_type(&tag, &props, options)
    };
    Vue3NodeKind::Element(Vue3Element {
        tag,
        tag_type,
        ns: namespace,
        props,
        self_closing,
        codegen_node: None,
        ssr_codegen_node: None,
    })
}

pub(crate) fn vue3_element_namespace(
    ast: &Vue3Ast,
    parent_id: vuec_ast::NodeId,
    tag: &str,
    parent: vuec_ast::HtmlNamespace,
    options: &Vue3CompilerOptions,
) -> vuec_ast::HtmlNamespace {
    if let Some(namespace) = options.namespaces.get(tag).copied() {
        return namespace;
    }
    let parent_element = ast.node(parent_id).and_then(|node| match &node.kind {
        Vue3AstKind::Element(element) => Some(element),
        _ => None,
    });
    let namespace = resolve_html_namespace(
        tag,
        html_namespace_to_html(parent),
        parent_element.map(|element| element.tag.as_str()),
        parent_element.is_some_and(|element| {
            vue3_element_has_attr_value(
                element,
                "encoding",
                &["text/html", "application/xhtml+xml"],
            )
        }),
        options.dom_namespaces,
    );
    html_namespace_from_html(namespace)
}

pub(crate) fn vue3_element_has_attr_value(
    element: &vuec_ast::Vue3Element,
    name: &str,
    values: &[&str],
) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Attribute(attr)
                if attr.name == name
                    && attr
                        .value
                        .as_deref()
                        .is_some_and(|value| values.contains(&value))
        )
    })
}

pub(crate) fn html_namespace_to_html(
    namespace: vuec_ast::HtmlNamespace,
) -> vuec_html::HtmlNamespace {
    match namespace {
        vuec_ast::HtmlNamespace::Html => vuec_html::HtmlNamespace::Html,
        vuec_ast::HtmlNamespace::Svg => vuec_html::HtmlNamespace::Svg,
        vuec_ast::HtmlNamespace::MathMl => vuec_html::HtmlNamespace::MathMl,
    }
}

pub(crate) fn html_namespace_from_html(
    namespace: vuec_html::HtmlNamespace,
) -> vuec_ast::HtmlNamespace {
    match namespace {
        vuec_html::HtmlNamespace::Html => vuec_ast::HtmlNamespace::Html,
        vuec_html::HtmlNamespace::Svg => vuec_ast::HtmlNamespace::Svg,
        vuec_html::HtmlNamespace::MathMl => vuec_ast::HtmlNamespace::MathMl,
    }
}

pub(crate) fn vue3_tag_type(
    tag: &str,
    props: &[Vue3Prop],
    options: &Vue3CompilerOptions,
) -> Vue3ElementType {
    if options
        .custom_elements
        .iter()
        .any(|candidate| candidate == tag)
    {
        return Vue3ElementType::Element;
    }
    if tag == "slot" {
        return Vue3ElementType::SlotOutlet;
    }
    if tag == "template" {
        return if props.iter().any(
            |prop| matches!(prop, Vue3Prop::Directive(dir) if is_template_directive(&dir.name)),
        ) {
            Vue3ElementType::Template
        } else {
            Vue3ElementType::Element
        };
    }
    if options
        .built_in_components
        .iter()
        .any(|candidate| candidate == tag)
    {
        return Vue3ElementType::Component;
    }
    if vue3_core_component_helper(tag).is_some() || matches!(tag, "component" | "Component") {
        return Vue3ElementType::Component;
    }
    if setup_reference_name_for_tag(tag, options).is_some() {
        return Vue3ElementType::Component;
    }
    if props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Attribute(attr)
                if attr.name == "is"
                    && attr
                        .value
                        .as_deref()
                        .is_some_and(|value| value.starts_with("vue:"))
        )
    }) {
        return Vue3ElementType::Component;
    }
    if options
        .native_tags
        .as_ref()
        .is_some_and(|native_tags| !native_tags.iter().any(|candidate| candidate == tag))
    {
        return Vue3ElementType::Component;
    }
    if tag.chars().next().is_some_and(|ch| ch.is_ascii_uppercase()) {
        return Vue3ElementType::Component;
    }
    Vue3ElementType::Element
}
