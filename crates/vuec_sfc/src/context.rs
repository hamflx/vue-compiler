use crate::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Controls public script AST projection during SFC script compilation.
pub enum SfcScriptAstMode {
    /// Do not project public script AST statements.
    None,
    /// Project only top-level statement metadata without recursive child nodes.
    TopLevel,
    /// Project the full public script AST shape.
    #[default]
    Full,
}

impl SfcScriptAstMode {
    /// Parses an internal script AST projection mode option.
    pub fn from_option_str(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "top-level" | "topLevel" | "top_level" => Some(Self::TopLevel),
            "full" => Some(Self::Full),
            _ => None,
        }
    }

    pub(crate) fn from_options(options: &SfcScriptCompileOptions) -> Self {
        options.script_ast_mode
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TemplateUsageFlavor {
    Vue27,
    Vue3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TemplateUsageCacheKey {
    pub(crate) flavor: TemplateUsageFlavor,
    pub(crate) is_ts: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TemplateUsageIndex {
    pub(crate) usage: String,
}

impl TemplateUsageIndex {
    pub(crate) fn new(template: &str, flavor: TemplateUsageFlavor, is_ts: bool) -> Self {
        let usage = match flavor {
            TemplateUsageFlavor::Vue27 => vue27_template_usage_check_string(template, is_ts),
            TemplateUsageFlavor::Vue3 => vue3_template_usage_check_string(template, is_ts),
        };
        Self { usage }
    }

    pub(crate) fn contains(&self, local: &str) -> bool {
        identifier_usage_contains(&self.usage, local)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TemplateUsageCacheSlot {
    pub(crate) key: Option<TemplateUsageCacheKey>,
    pub(crate) index: Option<TemplateUsageIndex>,
}

impl TemplateUsageCacheSlot {
    pub(crate) fn index(
        &mut self,
        template: &SfcBlock,
        flavor: TemplateUsageFlavor,
        is_ts: bool,
    ) -> &TemplateUsageIndex {
        let key = TemplateUsageCacheKey { flavor, is_ts };
        if self.key != Some(key) {
            self.key = Some(key);
            self.index = Some(TemplateUsageIndex::new(&template.content, flavor, is_ts));
        }
        self.index.as_ref().expect("template usage index")
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SfcScriptBlockMetadata<'a> {
    pub(crate) block: &'a SfcBlock,
    pub(crate) is_js_like: bool,
}

impl<'a> SfcScriptBlockMetadata<'a> {
    pub(crate) fn new(block: &'a SfcBlock) -> Self {
        Self {
            block,
            is_js_like: script_lang_is_js_like(&block.attrs),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SfcScriptCompileContext<'a> {
    pub(crate) descriptor: &'a SfcDescriptor,
    pub(crate) script: Option<SfcScriptBlockMetadata<'a>>,
    pub(crate) script_setup: Option<SfcScriptBlockMetadata<'a>>,
    pub(crate) raw_content: String,
    pub(crate) source_type: oxc_span::SourceType,
    pub(crate) ast_mode: SfcScriptAstMode,
    pub(crate) template_usage_cache: TemplateUsageCacheSlot,
    pub(crate) script_ast: Vec<Value>,
    pub(crate) script_setup_ast: Vec<Value>,
}

impl<'a> SfcScriptCompileContext<'a> {
    pub(crate) fn new(
        descriptor: &'a SfcDescriptor,
        options: &'a SfcScriptCompileOptions,
        js: &mut JsAstStore,
        vue3_js_like_only: bool,
    ) -> Self {
        let script = descriptor.script.as_ref().map(SfcScriptBlockMetadata::new);
        let script_setup = descriptor
            .script_setup
            .as_ref()
            .map(SfcScriptBlockMetadata::new);
        let source_type = script_source_type(descriptor);
        let ast_mode = SfcScriptAstMode::from_options(options);
        let mut context = Self {
            descriptor,
            script,
            script_setup,
            raw_content: String::new(),
            source_type,
            ast_mode,
            template_usage_cache: TemplateUsageCacheSlot::default(),
            script_ast: Vec::new(),
            script_setup_ast: Vec::new(),
        };
        context.collect_raw_content_and_ast(js, vue3_js_like_only);
        context
    }

    pub(crate) fn collect_raw_content_and_ast(
        &mut self,
        js: &mut JsAstStore,
        vue3_js_like_only: bool,
    ) {
        if let Some(script) = self.script.as_ref() {
            self.raw_content.push_str(&script.block.content);
            if !vue3_js_like_only || script.is_js_like {
                self.script_ast = self.project_block_ast(js, script.block);
            }
        }
        if let Some(script_setup) = self.script_setup.as_ref() {
            if !self.raw_content.is_empty() {
                self.raw_content.push('\n');
            }
            self.raw_content.push_str(&script_setup.block.content);
            if !vue3_js_like_only || script_setup.is_js_like {
                self.script_setup_ast = self.project_block_ast(js, script_setup.block);
            }
        }
    }

    pub(crate) fn project_block_ast(&self, js: &mut JsAstStore, block: &SfcBlock) -> Vec<Value> {
        match self.ast_mode {
            SfcScriptAstMode::None => Vec::new(),
            SfcScriptAstMode::TopLevel => {
                let id = js.register_program(
                    block.content.clone(),
                    Span::new(self.descriptor.source_file, block.loc.start, block.loc.end),
                    script_mode(&block.attrs),
                    self.source_type,
                );
                sfc_script_ast_body(js, id, &block.content, self.ast_mode)
            }
            SfcScriptAstMode::Full => {
                let id = js.register_program(
                    block.content.clone(),
                    Span::new(self.descriptor.source_file, block.loc.start, block.loc.end),
                    script_mode(&block.attrs),
                    self.source_type,
                );
                sfc_script_ast_body(js, id, &block.content, self.ast_mode)
            }
        }
    }

    pub(crate) fn descriptor(&self) -> &'a SfcDescriptor {
        self.descriptor
    }

    pub(crate) fn raw_content(&self) -> &str {
        &self.raw_content
    }

    pub(crate) fn source_type(&self) -> oxc_span::SourceType {
        self.source_type
    }

    pub(crate) fn filename(&self) -> &str {
        self.descriptor.filename.as_str()
    }

    pub(crate) fn has_script_setup(&self) -> bool {
        self.script_setup.is_some()
    }

    pub(crate) fn script_or_setup_attrs(&self) -> SfcBlockAttrs {
        self.script
            .as_ref()
            .or(self.script_setup.as_ref())
            .map(|metadata| metadata.block.attrs.clone())
            .unwrap_or_default()
    }

    pub(crate) fn script_or_setup_loc(&self) -> Option<SfcBlockLocation> {
        self.script
            .as_ref()
            .or(self.script_setup.as_ref())
            .map(|metadata| metadata.block.loc.clone())
    }

    pub(crate) fn setup_or_script_lang(&self) -> Option<String> {
        self.script_setup
            .as_ref()
            .or(self.script.as_ref())
            .and_then(|metadata| metadata.block.attrs.lang.clone())
    }

    pub(crate) fn all_script_blocks_are_js_like(&self) -> bool {
        self.script
            .as_ref()
            .into_iter()
            .chain(self.script_setup.as_ref())
            .all(|metadata| metadata.is_js_like)
            && (self.script.is_some() || self.script_setup.is_some())
    }

    pub(crate) fn into_script_ast(self) -> (Vec<Value>, Vec<Value>) {
        (self.script_ast, self.script_setup_ast)
    }

    pub(crate) fn template_usage_index(
        &mut self,
        flavor: TemplateUsageFlavor,
        is_ts: bool,
    ) -> Option<&TemplateUsageIndex> {
        let template = self.descriptor.template.as_ref()?;
        if template.attrs.src.is_some() || template.attrs.lang.is_some() {
            return None;
        }
        Some(self.template_usage_cache.index(template, flavor, is_ts))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Vue3ScriptCompileContext<'a> {
    pub(crate) inner: SfcScriptCompileContext<'a>,
    pub(crate) options: SfcScriptCompileOptions,
    pub(crate) script_compile_errors: Option<Vec<String>>,
    pub(crate) type_resolver: Option<Vue3TypeResolverContext>,
    pub(crate) normal_type_context: Option<Vue27TypeContext>,
    pub(crate) normal_user_imports: Option<Vue3UserImports>,
    pub(crate) script_setup_analysis: Option<Vue3ScriptSetupAnalysis>,
    pub(crate) normal_script_analysis: Option<Vue3NormalScriptAnalysis>,
    pub(crate) normal_script_return_bindings: Option<Vue27ScriptReturnBindings>,
    pub(crate) normal_script_option_bindings: Option<Option<BTreeMap<String, String>>>,
    pub(crate) script_binding_metadata: Option<BTreeMap<String, String>>,
}

impl<'a> Vue3ScriptCompileContext<'a> {
    pub(crate) fn new(
        descriptor: &'a SfcDescriptor,
        options: &'a SfcScriptCompileOptions,
        js: &mut JsAstStore,
    ) -> Self {
        let inner = SfcScriptCompileContext::new(descriptor, options, js, true);
        Self {
            inner,
            options: options.clone(),
            script_compile_errors: None,
            type_resolver: None,
            normal_type_context: None,
            normal_user_imports: None,
            script_setup_analysis: None,
            normal_script_analysis: None,
            normal_script_return_bindings: None,
            normal_script_option_bindings: None,
            script_binding_metadata: None,
        }
    }

    pub(crate) fn into_script_ast(self) -> (Vec<Value>, Vec<Value>) {
        self.inner.into_script_ast()
    }

    pub(crate) fn template_usage_index(&mut self, is_ts: bool) -> Option<&TemplateUsageIndex> {
        self.inner
            .template_usage_index(TemplateUsageFlavor::Vue3, is_ts)
    }

    pub(crate) fn script_compile_errors(&mut self) -> Vec<String> {
        if self.script_compile_errors.is_none() {
            self.script_compile_errors =
                Some(vue3_script_compile_errors(self.descriptor(), &self.options));
        }
        self.script_compile_errors
            .as_ref()
            .expect("vue 3 script compile errors")
            .clone()
    }

    pub(crate) fn type_resolver(&mut self) -> Vue3TypeResolverContext {
        if self.type_resolver.is_none() {
            self.type_resolver = Some(vue3_type_resolver_context_for_filename(
                &self.descriptor().filename,
            ));
        }
        self.type_resolver
            .as_ref()
            .expect("vue 3 type resolver")
            .clone()
    }

    pub(crate) fn normal_type_context(&mut self) -> Vue27TypeContext {
        if self.normal_type_context.is_none() {
            let type_resolver = self.type_resolver();
            self.normal_type_context = Some(vue3_normal_script_type_context(
                self.descriptor(),
                &self.options.global_type_files,
                &type_resolver,
            ));
        }
        self.normal_type_context
            .as_ref()
            .expect("vue 3 normal type context")
            .clone()
    }

    pub(crate) fn normal_user_imports(&mut self) -> Vue3UserImports {
        if self.normal_user_imports.is_none() {
            self.normal_user_imports = Some(vue3_normal_script_user_imports(self.descriptor()));
        }
        self.normal_user_imports
            .as_ref()
            .expect("vue 3 normal user imports")
            .clone()
    }

    pub(crate) fn script_setup_analysis(&mut self) -> Vue3ScriptSetupAnalysis {
        if self.script_setup_analysis.is_none() {
            let script_setup = self.script_setup.as_ref().map(|metadata| metadata.block);
            let analysis = script_setup
                .map(|script_setup| {
                    let normal_type_context = self.normal_type_context();
                    let normal_user_imports = self.normal_user_imports();
                    let type_resolver = self.type_resolver();
                    analyze_vue3_script_setup(
                        &self.descriptor().filename,
                        self.descriptor(),
                        script_setup,
                        &normal_type_context,
                        &normal_user_imports,
                        &type_resolver,
                        Vue3ScriptSetupAnalysisOptions {
                            hoist_static_literals: self.options.hoist_static
                                && self.script.is_none(),
                            props_destructure: self.options.props_destructure,
                            is_prod: self.options.is_prod,
                            custom_element: self.options.custom_element,
                        },
                    )
                })
                .unwrap_or_default();
            self.script_setup_analysis = Some(analysis);
        }
        self.script_setup_analysis
            .as_ref()
            .expect("vue 3 script setup analysis")
            .clone()
    }

    pub(crate) fn normal_script_analysis(&mut self) -> Vue3NormalScriptAnalysis {
        if self.normal_script_analysis.is_none() {
            self.normal_script_analysis =
                Some(analyze_vue3_normal_script_for_setup(self.descriptor()));
        }
        self.normal_script_analysis
            .as_ref()
            .expect("vue 3 normal script analysis")
            .clone()
    }

    pub(crate) fn normal_script_return_bindings(&mut self) -> Vue27ScriptReturnBindings {
        if self.normal_script_return_bindings.is_none() {
            self.normal_script_return_bindings = Some(
                self.script
                    .as_ref()
                    .map(|script| vue3_script_block_return_bindings(script.block))
                    .unwrap_or_default(),
            );
        }
        self.normal_script_return_bindings
            .as_ref()
            .expect("vue 3 normal script return bindings")
            .clone()
    }

    pub(crate) fn normal_script_option_bindings(&mut self) -> Option<BTreeMap<String, String>> {
        if self.normal_script_option_bindings.is_none() {
            self.normal_script_option_bindings = Some(vue3_normal_script_options_binding_metadata(
                self.descriptor(),
            ));
        }
        self.normal_script_option_bindings
            .as_ref()
            .expect("vue 3 normal script option bindings")
            .clone()
    }

    pub(crate) fn base_binding_metadata(&mut self) -> BTreeMap<String, String> {
        if self.has_script_setup() {
            return self.normal_script_option_bindings().unwrap_or_default();
        }
        let Some(mut bindings) = self.normal_script_option_bindings() else {
            return BTreeMap::new();
        };
        bindings.insert("__isScriptSetup".into(), "false".into());
        bindings
    }

    pub(crate) fn script_binding_metadata(
        &mut self,
        setup_analysis: &Vue3ScriptSetupAnalysis,
    ) -> BTreeMap<String, String> {
        if self.script_binding_metadata.is_none() {
            self.script_binding_metadata = Some(vue3_script_setup_script_binding_metadata(
                self.descriptor(),
                &setup_analysis.vue_import_aliases,
            ));
        }
        self.script_binding_metadata
            .as_ref()
            .expect("vue 3 script binding metadata")
            .clone()
    }
}

impl<'a> std::ops::Deref for Vue3ScriptCompileContext<'a> {
    type Target = SfcScriptCompileContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Vue27ScriptCompileContext<'a> {
    pub(crate) inner: SfcScriptCompileContext<'a>,
    pub(crate) is_prod: bool,
    pub(crate) script_setup_context: Option<Vue27ScriptSetupContext>,
    pub(crate) script_setup_analysis: Option<Vue27ScriptSetupAnalysis>,
    pub(crate) normal_script_analysis: Option<Vue27NormalScriptAnalysis>,
    pub(crate) normal_script_bindings: Option<BTreeMap<String, String>>,
    pub(crate) normal_script_return_bindings: Option<Vue27ScriptReturnBindings>,
    pub(crate) setup_binding_metadata: Option<BTreeMap<String, String>>,
}

impl<'a> Vue27ScriptCompileContext<'a> {
    pub(crate) fn new(
        descriptor: &'a SfcDescriptor,
        options: &'a SfcScriptCompileOptions,
        js: &mut JsAstStore,
    ) -> Self {
        let inner = SfcScriptCompileContext::new(descriptor, options, js, false);
        Self {
            inner,
            is_prod: options.is_prod,
            script_setup_context: None,
            script_setup_analysis: None,
            normal_script_analysis: None,
            normal_script_bindings: None,
            normal_script_return_bindings: None,
            setup_binding_metadata: None,
        }
    }

    pub(crate) fn into_script_ast(self) -> (Vec<Value>, Vec<Value>) {
        self.inner.into_script_ast()
    }

    pub(crate) fn template_usage_index(&mut self, is_ts: bool) -> Option<&TemplateUsageIndex> {
        self.inner
            .template_usage_index(TemplateUsageFlavor::Vue27, is_ts)
    }

    pub(crate) fn script_compile_errors(&mut self) -> Vec<String> {
        let Some(script_setup) = self.script_setup.as_ref() else {
            return Vec::new();
        };
        if self
            .script
            .as_ref()
            .is_some_and(|script| script.block.attrs.lang != script_setup.block.attrs.lang)
        {
            return vec!["<script> and <script setup> must have the same language type.".into()];
        }
        self.script_setup_analysis().errors.clone()
    }

    pub(crate) fn script_setup_context(&mut self) -> Vue27ScriptSetupContext {
        if self.script_setup_context.is_none() {
            let normal_imports = self.normal_script_return_bindings().imports.clone();
            self.script_setup_context = Some(Vue27ScriptSetupContext {
                normal_types: vue27_normal_script_type_context(self.descriptor()),
                normal_imports,
            });
        }
        self.script_setup_context
            .as_ref()
            .expect("vue 2.7 script setup context")
            .clone()
    }

    pub(crate) fn script_setup_analysis(&mut self) -> &Vue27ScriptSetupAnalysis {
        if self.script_setup_analysis.is_none() {
            let script_setup = self.script_setup.as_ref().map(|metadata| metadata.block);
            let analysis = script_setup
                .map(|script_setup| {
                    let setup_context = self.script_setup_context();
                    analyze_vue27_script_setup(script_setup, self.is_prod, &setup_context)
                })
                .unwrap_or_default();
            self.script_setup_analysis = Some(analysis);
        }
        self.script_setup_analysis
            .as_ref()
            .expect("vue 2.7 script setup analysis")
    }

    pub(crate) fn normal_script_analysis(&mut self) -> &Vue27NormalScriptAnalysis {
        if self.normal_script_analysis.is_none() {
            self.normal_script_analysis =
                Some(analyze_vue27_normal_script_for_setup(self.descriptor()));
        }
        self.normal_script_analysis
            .as_ref()
            .expect("vue 2.7 normal script analysis")
    }

    pub(crate) fn normal_script_bindings(&mut self) -> &BTreeMap<String, String> {
        if self.normal_script_bindings.is_none() {
            self.normal_script_bindings =
                Some(vue27_script_setup_script_bindings(self.descriptor()));
        }
        self.normal_script_bindings
            .as_ref()
            .expect("vue 2.7 normal script bindings")
    }

    pub(crate) fn normal_script_return_bindings(&mut self) -> &Vue27ScriptReturnBindings {
        if self.normal_script_return_bindings.is_none() {
            self.normal_script_return_bindings =
                Some(vue27_script_setup_script_return_bindings(self.descriptor()));
        }
        self.normal_script_return_bindings
            .as_ref()
            .expect("vue 2.7 normal script return bindings")
    }

    pub(crate) fn setup_binding_metadata(&mut self) -> BTreeMap<String, String> {
        if self.setup_binding_metadata.is_none() {
            let analysis = self.script_setup_analysis().clone();
            let mut bindings = self.normal_script_bindings().clone();
            bindings.extend(analysis.setup_bindings);
            for prop in analysis.props_bindings {
                bindings.insert(prop, "props".into());
            }
            bindings.insert("__isScriptSetup".into(), "true".into());
            self.setup_binding_metadata = Some(bindings);
        }
        self.setup_binding_metadata
            .as_ref()
            .expect("vue 2.7 setup binding metadata")
            .clone()
    }
}

impl<'a> std::ops::Deref for Vue27ScriptCompileContext<'a> {
    type Target = SfcScriptCompileContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub(crate) fn vue3_compile_script_template_usage_index(
    context: &mut Vue3ScriptCompileContext<'_>,
    script_compile_errors: &[String],
) -> Option<TemplateUsageIndex> {
    let script_setup = context.script_setup.as_ref()?;
    if !script_compile_errors.is_empty() || !script_setup.is_js_like {
        return None;
    }
    let is_ts = script_is_typescript(&script_setup.block.attrs)
        || context
            .script
            .as_ref()
            .is_some_and(|script| script_is_typescript(&script.block.attrs));
    context.template_usage_index(is_ts).cloned()
}

pub(crate) fn vue27_compile_script_template_usage_index(
    context: &mut Vue27ScriptCompileContext<'_>,
) -> Option<TemplateUsageIndex> {
    let is_ts = context
        .script_setup
        .as_ref()
        .map(|script_setup| script_is_typescript(&script_setup.block.attrs))?;
    context.template_usage_index(is_ts).cloned()
}
