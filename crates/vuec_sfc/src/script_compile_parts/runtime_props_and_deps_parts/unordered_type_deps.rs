pub(crate) fn collect_vue3_type_argument_deps(
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeSet<String> {
    let mut deps = BTreeSet::new();
    collect_vue3_type_argument_deps_into(type_argument, analysis, &mut deps);
    deps
}

pub(crate) fn collect_vue3_type_argument_deps_into(
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    match type_argument {
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
                if let Some(dependencies) = analysis.type_deps.get(&name) {
                    deps.extend(dependencies.iter().cloned());
                } else if let Some(dependency) = analysis.type_sources.get(&name) {
                    deps.insert(dependency.clone());
                }
            }
            if let Some(type_arguments) = reference.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    collect_vue3_type_argument_deps_into(ty, analysis, deps);
                }
            }
        }
        TSType::TSTypeLiteral(literal) => {
            for signature in &literal.members {
                collect_vue3_signature_type_deps(signature, analysis, deps);
            }
        }
        TSType::TSUnionType(union) => {
            for ty in &union.types {
                collect_vue3_type_argument_deps_into(ty, analysis, deps);
            }
        }
        TSType::TSIntersectionType(intersection) => {
            for ty in &intersection.types {
                collect_vue3_type_argument_deps_into(ty, analysis, deps);
            }
        }
        TSType::TSArrayType(array) => {
            collect_vue3_type_argument_deps_into(&array.element_type, analysis, deps);
        }
        TSType::TSTupleType(tuple) => {
            for element in &tuple.element_types {
                collect_vue3_tuple_element_type_deps(element, analysis, deps);
            }
        }
        TSType::TSParenthesizedType(parenthesized) => {
            collect_vue3_type_argument_deps_into(&parenthesized.type_annotation, analysis, deps);
        }
        TSType::TSTypeOperatorType(operator) => {
            collect_vue3_type_argument_deps_into(&operator.type_annotation, analysis, deps);
        }
        TSType::TSIndexedAccessType(indexed) => {
            collect_vue3_type_argument_deps_into(&indexed.object_type, analysis, deps);
            collect_vue3_type_argument_deps_into(&indexed.index_type, analysis, deps);
        }
        TSType::TSFunctionType(function) => {
            collect_vue3_formal_parameters_type_deps(&function.params, analysis, deps);
            collect_vue3_type_annotation_deps(&function.return_type, analysis, deps);
        }
        TSType::TSConstructorType(constructor) => {
            collect_vue3_formal_parameters_type_deps(&constructor.params, analysis, deps);
            collect_vue3_type_annotation_deps(&constructor.return_type, analysis, deps);
        }
        TSType::TSConditionalType(conditional) => {
            collect_vue3_type_argument_deps_into(&conditional.check_type, analysis, deps);
            collect_vue3_type_argument_deps_into(&conditional.extends_type, analysis, deps);
            collect_vue3_type_argument_deps_into(&conditional.true_type, analysis, deps);
            collect_vue3_type_argument_deps_into(&conditional.false_type, analysis, deps);
        }
        TSType::TSMappedType(mapped) => {
            collect_vue3_type_argument_deps_into(&mapped.constraint, analysis, deps);
            if let Some(name_type) = mapped.name_type.as_ref() {
                collect_vue3_type_argument_deps_into(name_type, analysis, deps);
            }
            if let Some(type_annotation) = mapped.type_annotation.as_ref() {
                collect_vue3_type_argument_deps_into(type_annotation, analysis, deps);
            }
        }
        TSType::TSTemplateLiteralType(template) => {
            for ty in &template.types {
                collect_vue3_type_argument_deps_into(ty, analysis, deps);
            }
        }
        TSType::TSNamedTupleMember(member) => {
            collect_vue3_tuple_element_type_deps(&member.element_type, analysis, deps);
        }
        TSType::TSTypePredicate(predicate) => {
            if let Some(type_annotation) = predicate.type_annotation.as_ref() {
                collect_vue3_type_annotation_deps(type_annotation, analysis, deps);
            }
        }
        TSType::TSTypeQuery(query) => {
            collect_vue3_type_query_deps(query, analysis, deps);
        }
        TSType::TSImportType(import_type) => {
            if let Some(resolved) = vue3_resolve_import_type(import_type, analysis) {
                deps.extend(
                    resolved
                        .context
                        .type_deps
                        .get(&resolved.name)
                        .cloned()
                        .unwrap_or_default(),
                );
                deps.insert(resolved.dependency);
            }
            if let Some(type_arguments) = import_type.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    collect_vue3_type_argument_deps_into(ty, analysis, deps);
                }
            }
        }
        TSType::TSInferType(infer) => {
            if let Some(constraint) = infer.type_parameter.constraint.as_ref() {
                collect_vue3_type_argument_deps_into(constraint, analysis, deps);
            }
            if let Some(default) = infer.type_parameter.default.as_ref() {
                collect_vue3_type_argument_deps_into(default, analysis, deps);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_named_type_deps(
    name: &str,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    if let Some(dependencies) = analysis.type_deps.get(name) {
        deps.extend(dependencies.iter().cloned());
    } else if let Some(dependency) = analysis.type_sources.get(name) {
        deps.insert(dependency.clone());
    }
}

pub(crate) fn collect_vue3_signature_type_deps(
    signature: &TSSignature<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    match signature {
        TSSignature::TSPropertySignature(property) => {
            if let Some(type_annotation) = property.type_annotation.as_ref() {
                collect_vue3_type_annotation_deps(type_annotation, analysis, deps);
            }
        }
        TSSignature::TSMethodSignature(method) => {
            collect_vue3_formal_parameters_type_deps(&method.params, analysis, deps);
            if let Some(return_type) = method.return_type.as_ref() {
                collect_vue3_type_annotation_deps(return_type, analysis, deps);
            }
        }
        TSSignature::TSCallSignatureDeclaration(signature) => {
            collect_vue3_formal_parameters_type_deps(&signature.params, analysis, deps);
            if let Some(return_type) = signature.return_type.as_ref() {
                collect_vue3_type_annotation_deps(return_type, analysis, deps);
            }
        }
        TSSignature::TSConstructSignatureDeclaration(signature) => {
            collect_vue3_formal_parameters_type_deps(&signature.params, analysis, deps);
            if let Some(return_type) = signature.return_type.as_ref() {
                collect_vue3_type_annotation_deps(return_type, analysis, deps);
            }
        }
        TSSignature::TSIndexSignature(signature) => {
            for parameter in &signature.parameters {
                collect_vue3_type_annotation_deps(&parameter.type_annotation, analysis, deps);
            }
            collect_vue3_type_annotation_deps(&signature.type_annotation, analysis, deps);
        }
    }
}

pub(crate) fn collect_vue3_formal_parameters_type_deps(
    parameters: &FormalParameters<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    for parameter in &parameters.items {
        if let Some(type_annotation) = parameter.type_annotation.as_ref() {
            collect_vue3_type_annotation_deps(type_annotation, analysis, deps);
        }
    }
    if let Some(rest) = parameters.rest.as_ref() {
        if let Some(type_annotation) = rest.type_annotation.as_ref() {
            collect_vue3_type_annotation_deps(type_annotation, analysis, deps);
        }
    }
}

pub(crate) fn collect_vue3_type_annotation_deps(
    annotation: &TSTypeAnnotation<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    collect_vue3_type_argument_deps_into(&annotation.type_annotation, analysis, deps);
}

pub(crate) fn collect_vue3_tuple_element_type_deps(
    element: &TSTupleElement<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    match element {
        TSTupleElement::TSOptionalType(optional) => {
            collect_vue3_type_argument_deps_into(&optional.type_annotation, analysis, deps);
        }
        TSTupleElement::TSRestType(rest) => {
            collect_vue3_type_argument_deps_into(&rest.type_annotation, analysis, deps);
        }
        _ => {
            if let Some(ty) = element.as_ts_type() {
                collect_vue3_type_argument_deps_into(ty, analysis, deps);
            }
        }
    }
}
