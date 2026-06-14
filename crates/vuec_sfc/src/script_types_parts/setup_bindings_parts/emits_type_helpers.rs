pub(crate) fn vue27_emits_type_from_function(
    source: &str,
    function: &TSFunctionType<'_>,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    collect_vue27_emits_from_parameters(&function.params.items, &mut events);
    Vue27EmitsType {
        source: source
            .get(function.span.start as usize..function.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax: Vue3EmitsTypeSyntax {
            has_call_signature: true,
            has_property: false,
        },
        call_count: 1,
    }
}

pub(crate) fn vue27_emits_type_from_literal(
    source: &str,
    literal: &TSTypeLiteral<'_>,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    let mut syntax = Vue3EmitsTypeSyntax::default();
    let mut call_count = 0usize;
    for member in &literal.members {
        if let TSSignature::TSCallSignatureDeclaration(signature) = member {
            syntax.has_call_signature = true;
            call_count += 1;
            collect_vue27_emits_from_parameters(&signature.params.items, &mut events);
        }
    }
    Vue27EmitsType {
        source: source
            .get(literal.span.start as usize..literal.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax,
        call_count,
    }
}

pub(crate) fn vue27_emits_type_from_interface_body(
    source: &str,
    body: &TSInterfaceBody<'_>,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    let mut syntax = Vue3EmitsTypeSyntax::default();
    let mut call_count = 0usize;
    for member in &body.body {
        if let TSSignature::TSCallSignatureDeclaration(signature) = member {
            syntax.has_call_signature = true;
            call_count += 1;
            collect_vue27_emits_from_parameters(&signature.params.items, &mut events);
        }
    }
    Vue27EmitsType {
        source: source
            .get(body.span.start as usize..body.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax,
        call_count,
    }
}

pub(crate) fn vue3_emits_type_from_function(
    source: &str,
    function: &TSFunctionType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    collect_vue3_emits_from_parameters(&function.params.items, &mut events, analysis);
    Vue27EmitsType {
        source: source
            .get(function.span.start as usize..function.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax: Vue3EmitsTypeSyntax {
            has_call_signature: true,
            has_property: false,
        },
        call_count: 1,
    }
}

pub(crate) fn vue3_emits_type_from_literal(
    source: &str,
    literal: &TSTypeLiteral<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    let mut syntax = Vue3EmitsTypeSyntax::default();
    let mut call_count = 0usize;
    for member in &literal.members {
        collect_vue3_emits_type_member(
            source,
            member,
            &mut events,
            &mut syntax,
            &mut call_count,
            analysis,
        );
    }
    Vue27EmitsType {
        source: source
            .get(literal.span.start as usize..literal.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax,
        call_count,
    }
}

pub(crate) fn vue3_emits_type_from_interface_body(
    source: &str,
    body: &TSInterfaceBody<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    let mut syntax = Vue3EmitsTypeSyntax::default();
    let mut call_count = 0usize;
    for member in &body.body {
        collect_vue3_emits_type_member(
            source,
            member,
            &mut events,
            &mut syntax,
            &mut call_count,
            analysis,
        );
    }
    Vue27EmitsType {
        source: source
            .get(body.span.start as usize..body.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax,
        call_count,
    }
}

pub(crate) fn collect_vue3_emits_type_member(
    _source: &str,
    member: &TSSignature<'_>,
    events: &mut Vec<String>,
    syntax: &mut Vue3EmitsTypeSyntax,
    call_count: &mut usize,
    analysis: &Vue3ScriptSetupAnalysis,
) {
    match member {
        TSSignature::TSCallSignatureDeclaration(signature) => {
            syntax.has_call_signature = true;
            *call_count += 1;
            collect_vue3_emits_from_parameters(&signature.params.items, events, analysis);
        }
        TSSignature::TSPropertySignature(property) if !property.computed => {
            if let Some(key) = vue27_property_key_static_name(&property.key) {
                syntax.has_property = true;
                push_unique(events, &key);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_emits_from_parameters(
    parameters: &[FormalParameter<'_>],
    names: &mut Vec<String>,
    analysis: &Vue3ScriptSetupAnalysis,
) {
    let Some(parameter) = parameters.first() else {
        return;
    };
    let Some(annotation) = parameter.type_annotation.as_ref() else {
        return;
    };
    collect_vue3_emits_from_type(&annotation.type_annotation, names, analysis);
}

pub(crate) fn collect_vue3_emits_from_type(
    ty: &TSType<'_>,
    names: &mut Vec<String>,
    analysis: &Vue3ScriptSetupAnalysis,
) {
    match ty {
        TSType::TSLiteralType(literal) => {
            if let Some(name) = vue3_literal_type_key(&literal.literal) {
                push_unique(names, &name);
            }
        }
        TSType::TSUnionType(union) => {
            for ty in &union.types {
                collect_vue3_emits_from_type(ty, names, analysis);
            }
        }
        TSType::TSParenthesizedType(parenthesized) => {
            collect_vue3_emits_from_type(&parenthesized.type_annotation, names, analysis);
        }
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
                if let Some(keys) = analysis.ordered_string_literal_type_declarations.get(&name) {
                    for key in keys {
                        push_unique(names, key);
                    }
                }
            }
        }
        TSType::TSImportType(import_type) => {
            if let Some(resolved) = vue3_resolve_import_type(import_type, analysis) {
                if let Some(keys) = resolved
                    .context
                    .ordered_string_literal_type_declarations
                    .get(&resolved.name)
                {
                    for key in keys {
                        push_unique(names, key);
                    }
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue27_emits_from_parameters(
    parameters: &[FormalParameter<'_>],
    names: &mut Vec<String>,
) {
    let Some(parameter) = parameters.first() else {
        return;
    };
    let Some(annotation) = parameter.type_annotation.as_ref() else {
        return;
    };
    collect_vue27_emits_from_type(&annotation.type_annotation, names);
}

pub(crate) fn collect_vue27_emits_from_type(ty: &TSType<'_>, names: &mut Vec<String>) {
    match ty {
        TSType::TSLiteralType(literal) => {
            if let Some(name) = vue27_literal_event_name(&literal.literal) {
                push_unique(names, &name);
            }
        }
        TSType::TSUnionType(union) => {
            for ty in &union.types {
                collect_vue27_emits_from_type(ty, names);
            }
        }
        _ => {}
    }
}

pub(crate) fn vue27_literal_event_name(literal: &TSLiteral<'_>) -> Option<String> {
    match literal {
        TSLiteral::StringLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::BooleanLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::NumericLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::BigIntLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

pub(crate) fn vue27_property_key_static_name(key: &PropertyKey<'_>) -> Option<String> {
    key.static_name().map(|name| name.into_owned())
}
