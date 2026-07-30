use crate::*;

impl SfcCompiler {
    /// Creates a new SFC compiler facade.
    pub fn new() -> Self {
        Self {
            sources: SourceMap::default(),
            js: JsAstStore::new(),
            descriptor_cache: BTreeMap::new(),
            cache_stats: SfcCacheStats::default(),
        }
    }

    /// Parses an SFC descriptor using Vue 3-style descriptor rules.
    ///
    /// This compatibility helper discards descriptor diagnostics. New compile
    /// facades should retain [`Vue3SfcParseResult`] from [`Self::parse_vue3`]
    /// and use the corresponding `compile_parsed_vue3_*` method.
    pub fn parse(&mut self, filename: impl Into<String>, source: &str) -> SfcDescriptor {
        self.parse_vue3(filename, source).descriptor
    }

    /// Parses an SFC descriptor and returns Vue 3 public `parse()` diagnostics.
    pub fn parse_vue3(&mut self, filename: impl Into<String>, source: &str) -> Vue3SfcParseResult {
        self.parse_vue3_with_options(filename, source, Vue3SfcParseOptions::default())
    }

    /// Parses an SFC descriptor and returns Vue 3 public diagnostics with parse options.
    pub fn parse_vue3_with_options(
        &mut self,
        filename: impl Into<String>,
        source: &str,
        options: Vue3SfcParseOptions,
    ) -> Vue3SfcParseResult {
        let filename = filename.into();
        let mode = SfcParseCacheMode::Vue3 {
            pad: options.pad.clone(),
            ignore_empty: options.ignore_empty,
        };
        let key = SfcCacheKey::new(filename.clone(), source, mode);
        if let Some(entry) = self.descriptor_cache.get(&key) {
            self.cache_stats.descriptor_hits += 1;
            return Vue3SfcParseResult {
                descriptor: entry.descriptor.clone(),
                errors: entry.vue3_errors.clone(),
            };
        }
        self.invalidate_stale_descriptor_entries(&filename, &key.mode);
        self.cache_stats.descriptor_misses += 1;
        let source_file = self.sources.add_file(
            Some(std::path::PathBuf::from(&filename)),
            source.to_string(),
        );
        let extracted = extract_sfc_blocks(
            source,
            source_file,
            SfcBlockContentMode::Vue3 { options: &options },
        );
        let mut result =
            vue3_descriptor_from_blocks(filename, source, source_file, extracted.blocks, &options);
        if !extracted.vue3_errors.is_empty() {
            let mut errors = extracted.vue3_errors;
            errors.extend(result.errors);
            result.errors = errors;
        }
        let cached_errors = result.errors.clone();
        self.descriptor_cache.insert(
            key,
            SfcDescriptorCacheEntry {
                descriptor: result.descriptor.clone(),
                vue3_errors: cached_errors,
                vue27_errors: Vec::new(),
            },
        );
        result
    }

    /// Parses an anonymous Vue 2.7 SFC component.
    pub fn parse_vue27_component(
        &mut self,
        source: &str,
        options: Vue27ParseComponentOptions,
    ) -> Vue27ParseComponentResult {
        self.parse_vue27_component_with_filename("anonymous.vue", source, options)
    }

    /// Parses a named Vue 2.7 SFC component.
    pub fn parse_vue27_component_with_filename(
        &mut self,
        filename: impl Into<String>,
        source: &str,
        options: Vue27ParseComponentOptions,
    ) -> Vue27ParseComponentResult {
        let filename = filename.into();
        let mode = SfcParseCacheMode::Vue27 {
            pad: options.pad.clone(),
            deindent: options.deindent,
            output_source_range: options.output_source_range,
        };
        let key = SfcCacheKey::new(filename.clone(), source, mode);
        if let Some(entry) = self.descriptor_cache.get(&key) {
            self.cache_stats.descriptor_hits += 1;
            return Vue27ParseComponentResult {
                descriptor: entry.descriptor.clone(),
                errors: project_vue27_errors(
                    entry.vue27_errors.clone(),
                    options.output_source_range,
                ),
            };
        }
        self.invalidate_stale_descriptor_entries(&filename, &key.mode);
        self.cache_stats.descriptor_misses += 1;
        let source_file = self.sources.add_file(
            Some(std::path::PathBuf::from(&filename)),
            source.to_string(),
        );
        let extracted = extract_sfc_blocks(
            source,
            source_file,
            SfcBlockContentMode::Vue27 { options: &options },
        );
        let descriptor = descriptor_from_blocks(filename, source, source_file, extracted.blocks);
        let cached_errors = extracted.errors.clone();
        self.descriptor_cache.insert(
            key,
            SfcDescriptorCacheEntry {
                descriptor: descriptor.clone(),
                vue3_errors: Vec::new(),
                vue27_errors: cached_errors,
            },
        );

        Vue27ParseComponentResult {
            descriptor,
            errors: project_vue27_errors(extracted.errors, options.output_source_range),
        }
    }

    /// Compiles the descriptor's template block.
    pub fn compile_template(
        &self,
        descriptor: &SfcDescriptor,
        options: SfcTemplateCompileOptions,
    ) -> SfcTemplateCompileResult {
        let Some(template) = descriptor.template.as_ref() else {
            return SfcTemplateCompileResult {
                code: String::new(),
                map: None,
                errors: vec![SfcTemplateError {
                    code: 0,
                    message: "Template block is missing.".into(),
                    loc: SfcSourceLocation {
                        start: SfcPosition {
                            column: 1,
                            line: 1,
                            offset: 0,
                        },
                        end: SfcPosition {
                            column: 1,
                            line: 1,
                            offset: 0,
                        },
                        source: String::new(),
                    },
                }],
                bindings: Vec::new(),
                ast_summary: "missing-template".into(),
                ast: String::new(),
                preamble: String::new(),
                source: String::new(),
                tips: Vec::new(),
            };
        };
        let mut core = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            hoist_static: options.hoist_static,
            stringify_static: options.stringify_static,
            cache_handlers: true,
            scope_id: options.scope_id.clone(),
            slotted: options.slotted,
            source_map: options.source_map,
            ..Vue3CompilerOptions::default()
        };
        apply_dom_parser_defaults(&mut core);
        let source = TemplateSource {
            filename: descriptor.filename.clone(),
            source: template.content.clone(),
            file_id: descriptor.source_file,
            base_offset: template.content_start,
        };
        if options.ssr {
            let result = compile_ssr(
                source,
                SsrCompilerOptions {
                    core,
                    scope_id: options.scope_id.clone(),
                    slotted: options.slotted,
                    slotted_is_explicit: true,
                    mode_is_explicit: true,
                    transform_asset_urls: options.transform_asset_urls,
                    asset_url_options: options.asset_url_options.clone(),
                },
            );
            let ast_summary = result.ast_summary;
            SfcTemplateCompileResult {
                code: result.code,
                map: result.map,
                errors: sfc_template_errors_from_diagnostics(
                    &result.diagnostics,
                    &template.content,
                ),
                bindings: Vec::new(),
                ast_summary: ast_summary.clone(),
                ast: format!("ast:{ast_summary}"),
                preamble: result.preamble,
                source: template.content.clone(),
                tips: Vec::new(),
            }
        } else {
            let result = compile_dom(
                source,
                DomCompilerOptions {
                    core,
                    transform_asset_urls: options.transform_asset_urls,
                    asset_url_options: options.asset_url_options.clone(),
                    ..DomCompilerOptions::default()
                },
            );
            let ast_summary = result.ast_summary;
            SfcTemplateCompileResult {
                code: result.code,
                map: result.map,
                errors: sfc_template_errors_from_diagnostics(
                    &result.diagnostics,
                    &template.content,
                ),
                bindings: Vec::new(),
                ast_summary: ast_summary.clone(),
                ast: format!("ast:{ast_summary}"),
                preamble: result.preamble,
                source: template.content.clone(),
                tips: Vec::new(),
            }
        }
    }

    /// Compiles a parsed Vue 3 SFC template and preserves descriptor parse errors.
    pub fn compile_parsed_vue3_template(
        &self,
        parsed: &Vue3SfcParseResult,
        options: SfcTemplateCompileOptions,
    ) -> SfcTemplateCompileResult {
        let mut result = self.compile_template(&parsed.descriptor, options);
        result
            .errors
            .splice(0..0, vue3_sfc_parse_template_errors(parsed));
        result
    }

    /// Compiles standalone template source through the SFC template path.
    pub fn compile_template_source(
        &self,
        filename: impl Into<String>,
        source: &str,
        options: SfcTemplateCompileOptions,
    ) -> SfcTemplateCompileResult {
        let filename = filename.into();
        let raw_source = source.to_string();
        let side_effect_errors = side_effect_tag_errors(source);
        let mut core = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            hoist_static: options.hoist_static,
            stringify_static: options.stringify_static,
            cache_handlers: true,
            scope_id: options.scope_id.clone(),
            slotted: options.slotted,
            source_map: options.source_map,
            ..Vue3CompilerOptions::default()
        };
        apply_dom_parser_defaults(&mut core);
        let template_source = TemplateSource {
            filename: filename.clone(),
            source: raw_source.clone(),
            file_id: FileId(0),
            base_offset: 0,
        };
        if options.ssr {
            let result = compile_ssr(
                template_source,
                SsrCompilerOptions {
                    core,
                    scope_id: options.scope_id.clone(),
                    slotted: options.slotted,
                    slotted_is_explicit: true,
                    mode_is_explicit: true,
                    transform_asset_urls: options.transform_asset_urls,
                    asset_url_options: options.asset_url_options.clone(),
                },
            );
            return SfcTemplateCompileResult {
                code: result.code,
                map: result.map,
                errors: merge_template_errors(
                    side_effect_errors,
                    sfc_template_errors_from_diagnostics(&result.diagnostics, &raw_source),
                ),
                bindings: Vec::new(),
                ast_summary: result.ast_summary.clone(),
                ast: json!({
                    "type": 0,
                    "source": raw_source,
                    "transformed": true,
                })
                .to_string(),
                preamble: result.preamble,
                source: raw_source,
                tips: Vec::new(),
            };
        }
        let result = compile_dom(
            template_source,
            DomCompilerOptions {
                core,
                transform_asset_urls: options.transform_asset_urls,
                asset_url_options: options.asset_url_options,
                ..DomCompilerOptions::default()
            },
        );
        SfcTemplateCompileResult {
            code: result.code,
            map: result.map,
            errors: merge_template_errors(
                side_effect_errors,
                sfc_template_errors_from_diagnostics(&result.diagnostics, &raw_source),
            ),
            bindings: Vec::new(),
            ast_summary: result.ast_summary.clone(),
            ast: json!({
                "type": 0,
                "source": raw_source.clone(),
                "transformed": true,
            })
            .to_string(),
            preamble: result.preamble,
            source: raw_source,
            tips: Vec::new(),
        }
    }

    /// Compiles Vue 3 SFC script blocks.
    pub fn compile_script(
        &mut self,
        descriptor: &SfcDescriptor,
        options: SfcScriptCompileOptions,
    ) -> SfcScriptBlock {
        let mut context = Vue3ScriptCompileContext::new(descriptor, &options, &mut self.js);
        let script_compile_errors = context.script_compile_errors();
        let summary = if script_compile_errors.is_empty() && context.all_script_blocks_are_js_like()
        {
            self.js
                .summarize_program(context.raw_content(), context.source_type())
        } else {
            Default::default()
        };
        let attrs = context.script_or_setup_attrs();
        let base_bindings = context.base_binding_metadata();
        let template_usage_index =
            vue3_compile_script_template_usage_index(&mut context, &script_compile_errors);
        let generated_content = script_content(
            &mut context,
            &options,
            &base_bindings,
            &script_compile_errors,
            template_usage_index.as_ref(),
        );
        let mut bindings = base_bindings;
        bindings.extend(generated_content.bindings.clone());
        for removed in &generated_content.removed_bindings {
            bindings.remove(removed);
        }
        let mut errors = summary.errors;
        errors.extend(generated_content.errors);
        let loc = context.script_or_setup_loc();
        let setup = context.has_script_setup();
        let lang = context.setup_or_script_lang();
        let (script_ast, script_setup_ast) = context.into_script_ast();
        SfcScriptBlock {
            type_name: "script".into(),
            content: generated_content.content,
            loc,
            attrs,
            setup,
            lang,
            bindings,
            props_aliases: generated_content.props_aliases,
            imports: generated_content.imports,
            errors,
            warnings: generated_content.warnings,
            map: generated_content.map,
            script_ast,
            script_setup_ast,
            deps: generated_content.deps,
        }
    }

    /// Compiles parsed Vue 3 SFC scripts and preserves descriptor parse errors.
    pub fn compile_parsed_vue3_script(
        &mut self,
        parsed: &Vue3SfcParseResult,
        options: SfcScriptCompileOptions,
    ) -> SfcScriptBlock {
        let mut result = self.compile_script(&parsed.descriptor, options);
        result.errors.splice(
            0..0,
            parsed.errors.iter().map(|error| error.message.clone()),
        );
        result
    }

    /// Resolves the first top-level `defineProps<T>()` type argument in Vue 3 script setup code.
    pub fn resolve_vue3_type(
        &mut self,
        filename: impl Into<String>,
        code: &str,
        options: SfcScriptCompileOptions,
    ) -> Vue3ResolveTypeResult {
        let filename = filename.into();
        let source = format!("<script setup lang=\"ts\">\n{code}\n</script>");
        let parsed = self.parse_vue3(filename, &source);
        let mut result = vue3_resolve_type_projection(&parsed.descriptor, &options);
        result
            .errors
            .splice(0..0, parsed.errors.into_iter().map(|error| error.message));
        result
    }

    /// Compiles Vue 2.7 SFC script blocks.
    pub fn compile_vue27_script(
        &mut self,
        descriptor: &SfcDescriptor,
        options: SfcScriptCompileOptions,
    ) -> SfcScriptBlock {
        let mut context = Vue27ScriptCompileContext::new(descriptor, &options, &mut self.js);
        let summary = self
            .js
            .summarize_program(context.raw_content(), context.source_type());
        let css_vars = descriptor_css_vars(
            context.descriptor(),
            CssVarCollectOptions {
                ignore_line_comments: false,
            },
        );
        let script_errors = context.script_compile_errors();
        let template_usage_index = vue27_compile_script_template_usage_index(&mut context);
        let bindings = if context.has_script_setup() {
            context.setup_binding_metadata()
        } else {
            vue27_normal_script_binding_metadata(context.descriptor())
        };
        let content = if let Some(script_setup) = descriptor.script_setup.as_ref() {
            let analysis = context.script_setup_analysis().clone();
            let normal_script = context.normal_script_analysis().clone();
            let normal_script_return_bindings = context.normal_script_return_bindings().clone();
            vue27_script_setup_content(
                descriptor,
                script_setup,
                &options,
                &css_vars,
                &bindings,
                &analysis,
                &normal_script,
                &normal_script_return_bindings,
                template_usage_index.as_ref(),
            )
        } else {
            vue27_normal_script_content(descriptor, &options, &css_vars, &bindings)
        };
        let attrs = context.script_or_setup_attrs();
        let loc = context.script_or_setup_loc();
        let setup = context.has_script_setup();
        let lang = context.setup_or_script_lang();
        let (script_ast, script_setup_ast) = context.into_script_ast();

        SfcScriptBlock {
            type_name: "script".into(),
            content,
            loc,
            attrs,
            setup,
            lang,
            bindings,
            props_aliases: BTreeMap::new(),
            imports: BTreeMap::new(),
            errors: if script_errors.is_empty() {
                summary.errors
            } else {
                script_errors
            },
            warnings: Vec::new(),
            map: None,
            script_ast,
            script_setup_ast,
            deps: Vec::new(),
        }
    }

    /// Compiles all style blocks in a descriptor.
    pub fn compile_style(
        &self,
        descriptor: &SfcDescriptor,
        options: SfcStyleCompileOptions,
    ) -> SfcStyleCompileResult {
        let mut code = String::new();
        let mut errors = Vec::new();
        let mut diagnostics = Vec::new();
        let mut dependencies = Vec::new();
        let mut modules = BTreeMap::new();
        let mut has_modules_result = false;
        let mut raw_result = Vec::new();
        let mut map_builder = options.source_map.then(|| {
            let mut builder = SourceMapBuilder::new().file(descriptor.filename.clone());
            builder.add_source_content(descriptor.filename.clone(), descriptor.source.clone());
            builder
        });
        let mut generated_line_offset = 0u32;
        for style in &descriptor.styles {
            let result = compile_style(
                &style.content,
                StyleCompileOptions {
                    id: options.id.clone(),
                    scoped: options.scoped || style.attrs.scoped,
                    modules: options.modules || style.attrs.module.is_some(),
                    modules_options: options.modules_options.clone(),
                    vars: options.vars.clone(),
                    is_prod: options.is_prod,
                    css_var_name_style: options.css_var_name_style,
                    css_var_ignore_line_comments: options.css_var_ignore_line_comments,
                    filename: Some(descriptor.filename.clone()),
                    source_map_source: None,
                    source_map_file_id: Some(descriptor.source_file),
                    source_map_base_offset: style.content_start,
                    source_map: false,
                    preprocess_lang: style
                        .attrs
                        .lang
                        .clone()
                        .or_else(|| options.preprocess_lang.clone()),
                    preprocess_options: options.preprocess_options.clone(),
                    warn_deprecated_scoped_selectors: options.warn_deprecated_scoped_selectors,
                },
            );
            let needs_join_newline = !code.is_empty() && !result.code.is_empty();
            if needs_join_newline {
                code.push('\n');
            }
            code.push_str(&result.code);
            errors.extend(result.errors);
            diagnostics.extend(result.diagnostics);
            if let Some(result_modules) = result.modules {
                has_modules_result = true;
                modules.extend(result_modules);
            }
            if let Some(builder) = map_builder.as_mut() {
                add_style_block_mappings(
                    builder,
                    descriptor,
                    style,
                    &result.code,
                    generated_line_offset,
                );
            }
            if !result.code.is_empty() {
                generated_line_offset += generated_line_count(&result.code);
            }
            if needs_join_newline {
                generated_line_offset += 1;
            }
            dependencies.extend(style_src_dependency(style));
            if result.dependencies.is_empty() {
                dependencies.extend(style_import_dependencies(style));
            }
            dependencies.extend(result.dependencies);
            raw_result.push("postcss-result".to_string());
        }
        dependencies.sort();
        dependencies.dedup();
        let map = map_builder.map(SourceMapBuilder::build);
        let modules = has_modules_result.then_some(modules);
        SfcStyleCompileResult {
            code,
            map,
            errors,
            diagnostics,
            dependencies,
            modules,
            raw_result,
        }
    }

    /// Compiles parsed Vue 3 SFC styles and preserves descriptor parse errors.
    pub fn compile_parsed_vue3_style(
        &self,
        parsed: &Vue3SfcParseResult,
        options: SfcStyleCompileOptions,
    ) -> SfcStyleCompileResult {
        let mut result = self.compile_style(&parsed.descriptor, options);
        result.errors.splice(
            0..0,
            parsed.errors.iter().map(|error| error.message.clone()),
        );
        result
            .diagnostics
            .splice(0..0, vue3_sfc_parse_diagnostics(parsed));
        result
    }

    /// Rewrites Vue 2.7 default exports to an assigned variable.
    pub fn rewrite_vue27_default(
        &self,
        input: &str,
        variable: &str,
        options: Vue27RewriteDefaultOptions,
    ) -> String {
        rewrite_vue27_default(input, variable, options)
    }

    /// Rewrites Vue 3 default exports to an assigned variable.
    pub fn rewrite_vue3_default(
        &self,
        input: &str,
        variable: &str,
        options: Vue3RewriteDefaultOptions,
    ) -> Result<String, String> {
        rewrite_vue3_default(input, variable, options)
    }

    /// Prefixes Vue 2.7 template identifiers for render-function generation.
    pub fn prefix_vue27_identifiers(
        &self,
        input: &str,
        options: Vue27PrefixIdentifiersOptions,
    ) -> String {
        prefix_vue27_identifiers(input, options)
    }

    /// Generates Vue 2.7 SFC `compileTemplate` render code from Vue 2 compiler output.
    pub fn vue27_sfc_template_code(
        &self,
        render: &str,
        static_render_fns: &[String],
        options: Vue27PrefixIdentifiersOptions,
        is_production: bool,
    ) -> String {
        vue27_sfc_template_code(render, static_render_fns, options, is_production)
    }

    /// Preprocesses Vue 2.7 template source.
    pub fn preprocess_vue27_template(
        &self,
        source: &str,
        options: Vue27TemplatePreprocessOptions,
    ) -> Vue27TemplatePreprocessResult {
        preprocess_vue27_template(source, options)
    }

    /// Preprocesses Vue 3 template source.
    pub fn preprocess_vue3_template(
        &self,
        source: &str,
        options: Vue3TemplatePreprocessOptions,
    ) -> Vue3TemplatePreprocessResult {
        preprocess_vue3_template(source, options)
    }

    /// Returns the JavaScript side store used by SFC script compilation.
    pub fn js(&self) -> &JsAstStore {
        &self.js
    }

    /// Returns descriptor cache statistics.
    pub fn cache_stats(&self) -> SfcCacheStats {
        self.cache_stats.clone()
    }

    /// Returns the number of cached descriptors.
    pub fn descriptor_cache_len(&self) -> usize {
        self.descriptor_cache.len()
    }

    /// Clears descriptor, source, JavaScript, and cache-stat state.
    ///
    /// This is intended for long-lived compiler services that want to release
    /// retained source text and parser arena allocations between independent
    /// compile batches.
    pub fn clear_caches(&mut self) {
        self.sources = SourceMap::default();
        self.js.clear();
        self.descriptor_cache.clear();
        self.cache_stats = SfcCacheStats::default();
    }

    pub(crate) fn invalidate_stale_descriptor_entries(
        &mut self,
        filename: &str,
        mode: &SfcParseCacheMode,
    ) {
        let before = self.descriptor_cache.len();
        self.descriptor_cache
            .retain(|key, _| key.filename != filename || &key.mode != mode);
        let removed = before.saturating_sub(self.descriptor_cache.len());
        self.cache_stats.descriptor_invalidations += removed as u64;
    }
}

impl Default for SfcCompiler {
    fn default() -> Self {
        Self::new()
    }
}
