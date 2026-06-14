pub(crate) fn record_vue3_type_argument_deps(
    type_argument: &TSType<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for dependency in collect_vue3_type_argument_deps_ordered(type_argument, analysis) {
        push_unique(&mut analysis.deps, &dependency);
    }
}

pub(crate) fn collect_vue3_type_argument_deps_ordered(
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<String> {
    let mut deps = Vec::new();
    collect_vue3_type_argument_deps_ordered_into(type_argument, analysis, &mut deps);
    deps
}

pub(crate) fn collect_vue3_type_argument_deps_ordered_into(
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    match type_argument {
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
                collect_vue3_named_type_deps_ordered(&name, analysis, deps);
            }
            if let Some(type_arguments) = reference.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
                }
            }
        }
        TSType::TSTypeLiteral(literal) => {
            for signature in &literal.members {
                collect_vue3_signature_type_deps_ordered(signature, analysis, deps);
            }
        }
        TSType::TSUnionType(union) => {
            for ty in &union.types {
                collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
            }
        }
        TSType::TSIntersectionType(intersection) => {
            for ty in &intersection.types {
                collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
            }
        }
        TSType::TSArrayType(array) => {
            collect_vue3_type_argument_deps_ordered_into(&array.element_type, analysis, deps);
        }
        TSType::TSTupleType(tuple) => {
            for element in &tuple.element_types {
                collect_vue3_tuple_element_type_deps_ordered(element, analysis, deps);
            }
        }
        TSType::TSParenthesizedType(parenthesized) => {
            collect_vue3_type_argument_deps_ordered_into(
                &parenthesized.type_annotation,
                analysis,
                deps,
            );
        }
        TSType::TSTypeOperatorType(operator) => {
            collect_vue3_type_argument_deps_ordered_into(&operator.type_annotation, analysis, deps);
        }
        TSType::TSIndexedAccessType(indexed) => {
            collect_vue3_type_argument_deps_ordered_into(&indexed.object_type, analysis, deps);
            collect_vue3_type_argument_deps_ordered_into(&indexed.index_type, analysis, deps);
        }
        TSType::TSFunctionType(function) => {
            collect_vue3_formal_parameters_type_deps_ordered(&function.params, analysis, deps);
            collect_vue3_type_annotation_deps_ordered(&function.return_type, analysis, deps);
        }
        TSType::TSConstructorType(constructor) => {
            collect_vue3_formal_parameters_type_deps_ordered(&constructor.params, analysis, deps);
            collect_vue3_type_annotation_deps_ordered(&constructor.return_type, analysis, deps);
        }
        TSType::TSConditionalType(conditional) => {
            collect_vue3_type_argument_deps_ordered_into(&conditional.check_type, analysis, deps);
            collect_vue3_type_argument_deps_ordered_into(&conditional.extends_type, analysis, deps);
            collect_vue3_type_argument_deps_ordered_into(&conditional.true_type, analysis, deps);
            collect_vue3_type_argument_deps_ordered_into(&conditional.false_type, analysis, deps);
        }
        TSType::TSMappedType(mapped) => {
            collect_vue3_type_argument_deps_ordered_into(&mapped.constraint, analysis, deps);
            if let Some(name_type) = mapped.name_type.as_ref() {
                collect_vue3_type_argument_deps_ordered_into(name_type, analysis, deps);
            }
            if let Some(type_annotation) = mapped.type_annotation.as_ref() {
                collect_vue3_type_argument_deps_ordered_into(type_annotation, analysis, deps);
            }
        }
        TSType::TSTemplateLiteralType(template) => {
            for ty in &template.types {
                collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
            }
        }
        TSType::TSNamedTupleMember(member) => {
            collect_vue3_tuple_element_type_deps_ordered(&member.element_type, analysis, deps);
        }
        TSType::TSTypePredicate(predicate) => {
            if let Some(type_annotation) = predicate.type_annotation.as_ref() {
                collect_vue3_type_annotation_deps_ordered(type_annotation, analysis, deps);
            }
        }
        TSType::TSTypeQuery(query) => {
            collect_vue3_type_query_deps_ordered(query, analysis, deps);
        }
        TSType::TSImportType(import_type) => {
            if let Some(resolved) = vue3_resolve_import_type(import_type, analysis) {
                push_unique(deps, &resolved.dependency);
                collect_vue3_context_type_deps_ordered(&resolved.context, &resolved.name, deps);
            }
            if let Some(type_arguments) = import_type.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
                }
            }
        }
        TSType::TSInferType(infer) => {
            if let Some(constraint) = infer.type_parameter.constraint.as_ref() {
                collect_vue3_type_argument_deps_ordered_into(constraint, analysis, deps);
            }
            if let Some(default) = infer.type_parameter.default.as_ref() {
                collect_vue3_type_argument_deps_ordered_into(default, analysis, deps);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_named_type_deps_ordered(
    name: &str,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    if let Some(dependency) = analysis.type_sources.get(name) {
        push_unique(deps, dependency);
    }
    if let Some(direct_dependencies) = analysis.type_direct_deps.get(name) {
        for dependency in direct_dependencies {
            push_unique(deps, dependency);
        }
    }
    if let Some(dependencies) = analysis.type_deps.get(name) {
        for dependency in dependencies {
            push_unique(deps, dependency);
        }
    }
}

pub(crate) fn collect_vue3_context_type_deps_ordered(
    context: &Vue27TypeContext,
    name: &str,
    deps: &mut Vec<String>,
) {
    if let Some(dependency) = context.type_sources.get(name) {
        push_unique(deps, dependency);
    }
    if let Some(direct_dependencies) = context.type_direct_deps.get(name) {
        for dependency in direct_dependencies {
            push_unique(deps, dependency);
        }
    }
    if let Some(dependencies) = context.type_deps.get(name) {
        for dependency in dependencies {
            push_unique(deps, dependency);
        }
    }
}

pub(crate) fn collect_vue3_signature_type_deps_ordered(
    signature: &TSSignature<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    match signature {
        TSSignature::TSPropertySignature(property) => {
            if let Some(type_annotation) = property.type_annotation.as_ref() {
                collect_vue3_type_annotation_deps_ordered(type_annotation, analysis, deps);
            }
        }
        TSSignature::TSMethodSignature(method) => {
            collect_vue3_formal_parameters_type_deps_ordered(&method.params, analysis, deps);
            if let Some(return_type) = method.return_type.as_ref() {
                collect_vue3_type_annotation_deps_ordered(return_type, analysis, deps);
            }
        }
        TSSignature::TSCallSignatureDeclaration(signature) => {
            collect_vue3_formal_parameters_type_deps_ordered(&signature.params, analysis, deps);
            if let Some(return_type) = signature.return_type.as_ref() {
                collect_vue3_type_annotation_deps_ordered(return_type, analysis, deps);
            }
        }
        TSSignature::TSConstructSignatureDeclaration(signature) => {
            collect_vue3_formal_parameters_type_deps_ordered(&signature.params, analysis, deps);
            if let Some(return_type) = signature.return_type.as_ref() {
                collect_vue3_type_annotation_deps_ordered(return_type, analysis, deps);
            }
        }
        TSSignature::TSIndexSignature(signature) => {
            for parameter in &signature.parameters {
                collect_vue3_type_annotation_deps_ordered(
                    &parameter.type_annotation,
                    analysis,
                    deps,
                );
            }
            collect_vue3_type_annotation_deps_ordered(&signature.type_annotation, analysis, deps);
        }
    }
}

pub(crate) fn collect_vue3_formal_parameters_type_deps_ordered(
    parameters: &FormalParameters<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    for parameter in &parameters.items {
        if let Some(type_annotation) = parameter.type_annotation.as_ref() {
            collect_vue3_type_annotation_deps_ordered(type_annotation, analysis, deps);
        }
    }
    if let Some(rest) = parameters.rest.as_ref() {
        if let Some(type_annotation) = rest.type_annotation.as_ref() {
            collect_vue3_type_annotation_deps_ordered(type_annotation, analysis, deps);
        }
    }
}

pub(crate) fn collect_vue3_type_annotation_deps_ordered(
    annotation: &TSTypeAnnotation<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    collect_vue3_type_argument_deps_ordered_into(&annotation.type_annotation, analysis, deps);
}

pub(crate) fn collect_vue3_tuple_element_type_deps_ordered(
    element: &TSTupleElement<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    match element {
        TSTupleElement::TSOptionalType(optional) => {
            collect_vue3_type_argument_deps_ordered_into(&optional.type_annotation, analysis, deps);
        }
        TSTupleElement::TSRestType(rest) => {
            collect_vue3_type_argument_deps_ordered_into(&rest.type_annotation, analysis, deps);
        }
        _ => {
            if let Some(ty) = element.as_ts_type() {
                collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
            }
        }
    }
}
