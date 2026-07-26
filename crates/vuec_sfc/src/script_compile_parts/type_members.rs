pub(crate) fn vue3_merged_type_members(
    source: &str,
    span: oxc_span::Span,
    members: Vec<Vue27RuntimeProp>,
    errors: Vec<String>,
) -> Option<Vue27TypeMembers> {
    if members.is_empty() && errors.is_empty() {
        None
    } else {
        Some(Vue27TypeMembers {
            source: source
                .get(span.start as usize..span.end as usize)
                .unwrap_or_default()
                .to_string(),
            members,
            errors,
            interface_heritage: None,
        })
    }
}

pub(crate) fn vue3_type_members_empty(
    source: &str,
    span: oxc_span::Span,
    errors: Vec<String>,
) -> Vue27TypeMembers {
    Vue27TypeMembers {
        source: source
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: Vec::new(),
        errors,
        interface_heritage: None,
    }
}

pub(crate) fn vue3_unresolvable_type_reference_error() -> String {
    "Unresolvable type reference or unsupported built-in utility type".to_string()
}

pub(crate) fn vue3_failed_import_source_error(source: &str) -> String {
    format!("Failed to resolve import source {source:?}.")
}

pub(crate) fn vue3_unsupported_computed_key_error() -> String {
    "Unsupported computed key in type referenced by a macro".to_string()
}

pub(crate) fn vue3_unsupported_index_type_error() -> String {
    "Unsupported type when resolving index type".to_string()
}

pub(crate) fn vue3_unresolvable_type_error(ty: &TSType<'_>) -> String {
    format!("Unresolvable type: {}", vue3_ts_type_kind_name(ty))
}

pub(crate) fn vue3_ts_type_kind_name(ty: &TSType<'_>) -> &'static str {
    match ty {
        TSType::TSAnyKeyword(_) => "TSAnyKeyword",
        TSType::TSBigIntKeyword(_) => "TSBigIntKeyword",
        TSType::TSBooleanKeyword(_) => "TSBooleanKeyword",
        TSType::TSIntrinsicKeyword(_) => "TSIntrinsicKeyword",
        TSType::TSNeverKeyword(_) => "TSNeverKeyword",
        TSType::TSNullKeyword(_) => "TSNullKeyword",
        TSType::TSNumberKeyword(_) => "TSNumberKeyword",
        TSType::TSObjectKeyword(_) => "TSObjectKeyword",
        TSType::TSStringKeyword(_) => "TSStringKeyword",
        TSType::TSSymbolKeyword(_) => "TSSymbolKeyword",
        TSType::TSThisType(_) => "TSThisType",
        TSType::TSUndefinedKeyword(_) => "TSUndefinedKeyword",
        TSType::TSUnknownKeyword(_) => "TSUnknownKeyword",
        TSType::TSVoidKeyword(_) => "TSVoidKeyword",
        TSType::TSLiteralType(_) => "TSLiteralType",
        TSType::TSTemplateLiteralType(_) => "TSTemplateLiteralType",
        TSType::TSTypeReference(_) => "TSTypeReference",
        TSType::TSTypeLiteral(_) => "TSTypeLiteral",
        TSType::TSArrayType(_) => "TSArrayType",
        TSType::TSTupleType(_) => "TSTupleType",
        TSType::TSUnionType(_) => "TSUnionType",
        TSType::TSIntersectionType(_) => "TSIntersectionType",
        TSType::TSParenthesizedType(_) => "TSParenthesizedType",
        TSType::TSFunctionType(_) => "TSFunctionType",
        TSType::TSConstructorType(_) => "TSConstructorType",
        TSType::TSTypeQuery(_) => "TSTypeQuery",
        TSType::TSTypeOperatorType(_) => "TSTypeOperatorType",
        TSType::TSIndexedAccessType(_) => "TSIndexedAccessType",
        TSType::TSMappedType(_) => "TSMappedType",
        TSType::TSConditionalType(_) => "TSConditionalType",
        TSType::TSInferType(_) => "TSInferType",
        TSType::TSImportType(_) => "TSImportType",
        TSType::TSNamedTupleMember(_) => "TSNamedTupleMember",
        TSType::TSTypePredicate(_) => "TSTypePredicate",
        TSType::JSDocNullableType(_) => "JSDocNullableType",
        TSType::JSDocNonNullableType(_) => "JSDocNonNullableType",
        TSType::JSDocUnknownType(_) => "JSDocUnknownType",
    }
}

pub(crate) fn vue3_merge_props_type_members(
    members: impl IntoIterator<Item = Vue27TypeMembers>,
    filter_duplicate_unknown: bool,
) -> (Vec<Vue27RuntimeProp>, Vec<String>) {
    let mut merged: Vec<Vue27RuntimeProp> = Vec::new();
    let mut indexes = std::collections::HashMap::<String, usize>::new();
    let mut errors = Vec::new();
    for type_members in members {
        errors.extend(type_members.errors);
        for prop in type_members.members {
            if let Some(index) = indexes.get(prop.key.as_str()).copied() {
                let existing = &mut merged[index];
                let mut types = Vec::new();
                let mut seen_types = std::collections::HashSet::new();
                for runtime_type in existing.types.iter().chain(prop.types.iter()) {
                    if filter_duplicate_unknown && runtime_type == "Unknown" {
                        continue;
                    }
                    if seen_types.insert(runtime_type.as_str()) {
                        types.push(runtime_type.clone());
                    }
                }
                if types.is_empty() {
                    types.push("Unknown".to_string());
                }
                existing.types = types;
                existing.required &= prop.required;
                continue;
            }
            indexes.insert(prop.key.clone(), merged.len());
            merged.push(prop);
        }
    }
    (merged, errors)
}

pub(crate) fn vue3_take_and_merge_interface_heritage_evidence(
    members: &mut [Vue27TypeMembers],
) -> Option<Vue3InterfaceHeritageEvidence> {
    let mut merged = None::<Vue3InterfaceHeritageEvidence>;
    for members in members {
        let Some(heritage) = members.interface_heritage.take() else {
            continue;
        };
        let target = merged.get_or_insert_with(Vue3InterfaceHeritageEvidence::default);
        for (target_members, source_members) in [
            (&mut target.own_members, heritage.own_members),
            (&mut target.inherited_members, heritage.inherited_members),
        ] {
            for (key, signatures) in source_members {
                target_members
                    .entry(key)
                    .or_default()
                    .extend(signatures);
            }
        }
    }
    merged
}

pub(crate) fn vue3_direct_interface_member_evidence(
    member: &Vue27RuntimeProp,
) -> Vue3InterfaceHeritageMemberEvidence {
    let exact_primitive_types = if member.is_method {
        None
    } else {
        member
            .type_annotation_source
            .as_deref()
            .and_then(vue3_exact_primitive_type_source)
    };
    Vue3InterfaceHeritageMemberEvidence {
        exact_primitive_types,
        required: Some(member.required),
    }
}

pub(crate) fn vue3_inherited_interface_member_evidence(
    base: &Vue27TypeMembers,
    member: &Vue27RuntimeProp,
) -> Vue3InterfaceHeritageMemberEvidence {
    let Some(heritage) = &base.interface_heritage else {
        return Vue3InterfaceHeritageMemberEvidence {
            exact_primitive_types: None,
            required: None,
        };
    };
    let candidates = heritage
        .own_members
        .get(&member.key)
        .or_else(|| heritage.inherited_members.get(&member.key));
    let Some(candidates) = candidates else {
        return Vue3InterfaceHeritageMemberEvidence {
            exact_primitive_types: None,
            required: None,
        };
    };
    let mut exact_primitive_types = None::<&BTreeSet<String>>;
    let mut required = None::<bool>;
    for candidate in candidates {
        let Some(candidate_types) = candidate.exact_primitive_types.as_ref() else {
            exact_primitive_types = None;
            break;
        };
        if exact_primitive_types.is_some_and(|types| types != candidate_types) {
            exact_primitive_types = None;
            break;
        }
        exact_primitive_types = Some(candidate_types);
    }
    for candidate in candidates {
        let Some(candidate_required) = candidate.required else {
            required = None;
            break;
        };
        if required.is_some_and(|value| value != candidate_required) {
            required = None;
            break;
        }
        required = Some(candidate_required);
    }
    Vue3InterfaceHeritageMemberEvidence {
        exact_primitive_types: exact_primitive_types.cloned(),
        required: required.filter(|required| *required == member.required),
    }
}

fn vue3_exact_primitive_type_source(source: &str) -> Option<BTreeSet<String>> {
    let source = source
        .strip_prefix(':')
        .map_or(source, str::trim)
        .trim();
    if source.is_empty() {
        return None;
    }
    let wrapped = format!("type __VuecExactPrimitive = {source}");
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
    parsed.program.body.iter().find_map(|statement| {
        let Statement::TSTypeAliasDeclaration(declaration) = statement else {
            return None;
        };
        vue3_exact_primitive_type(&declaration.type_annotation)
    })
}

fn vue3_exact_primitive_type(ty: &TSType<'_>) -> Option<BTreeSet<String>> {
    let primitive = match ty {
        TSType::TSBigIntKeyword(_) => "bigint",
        TSType::TSBooleanKeyword(_) => "boolean",
        TSType::TSNumberKeyword(_) => "number",
        TSType::TSStringKeyword(_) => "string",
        TSType::TSSymbolKeyword(_) => "symbol",
        TSType::TSNeverKeyword(_) => return Some(BTreeSet::new()),
        TSType::TSParenthesizedType(parenthesized) => {
            return vue3_exact_primitive_type(&parenthesized.type_annotation);
        }
        TSType::TSUnionType(union) => {
            let mut exact = BTreeSet::new();
            for ty in &union.types {
                exact.extend(vue3_exact_primitive_type(ty)?);
            }
            return Some(exact);
        }
        _ => return None,
    };
    Some(BTreeSet::from([primitive.to_string()]))
}

pub(crate) fn vue3_interface_heritage_has_proven_conflict(
    heritage: &Vue3InterfaceHeritageEvidence,
) -> bool {
    for (key, inherited_members) in &heritage.inherited_members {
        if let Some(own_members) = heritage.own_members.get(key) {
            for own in own_members {
                for inherited in inherited_members {
                    if matches!((inherited.required, own.required), (Some(true), Some(false)))
                        || match (
                            &own.exact_primitive_types,
                            &inherited.exact_primitive_types,
                        ) {
                            (Some(own), Some(inherited)) => !own.is_subset(inherited),
                            _ => false,
                        }
                    {
                        return true;
                    }
                }
            }
            continue;
        }
        let mut inherited = None::<(Option<bool>, Option<&BTreeSet<String>>)>;
        for member in inherited_members {
            let exact_types = member.exact_primitive_types.as_ref();
            if let Some((required, existing_types)) = &mut inherited {
                if matches!((*required, member.required), (Some(left), Some(right)) if left != right)
                {
                    return true;
                }
                if required.is_none() {
                    *required = member.required;
                }
                match (*existing_types, exact_types) {
                    (Some(existing), Some(current)) if existing != current => return true,
                    (None, Some(current)) => *existing_types = Some(current),
                    _ => {}
                }
            } else {
                inherited = Some((member.required, exact_types));
            }
        }
    }
    false
}

#[cfg(test)]
mod vue3_merge_props_type_members_tests {
    use super::*;

    fn prop(
        key: &str,
        types: &[&str],
        required: bool,
        marker: &str,
    ) -> Vue27RuntimeProp {
        Vue27RuntimeProp {
            key: key.to_string(),
            types: types.iter().map(|value| (*value).to_string()).collect(),
            required,
            default: Some(format!("default-{marker}")),
            is_method: marker == "method",
            type_annotation_source: Some(format!("type-{marker}")),
            member_source: Some(format!("member-{marker}")),
        }
    }

    fn members(
        source: &str,
        members: Vec<Vue27RuntimeProp>,
        errors: &[&str],
    ) -> Vue27TypeMembers {
        Vue27TypeMembers {
            source: source.to_string(),
            members,
            errors: errors.iter().map(|error| (*error).to_string()).collect(),
            interface_heritage: None,
        }
    }

    fn heritage_member(
        exact_primitive_types: Option<&[&str]>,
        required: bool,
    ) -> Vue3InterfaceHeritageMemberEvidence {
        Vue3InterfaceHeritageMemberEvidence {
            exact_primitive_types: exact_primitive_types.map(|types| {
                types.iter().map(|value| (*value).to_string()).collect()
            }),
            required: Some(required),
        }
    }

    fn heritage(
        own: &[(&str, Option<&[&str]>, bool)],
        bases: &[(&str, Option<&[&str]>, bool)],
    ) -> Vue3InterfaceHeritageEvidence {
        Vue3InterfaceHeritageEvidence {
            own_members: own.iter().fold(
                BTreeMap::<String, BTreeSet<Vue3InterfaceHeritageMemberEvidence>>::new(),
                |mut members, (key, types, required)| {
                    members
                        .entry((*key).to_string())
                        .or_default()
                        .insert(heritage_member(*types, *required));
                    members
                },
            ),
            inherited_members: bases.iter().fold(
                BTreeMap::<String, BTreeSet<Vue3InterfaceHeritageMemberEvidence>>::new(),
                |mut members, (key, types, required)| {
                    members
                        .entry((*key).to_string())
                        .or_default()
                        .insert(heritage_member(*types, *required));
                    members
                },
            ),
        }
    }

    #[test]
    fn indexed_member_merge_preserves_order_and_first_wins_metadata() {
        let first_b = prop("b", &["Unknown", "String", "String"], true, "first");
        let first_b_metadata = (
            first_b.default.clone(),
            first_b.is_method,
            first_b.type_annotation_source.clone(),
            first_b.member_source.clone(),
        );
        let inputs = vec![
            members(
                "first",
                vec![
                    first_b,
                    prop("a", &["Unknown", "Unknown"], true, "only"),
                    prop("unknown", &["Unknown"], true, "unknown-first"),
                ],
                &["first-error"],
            ),
            members(
                "second",
                vec![
                    prop("b", &["Number", "Unknown", "String"], false, "second"),
                    prop("c", &["Boolean"], true, "only"),
                    prop("unknown", &["Unknown"], false, "unknown-second"),
                ],
                &["second-error"],
            ),
        ];

        let (merged, errors) = vue3_merge_props_type_members(inputs, true);

        assert_eq!(
            merged.iter().map(|prop| prop.key.as_str()).collect::<Vec<_>>(),
            ["b", "a", "unknown", "c"]
        );
        assert_eq!(merged[0].types, ["String", "Number"]);
        assert!(!merged[0].required);
        assert_eq!(
            (
                merged[0].default.clone(),
                merged[0].is_method,
                merged[0].type_annotation_source.clone(),
                merged[0].member_source.clone(),
            ),
            first_b_metadata
        );
        assert_eq!(merged[1].types, ["Unknown", "Unknown"]);
        assert_eq!(merged[2].types, ["Unknown"]);
        assert!(!merged[2].required);
        assert_eq!(errors, ["first-error", "second-error"]);
    }

    #[test]
    fn indexed_member_merge_retains_unknown_without_filtering() {
        let inputs = vec![
            members(
                "first",
                vec![prop("value", &["Unknown", "String"], true, "first")],
                &[],
            ),
            members(
                "second",
                vec![prop("value", &["Unknown", "Number"], true, "second")],
                &[],
            ),
        ];

        let (merged, errors) = vue3_merge_props_type_members(inputs, false);

        assert!(errors.is_empty());
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].types, ["Unknown", "String", "Number"]);
    }

    #[test]
    fn heritage_conflicts_require_proof_and_respect_own_members() {
        assert!(vue3_interface_heritage_has_proven_conflict(
            &heritage(
                &[],
                &[
                    ("value", Some(&["string"]), true),
                    ("value", Some(&["number"]), true),
                ]
            )
        ));
        assert!(vue3_interface_heritage_has_proven_conflict(
            &heritage(
                &[],
                &[("value", None, true), ("value", None, false)]
            )
        ));
        assert!(!vue3_interface_heritage_has_proven_conflict(
            &heritage(
                &[("value", Some(&[]), true)],
                &[
                    ("value", Some(&["string"]), true),
                    ("value", Some(&["number"]), true),
                ]
            )
        ));
        assert!(vue3_interface_heritage_has_proven_conflict(
            &heritage(
                &[("value", Some(&["boolean"]), true)],
                &[
                    ("value", Some(&["string"]), true),
                    ("value", Some(&["number"]), true),
                ]
            )
        ));
        assert!(vue3_interface_heritage_has_proven_conflict(
            &heritage(
                &[("value", Some(&["string"]), false)],
                &[("value", Some(&["string"]), true)]
            )
        ));
        assert!(!vue3_interface_heritage_has_proven_conflict(
            &heritage(
                &[],
                &[
                    ("value", None, true),
                    ("value", Some(&["number"]), true),
                ]
            )
        ));
        assert!(vue3_interface_heritage_has_proven_conflict(
            &heritage(
                &[],
                &[
                    ("value", None, true),
                    ("value", Some(&["string"]), true),
                    ("value", Some(&["number"]), true),
                ]
            )
        ));
    }

    #[test]
    fn exact_primitive_heritage_types_require_syntax_proof() {
        assert_eq!(
            vue3_exact_primitive_type_source("string | (number | never)"),
            Some(BTreeSet::from(["number".to_string(), "string".to_string()]))
        );
        assert_eq!(
            vue3_exact_primitive_type_source(": boolean"),
            Some(BTreeSet::from(["boolean".to_string()]))
        );
        for source in [
            "Date",
            "Shape",
            "Exclude<string | number, number>",
            "ReturnType<Fn>",
            "() => string",
            "'literal'",
            "any",
            "unknown",
            "null",
            "undefined",
            "void",
            "string | null",
        ] {
            assert_eq!(vue3_exact_primitive_type_source(source), None, "{source}");
        }
    }

    #[test]
    fn inherited_heritage_evidence_requires_interface_provenance() {
        let mut member = prop("value", &["String", "Number"], true, "base");
        member.type_annotation_source = Some("string".to_string());
        let unproven_base = members("alias intersection", vec![member.clone()], &[]);
        assert_eq!(
            vue3_inherited_interface_member_evidence(&unproven_base, &member),
            Vue3InterfaceHeritageMemberEvidence {
                exact_primitive_types: None,
                required: None,
            }
        );

        let mut proven_base = members("merged interface", vec![member.clone()], &[]);
        proven_base.interface_heritage = Some(heritage(
            &[("value", Some(&[]), true)],
            &[
                ("value", Some(&["string"]), true),
                ("value", Some(&["number"]), true),
            ],
        ));
        assert_eq!(
            vue3_inherited_interface_member_evidence(&proven_base, &member),
            Vue3InterfaceHeritageMemberEvidence {
                exact_primitive_types: Some(BTreeSet::new()),
                required: Some(true),
            }
        );

        member.required = false;
        assert_eq!(
            vue3_inherited_interface_member_evidence(&proven_base, &member),
            Vue3InterfaceHeritageMemberEvidence {
                exact_primitive_types: Some(BTreeSet::new()),
                required: None,
            }
        );
    }
}

pub(crate) fn vue3_resolve_projectable_props_type(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    vue3_resolve_props_type(source, type_argument, analysis)
}

pub(crate) fn vue3_resolve_extract_prop_types(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    vue3_resolve_props_options_type(source, type_argument, analysis)
}

pub(crate) fn vue3_resolve_props_options_type(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    match type_argument {
        TSType::TSTypeLiteral(_) => {
            vue3_props_options_type_members(source, type_argument, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            if name == "ReturnType" {
                let ty = vue3_type_reference_first_type_argument(reference)?;
                return vue3_resolve_return_type_props_options(source, ty, analysis);
            }
            analysis.props_options_type_declarations.get(&name).cloned()
        }
        TSType::TSTypeQuery(query) => vue3_type_query_props_options_declaration(query, analysis),
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_resolve_props_options_type(source, &parenthesized.type_annotation, analysis)
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .props_options_type_declarations
                .get(&resolved.name)
                .cloned()
        }
        _ => None,
    }
}

pub(crate) fn vue3_resolve_return_type_props_options(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    match type_argument {
        TSType::TSTypeQuery(query) => {
            vue3_type_query_return_props_options_declaration(query, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            analysis
                .return_type_props_options_declarations
                .get(&name)
                .cloned()
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_resolve_return_type_props_options(source, &parenthesized.type_annotation, analysis)
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .return_type_props_options_declarations
                .get(&resolved.name)
                .cloned()
        }
        _ => vue3_props_options_type_members(source, type_argument, analysis),
    }
}

pub(crate) fn vue3_type_reference_first_type_argument<'a>(
    reference: &'a TSTypeReference<'a>,
) -> Option<&'a TSType<'a>> {
    vue3_type_reference_type_argument(reference, 0)
}

pub(crate) fn vue3_type_reference_type_argument<'a>(
    reference: &'a TSTypeReference<'a>,
    index: usize,
) -> Option<&'a TSType<'a>> {
    reference
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.get(index))
}

pub(crate) fn vue3_import_type_first_type_argument<'a>(
    import_type: &'a TSImportType<'a>,
) -> Option<&'a TSType<'a>> {
    import_type
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
}

pub(crate) fn vue3_type_members_optional(mut members: Vue27TypeMembers) -> Vue27TypeMembers {
    for prop in &mut members.members {
        prop.required = false;
    }
    members
}

pub(crate) fn vue3_type_members_required(mut members: Vue27TypeMembers) -> Vue27TypeMembers {
    for prop in &mut members.members {
        prop.required = true;
    }
    members
}

pub(crate) fn vue3_type_members_pick(
    mut members: Vue27TypeMembers,
    keys: &BTreeSet<String>,
) -> Vue27TypeMembers {
    members.members.retain(|prop| keys.contains(&prop.key));
    members
}

pub(crate) fn vue3_type_members_omit(
    mut members: Vue27TypeMembers,
    keys: &BTreeSet<String>,
) -> Vue27TypeMembers {
    members.members.retain(|prop| !keys.contains(&prop.key));
    members
}

pub(crate) fn vue3_resolve_indexed_access_props_type(
    source: &str,
    indexed: &oxc_ast::ast::TSIndexedAccessType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3PropsTypeResolveMode,
) -> Option<Vue27TypeMembers> {
    let object_members =
        vue3_resolve_props_type_with_mode(source, &indexed.object_type, analysis, mode)?;
    let keys = match vue3_indexed_access_member_keys(&indexed.index_type, &object_members, analysis)
    {
        Some(keys) => keys,
        None if mode == Vue3PropsTypeResolveMode::Consumed => {
            return Some(vue3_type_members_empty(
                source,
                indexed.index_type.span(),
                vec![vue3_unsupported_index_type_error()],
            ));
        }
        None => return None,
    };
    let mut projected = Vec::new();
    let mut errors = object_members.errors;
    for key in keys {
        let Some(prop) = object_members.members.iter().find(|prop| prop.key == key) else {
            continue;
        };
        if let Some(members) = vue3_resolve_props_type_from_runtime_prop(prop, analysis) {
            errors.extend(members.errors);
            projected.extend(members.members);
        }
    }
    vue3_merged_type_members(source, indexed.span, projected, errors)
}

pub(crate) fn vue3_indexed_access_member_keys(
    index_type: &TSType<'_>,
    members: &Vue27TypeMembers,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(keys) = vue3_resolve_ordered_string_type_keys(index_type, analysis) {
        return Some(keys);
    }
    if vue3_indexed_access_is_string_key(index_type, analysis) {
        let mut keys = Vec::new();
        for prop in &members.members {
            push_unique(&mut keys, &prop.key);
        }
        return Some(keys);
    }
    None
}

pub(crate) fn vue3_indexed_access_is_string_key(
    index_type: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> bool {
    match index_type {
        TSType::TSStringKeyword(_) => true,
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_indexed_access_is_string_key(&parenthesized.type_annotation, analysis)
        }
        TSType::TSTypeReference(reference) => vue3_ts_type_name_key(&reference.type_name)
            .and_then(|name| analysis.declared_types.get(&name))
            .is_some_and(|types| types.len() == 1 && types[0] == "String"),
        _ => false,
    }
}

pub(crate) fn vue3_resolve_props_type_from_runtime_prop(
    prop: &Vue27RuntimeProp,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let type_source = prop
        .type_annotation_source
        .as_deref()
        .and_then(|annotation| annotation.strip_prefix(':').map(str::trim))
        .or(prop.type_annotation_source.as_deref())
        .map(str::trim)?;
    if type_source.is_empty() {
        return None;
    }
    let wrapped = format!("type __VuecResolved = {type_source}");
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

pub(crate) fn vue3_resolve_string_type_keys(
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<BTreeSet<String>> {
    Some(
        vue3_resolve_ordered_string_type_keys(ty, analysis)?
            .into_iter()
            .collect(),
    )
}

pub(crate) fn vue3_resolve_ordered_string_type_keys(
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    match ty {
        TSType::TSLiteralType(literal) => {
            vue3_literal_type_key(&literal.literal).map(|key| vec![key])
        }
        TSType::TSUnionType(union) => {
            let mut keys = Vec::new();
            for ty in &union.types {
                for key in vue3_resolve_ordered_string_type_keys(ty, analysis)? {
                    push_unique(&mut keys, &key);
                }
            }
            Some(keys)
        }
        TSType::TSTemplateLiteralType(template) => {
            vue3_resolve_template_literal_type_keys(template, analysis)
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_resolve_ordered_string_type_keys(&parenthesized.type_annotation, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            match name.as_str() {
                "Extract" => {
                    let source = vue3_type_reference_type_argument(reference, 0)?;
                    let filter = vue3_type_reference_type_argument(reference, 1)?;
                    let source_keys = vue3_resolve_ordered_string_type_keys(source, analysis)?;
                    let filter_keys = vue3_resolve_string_type_keys(filter, analysis)?;
                    Some(
                        source_keys
                            .into_iter()
                            .filter(|key| filter_keys.contains(key))
                            .collect(),
                    )
                }
                "Exclude" => {
                    let source = vue3_type_reference_type_argument(reference, 0)?;
                    let excluded = vue3_type_reference_type_argument(reference, 1)?;
                    let source_keys = vue3_resolve_ordered_string_type_keys(source, analysis)?;
                    let excluded_keys = vue3_resolve_string_type_keys(excluded, analysis)?;
                    Some(
                        source_keys
                            .into_iter()
                            .filter(|key| !excluded_keys.contains(key))
                            .collect(),
                    )
                }
                "Uppercase" | "Lowercase" | "Capitalize" | "Uncapitalize" => {
                    let ty = vue3_type_reference_first_type_argument(reference)?;
                    let mut keys = Vec::new();
                    for key in vue3_resolve_ordered_string_type_keys(ty, analysis)? {
                        let mapped = vue3_string_mapping_type(name.as_str(), &key);
                        push_unique(&mut keys, &mapped);
                    }
                    Some(keys)
                }
                _ => analysis
                    .ordered_string_literal_type_declarations
                    .get(&name)
                    .cloned()
                    .or_else(|| {
                        analysis
                            .string_literal_type_declarations
                            .get(&name)
                            .map(|keys| keys.iter().cloned().collect())
                    }),
            }
        }
        TSType::TSTypeOperatorType(operator)
            if operator.operator == TSTypeOperatorOperator::Keyof =>
        {
            let members = vue3_resolve_props_type("", &operator.type_annotation, analysis)?;
            let mut keys = Vec::new();
            for prop in members.members {
                push_unique(&mut keys, &prop.key);
            }
            Some(keys)
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .ordered_string_literal_type_declarations
                .get(&resolved.name)
                .cloned()
                .or_else(|| {
                    resolved
                        .context
                        .string_literal_type_declarations
                        .get(&resolved.name)
                        .map(|keys| keys.iter().cloned().collect())
                })
        }
        _ => None,
    }
}

pub(crate) fn vue3_resolve_template_literal_type_keys(
    template: &TSTemplateLiteralType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let mut values = vec![vue3_template_type_quasi_value(template.quasis.first()?)];
    for (index, ty) in template.types.iter().enumerate() {
        let keys = vue3_resolve_ordered_string_type_keys(ty, analysis)?;
        let suffix = vue3_template_type_quasi_value(template.quasis.get(index + 1)?);
        let mut next = Vec::new();
        for prefix in &values {
            for key in &keys {
                next.push(format!("{prefix}{key}{suffix}"));
            }
        }
        values = next;
    }
    Some(values)
}

pub(crate) fn vue3_template_type_quasi_value(quasi: &oxc_ast::ast::TemplateElement<'_>) -> String {
    quasi
        .value
        .cooked
        .as_ref()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| quasi.value.raw.as_str().to_string())
}

pub(crate) fn vue3_string_mapping_type(name: &str, value: &str) -> String {
    match name {
        "Uppercase" => value.to_uppercase(),
        "Lowercase" => value.to_lowercase(),
        "Capitalize" => {
            let mut chars = value.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut mapped = first.to_uppercase().collect::<String>();
            mapped.push_str(chars.as_str());
            mapped
        }
        "Uncapitalize" => {
            let mut chars = value.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut mapped = first.to_lowercase().collect::<String>();
            mapped.push_str(chars.as_str());
            mapped
        }
        _ => value.to_string(),
    }
}

pub(crate) fn vue3_literal_type_key(literal: &TSLiteral<'_>) -> Option<String> {
    match literal {
        TSLiteral::StringLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::NumericLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::BigIntLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}
