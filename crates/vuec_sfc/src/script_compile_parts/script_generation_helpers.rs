pub(crate) fn vue3_top_level_await_entry_statement(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::VariableDeclaration(declaration) => !declaration.declare,
        Statement::BlockStatement(_)
        | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::DoWhileStatement(_)
        | Statement::EmptyStatement(_)
        | Statement::ExpressionStatement(_)
        | Statement::ForInStatement(_)
        | Statement::ForOfStatement(_)
        | Statement::ForStatement(_)
        | Statement::IfStatement(_)
        | Statement::LabeledStatement(_)
        | Statement::ReturnStatement(_)
        | Statement::SwitchStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::TryStatement(_)
        | Statement::WhileStatement(_)
        | Statement::WithStatement(_) => true,
        _ => false,
    }
}

pub(crate) fn contains_js_await_word(source: &str) -> bool {
    let bytes = source.as_bytes();
    let needle = b"await";
    if bytes.len() < needle.len() {
        return false;
    }
    bytes
        .windows(needle.len())
        .enumerate()
        .any(|(index, window)| {
            window == needle
                && !bytes
                    .get(index.wrapping_sub(1))
                    .is_some_and(|byte| is_js_identifier_byte(*byte))
                && !bytes
                    .get(index + needle.len())
                    .is_some_and(|byte| is_js_identifier_byte(*byte))
        })
}

pub(crate) fn is_js_identifier_byte(byte: u8) -> bool {
    byte == b'_' || byte == b'$' || byte.is_ascii_alphanumeric()
}

pub(crate) fn vue3_props_access_exp(prop: &str) -> String {
    if is_ascii_js_identifier(prop) {
        format!("__props.{prop}")
    } else {
        format!("__props[\"{}\"]", escape_js_double(prop))
    }
}

pub(crate) fn vue3_is_define_props_call(expression: &Expression<'_>) -> bool {
    matches!(unwrap_vue3_ts_expression(expression), Expression::CallExpression(call) if is_call_named(call, "defineProps"))
}

pub(crate) fn vue3_call_argument_expression<'a>(
    argument: &'a Argument<'a>,
) -> Option<&'a Expression<'a>> {
    match argument {
        Argument::SpreadElement(_) => None,
        _ => Some(argument.to_expression()),
    }
}

pub(crate) fn vue3_expression_references_non_literal_setup_local(
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> bool {
    let non_literal_bindings = analysis
        .local_setup_binding_types
        .iter()
        .filter_map(|(name, binding_type)| {
            (binding_type != "literal-const").then_some(name.clone())
        })
        .collect::<BTreeSet<_>>();
    vue27_expression_references_setup_local(expression, &non_literal_bindings)
}

pub(crate) fn check_vue3_invalid_non_literal_scope_reference(
    expression: &Expression<'_>,
    macro_name: &str,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if vue3_expression_references_non_literal_setup_local(expression, analysis) {
        analysis
            .errors
            .push(vue3_invalid_scope_reference_error(macro_name));
    }
}

pub(crate) fn vue3_invalid_scope_reference_error(macro_name: &str) -> String {
    format!(
        "`{macro_name}()` in <script setup> cannot reference locally declared variables because it will be hoisted outside of the setup() function. If your component options require initialization in the module scope, use a separate normal <script> to export the options instead."
    )
}

pub(crate) fn collect_vue3_define_emits_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&str>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_emits {
        analysis.errors.push("duplicate defineEmits() call".into());
    }
    analysis.has_define_emits = true;
    if analysis.emit_binding.is_none() {
        if let Some(binding) = binding {
            analysis.emit_binding = Some(binding.to_string());
        }
    }
    if let Some(type_argument) = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    {
        if !call.arguments.is_empty() {
            analysis
                .errors
                .push(vue27_macro_type_and_runtime_error("defineEmits"));
        }
        collect_vue3_define_emits_type(source, type_argument, analysis);
        return;
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    let expression = argument.to_expression();
    check_vue3_invalid_non_literal_scope_reference(expression, "defineEmits", analysis);
    analysis.emits_runtime = source
        .get(expression.span().start as usize..expression.span().end as usize)
        .map(ToOwned::to_owned);
}

pub(crate) fn collect_vue3_define_emits_type(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    record_vue3_type_argument_deps(type_argument, analysis);
    let Some(emits_type) = vue3_resolve_emits_type(source, type_argument, analysis) else {
        return;
    };
    if emits_type.syntax.has_call_signature && emits_type.syntax.has_property {
        analysis
            .errors
            .push("defineEmits() type cannot mixed call signature and property syntax.".into());
    }
    if !emits_type.events.is_empty() {
        analysis.emits_runtime = Some(format!(
            "[{}]",
            emits_type
                .events
                .iter()
                .map(|name| format!("\"{}\"", escape_js_double(name)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

pub(crate) fn vue3_resolve_emits_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27EmitsType> {
    match type_argument {
        TSType::TSFunctionType(function) => {
            Some(vue3_emits_type_from_function(source, function, analysis))
        }
        TSType::TSTypeLiteral(literal) => {
            Some(vue3_emits_type_from_literal(source, literal, analysis))
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            analysis.emits_type_declarations.get(&name).cloned()
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .emits_type_declarations
                .get(&resolved.name)
                .cloned()
        }
        TSType::TSIntersectionType(intersection) => {
            let mut events = Vec::new();
            let mut syntax = Vue3EmitsTypeSyntax::default();
            let mut call_count = 0usize;
            for ty in &intersection.types {
                let Some(resolved) = vue3_resolve_emits_type(source, ty, analysis) else {
                    continue;
                };
                syntax.has_call_signature |= resolved.syntax.has_call_signature;
                syntax.has_property |= resolved.syntax.has_property;
                call_count += resolved.call_count;
                for event in resolved.events {
                    push_unique(&mut events, &event);
                }
            }
            if events.is_empty() && call_count == 0 {
                None
            } else {
                Some(Vue27EmitsType {
                    source: source
                        .get(intersection.span.start as usize..intersection.span.end as usize)
                        .unwrap_or_default()
                        .to_string(),
                    events,
                    syntax,
                    call_count,
                })
            }
        }
        TSType::TSUnionType(union) => {
            let mut events = Vec::new();
            let mut syntax = Vue3EmitsTypeSyntax::default();
            let mut call_count = 0usize;
            for ty in &union.types {
                let Some(resolved) = vue3_resolve_emits_type(source, ty, analysis) else {
                    continue;
                };
                syntax.has_call_signature |= resolved.syntax.has_call_signature;
                syntax.has_property |= resolved.syntax.has_property;
                call_count += resolved.call_count;
                for event in resolved.events {
                    push_unique(&mut events, &event);
                }
            }
            if events.is_empty() && call_count == 0 {
                None
            } else {
                Some(Vue27EmitsType {
                    source: source
                        .get(union.span.start as usize..union.span.end as usize)
                        .unwrap_or_default()
                        .to_string(),
                    events,
                    syntax,
                    call_count,
                })
            }
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_resolve_emits_type(source, &parenthesized.type_annotation, analysis)
        }
        _ => None,
    }
}

pub(crate) fn vue3_runtime_prop_keys(expression: &Expression<'_>) -> Vec<String> {
    match expression {
        Expression::ObjectExpression(object) => object_expression_keys(object),
        Expression::ArrayExpression(array) => array
            .elements
            .iter()
            .filter_map(|element| match element.as_expression() {
                Some(Expression::StringLiteral(literal)) => Some(literal.value.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn vue3_setup_binding_type(
    kind: VariableDeclarationKind,
    init: Option<&Expression<'_>>,
    is_all_static: bool,
    literal_const_enabled: bool,
    vue_import_aliases: &BTreeMap<String, String>,
) -> &'static str {
    if kind != VariableDeclarationKind::Const {
        return "setup-let";
    }
    if literal_const_enabled && (is_all_static || init.is_some_and(vue3_is_static_node)) {
        return "literal-const";
    }
    if init.is_some_and(|init| {
        vue3_is_call_named_alias(init, vue_import_aliases.get("reactive").map(String::as_str))
    }) {
        return "setup-reactive-const";
    }
    if init.is_some_and(|init| vue3_can_never_be_ref(init, vue_import_aliases)) {
        return "setup-const";
    }
    if init.is_some_and(|init| vue3_is_ref_like_call(init, vue_import_aliases)) {
        return "setup-ref";
    }
    "setup-maybe-ref"
}

pub(crate) fn vue3_ts_enum_binding_type(declaration: &TSEnumDeclaration<'_>) -> &'static str {
    if vue3_ts_enum_is_static_literal(declaration) {
        "literal-const"
    } else {
        "setup-const"
    }
}

pub(crate) fn vue3_ts_enum_is_static_literal(declaration: &TSEnumDeclaration<'_>) -> bool {
    declaration
        .body
        .members
        .iter()
        .all(|member| member.initializer.as_ref().is_none_or(vue3_is_static_node))
}

pub(crate) fn vue3_is_static_node(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::UnaryExpression(expression) => vue3_is_static_node(&expression.argument),
        Expression::LogicalExpression(expression) => {
            vue3_is_static_node(&expression.left) && vue3_is_static_node(&expression.right)
        }
        Expression::BinaryExpression(expression) => {
            vue3_is_static_node(&expression.left) && vue3_is_static_node(&expression.right)
        }
        Expression::ConditionalExpression(expression) => {
            vue3_is_static_node(&expression.test)
                && vue3_is_static_node(&expression.consequent)
                && vue3_is_static_node(&expression.alternate)
        }
        Expression::SequenceExpression(expression) => {
            expression.expressions.iter().all(vue3_is_static_node)
        }
        Expression::TemplateLiteral(expression) => {
            expression.expressions.iter().all(vue3_is_static_node)
        }
        Expression::ParenthesizedExpression(expression) => {
            vue3_is_static_node(&expression.expression)
        }
        Expression::TSAsExpression(expression) => vue3_is_static_node(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => {
            vue3_is_static_node(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => vue3_is_static_node(&expression.expression),
        Expression::TSNonNullExpression(expression) => vue3_is_static_node(&expression.expression),
        Expression::TSInstantiationExpression(expression) => {
            vue3_is_static_node(&expression.expression)
        }
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::BigIntLiteral(_) => true,
        _ => false,
    }
}

pub(crate) fn analyze_vue3_normal_script_for_setup(
    descriptor: &SfcDescriptor,
) -> Vue3NormalScriptAnalysis {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue3NormalScriptAnalysis::default();
    };
    let moved_after_setup = descriptor
        .script_setup
        .as_ref()
        .is_some_and(|script_setup| script.content_start > script_setup.content_start);
    if !script_lang_is_js_like(&script.attrs) {
        return Vue3NormalScriptAnalysis {
            module_content: script.content.clone(),
            moved_after_setup,
            ..Vue3NormalScriptAnalysis::default()
        };
    }
    let source = script.content.as_str();
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        source,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue3NormalScriptAnalysis {
            module_content: source.to_string(),
            moved_after_setup,
            errors: parsed.errors.iter().map(ToString::to_string).collect(),
            ..Vue3NormalScriptAnalysis::default()
        };
    }

    let mut edits = SourceEdits::new(source);
    let mut analysis = Vue3NormalScriptAnalysis {
        moved_after_setup,
        ..Vue3NormalScriptAnalysis::default()
    };
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                analysis.has_default_export = true;
                analysis.has_default_export_name = default_export_has_name(declaration);
                rewrite_vue3_export_default("__default__", declaration, &mut edits);
            }
            Statement::ExportNamedDeclaration(declaration)
                if rewrite_vue3_compile_script_named_default_export(
                    source,
                    "__default__",
                    declaration,
                    &mut edits,
                ) =>
            {
                analysis.has_default_export = true;
            }
            _ => {}
        }
    }
    analysis.module_content = trim_trailing_blank_lines(&edits.apply()).to_string();
    analysis
}

pub(crate) fn rewrite_vue3_compile_script_named_default_export(
    input: &str,
    variable: &str,
    declaration: &ExportNamedDeclaration<'_>,
    edits: &mut SourceEdits,
) -> bool {
    let Some(specifier) = declaration
        .specifiers
        .iter()
        .find(|specifier| module_export_name(specifier.exported()) == Some("default"))
    else {
        return false;
    };

    if export_named_declaration_only_exports_default(declaration) {
        edits.remove(
            declaration.span.start as usize,
            declaration.span.end as usize,
        );
    } else {
        let end = specifier_end(
            input,
            specifier.span.end as usize,
            declaration.span.end as usize,
        );
        edits.remove(specifier.span.start as usize, end);
    }

    let local_name = module_export_name(specifier.local()).unwrap_or("default");
    if let Some(source) = declaration.source.as_ref() {
        let source_value = source.value.to_string();
        let local_source =
            &input[specifier.local().span().start as usize..specifier.local().span().end as usize];
        edits.prepend(format!(
            "import {{ {local_source} as {variable} }} from '{}'\n",
            source_value
        ));
    } else {
        edits.append(format!("\nconst {variable} = {local_name}\n"));
    }
    true
}

pub(crate) struct Vue3ScriptSetupExportOptions<'a> {
    pub(crate) filename: &'a str,
    pub(crate) is_ts: bool,
    pub(crate) is_prod: bool,
    pub(crate) inline_render: Option<&'a Vue3InlineTemplateRender>,
    pub(crate) css_vars_code: Option<&'a str>,
    pub(crate) emit_script_setup_marker: bool,
    pub(crate) gen_default_as: Option<&'a str>,
}

pub(crate) fn vue3_script_setup_export(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    bindings: &[Vue3ScriptSetupReturnBinding],
    script_bindings: &BTreeMap<String, String>,
    normal_script: &Vue3NormalScriptAnalysis,
    options: Vue3ScriptSetupExportOptions<'_>,
) -> String {
    let Vue3ScriptSetupExportOptions {
        filename,
        is_ts,
        is_prod,
        inline_render,
        css_vars_code,
        emit_script_setup_marker,
        gen_default_as,
    } = options;
    let export_prefix = vue3_script_setup_default_export_prefix(gen_default_as);
    let runtime_options = vue3_script_setup_runtime_options(
        filename,
        normal_script,
        setup_analysis,
        is_prod,
        inline_render,
    );
    let setup_params = vue3_script_setup_params(setup_analysis, inline_render.is_some());
    let setup_body = vue3_script_setup_body(
        setup_analysis,
        bindings,
        script_bindings,
        inline_render,
        css_vars_code,
        emit_script_setup_marker,
        is_ts,
    );
    if is_ts {
        let options_spread = setup_analysis
            .options_runtime
            .as_ref()
            .map(|options| format!("\n  ...{options},"))
            .unwrap_or_default();
        let spread = if normal_script.has_default_export {
            "\n  ...__default__,"
        } else {
            ""
        };
        return format!(
            "{export_prefix} /*@__PURE__*/_defineComponent({{{spread}{options_spread}{runtime_options}\n  {async_prefix}setup({setup_params}) {{\n{setup_body}\n}}\n\n}})",
            async_prefix = vue3_script_setup_async_prefix(setup_analysis),
        );
    }
    if normal_script.has_default_export || setup_analysis.options_runtime.is_some() {
        let default_arg = if normal_script.has_default_export {
            "__default__, "
        } else {
            ""
        };
        let options_arg = setup_analysis
            .options_runtime
            .as_ref()
            .map(|options| format!("{options}, "))
            .unwrap_or_default();
        format!(
            "{export_prefix} /*@__PURE__*/Object.assign({default_arg}{options_arg}{{{runtime_options}\n  {async_prefix}setup({setup_params}) {{\n{setup_body}\n}}\n\n}})",
            async_prefix = vue3_script_setup_async_prefix(setup_analysis),
        )
    } else {
        format!(
            "{export_prefix} {{{runtime_options}\n  {async_prefix}setup({setup_params}) {{\n{setup_body}\n}}\n\n}}",
            async_prefix = vue3_script_setup_async_prefix(setup_analysis),
        )
    }
}

pub(crate) fn vue3_script_setup_default_export_prefix(gen_default_as: Option<&str>) -> String {
    gen_default_as
        .map(|name| format!("const {name} ="))
        .unwrap_or_else(|| "export default".to_string())
}

pub(crate) fn vue3_script_setup_async_prefix(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> &'static str {
    if setup_analysis.has_top_level_await {
        "async "
    } else {
        ""
    }
}

pub(crate) fn vue3_script_setup_runtime_options(
    filename: &str,
    normal_script: &Vue3NormalScriptAnalysis,
    setup_analysis: &Vue3ScriptSetupAnalysis,
    is_prod: bool,
    inline_render: Option<&Vue3InlineTemplateRender>,
) -> String {
    let mut runtime_options = String::new();
    if !normal_script.has_default_export_name && should_infer_vue3_script_name(filename) {
        if let Some(name) = script_component_name(filename) {
            runtime_options.push_str(&format!("\n  __name: '{}',", escape_js_single(&name)));
        }
    }
    if inline_render.is_some_and(|render| render.ssr) {
        runtime_options.push_str("\n  __ssrInlineRender: true,");
    }
    if let Some(props) = vue3_script_setup_props_runtime(setup_analysis, is_prod) {
        runtime_options.push_str(&format!("\n  props: {},", props.trim()));
    }
    if let Some(emits) = vue3_script_setup_emits_runtime(setup_analysis) {
        runtime_options.push_str(&format!("\n  emits: {},", emits.trim()));
    }
    runtime_options
}

pub(crate) fn should_infer_vue3_script_name(filename: &str) -> bool {
    !filename.is_empty() && filename.replace('\\', "/") != "anonymous.vue"
}

pub(crate) fn vue3_script_setup_needs_merge_models(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> bool {
    !setup_analysis.models.is_empty()
        && (setup_analysis.props_runtime.is_some() || setup_analysis.emits_runtime.is_some())
}

pub(crate) fn vue3_script_setup_props_runtime(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    is_prod: bool,
) -> Option<String> {
    let props = setup_analysis.props_runtime.as_ref();
    let model_props = vue3_script_setup_model_props_runtime(&setup_analysis.models, is_prod);
    match (props, model_props) {
        (Some(props), Some(model_props)) => Some(format!(
            "/*@__PURE__*/_mergeModels({}, {})",
            props.trim(),
            model_props
        )),
        (Some(props), None) => Some(props.clone()),
        (None, Some(model_props)) => Some(model_props),
        (None, None) => None,
    }
}

pub(crate) fn vue3_script_setup_model_props_runtime(
    models: &[Vue3ModelDecl],
    is_prod: bool,
) -> Option<String> {
    if models.is_empty() {
        return None;
    }
    let mut entries = Vec::new();
    for model in models {
        entries.push(format!(
            "    \"{}\": {},",
            escape_js_double(&model.name),
            vue3_define_model_runtime_decl(model, is_prod)
        ));
        entries.push(format!(
            "    \"{}\": {{}},",
            escape_js_double(&vue3_model_modifiers_prop_name(&model.name))
        ));
    }
    Some(format!("{{\n{}\n  }}", entries.join("\n")))
}

pub(crate) fn vue3_define_model_runtime_decl(model: &Vue3ModelDecl, is_prod: bool) -> String {
    let mut runtime_types = model.runtime_types.clone();
    let has_runtime_options = model.prop_runtime.is_some();
    let mut skip_check = false;
    let mut codegen_options = String::new();

    if let Some(types) = runtime_types.as_mut() {
        let has_boolean = types.iter().any(|ty| ty == "Boolean");
        let has_function = types.iter().any(|ty| ty == "Function");
        let has_unknown = types.iter().any(|ty| ty == "Unknown");

        if has_unknown {
            if has_boolean || has_function {
                types.retain(|ty| ty != "Unknown");
                skip_check = true;
            } else {
                types.clear();
                types.push("null".to_string());
            }
        }

        if !is_prod {
            codegen_options = format!("type: {}", vue27_runtime_type_string(types));
            if skip_check {
                codegen_options.push_str(", skipCheck: true");
            }
        } else if has_boolean || (has_runtime_options && has_function) {
            codegen_options = format!("type: {}", vue27_runtime_type_string(types));
        }
    }

    match (codegen_options.is_empty(), model.prop_runtime.as_deref()) {
        (false, Some(runtime_options)) => {
            format!("{{ {codegen_options}, ...{runtime_options} }}")
        }
        (false, None) => format!("{{ {codegen_options} }}"),
        (true, Some(runtime_options)) => runtime_options.to_string(),
        (true, None) => "{}".to_string(),
    }
}

pub(crate) fn vue3_script_setup_emits_runtime(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> Option<String> {
    let emits = setup_analysis.emits_runtime.as_ref();
    let model_emits = vue3_script_setup_model_emits_runtime(&setup_analysis.models);
    match (emits, model_emits) {
        (Some(emits), Some(model_emits)) => Some(format!(
            "/*@__PURE__*/_mergeModels({}, {})",
            emits.trim(),
            model_emits
        )),
        (Some(emits), None) => Some(emits.clone()),
        (None, Some(model_emits)) => Some(model_emits),
        (None, None) => None,
    }
}

pub(crate) fn vue3_script_setup_model_emits_runtime(models: &[Vue3ModelDecl]) -> Option<String> {
    if models.is_empty() {
        return None;
    }
    Some(format!(
        "[{}]",
        models
            .iter()
            .map(|model| format!("\"update:{}\"", escape_js_double(&model.name)))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

pub(crate) fn vue3_model_modifiers_prop_name(name: &str) -> String {
    if name == "modelValue" {
        "modelModifiers".to_string()
    } else {
        format!("{name}Modifiers")
    }
}

pub(crate) fn vue3_script_setup_params(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    inline_template: bool,
) -> String {
    let props = if setup_analysis.props_type_runtime {
        "__props: any"
    } else {
        "__props"
    };
    let mut context_parts = Vec::new();
    if setup_analysis.has_define_expose || !inline_template {
        context_parts.push("expose: __expose");
    }
    if setup_analysis.emit_binding.is_some() {
        context_parts.push("emit: __emit");
    }
    if context_parts.is_empty() {
        props.to_string()
    } else {
        format!("{props}, {{ {} }}", context_parts.join(", "))
    }
}

pub(crate) fn vue3_script_setup_body(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    bindings: &[Vue3ScriptSetupReturnBinding],
    script_bindings: &BTreeMap<String, String>,
    inline_render: Option<&Vue3InlineTemplateRender>,
    css_vars_code: Option<&str>,
    emit_script_setup_marker: bool,
    is_ts: bool,
) -> String {
    let mut returned_binding_types = script_bindings.clone();
    returned_binding_types.extend(setup_analysis.setup_bindings.clone());
    let returned = script_setup_returned_bindings(bindings, &returned_binding_types);
    let mut body = String::new();
    if inline_render.is_none() && !setup_analysis.has_define_expose {
        body.push_str("  __expose();\n");
    }
    if setup_analysis.has_top_level_await {
        if !body.is_empty() && !body.ends_with("\n\n") {
            body.push('\n');
        }
        if is_ts {
            body.push_str("let __temp: any, __restore: any\n");
        } else {
            body.push_str("let __temp, __restore\n");
        }
    }
    let has_css_vars_code = css_vars_code.is_some();
    if let Some(css_vars_code) = css_vars_code {
        body.push('\n');
        body.push_str(css_vars_code);
        body.push_str("\n\n");
    }
    if setup_analysis.setup_content.is_empty() {
        if !has_css_vars_code {
            body.push('\n');
        }
    } else {
        let setup_content = if has_css_vars_code {
            setup_analysis
                .setup_content
                .strip_prefix('\n')
                .unwrap_or(&setup_analysis.setup_content)
        } else {
            &setup_analysis.setup_content
        };
        body.push_str(setup_content);
        if !setup_content.ends_with('\n') {
            body.push('\n');
        }
    }
    if let Some(render) = inline_render {
        body.push_str("return ");
        body.push_str(&render.code);
        return body;
    }
    body.push_str(vue3_return_separator(setup_analysis, &body));
    if emit_script_setup_marker {
        body.push_str(&format!(
            "const __returned__ = {returned}\nObject.defineProperty(__returned__, '__isScriptSetup', {{ enumerable: false, value: true }})\nreturn __returned__"
        ));
    } else {
        body.push_str(&format!("return {returned}"));
    }
    body
}

pub(crate) fn vue3_return_separator(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    setup_body: &str,
) -> &'static str {
    if !setup_analysis.setup_content.starts_with('\n') {
        return "";
    }
    if setup_body.is_empty() {
        return "\n";
    }
    if setup_body.chars().all(|ch| matches!(ch, '\n' | '\r')) {
        return "";
    }
    if !setup_body.ends_with('\n') {
        return "\n";
    }
    let without_trailing_newlines = setup_body.trim_end_matches(['\n', '\r']);
    let Some(last_line) = without_trailing_newlines.rsplit('\n').next() else {
        return "";
    };
    if last_line.trim().is_empty() {
        ""
    } else {
        "\n"
    }
}

pub(crate) fn script_setup_returned_bindings(
    bindings: &[Vue3ScriptSetupReturnBinding],
    setup_bindings: &BTreeMap<String, String>,
) -> String {
    let returned = bindings
        .iter()
        .filter(|binding| {
            !binding.name.starts_with("import:") && !binding.name.starts_with("export:")
        })
        .map(|binding| vue3_script_setup_return_binding_source(binding, setup_bindings))
        .collect::<Vec<_>>()
        .join(", ");
    if returned.is_empty() {
        "{  }".to_string()
    } else {
        format!("{{ {returned} }}")
    }
}

pub(crate) fn vue3_script_setup_return_binding_source(
    binding: &Vue3ScriptSetupReturnBinding,
    setup_bindings: &BTreeMap<String, String>,
) -> String {
    match &binding.kind {
        Vue3ScriptSetupReturnBindingKind::Import { source }
            if source != "vue" && !source.ends_with(".vue") =>
        {
            format!("get {0}() {{ return {0} }}", binding.name)
        }
        _ if setup_bindings
            .get(&binding.name)
            .is_some_and(|binding_type| binding_type == "setup-let") =>
        {
            let set_arg = if binding.name == "v" { "_v" } else { "v" };
            format!(
                "get {0}() {{ return {0} }}, set {0}({1}) {{ {0} = {1} }}",
                binding.name, set_arg
            )
        }
        _ => binding.name.clone(),
    }
}

pub(crate) fn append_vue3_module_chunk(output: &mut String, chunk: &str) {
    let chunk = trim_trailing_blank_lines(chunk);
    if chunk.is_empty() {
        return;
    }
    if !output.is_empty() && !output.ends_with('\n') && !output_has_pending_blank_line(output) {
        output.push('\n');
    }
    output.push_str(chunk);
}

pub(crate) fn append_vue3_export_chunk(output: &mut String, chunk: &str) {
    let chunk = trim_trailing_blank_lines(chunk);
    if chunk.is_empty() {
        return;
    }
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str(chunk);
}

pub(crate) fn ensure_vue3_moved_normal_script_gap_before_export(output: &mut String) {
    if output.is_empty() {
        return;
    }
    if output.ends_with('\n') {
        output.push('\n');
    } else {
        output.push_str("\n\n");
    }
}

pub(crate) fn vue3_removed_setup_import_leading_padding(
    source: &str,
    statement: &Statement<'_>,
) -> Option<String> {
    let start = statement.span().start as usize;
    let leading = source.get(..start)?;
    if leading.is_empty() || !leading.trim().is_empty() {
        return None;
    }
    Some(leading.to_string())
}

pub(crate) fn vue3_trailing_blank_line_padding(value: &str) -> Option<&str> {
    let line_start = value.rfind('\n')?;
    let trailing = &value[line_start..];
    trailing.trim().is_empty().then_some(trailing)
}

pub(crate) fn vue3_script_setup_needs_blank_before_export(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> bool {
    setup_analysis.setup_content.starts_with('\n')
        || (!setup_analysis.setup_content.is_empty()
            && setup_analysis.setup_content.trim().is_empty()
            && setup_analysis.setup_content.contains('\n'))
}

pub(crate) fn ensure_vue3_blank_line_before_export(output: &mut String) {
    if output.ends_with("\n\n") || output_has_pending_blank_line(output) {
        return;
    }
    if output.ends_with('\n') {
        output.push('\n');
    } else {
        output.push_str("\n\n");
    }
}

pub(crate) fn script_component_name(filename: &str) -> Option<String> {
    std::path::Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToOwned::to_owned)
}

pub(crate) fn quoted_import_path(source: &str) -> Option<&str> {
    let start = source.find(['"', '\''])?;
    let quote = source[start..].chars().next()?;
    let rest = &source[start + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(&rest[..end])
}

pub(crate) fn side_effect_tag_errors(source: &str) -> Vec<SfcTemplateError> {
    side_effect_tag_ranges(source)
        .into_iter()
        .filter_map(|(start, end, _)| {
            let start_pos = position_at(source, start)?;
            let end_pos = position_at(source, end)?;
            Some(SfcTemplateError {
                code: 64,
                message: "Tags with side effect (<script> and <style>) are ignored in client component templates.".into(),
                loc: SfcSourceLocation {
                    start: start_pos,
                    end: end_pos,
                    source: source[start..end].to_string(),
                },
            })
        })
        .collect()
}

pub(crate) fn side_effect_tag_ranges(source: &str) -> Vec<(usize, usize, &'static str)> {
    let mut ranges = Vec::new();
    for tag in ["script", "style"] {
        let mut cursor = 0usize;
        while let Some(start_offset) = source[cursor..].find(&format!("<{tag}")) {
            let start = cursor + start_offset;
            let Some(after_open_offset) = source[start..].find('>') else {
                break;
            };
            let after_open = start + after_open_offset + 1;
            let close_tag = format!("</{tag}>");
            let Some(close_offset) = source[after_open..].find(&close_tag) else {
                break;
            };
            let end = after_open + close_offset + close_tag.len();
            ranges.push((start, end, tag));
            cursor = end;
        }
    }
    ranges.sort_by_key(|(start, _, _)| *start);
    ranges
}
