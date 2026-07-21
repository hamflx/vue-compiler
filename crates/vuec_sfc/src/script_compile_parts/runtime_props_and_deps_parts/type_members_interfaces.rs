pub(crate) fn vue3_type_members_from_literal(
    source: &str,
    literal: &TSTypeLiteral<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    let (members, errors) = vue3_runtime_props_from_signatures(source, &literal.members, analysis);
    Vue27TypeMembers {
        source: source
            .get(literal.span.start as usize..literal.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members,
        errors,
    }
}

pub(crate) fn vue3_type_members_from_mapped_type(
    source: &str,
    mapped: &TSMappedType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let constraint_keys = vue3_resolve_ordered_string_type_keys(&mapped.constraint, analysis)?;
    let mut scoped_analysis = analysis.clone();
    let key_param = mapped.key.name.to_string();
    scoped_analysis.string_literal_type_declarations.insert(
        key_param.clone(),
        constraint_keys.iter().cloned().collect::<BTreeSet<_>>(),
    );
    scoped_analysis
        .ordered_string_literal_type_declarations
        .insert(key_param, constraint_keys.clone());

    let keys = match mapped.name_type.as_ref() {
        Some(name_type) => vue3_resolve_ordered_string_type_keys(name_type, &scoped_analysis)?,
        None => constraint_keys,
    };
    let types = mapped
        .type_annotation
        .as_ref()
        .map(|ty| infer_vue3_runtime_type(ty, &scoped_analysis))
        .unwrap_or_else(|| vec!["null".into()]);
    let required = !matches!(
        mapped.optional,
        Some(TSMappedTypeModifierOperator::True | TSMappedTypeModifierOperator::Plus)
    );
    let type_annotation_source = mapped.type_annotation.as_ref().and_then(|ty| {
        source
            .get(ty.span().start as usize..ty.span().end as usize)
            .map(ToOwned::to_owned)
    });
    let member_source = source
        .get(mapped.span.start as usize..mapped.span.end as usize)
        .map(ToOwned::to_owned);

    Some(Vue27TypeMembers {
        source: source
            .get(mapped.span.start as usize..mapped.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: keys
            .into_iter()
            .map(|key| Vue27RuntimeProp {
                key,
                types: types.clone(),
                required,
                default: None,
                is_method: false,
                type_annotation_source: type_annotation_source.clone(),
                member_source: member_source.clone(),
            })
            .collect(),
        errors: Vec::new(),
    })
}

pub(crate) fn vue3_type_members_from_record_type(
    source: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let keys = vue3_resolve_ordered_string_type_keys(
        vue3_type_reference_type_argument(reference, 0)?,
        analysis,
    )?;
    let value = vue3_type_reference_type_argument(reference, 1)?;
    let types = infer_vue3_runtime_type(value, analysis);
    let span = reference.span();
    let type_annotation_source = source
        .get(value.span().start as usize..value.span().end as usize)
        .map(ToOwned::to_owned);
    let member_source = source
        .get(span.start as usize..span.end as usize)
        .map(ToOwned::to_owned);

    Some(Vue27TypeMembers {
        source: source
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: keys
            .into_iter()
            .map(|key| Vue27RuntimeProp {
                key,
                types: types.clone(),
                required: true,
                default: None,
                is_method: false,
                type_annotation_source: type_annotation_source.clone(),
                member_source: member_source.clone(),
            })
            .collect(),
        errors: Vec::new(),
    })
}

pub(crate) fn vue3_type_members_from_interface_body(
    source: &str,
    body: &TSInterfaceBody<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    let (members, errors) = vue3_runtime_props_from_signatures(source, &body.body, analysis);
    Vue27TypeMembers {
        source: source
            .get(body.span.start as usize..body.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members,
        errors,
    }
}

pub(crate) fn vue3_type_members_from_interface(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    let mut members = vue3_type_members_from_interface_body(source, &declaration.body, analysis);
    for heritage in &declaration.extends {
        if vue3_interface_heritage_has_vue_ignore(source, heritage) {
            continue;
        }
        let Some(base) = vue3_resolve_interface_heritage_props_type(source, heritage, analysis)
        else {
            members.errors.push(vue3_failed_extends_base_type_error());
            continue;
        };
        members.errors.extend(base.errors);
        for prop in base.members {
            if !members.members.iter().any(|member| member.key == prop.key) {
                members.members.push(prop);
            }
        }
    }
    members
}

pub(crate) fn infer_vue3_runtime_type_from_interface_declarations(
    declarations: &[&TSInterfaceDeclaration<'_>],
) -> Vec<String> {
    let mut types = Vec::new();
    for declaration in declarations {
        for signature in &declaration.body.body {
            let runtime_type = match signature {
                TSSignature::TSCallSignatureDeclaration(_)
                | TSSignature::TSConstructSignatureDeclaration(_) => "Function",
                _ => "Object",
            };
            push_unique(&mut types, runtime_type);
        }
    }
    if types.is_empty() {
        vec!["Object".into()]
    } else {
        types
    }
}

pub(crate) fn vue3_type_members_from_interface_declarations(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    let source_text = vue3_interface_declarations_source(source, declarations);
    let (members, errors) = vue3_merge_props_type_members(
        declarations
            .iter()
            .map(|declaration| vue3_type_members_from_interface(source, declaration, analysis)),
        false,
    );
    Vue27TypeMembers {
        source: source_text,
        members,
        errors,
    }
}

pub(crate) fn vue3_interface_declarations_source(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
) -> String {
    declarations
        .iter()
        .filter_map(|declaration| {
            source.get(declaration.body.span.start as usize..declaration.body.span.end as usize)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn vue3_resolve_interface_heritage_props_type(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let ty_source = vue3_interface_heritage_type_source(source, heritage)?;
    let wrapped = format!("type __VuecResolved = {ty_source}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::ts())
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    for statement in &parsed.program.body {
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return vue3_resolve_props_type(&wrapped, &declaration.type_annotation, analysis);
        }
    }
    None
}

pub(crate) fn vue3_emits_type_from_interface(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27EmitsType {
    let mut emits = vue3_emits_type_from_interface_body(source, &declaration.body, analysis);
    for heritage in &declaration.extends {
        if vue3_interface_heritage_has_vue_ignore(source, heritage) {
            continue;
        }
        let Some(base) = vue3_resolve_interface_heritage_emits_type(source, heritage, analysis)
        else {
            continue;
        };
        emits.syntax.has_call_signature |= base.syntax.has_call_signature;
        emits.syntax.has_property |= base.syntax.has_property;
        emits.call_count += base.call_count;
        for event in base.events {
            push_unique(&mut emits.events, &event);
        }
    }
    emits
}

pub(crate) fn vue3_emits_type_from_interface_declarations(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27EmitsType {
    let mut merged = Vue27EmitsType {
        source: vue3_interface_declarations_source(source, declarations),
        events: Vec::new(),
        syntax: Vue3EmitsTypeSyntax::default(),
        call_count: 0,
    };
    for declaration in declarations {
        let emits = vue3_emits_type_from_interface(source, declaration, analysis);
        merged.syntax.has_call_signature |= emits.syntax.has_call_signature;
        merged.syntax.has_property |= emits.syntax.has_property;
        merged.call_count += emits.call_count;
        for event in emits.events {
            push_unique(&mut merged.events, &event);
        }
    }
    merged
}

pub(crate) fn vue3_resolve_interface_heritage_emits_type(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27EmitsType> {
    let ty_source = vue3_interface_heritage_type_source(source, heritage)?;
    let wrapped = format!("type __VuecResolved = {ty_source}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::ts())
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    for statement in &parsed.program.body {
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return vue3_resolve_emits_type(&wrapped, &declaration.type_annotation, analysis);
        }
    }
    None
}

pub(crate) fn vue3_interface_heritage_type_source(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
) -> Option<String> {
    let start = heritage.expression.span().start as usize;
    let end = heritage
        .type_arguments
        .as_ref()
        .map(|arguments| arguments.span.end as usize)
        .unwrap_or(heritage.expression.span().end as usize);
    source.get(start..end).map(str::trim).map(ToOwned::to_owned)
}

pub(crate) fn vue3_interface_heritage_has_vue_ignore(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
) -> bool {
    if source
        .get(heritage.span.start as usize..heritage.expression.span().start as usize)
        .is_some_and(|prefix| prefix.contains("@vue-ignore"))
    {
        return true;
    }
    vue3_source_has_immediate_leading_vue_ignore_comment(
        source,
        heritage.expression.span().start as usize,
    )
}

pub(crate) fn vue3_type_annotation_has_vue_ignore(
    source: &str,
    annotation: &TSTypeAnnotation<'_>,
) -> bool {
    let type_start = annotation.type_annotation.span().start as usize;
    if source
        .get(annotation.span.start as usize..type_start)
        .is_some_and(|prefix| prefix.contains("@vue-ignore"))
    {
        return true;
    }
    vue3_source_has_immediate_leading_vue_ignore_comment(source, type_start)
}

pub(crate) fn vue3_source_has_immediate_leading_vue_ignore_comment(
    source: &str,
    offset: usize,
) -> bool {
    let mut cursor = offset.min(source.len());
    loop {
        cursor = trim_ascii_whitespace_before(source, cursor);
        if cursor == 0 {
            return false;
        }
        let before = &source[..cursor];
        if before.ends_with("*/") {
            let Some(start) = before.rfind("/*") else {
                return false;
            };
            let comment = &source[start..cursor];
            if comment.contains("@vue-ignore") {
                return true;
            }
            cursor = start;
            continue;
        }
        let line_start = before.rfind('\n').map(|index| index + 1).unwrap_or(0);
        if let Some(start) = before[line_start..].rfind("//") {
            let comment_start = line_start + start;
            let comment = &source[comment_start..cursor];
            if comment.contains("@vue-ignore") {
                return true;
            }
            cursor = comment_start;
            continue;
        }
        return false;
    }
}

pub(crate) fn trim_ascii_whitespace_before(source: &str, mut cursor: usize) -> usize {
    while cursor > 0 && source.as_bytes()[cursor - 1].is_ascii_whitespace() {
        cursor -= 1;
    }
    cursor
}

pub(crate) fn vue3_failed_extends_base_type_error() -> String {
    "Failed to resolve extends base type.\nIf this previously worked in 3.2, you can instruct the compiler to ignore this extend by adding /* @vue-ignore */ before it, for example:\n\ninterface Props extends /* @vue-ignore */ Base {}\n\nNote: both in 3.2 or with the ignore, the properties in the base type are treated as fallthrough attrs at runtime.".to_string()
}

pub(crate) fn vue3_interface_heritage_name(heritage: &TSInterfaceHeritage<'_>) -> Option<String> {
    vue3_expression_type_name_key(&heritage.expression)
}

pub(crate) fn vue3_expression_type_name_key(expression: &Expression<'_>) -> Option<String> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => {
            let left = vue3_expression_type_name_key(&member.object)?;
            Some(format!("{left}.{}", member.property.name))
        }
        Expression::ComputedMemberExpression(member) => {
            let left = vue3_expression_type_name_key(&member.object)?;
            let property = member.static_property_name()?;
            Some(format!("{left}.{property}"))
        }
        _ => None,
    }
}
