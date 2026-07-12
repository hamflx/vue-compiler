use crate::*;

pub(crate) fn style_src_dependency(style: &SfcBlock) -> Vec<String> {
    style.attrs.src.iter().cloned().collect()
}

pub(crate) fn style_import_dependencies(style: &SfcBlock) -> Vec<String> {
    let mut dependencies = Vec::new();
    for line in style.content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("@import") {
            continue;
        }
        if let Some(dep) = quoted_import_path(trimmed) {
            dependencies.push(dep.to_string());
        }
    }
    dependencies
}

pub(crate) fn descriptor_css_vars(
    descriptor: &SfcDescriptor,
    options: CssVarCollectOptions,
) -> Vec<String> {
    let mut vars = Vec::new();
    for style in &descriptor.styles {
        for var in collect_css_vars_with_options(&style.content, options) {
            if !vars.iter().any(|existing| existing == &var) {
                vars.push(var);
            }
        }
    }
    vars
}

pub(crate) fn vue3_css_vars(descriptor: &SfcDescriptor) -> Vec<String> {
    descriptor_css_vars(
        descriptor,
        CssVarCollectOptions {
            ignore_line_comments: true,
        },
    )
}

pub(crate) fn add_style_block_mappings(
    builder: &mut SourceMapBuilder,
    descriptor: &SfcDescriptor,
    style: &SfcBlock,
    generated_code: &str,
    generated_line_offset: u32,
) {
    if generated_code.is_empty() {
        return;
    }
    let original_line_starts = style_line_starts(&style.content);
    let generated_lines = generated_line_count(generated_code).max(1);
    for generated_line in 0..generated_lines {
        let local_start = original_line_starts
            .get(generated_line as usize)
            .copied()
            .unwrap_or_else(|| *original_line_starts.last().unwrap_or(&0));
        let absolute = style.content_start + local_start;
        builder.add_mapping(
            generated_line_offset as usize + generated_line as usize + 1,
            0,
            Some(Span::new(descriptor.source_file, absolute, absolute)),
            Some(descriptor.filename.clone()),
        );
    }
}

pub(crate) fn style_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(index + ch.len_utf8());
        }
    }
    starts
}

pub(crate) fn generated_line_count(source: &str) -> u32 {
    source.lines().count().max(1) as u32
}

pub(crate) fn vue27_normal_script_content(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
    css_vars: &[String],
    bindings: &BTreeMap<String, String>,
) -> String {
    let Some(script) = descriptor.script.as_ref() else {
        return String::new();
    };
    if css_vars.is_empty() {
        return script.content.clone();
    }
    let scope_id = vue27_scope_id(options.id.as_deref());
    let content = rewrite_vue27_default(
        &script.content,
        "__default__",
        Vue27RewriteDefaultOptions {
            typescript: script_is_typescript(&script.attrs),
            decorators: script_is_typescript(&script.attrs),
        },
    );
    format!(
        "{}{}\nexport default __default__",
        content,
        gen_vue27_normal_script_css_vars_code(css_vars, bindings, &scope_id, options.is_prod)
    )
}

pub(crate) fn vue27_script_setup_content(
    descriptor: &SfcDescriptor,
    script_setup: &SfcBlock,
    options: &SfcScriptCompileOptions,
    css_vars: &[String],
    bindings: &BTreeMap<String, String>,
    analysis: &Vue27ScriptSetupAnalysis,
    normal_script: &Vue27NormalScriptAnalysis,
    normal_script_return_bindings: &Vue27ScriptReturnBindings,
    template_usage_index: Option<&TemplateUsageIndex>,
) -> String {
    let scope_id = vue27_scope_id(options.id.as_deref());
    let is_ts = script_is_typescript(&script_setup.attrs);
    let css_vars_code = if css_vars.is_empty() {
        String::new()
    } else {
        format!(
            "\n{}\n",
            gen_vue27_css_vars_code(css_vars, bindings, &scope_id, options.is_prod)
        )
    };
    let return_bindings = vue27_script_setup_return_bindings(
        descriptor,
        normal_script_return_bindings,
        analysis,
        is_ts,
        template_usage_index,
    );
    let returned = if return_bindings.is_empty() {
        if options.emit_script_setup_marker {
            "{ __sfc: true, }".to_string()
        } else {
            "{  }".to_string()
        }
    } else if options.emit_script_setup_marker {
        format!("{{ __sfc: true,{} }}", return_bindings.join(", "))
    } else {
        format!("{{ {} }}", return_bindings.join(", "))
    };
    let helper_import = if css_vars.is_empty() {
        ""
    } else {
        "import { useCssVars as _useCssVars } from 'vue'\n"
    };
    let helper_import = if analysis.needs_merge_defaults {
        format!("import {{ mergeDefaults as _mergeDefaults }} from 'vue'\n{helper_import}")
    } else {
        helper_import.to_string()
    };
    let runtime_options = vue27_script_setup_runtime_options(descriptor, analysis, normal_script);
    let setup_params = vue27_script_setup_params(analysis, is_ts);
    let setup_prefix = format!(
        "{}{}{}",
        css_vars_code, analysis.setup_prelude, analysis.setup_content
    );
    let return_separator = vue27_return_separator(&setup_prefix);
    let setup_body = format!("{setup_prefix}{return_separator}return {returned}");
    let export_prefix = vue27_script_setup_export_prefix(
        normal_script,
        &runtime_options,
        is_ts,
        &setup_params,
        &setup_body,
    );
    let helper_import = if is_ts {
        if analysis.needs_merge_defaults {
            helper_import.replace(
                "import { mergeDefaults as _mergeDefaults } from 'vue'\n",
                "import { mergeDefaults as _mergeDefaults, defineComponent as _defineComponent } from 'vue'\n",
            )
        } else {
            "import { defineComponent as _defineComponent } from 'vue'\n".to_string()
                + &helper_import
        }
    } else {
        helper_import
    };
    let normal_script_after_setup = descriptor
        .script
        .as_ref()
        .is_some_and(|script| script.content_start > script_setup.content_start);
    let mut content = helper_import;
    let mut first_module_chunk = true;
    if normal_script_after_setup {
        append_vue27_module_chunk(
            &mut content,
            &normal_script.module_content,
            first_module_chunk,
            false,
        );
        first_module_chunk = first_module_chunk && normal_script.module_content.is_empty();
        append_vue27_module_chunk(
            &mut content,
            &analysis.module_content,
            first_module_chunk,
            normal_script.has_default_export,
        );
    } else {
        append_vue27_module_chunk(
            &mut content,
            &analysis.module_content,
            first_module_chunk,
            false,
        );
        first_module_chunk = first_module_chunk && analysis.module_content.is_empty();
        append_vue27_module_chunk(
            &mut content,
            &normal_script.module_content,
            first_module_chunk,
            false,
        );
    }
    content.push_str(&export_prefix);
    content.trim().to_string()
}

pub(crate) fn append_vue27_module_chunk(
    output: &mut String,
    chunk: &str,
    first_module_chunk: bool,
    blank_between_plain_chunks: bool,
) {
    if chunk.is_empty() {
        return;
    }
    let chunk = if output.is_empty() {
        chunk
    } else {
        let mut chunk = chunk;
        let pending_blank = output_has_pending_blank_line(output);
        if chunk.starts_with('\n')
            && ((first_module_chunk && output.ends_with('\n')) || pending_blank)
        {
            chunk = &chunk[1..];
        }
        if !output.ends_with('\n') && !chunk.starts_with('\n') {
            output.push('\n');
            if !first_module_chunk && blank_between_plain_chunks && !pending_blank {
                output.push('\n');
            }
        } else if output.ends_with('\n')
            && !chunk.starts_with('\n')
            && !first_module_chunk
            && blank_between_plain_chunks
            && !pending_blank
        {
            output.push('\n');
        } else if !output.ends_with('\n')
            && chunk.starts_with('\n')
            && !first_module_chunk
            && blank_between_plain_chunks
            && !pending_blank
            && !chunk.starts_with("\n\n")
        {
            output.push_str("\n\n");
        }
        chunk
    };
    if first_module_chunk && output_has_pending_blank_line(output) {
        output.push_str(chunk.strip_prefix('\n').unwrap_or(chunk));
    } else {
        output.push_str(chunk);
    }
}

pub(crate) fn output_has_pending_blank_line(output: &str) -> bool {
    if output.is_empty() {
        return false;
    }
    let current = if let Some(without_final_newline) = output.strip_suffix('\n') {
        let line_start = without_final_newline
            .rfind('\n')
            .map_or(0, |index| index + 1);
        &without_final_newline[line_start..]
    } else {
        let line_start = output.rfind('\n').map_or(0, |index| index + 1);
        &output[line_start..]
    };
    current.trim().is_empty()
}

pub(crate) fn vue27_script_setup_runtime_options(
    descriptor: &SfcDescriptor,
    analysis: &Vue27ScriptSetupAnalysis,
    normal_script: &Vue27NormalScriptAnalysis,
) -> String {
    let mut runtime_options = String::new();
    if !normal_script.has_default_export_name {
        if let Some(name) = vue27_inferred_component_name(&descriptor.filename) {
            runtime_options.push_str(&format!("\n  __name: '{}',", escape_js_single(&name)));
        }
    }
    if let Some(props) = analysis.props_runtime.as_ref() {
        runtime_options.push_str(&format!("\n  props: {},", props.trim()));
    }
    if let Some(emits) = analysis.emits_runtime.as_ref() {
        runtime_options.push_str(&format!("\n  emits: {},", emits.trim()));
    }
    runtime_options
}

pub(crate) fn vue27_script_setup_return_bindings(
    descriptor: &SfcDescriptor,
    normal_script_return_bindings: &Vue27ScriptReturnBindings,
    analysis: &Vue27ScriptSetupAnalysis,
    is_ts: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) -> Vec<String> {
    let mut bindings = normal_script_return_bindings.bindings.clone();
    for value in &analysis.return_bindings {
        push_unique(&mut bindings, value);
    }
    for import in &normal_script_return_bindings.imports {
        if import.is_type {
            continue;
        }
        if vue27_script_setup_import_is_returned(descriptor, import, is_ts, template_usage_index) {
            push_unique(&mut bindings, &import.local);
        }
    }
    for import in &analysis.imports {
        if import.is_type {
            continue;
        }
        if vue27_script_setup_import_is_returned(descriptor, import, is_ts, template_usage_index) {
            push_unique(&mut bindings, &import.local);
        }
    }
    bindings
        .into_iter()
        .filter(|name| {
            !analysis
                .removed_bindings
                .iter()
                .any(|removed| removed == name)
        })
        .collect()
}

pub(crate) fn vue27_script_setup_import_is_returned(
    descriptor: &SfcDescriptor,
    import: &Vue27ScriptImport,
    is_ts: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) -> bool {
    let Some(template) = descriptor.template.as_ref() else {
        return true;
    };
    if template.attrs.src.is_some() || template.attrs.lang.is_some() {
        return true;
    }
    template_usage_index
        .map(|index| index.contains(&import.local))
        .unwrap_or_else(|| vue27_template_uses_identifier(&template.content, &import.local, is_ts))
}

pub(crate) fn vue27_script_setup_params(
    analysis: &Vue27ScriptSetupAnalysis,
    is_ts: bool,
) -> String {
    let props_param = if is_ts && analysis.props_type_runtime {
        "__props: any"
    } else {
        "__props"
    };
    let mut context_parts = Vec::new();
    if let Some(binding) = analysis.emit_binding.as_deref() {
        if binding == "emit" {
            context_parts.push("emit".to_string());
        } else {
            context_parts.push(format!("emit: {binding}"));
        }
    }
    if analysis.needs_expose {
        context_parts.push("expose".to_string());
    }
    if context_parts.is_empty() {
        props_param.to_string()
    } else if is_ts {
        if let Some(emit_type_source) = analysis.emit_type_source.as_deref() {
            format!(
                "{props_param}, {{ {} }}: {{ emit: ({emit_type_source}), expose: any, slots: any, attrs: any }}",
                context_parts.join(", ")
            )
        } else {
            format!("{props_param}, {{ {} }}", context_parts.join(", "))
        }
    } else {
        format!("{props_param}, {{ {} }}", context_parts.join(", "))
    }
}

pub(crate) fn vue27_return_separator(setup_prefix: &str) -> &'static str {
    if setup_prefix.is_empty() {
        return "\n\n\n\n";
    }
    if setup_prefix.chars().all(|ch| matches!(ch, '\n' | '\r')) {
        let newlines = setup_prefix.chars().filter(|ch| *ch == '\n').count();
        return if newlines <= 1 { "\n\n" } else { "\n" };
    }
    if !setup_prefix.ends_with('\n') {
        return "\n";
    }
    let without_trailing_newlines = setup_prefix.trim_end_matches(['\n', '\r']);
    let Some(last_line) = without_trailing_newlines.rsplit('\n').next() else {
        return "";
    };
    if last_line.trim().is_empty() {
        ""
    } else {
        "\n"
    }
}

pub(crate) fn vue27_script_setup_export_prefix(
    normal_script: &Vue27NormalScriptAnalysis,
    runtime_options: &str,
    is_ts: bool,
    setup_params: &str,
    setup_body: &str,
) -> String {
    if is_ts {
        let spread = if normal_script.has_default_export {
            "\n  ...__default__,"
        } else {
            ""
        };
        return format!(
            "\nexport default /*#__PURE__*/_defineComponent({{{spread}{runtime_options}\n  setup({setup_params}) {{\n{setup_body}\n}}\n\n}})"
        );
    }
    if normal_script.has_default_export {
        format!(
            "\nexport default /*#__PURE__*/Object.assign(__default__, {{{runtime_options}\n  setup({setup_params}) {{\n{setup_body}\n}}\n\n}})"
        )
    } else {
        format!("\nexport default {{{runtime_options}\n  setup({setup_params}) {{\n{setup_body}\n}}\n\n}}")
    }
}

pub(crate) fn vue27_inferred_component_name(filename: &str) -> Option<String> {
    if filename.is_empty() || filename == "anonymous.vue" {
        return None;
    }
    let name = filename
        .rsplit(['/', '\\'])
        .next()
        .and_then(|file| file.rsplit_once('.').map(|(stem, _)| stem))
        .filter(|stem| !stem.is_empty())?;
    Some(name.to_string())
}

pub(crate) fn escape_js_single(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

pub(crate) fn analyze_vue27_normal_script_for_setup(
    descriptor: &SfcDescriptor,
) -> Vue27NormalScriptAnalysis {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue27NormalScriptAnalysis::default();
    };
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
        return Vue27NormalScriptAnalysis {
            module_content: source.to_string(),
            ..Vue27NormalScriptAnalysis::default()
        };
    }

    let mut edits = SourceEdits::new(source);
    let mut named_default_exports = Vec::new();
    let mut analysis = Vue27NormalScriptAnalysis::default();
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                analysis.has_default_export = true;
                analysis.has_default_export_name = default_export_has_name(declaration);
                edits.overwrite(
                    declaration.span.start as usize,
                    declaration.declaration.span().start as usize,
                    "const __default__ = ",
                );
            }
            Statement::ExportNamedDeclaration(declaration)
                if rewrite_named_default_exports(
                    source,
                    "__default__",
                    declaration,
                    &mut edits,
                ) =>
            {
                analysis.has_default_export = true;
                if export_named_declaration_only_exports_default(declaration) {
                    named_default_exports.push((
                        declaration.span.start as usize,
                        declaration.span.end as usize,
                    ));
                }
            }
            _ => {}
        }
    }
    for (start, end) in named_default_exports {
        edits.remove(start, end);
    }
    analysis.module_content = trim_trailing_blank_lines(&edits.apply()).to_string();
    if analysis.module_content.starts_with('\n') {
        analysis.module_content.insert(0, '\n');
    }
    analysis
}

pub(crate) fn default_export_has_name(declaration: &ExportDefaultDeclaration<'_>) -> bool {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::ObjectExpression(object) => {
            object_expression_has_static_key(object, "name")
        }
        ExportDefaultDeclarationKind::CallExpression(call) => {
            call.arguments.first().is_some_and(|argument| {
                matches!(argument.to_expression(), Expression::ObjectExpression(object) if object_expression_has_static_key(object, "name"))
            })
        }
        _ => false,
    }
}

pub(crate) fn object_expression_has_static_key(object: &ObjectExpression<'_>, key: &str) -> bool {
    object
        .properties
        .iter()
        .filter_map(|property| property.as_property())
        .filter(|property| !property.computed)
        .any(|property| property.key.static_name().as_deref() == Some(key))
}

pub(crate) fn vue27_scope_id(id: Option<&str>) -> String {
    id.and_then(|id| id.strip_prefix("data-v-").or(Some(id)))
        .unwrap_or("")
        .to_string()
}

pub(crate) fn gen_vue27_normal_script_css_vars_code(
    css_vars: &[String],
    bindings: &BTreeMap<String, String>,
    id: &str,
    is_prod: bool,
) -> String {
    format!(
        "\nimport {{ useCssVars as _useCssVars }} from 'vue'\nconst __injectCSSVars__ = () => {{\n{}}}\nconst __setup__ = __default__.setup\n__default__.setup = __setup__\n  ? (props, ctx) => {{ __injectCSSVars__();return __setup__(props, ctx) }}\n  : __injectCSSVars__\n",
        gen_vue27_css_vars_code(css_vars, bindings, id, is_prod)
    )
}

pub(crate) fn gen_vue27_css_vars_code(
    css_vars: &[String],
    bindings: &BTreeMap<String, String>,
    id: &str,
    is_prod: bool,
) -> String {
    let vars = css_vars
        .iter()
        .map(|var| {
            format!(
                "\"{}\": ({})",
                gen_css_var_name_with_style(id, var, is_prod, CssVarNameStyle::Vue27Legacy),
                var
            )
        })
        .collect::<Vec<_>>()
        .join(",\n  ");
    let expression = format!("({{\n  {vars}\n}})");
    let prefixed = prefix_vue27_identifiers(
        &expression,
        Vue27PrefixIdentifiersOptions {
            is_functional: false,
            is_ts: false,
            bindings: bindings.clone(),
        },
    );
    format!("_useCssVars((_vm, _setup) => {prefixed})")
}
