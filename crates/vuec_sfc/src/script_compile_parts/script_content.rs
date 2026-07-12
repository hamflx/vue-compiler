pub(crate) fn script_content(
    context: &mut Vue3ScriptCompileContext<'_>,
    options: &SfcScriptCompileOptions,
    base_bindings: &BTreeMap<String, String>,
    script_errors: &[String],
    template_usage_index: Option<&TemplateUsageIndex>,
) -> GeneratedScriptContent {
    {
        let descriptor = context.descriptor();
        let raw_content = context.raw_content();
        let Some(script_setup) = descriptor.script_setup.as_ref() else {
            let content =
                vue3_normal_script_content(descriptor, raw_content, options, base_bindings);
            return GeneratedScriptContent {
                map: options
                    .source_map
                    .then(|| vue3_compile_script_source_map(descriptor, &content, None)),
                content,
                errors: script_errors.to_vec(),
                warnings: Vec::new(),
                bindings: BTreeMap::new(),
                props_aliases: BTreeMap::new(),
                imports: BTreeMap::new(),
                removed_bindings: BTreeSet::new(),
                deps: Vec::new(),
            };
        };
        if !script_lang_is_js_like(&script_setup.attrs) && script_errors.is_empty() {
            return GeneratedScriptContent {
                map: options
                    .source_map
                    .then(|| vue3_compile_script_source_map(descriptor, raw_content, None)),
                content: raw_content.to_string(),
                errors: Vec::new(),
                warnings: Vec::new(),
                bindings: BTreeMap::new(),
                props_aliases: BTreeMap::new(),
                imports: BTreeMap::new(),
                removed_bindings: BTreeSet::new(),
                deps: Vec::new(),
            };
        }
        if !script_errors.is_empty() {
            return GeneratedScriptContent {
                map: options
                    .source_map
                    .then(|| vue3_compile_script_source_map(descriptor, raw_content, None)),
                content: raw_content.to_string(),
                errors: script_errors.to_vec(),
                warnings: Vec::new(),
                bindings: BTreeMap::new(),
                props_aliases: BTreeMap::new(),
                imports: BTreeMap::new(),
                removed_bindings: BTreeSet::new(),
                deps: Vec::new(),
            };
        }
    }

    let setup_analysis = context.script_setup_analysis();
    let normal_script = context.normal_script_analysis();
    let normal_script_return_bindings = context.normal_script_return_bindings();
    let script_binding_metadata = context.script_binding_metadata(&setup_analysis);
    let descriptor = context.descriptor();
    let filename = context.filename();
    let script_setup = descriptor
        .script_setup
        .as_ref()
        .expect("vue 3 script setup block");
    let is_ts = script_is_typescript(&script_setup.attrs)
        || descriptor
            .script
            .as_ref()
            .is_some_and(|script| script_is_typescript(&script.attrs));
    let return_bindings = vue3_script_setup_return_bindings(
        descriptor,
        &normal_script_return_bindings,
        &setup_analysis,
        is_ts,
        template_usage_index,
    );
    let template_binding_metadata = vue3_script_setup_template_binding_metadata(
        &normal_script_return_bindings,
        base_bindings,
        &script_binding_metadata,
        &setup_analysis,
    );
    let imports = vue3_script_setup_import_metadata(
        descriptor,
        &normal_script_return_bindings,
        &setup_analysis,
        is_ts,
        options.inline_template,
        template_usage_index,
    );
    let template_props_aliases = vue3_script_setup_template_props_aliases(&setup_analysis);
    let public_props_aliases = vue3_script_setup_public_props_aliases(&setup_analysis);
    let inline_render = vue3_inline_template_render(
        descriptor,
        options,
        &template_binding_metadata,
        &template_props_aliases,
        is_ts,
    );
    let css_vars_code = vue3_script_setup_css_vars_code(
        descriptor,
        options,
        &template_binding_metadata,
        &template_props_aliases,
    );
    let mut content = String::new();
    let has_helper_import = if let Some(import) = vue3_script_setup_helper_import(
        &setup_analysis,
        options,
        is_ts,
        css_vars_code.is_some(),
        inline_render
            .as_ref()
            .is_some_and(|render| render.preamble.contains("unref as _unref")),
    ) {
        append_vue3_module_chunk(&mut content, &import);
        true
    } else {
        false
    };
    if let Some(render) = inline_render.as_ref() {
        append_vue3_module_chunk(&mut content, &render.preamble);
        if !render.preamble.is_empty()
            && (!setup_analysis.module_content.is_empty()
                || !normal_script.module_content.is_empty())
            && !content.ends_with("\n\n")
        {
            content.push_str("\n\n");
        }
    }
    append_vue3_module_chunk(&mut content, &setup_analysis.module_content);
    if content.is_empty()
        && !normal_script.module_content.is_empty()
        && setup_analysis.removed_leading_import_padding.is_some()
    {
        if let Some(padding) = vue3_trailing_blank_line_padding(&normal_script.module_content)
            .or(setup_analysis.removed_leading_import_padding.as_deref())
        {
            content.push_str(padding);
        }
    }
    append_vue3_module_chunk(&mut content, &normal_script.module_content);
    if normal_script.module_content.is_empty()
        && setup_analysis.module_content.is_empty()
        && setup_analysis.setup_content.starts_with('\n')
    {
        if content.is_empty() {
            content.push('\n');
        } else if inline_render
            .as_ref()
            .is_some_and(|render| !render.preamble.is_empty())
        {
            content.push_str("\n\n\n");
        } else {
            content.push_str("\n\n");
        }
    }
    let moved_normal_script_had_pending_blank =
        normal_script.moved_after_setup && output_has_pending_blank_line(&content);
    if !content.is_empty()
        && !content.trim().is_empty()
        && inline_render.is_none()
        && (vue3_script_setup_needs_blank_before_export(&setup_analysis)
            || (has_helper_import
                && setup_analysis.module_content.is_empty()
                && normal_script.module_content.is_empty()))
    {
        ensure_vue3_blank_line_before_export(&mut content);
    }
    if normal_script.moved_after_setup
        && inline_render.is_none()
        && !normal_script.module_content.is_empty()
        && (normal_script.has_default_export || moved_normal_script_had_pending_blank)
    {
        ensure_vue3_moved_normal_script_gap_before_export(&mut content);
    }
    if content.is_empty()
        && setup_analysis.module_content.is_empty()
        && normal_script.module_content.is_empty()
        && !setup_analysis.setup_content.starts_with('\n')
        && descriptor.script.is_none()
    {
        content.push('\n');
    }
    let export = vue3_script_setup_export(
        &setup_analysis,
        &return_bindings,
        &script_binding_metadata,
        &normal_script,
        Vue3ScriptSetupExportOptions {
            filename,
            is_ts,
            is_prod: options.is_prod,
            inline_render: inline_render.as_ref(),
            css_vars_code: css_vars_code.as_deref(),
            emit_script_setup_marker: options.emit_script_setup_marker,
            gen_default_as: options.gen_default_as.as_deref(),
        },
    );
    append_vue3_export_chunk(&mut content, &export);
    let mut bindings = BTreeMap::new();
    for import in normal_script_return_bindings
        .imports
        .iter()
        .chain(setup_analysis.imports.iter())
    {
        if !import.is_type {
            bindings.insert(
                import.local.clone(),
                vue3_script_import_binding_type(import).into(),
            );
        }
    }
    bindings.extend(script_binding_metadata);
    bindings.extend(setup_analysis.setup_bindings.clone());
    for prop in &setup_analysis.props_bindings {
        bindings
            .entry(prop.clone())
            .or_insert_with(|| "props".into());
    }
    let mut errors = normal_script.errors;
    errors.extend(setup_analysis.errors);
    if let Some(render) = inline_render.as_ref() {
        errors.extend(render.errors.clone());
    }
    let content = trim_trailing_blank_lines(&content).to_string();
    let map = options
        .source_map
        .then(|| vue3_compile_script_source_map(descriptor, &content, inline_render.as_ref()));
    GeneratedScriptContent {
        content,
        errors,
        warnings: setup_analysis.warnings,
        bindings,
        props_aliases: public_props_aliases,
        imports,
        removed_bindings: setup_analysis.removed_bindings,
        deps: setup_analysis.deps.to_vec(),
        map,
    }
}

pub(crate) fn vue3_script_setup_template_binding_metadata(
    normal_script_return_bindings: &Vue27ScriptReturnBindings,
    base_bindings: &BTreeMap<String, String>,
    script_bindings: &BTreeMap<String, String>,
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeMap<String, String> {
    let mut bindings = base_bindings.clone();
    for import in normal_script_return_bindings
        .imports
        .iter()
        .chain(setup_analysis.imports.iter())
    {
        if !import.is_type {
            bindings.insert(
                import.local.clone(),
                vue3_script_import_binding_type(import).into(),
            );
        }
    }
    bindings.extend(script_bindings.clone());
    bindings.extend(setup_analysis.setup_bindings.clone());
    for prop in &setup_analysis.props_bindings {
        bindings
            .entry(prop.clone())
            .or_insert_with(|| "props".into());
    }
    for removed in &setup_analysis.removed_bindings {
        bindings.remove(removed);
    }
    bindings
}

pub(crate) fn vue3_compile_script_source_map(
    descriptor: &SfcDescriptor,
    generated: &str,
    inline_render: Option<&Vue3InlineTemplateRender>,
) -> SourceMapArtifact {
    let source_name = descriptor.filename.replace('\\', "/");
    let mut builder = SourceMapBuilder::new().file(source_name.clone());
    builder.add_source_content(source_name.clone(), descriptor.source.clone());
    match (descriptor.script.as_ref(), descriptor.script_setup.as_ref()) {
        (Some(script), Some(script_setup)) if script_setup.content_start < script.content_start => {
            add_script_block_source_mappings(
                &mut builder,
                descriptor,
                script_setup,
                generated,
                &source_name,
            );
            add_script_block_source_mappings(
                &mut builder,
                descriptor,
                script,
                generated,
                &source_name,
            );
        }
        (script, script_setup) => {
            if let Some(script) = script {
                add_script_block_source_mappings(
                    &mut builder,
                    descriptor,
                    script,
                    generated,
                    &source_name,
                );
            }
            if let Some(script_setup) = script_setup {
                add_script_block_source_mappings(
                    &mut builder,
                    descriptor,
                    script_setup,
                    generated,
                    &source_name,
                );
            }
        }
    }
    if let Some(render) = inline_render {
        add_inline_template_source_mappings(
            &mut builder,
            descriptor,
            render,
            generated,
            &source_name,
        );
    }
    builder.build()
}

pub(crate) fn add_script_block_source_mappings(
    builder: &mut SourceMapBuilder,
    descriptor: &SfcDescriptor,
    block: &SfcBlock,
    generated: &str,
    source_name: &str,
) {
    let mut generated_cursor = 0usize;
    let mut source_line_start = 0usize;
    for line in block.content.split_inclusive('\n') {
        let line_without_newline = line.trim_end_matches(['\r', '\n']);
        let trimmed = line_without_newline.trim();
        if !trimmed.is_empty() {
            let leading = line_without_newline.find(trimmed).unwrap_or(0);
            if let Some(relative_generated_start) = generated[generated_cursor..].find(trimmed) {
                let generated_start = generated_cursor + relative_generated_start;
                for (char_offset, ch) in trimmed.char_indices() {
                    if ch.is_whitespace() {
                        continue;
                    }
                    let generated_offset = generated_start + char_offset;
                    let source_offset =
                        block.content_start + source_line_start + leading + char_offset;
                    if let Some((generated_line, generated_column)) =
                        utf16_line_column_for_byte_offset(generated, generated_offset)
                    {
                        builder.add_mapping(
                            generated_line,
                            generated_column,
                            Some(Span::new(
                                descriptor.source_file,
                                source_offset,
                                source_offset,
                            )),
                            Some(source_name.to_string()),
                        );
                    }
                }
                generated_cursor = generated_start + trimmed.len();
            }
        }
        source_line_start += line.len();
    }
}

pub(crate) fn add_inline_template_source_mappings(
    builder: &mut SourceMapBuilder,
    descriptor: &SfcDescriptor,
    render: &Vue3InlineTemplateRender,
    generated: &str,
    source_name: &str,
) {
    let Some(render_map) = render.map.as_ref() else {
        return;
    };
    let Some(render_start) = generated.find(&render.code) else {
        return;
    };
    let Some((render_start_line, render_start_column)) =
        utf16_zero_based_line_column_for_byte_offset(generated, render_start)
    else {
        return;
    };
    let Ok(source_map) = render_map.to_oxc_source_map() else {
        return;
    };
    for token in source_map.get_tokens() {
        let generated_line = render_start_line + token.get_dst_line() as usize;
        let generated_column = if token.get_dst_line() == 0 {
            render_start_column + token.get_dst_col() as usize
        } else {
            token.get_dst_col() as usize
        };
        let Some(absolute) = byte_offset_at_utf16_line_column(
            &descriptor.source,
            token.get_src_line() as usize + 1,
            token.get_src_col() as usize,
        ) else {
            continue;
        };
        let name = token
            .get_name_id()
            .and_then(|name_id| source_map.get_name(name_id).map(ToString::to_string));
        builder.add_named_mapping(
            generated_line + 1,
            generated_column,
            Some(Span::new(descriptor.source_file, absolute, absolute)),
            Some(source_name.to_string()),
            name,
        );
    }
}

pub(crate) fn utf16_line_column_for_byte_offset(
    source: &str,
    offset: usize,
) -> Option<(usize, usize)> {
    utf16_zero_based_line_column_for_byte_offset(source, offset)
        .map(|(line, column)| (line + 1, column))
}

pub(crate) fn utf16_zero_based_line_column_for_byte_offset(
    source: &str,
    offset: usize,
) -> Option<(usize, usize)> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let mut line = 0usize;
    let mut column = 0usize;
    for ch in source[..offset].chars() {
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += ch.len_utf16();
        }
    }
    Some((line, column))
}

pub(crate) fn vue3_inline_template_render(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
    binding_metadata: &BTreeMap<String, String>,
    props_aliases: &BTreeMap<String, String>,
    is_ts: bool,
) -> Option<Vue3InlineTemplateRender> {
    if !options.inline_template {
        return None;
    }
    let Some(template) = descriptor.template.as_ref() else {
        return Some(Vue3InlineTemplateRender {
            preamble: String::new(),
            code: "() => {}".into(),
            ssr: false,
            map: None,
            errors: Vec::new(),
        });
    };
    if template.attrs.src.is_some() {
        return Some(Vue3InlineTemplateRender {
            preamble: String::new(),
            code: "() => {}".into(),
            ssr: false,
            map: None,
            errors: Vec::new(),
        });
    }

    let scoped = descriptor.styles.iter().any(|style| style.attrs.scoped);
    let scope_id = scoped
        .then(|| vue3_compile_script_scope_id(options.id.as_deref()))
        .flatten();
    let mut core = Vue3CompilerOptions {
        prefix_identifiers: true,
        mode: "module".into(),
        hoist_static: true,
        cache_handlers: true,
        scope_id: scope_id.clone(),
        is_ts,
        source_map: options.source_map,
        source_map_source: options.source_map.then(|| descriptor.source.clone()),
        source_map_base_offset: 0,
        binding_metadata: binding_metadata.clone(),
        props_aliases: props_aliases.clone(),
        inline: true,
        ..Vue3CompilerOptions::default()
    };
    apply_dom_parser_defaults(&mut core);
    let template_source = TemplateSource {
        filename: descriptor.filename.clone(),
        source: template.content.clone(),
        file_id: descriptor.source_file,
        base_offset: template.content_start,
    };
    if options.inline_template_ssr {
        core.ssr_css_vars = vue3_inline_ssr_css_vars(descriptor, options);
        core.source_map = options.source_map;
        let result = compile_ssr(
            template_source,
            SsrCompilerOptions {
                core,
                scope_id,
                slotted: vue3_sfc_descriptor_has_slotted_styles(descriptor),
                slotted_is_explicit: true,
                mode_is_explicit: true,
                transform_asset_urls: true,
                asset_url_options: AssetUrlOptions::default(),
            },
        );
        let errors = result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(|diagnostic| format!("{:?}: {}", diagnostic.code, diagnostic.message))
            .collect();
        return Some(Vue3InlineTemplateRender {
            preamble: result.preamble,
            code: result.code,
            ssr: true,
            map: result.map,
            errors,
        });
    }
    let result = compile_dom(
        template_source,
        DomCompilerOptions {
            core,
            transform_asset_urls: true,
            asset_url_options: AssetUrlOptions::default(),
            ..DomCompilerOptions::default()
        },
    );
    let errors = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| format!("{:?}: {}", diagnostic.code, diagnostic.message))
        .collect();
    Some(Vue3InlineTemplateRender {
        preamble: result.preamble,
        code: result.code,
        ssr: false,
        map: result.map,
        errors,
    })
}

pub(crate) fn vue3_compile_script_scope_id(id: Option<&str>) -> Option<String> {
    id.map(|id| {
        if id.starts_with("data-v-") {
            id.to_string()
        } else {
            format!("data-v-{id}")
        }
    })
}

pub(crate) fn vue3_compile_script_short_id(id: Option<&str>) -> String {
    id.and_then(|id| id.strip_prefix("data-v-").or(Some(id)))
        .unwrap_or("")
        .to_string()
}

pub(crate) fn vue3_inline_ssr_css_vars(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
) -> Option<String> {
    let vars = vue3_css_vars(descriptor);
    if vars.is_empty() {
        return None;
    }
    let id = vue3_compile_script_short_id(options.id.as_deref());
    let entries = vars
        .iter()
        .map(|var| {
            let name = format!(
                ":--{}",
                gen_css_var_name_with_style(
                    &id,
                    var,
                    options.is_prod,
                    CssVarNameStyle::Vue3Escaped
                )
            );
            format!("\"{}\": ({})", escape_js_double(&name), var)
        })
        .collect::<Vec<_>>()
        .join(",\n  ");
    Some(format!("{{\n  {entries}\n}}"))
}

pub(crate) fn vue3_normal_script_content(
    descriptor: &SfcDescriptor,
    raw_content: &str,
    options: &SfcScriptCompileOptions,
    _base_bindings: &BTreeMap<String, String>,
) -> String {
    let Some(script) = descriptor.script.as_ref() else {
        return raw_content.to_string();
    };
    if !script_lang_is_js_like(&script.attrs) {
        return raw_content.to_string();
    }
    let css_vars = vue3_css_vars(descriptor);
    if css_vars.is_empty() && options.gen_default_as.is_none() {
        return raw_content.to_string();
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
        return raw_content.to_string();
    }

    let mut edits = SourceEdits::new(source);
    let default_export_name = options.gen_default_as.as_deref().unwrap_or("__default__");
    let mut has_default_export = false;
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                has_default_export = true;
                rewrite_vue3_export_default(default_export_name, declaration, &mut edits);
            }
            Statement::ExportNamedDeclaration(declaration)
                if rewrite_vue3_compile_script_named_default_export(
                    source,
                    default_export_name,
                    declaration,
                    &mut edits,
                ) =>
            {
                has_default_export = true;
            }
            _ => {}
        }
    }
    if !has_default_export {
        edits.append(format!("\nconst {default_export_name} = {{}}"));
    }

    let content = trim_trailing_blank_lines(&edits.apply()).to_string();
    if css_vars.is_empty() || options.inline_template_ssr {
        return content;
    }
    let css_vars_code = vue3_normal_script_css_vars_code(&css_vars, options, default_export_name);
    if options.gen_default_as.is_some() {
        format!("{content}{css_vars_code}")
    } else {
        format!("{content}{css_vars_code}\nexport default __default__")
    }
}

pub(crate) fn vue3_normal_script_css_vars_code(
    css_vars: &[String],
    options: &SfcScriptCompileOptions,
    default_export_name: &str,
) -> String {
    format!(
        "\nimport {{ useCssVars as _useCssVars }} from {}\nconst __injectCSSVars__ = () => {{\n{}}}\nconst __setup__ = {default_export_name}.setup\n{default_export_name}.setup = __setup__\n  ? (props, ctx) => {{ __injectCSSVars__();return __setup__(props, ctx) }}\n  : __injectCSSVars__\n",
        vue3_script_setup_helper_import_source(options),
        vue3_css_vars_code(css_vars, options, &BTreeMap::new(), &BTreeMap::new())
    )
}

pub(crate) fn vue3_script_setup_css_vars_code(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
    binding_metadata: &BTreeMap<String, String>,
    props_aliases: &BTreeMap<String, String>,
) -> Option<String> {
    if options.inline_template_ssr {
        return None;
    }
    let css_vars = vue3_css_vars(descriptor);
    if css_vars.is_empty() {
        return None;
    }
    Some(vue3_css_vars_code(
        &css_vars,
        options,
        binding_metadata,
        props_aliases,
    ))
}

pub(crate) fn vue3_css_vars_code(
    css_vars: &[String],
    options: &SfcScriptCompileOptions,
    binding_metadata: &BTreeMap<String, String>,
    props_aliases: &BTreeMap<String, String>,
) -> String {
    let id = vue3_compile_script_short_id(options.id.as_deref());
    let vars = css_vars
        .iter()
        .map(|var| {
            let name = gen_css_var_name_with_style(
                &id,
                var,
                options.is_prod,
                CssVarNameStyle::Vue3Escaped,
            );
            format!("\"{}\": ({})", name, var)
        })
        .collect::<Vec<_>>()
        .join(",\n  ");
    let expression = format!("{{\n  {vars}\n}}");
    let prefixed = vue3_css_vars_expression_code(&expression, binding_metadata, props_aliases);
    format!("_useCssVars(_ctx => ({prefixed}))")
}

pub(crate) fn vue3_css_vars_expression_code(
    expression: &str,
    binding_metadata: &BTreeMap<String, String>,
    props_aliases: &BTreeMap<String, String>,
) -> String {
    let mut metadata = serde_json::Map::new();
    for (name, kind) in binding_metadata {
        metadata.insert(name.clone(), json!(kind));
    }
    if !props_aliases.is_empty() {
        metadata.insert("__propsAliases".to_string(), json!(props_aliases));
    }
    let projection = process_expression_projection(&json!({
        "node": {
            "type": 4,
            "content": expression,
            "isStatic": false,
            "loc": {
                "start": { "offset": 0, "line": 1, "column": 1 },
                "end": { "offset": expression.len(), "line": 1, "column": expression.len() + 1 },
                "source": expression,
            }
        },
        "context": {
            "prefixIdentifiers": true,
            "inline": true,
            "isTS": false,
            "identifiers": {},
            "bindingMetadata": metadata,
        }
    }));
    vue3_projection_code(&projection).unwrap_or_else(|| expression.to_string())
}

pub(crate) fn vue3_projection_code(value: &Value) -> Option<String> {
    match value.get("kind").and_then(Value::as_str) {
        Some("simple") => value
            .get("content")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        Some("compound") => {
            let children = value.get("children")?.as_array()?;
            let mut code = String::new();
            for child in children {
                if let Some(source) = child.as_str() {
                    code.push_str(source);
                } else if let Some(source) = vue3_projection_code(child) {
                    code.push_str(&source);
                }
            }
            Some(code)
        }
        _ => value.as_str().map(ToOwned::to_owned),
    }
}

pub(crate) fn vue3_script_setup_template_props_aliases(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeMap<String, String> {
    setup_analysis.props_destructured_bindings.clone()
}

pub(crate) fn vue3_script_setup_public_props_aliases(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeMap<String, String> {
    setup_analysis
        .props_destructured_bindings
        .iter()
        .filter(|(local, public_key)| *local != *public_key)
        .map(|(local, public_key)| (local.clone(), public_key.clone()))
        .collect()
}

pub(crate) fn vue3_script_setup_helper_import(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    options: &SfcScriptCompileOptions,
    is_ts: bool,
    needs_css_vars: bool,
    inline_render_has_unref: bool,
) -> Option<String> {
    let mut helpers = Vec::new();
    if needs_css_vars {
        helpers.push("useCssVars as _useCssVars");
        if !inline_render_has_unref {
            helpers.push("unref as _unref");
        }
    }
    if setup_analysis.has_top_level_await {
        helpers.push("withAsyncContext as _withAsyncContext");
    }
    if !setup_analysis.models.is_empty() {
        helpers.push("useModel as _useModel");
    }
    if setup_analysis.needs_use_slots {
        helpers.push("useSlots as _useSlots");
    }
    if setup_analysis.needs_merge_defaults {
        helpers.push("mergeDefaults as _mergeDefaults");
    }
    if setup_analysis.props_destructured_rest_id.is_some() {
        helpers.push("createPropsRestProxy as _createPropsRestProxy");
    }
    if vue3_script_setup_needs_merge_models(setup_analysis) {
        helpers.push("mergeModels as _mergeModels");
    }
    if is_ts {
        helpers.push("defineComponent as _defineComponent");
    }
    if helpers.is_empty() {
        None
    } else {
        Some(format!(
            "import {{ {} }} from {}\n",
            helpers.join(", "),
            vue3_script_setup_helper_import_source(options)
        ))
    }
}

pub(crate) fn vue3_script_setup_helper_import_source(options: &SfcScriptCompileOptions) -> String {
    options
        .runtime_module_name
        .as_ref()
        .map(|source| format!("\"{}\"", escape_js_double(source)))
        .unwrap_or_else(|| "'vue'".to_string())
}

pub(crate) fn vue3_script_setup_return_bindings(
    descriptor: &SfcDescriptor,
    normal_script_return_bindings: &Vue27ScriptReturnBindings,
    setup_analysis: &Vue3ScriptSetupAnalysis,
    is_ts: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) -> Vec<Vue3ScriptSetupReturnBinding> {
    let mut bindings = Vec::new();
    for binding in &normal_script_return_bindings.bindings {
        push_unique_vue3_return_binding(
            &mut bindings,
            Vue3ScriptSetupReturnBinding {
                name: binding.clone(),
                kind: Vue3ScriptSetupReturnBindingKind::Local,
            },
        );
    }
    for binding in &setup_analysis.return_bindings {
        push_unique_vue3_return_binding(
            &mut bindings,
            Vue3ScriptSetupReturnBinding {
                name: binding.clone(),
                kind: Vue3ScriptSetupReturnBindingKind::Local,
            },
        );
    }
    for import in normal_script_return_bindings
        .imports
        .iter()
        .chain(setup_analysis.imports.iter())
    {
        if import.is_type {
            continue;
        }
        if vue3_script_setup_import_is_returned(descriptor, import, is_ts, template_usage_index) {
            push_unique_vue3_return_binding(
                &mut bindings,
                Vue3ScriptSetupReturnBinding {
                    name: import.local.clone(),
                    kind: Vue3ScriptSetupReturnBindingKind::Import {
                        source: import.source.clone(),
                    },
                },
            );
        }
    }
    bindings
}

pub(crate) fn vue3_script_setup_import_metadata(
    descriptor: &SfcDescriptor,
    normal_script_return_bindings: &Vue27ScriptReturnBindings,
    setup_analysis: &Vue3ScriptSetupAnalysis,
    is_ts: bool,
    inline_template: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) -> BTreeMap<String, SfcScriptImportBinding> {
    let mut imports = BTreeMap::new();
    for import in &normal_script_return_bindings.imports {
        vue3_insert_script_import_metadata(
            &mut imports,
            descriptor,
            import,
            false,
            is_ts,
            inline_template,
            template_usage_index,
        );
    }
    for import in &setup_analysis.imports {
        vue3_insert_script_import_metadata(
            &mut imports,
            descriptor,
            import,
            true,
            is_ts,
            inline_template,
            template_usage_index,
        );
    }
    imports
}

pub(crate) fn vue3_insert_script_import_metadata(
    imports: &mut BTreeMap<String, SfcScriptImportBinding>,
    descriptor: &SfcDescriptor,
    import: &Vue27ScriptImport,
    is_from_setup: bool,
    is_ts: bool,
    inline_template: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) {
    imports.entry(import.local.clone()).or_insert_with(|| {
        let is_used_in_template = vue3_script_import_is_used_in_template(
            descriptor,
            &import.local,
            is_ts,
            inline_template,
            template_usage_index,
        );
        SfcScriptImportBinding {
            is_type: import.is_type,
            imported: import.imported.clone(),
            local: import.local.clone(),
            source: import.source.clone(),
            is_from_setup,
            is_used_in_template,
        }
    });
}

pub(crate) fn vue3_script_import_is_used_in_template(
    descriptor: &SfcDescriptor,
    local: &str,
    is_ts: bool,
    inline_template: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) -> bool {
    if inline_template {
        return false;
    }
    if !is_ts {
        return true;
    }
    let Some(template) = descriptor.template.as_ref() else {
        return true;
    };
    if template.attrs.src.is_some() || template.attrs.lang.is_some() {
        return true;
    }
    template_usage_index
        .map(|index| index.contains(local))
        .unwrap_or_else(|| vue3_template_uses_identifier(&template.content, local, is_ts))
}

pub(crate) fn push_unique_vue3_return_binding(
    bindings: &mut Vec<Vue3ScriptSetupReturnBinding>,
    binding: Vue3ScriptSetupReturnBinding,
) {
    if bindings
        .iter()
        .any(|existing| existing.name == binding.name)
    {
        return;
    }
    bindings.push(binding);
}

pub(crate) fn vue3_script_setup_import_is_returned(
    descriptor: &SfcDescriptor,
    import: &Vue27ScriptImport,
    is_ts: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) -> bool {
    if import.source == "vue" {
        return true;
    }
    let Some(template) = descriptor.template.as_ref() else {
        return true;
    };
    if template.attrs.src.is_some() || template.attrs.lang.is_some() {
        return true;
    }
    template_usage_index
        .map(|index| index.contains(&import.local))
        .unwrap_or_else(|| vue3_template_uses_identifier(&template.content, &import.local, is_ts))
}

pub(crate) fn vue3_script_import_binding_type(import: &Vue27ScriptImport) -> &'static str {
    if import.imported == "*"
        || (import.imported == "default" && import.source.ends_with(".vue"))
        || import.source == "vue"
    {
        "setup-const"
    } else {
        "setup-maybe-ref"
    }
}

pub(crate) fn vue3_script_block_return_bindings(block: &SfcBlock) -> Vue27ScriptReturnBindings {
    if !script_lang_is_js_like(&block.attrs) {
        return Vue27ScriptReturnBindings::default();
    }
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &block.content,
        script_source_type_from_attrs(&block.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27ScriptReturnBindings::default();
    }
    let mut result = Vue27ScriptReturnBindings::default();
    for statement in &parsed.program.body {
        collect_vue27_top_level_script_return_binding(statement, &mut result);
    }
    result
}

pub(crate) fn vue3_script_compile_errors(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(script_setup) = descriptor.script_setup.as_ref() else {
        if let Some(script) = descriptor.script.as_ref() {
            errors.extend(vue3_deprecated_import_assert_syntax_errors(
                script,
                options.allow_deprecated_import_assert_syntax,
            ));
        }
        return errors;
    };
    if descriptor
        .script
        .as_ref()
        .is_some_and(|script| script.attrs.lang != script_setup.attrs.lang)
    {
        return vec!["<script> and <script setup> must have the same language type.".to_string()];
    }
    if !script_lang_is_js_like(&script_setup.attrs) {
        return errors;
    }
    if let Some(script) = descriptor.script.as_ref() {
        errors.extend(vue3_deprecated_import_assert_syntax_errors(
            script,
            options.allow_deprecated_import_assert_syntax,
        ));
    }
    errors.extend(vue3_deprecated_import_assert_syntax_errors(
        script_setup,
        options.allow_deprecated_import_assert_syntax,
    ));
    errors.extend(vue3_script_setup_module_export_errors(script_setup));
    errors
}

pub(crate) fn vue3_deprecated_import_assert_syntax_errors(
    block: &SfcBlock,
    allow_deprecated_import_assert_syntax: bool,
) -> Vec<String> {
    if allow_deprecated_import_assert_syntax || !script_lang_is_js_like(&block.attrs) {
        return Vec::new();
    }
    let source = block.content.as_str();
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        source,
        script_source_type_from_attrs(&block.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vec::new();
    }
    parsed
        .program
        .body
        .iter()
        .filter_map(|statement| match statement {
            Statement::ImportDeclaration(declaration)
                if declaration
                    .with_clause
                    .as_ref()
                    .is_some_and(|clause| clause.keyword == WithClauseKeyword::Assert) =>
            {
                Some("The `assert` keyword in import attributes is deprecated. Use `with` instead, or enable the importAttributes parser plugin with deprecatedAssertSyntax.".to_string())
            }
            Statement::ExportNamedDeclaration(declaration)
                if declaration
                    .with_clause
                    .as_ref()
                    .is_some_and(|clause| clause.keyword == WithClauseKeyword::Assert) =>
            {
                Some("The `assert` keyword in export attributes is deprecated. Use `with` instead, or enable the importAttributes parser plugin with deprecatedAssertSyntax.".to_string())
            }
            Statement::ExportAllDeclaration(declaration)
                if declaration
                    .with_clause
                    .as_ref()
                    .is_some_and(|clause| clause.keyword == WithClauseKeyword::Assert) =>
            {
                Some("The `assert` keyword in export attributes is deprecated. Use `with` instead, or enable the importAttributes parser plugin with deprecatedAssertSyntax.".to_string())
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn vue3_script_setup_module_export_errors(script_setup: &SfcBlock) -> Vec<String> {
    let source = script_setup.content.as_str();
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        source,
        script_source_type_from_attrs(&script_setup.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vec::new();
    }
    parsed
        .program
        .body
        .iter()
        .filter_map(|statement| match statement {
            Statement::ExportNamedDeclaration(declaration)
                if declaration.export_kind != ImportOrExportKind::Type =>
            {
                Some(vue27_script_setup_module_export_error())
            }
            Statement::ExportAllDeclaration(declaration)
                if declaration.export_kind != ImportOrExportKind::Type =>
            {
                Some(vue27_script_setup_module_export_error())
            }
            Statement::ExportDefaultDeclaration(_) => {
                Some(vue27_script_setup_module_export_error())
            }
            _ => None,
        })
        .collect()
}
