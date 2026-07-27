#[derive(Clone, Debug)]
pub(crate) struct Vue3ExternalTypeSource {
    pub(crate) source: String,
    pub(crate) source_type: oxc_span::SourceType,
    pub(crate) resolution_mode: Vue3TypeResolutionMode,
    pub(crate) dynamic_resolution_mode: Vue3TypeResolutionMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Vue3ExternalTypeFormat {
    source_type: oxc_span::SourceType,
    resolution_mode: Vue3TypeResolutionMode,
    dynamic_resolution_mode: Vue3TypeResolutionMode,
}

pub(crate) const VUE3_EXTERNAL_TYPE_MAX_ACTIVE_FILES: usize = 64;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_IMPORT_FILES: usize = 512;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_GLOBAL_FILES: usize = 16_384;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_IMPORT_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_GLOBAL_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_CONTEXT_LOOKUPS: usize = 16_384;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_CONTEXT_BUILDS: usize = 2048;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_CONTEXT_BUILD_WEIGHT: usize = 64 * 1024 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_CONTEXT_CACHE_WEIGHT: usize = 8 * 1024 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_CONTEXT_CACHE_ENTRY_WEIGHT: usize = 1024 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_RESOLUTION_LOOKUPS: usize = 65_536;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_RESOLUTION_CACHE_ENTRIES: usize = 16_384;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_RESOLUTION_CACHE_WEIGHT: usize = 4 * 1024 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_RESOLUTION_CACHE_ENTRY_WEIGHT: usize = 64 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_METADATA_FILES: usize = 16_384;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_METADATA_FILE_BYTES: usize = 1024 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_METADATA_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_METADATA_FANOUT_ENTRIES: usize = 65_536;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_METADATA_RESOLUTION_PATH_PROBES: usize = 131_072;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_GENERATED_PATH_BYTES: usize = 64 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_NODES: usize = 512;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_DEPTH: usize = 64;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_DISCOVERY_DEPTH: usize = 64;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_DISCOVERY_ENTRIES: usize = 65_536;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_DISCOVERY_FILES: usize = 16_384;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_GLOB_MATCH_STEPS: usize = 16 * 1024 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_PACKAGE_RESOLUTION_DEPTH: usize = 64;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_ANCESTOR_SEARCH_DEPTH: usize = 128;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_ANCESTOR_SEARCH_ENTRIES: usize = 65_536;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_ANCESTOR_SEARCH_WEIGHT: usize = 64 * 1024 * 1024;
pub(crate) const VUE3_EXTERNAL_TYPE_MAX_ANCESTOR_SEARCH_PATH_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Vue3ExternalTypeLoadLimits {
    pub(crate) max_import_files: usize,
    pub(crate) max_global_files: usize,
    pub(crate) max_file_bytes: usize,
    pub(crate) max_import_bytes: usize,
    pub(crate) max_global_bytes: usize,
    pub(crate) max_context_lookups: usize,
    pub(crate) max_context_builds: usize,
    pub(crate) max_context_build_weight: usize,
    pub(crate) max_context_cache_weight: usize,
    pub(crate) max_context_cache_entry_weight: usize,
    pub(crate) max_resolution_lookups: usize,
    pub(crate) max_resolution_cache_entries: usize,
    pub(crate) max_resolution_cache_weight: usize,
    pub(crate) max_resolution_cache_entry_weight: usize,
    pub(crate) max_metadata_files: usize,
    pub(crate) max_metadata_file_bytes: usize,
    pub(crate) max_metadata_bytes: usize,
    pub(crate) max_metadata_fanout_entries: usize,
    pub(crate) max_metadata_resolution_path_probes: usize,
    pub(crate) max_generated_path_bytes: usize,
    pub(crate) max_tsconfig_nodes: usize,
    pub(crate) max_tsconfig_depth: usize,
    pub(crate) max_tsconfig_discovery_depth: usize,
    pub(crate) max_tsconfig_discovery_entries: usize,
    pub(crate) max_tsconfig_discovery_files: usize,
    pub(crate) max_tsconfig_glob_match_steps: usize,
    pub(crate) max_package_resolution_depth: usize,
    pub(crate) max_ancestor_search_depth: usize,
    pub(crate) max_ancestor_search_entries: usize,
    pub(crate) max_ancestor_search_weight: usize,
    pub(crate) max_ancestor_search_path_bytes: usize,
}

impl Default for Vue3ExternalTypeLoadLimits {
    fn default() -> Self {
        Self {
            max_import_files: VUE3_EXTERNAL_TYPE_MAX_IMPORT_FILES,
            max_global_files: VUE3_EXTERNAL_TYPE_MAX_GLOBAL_FILES,
            max_file_bytes: VUE3_EXTERNAL_TYPE_MAX_FILE_BYTES,
            max_import_bytes: VUE3_EXTERNAL_TYPE_MAX_IMPORT_BYTES,
            max_global_bytes: VUE3_EXTERNAL_TYPE_MAX_GLOBAL_BYTES,
            max_context_lookups: VUE3_EXTERNAL_TYPE_MAX_CONTEXT_LOOKUPS,
            max_context_builds: VUE3_EXTERNAL_TYPE_MAX_CONTEXT_BUILDS,
            max_context_build_weight: VUE3_EXTERNAL_TYPE_MAX_CONTEXT_BUILD_WEIGHT,
            max_context_cache_weight: VUE3_EXTERNAL_TYPE_MAX_CONTEXT_CACHE_WEIGHT,
            max_context_cache_entry_weight: VUE3_EXTERNAL_TYPE_MAX_CONTEXT_CACHE_ENTRY_WEIGHT,
            max_resolution_lookups: VUE3_EXTERNAL_TYPE_MAX_RESOLUTION_LOOKUPS,
            max_resolution_cache_entries: VUE3_EXTERNAL_TYPE_MAX_RESOLUTION_CACHE_ENTRIES,
            max_resolution_cache_weight: VUE3_EXTERNAL_TYPE_MAX_RESOLUTION_CACHE_WEIGHT,
            max_resolution_cache_entry_weight:
                VUE3_EXTERNAL_TYPE_MAX_RESOLUTION_CACHE_ENTRY_WEIGHT,
            max_metadata_files: VUE3_EXTERNAL_TYPE_MAX_METADATA_FILES,
            max_metadata_file_bytes: VUE3_EXTERNAL_TYPE_MAX_METADATA_FILE_BYTES,
            max_metadata_bytes: VUE3_EXTERNAL_TYPE_MAX_METADATA_BYTES,
            max_metadata_fanout_entries: VUE3_EXTERNAL_TYPE_MAX_METADATA_FANOUT_ENTRIES,
            max_metadata_resolution_path_probes:
                VUE3_EXTERNAL_TYPE_MAX_METADATA_RESOLUTION_PATH_PROBES,
            max_generated_path_bytes: VUE3_EXTERNAL_TYPE_MAX_GENERATED_PATH_BYTES,
            max_tsconfig_nodes: VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_NODES,
            max_tsconfig_depth: VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_DEPTH,
            max_tsconfig_discovery_depth: VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_DISCOVERY_DEPTH,
            max_tsconfig_discovery_entries: VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_DISCOVERY_ENTRIES,
            max_tsconfig_discovery_files: VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_DISCOVERY_FILES,
            max_tsconfig_glob_match_steps: VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_GLOB_MATCH_STEPS,
            max_package_resolution_depth: VUE3_EXTERNAL_TYPE_MAX_PACKAGE_RESOLUTION_DEPTH,
            max_ancestor_search_depth: VUE3_EXTERNAL_TYPE_MAX_ANCESTOR_SEARCH_DEPTH,
            max_ancestor_search_entries: VUE3_EXTERNAL_TYPE_MAX_ANCESTOR_SEARCH_ENTRIES,
            max_ancestor_search_weight: VUE3_EXTERNAL_TYPE_MAX_ANCESTOR_SEARCH_WEIGHT,
            max_ancestor_search_path_bytes: VUE3_EXTERNAL_TYPE_MAX_ANCESTOR_SEARCH_PATH_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue3ExternalTypeLoadStats {
    pub(crate) import_files_read: usize,
    pub(crate) global_files_read: usize,
    pub(crate) import_bytes: usize,
    pub(crate) global_bytes: usize,
    pub(crate) source_cache_hits: usize,
    pub(crate) context_lookups: usize,
    pub(crate) context_builds: usize,
    pub(crate) context_build_weight: usize,
    pub(crate) context_cache_hits: usize,
    pub(crate) cached_context_weight: usize,
    pub(crate) resolution_lookups: usize,
    pub(crate) resolution_cache_hits: usize,
    pub(crate) cached_resolution_weight: usize,
    pub(crate) metadata_files_read: usize,
    pub(crate) metadata_bytes: usize,
    pub(crate) metadata_source_cache_hits: usize,
    pub(crate) metadata_parse_cache_hits: usize,
    pub(crate) metadata_fanout_entries: usize,
    pub(crate) metadata_resolution_path_probes: usize,
    pub(crate) tsconfig_nodes: usize,
    pub(crate) tsconfig_discovery_entries: usize,
    pub(crate) tsconfig_discovery_files: usize,
    pub(crate) tsconfig_glob_match_steps: usize,
    pub(crate) ancestor_search_entries: usize,
    pub(crate) ancestor_search_weight: usize,
}

include!("external_type_loading_parts/single_flight.rs");
include!("external_type_loading_parts/source_single_flight.rs");
include!("external_type_loading_parts/context_single_flight.rs");

#[derive(Clone, Debug)]
enum Vue3ExternalTypeContextCacheEntry {
    Loading(std::sync::Arc<Vue3ExternalTypeContextFlight>),
    Ready(std::sync::Arc<Vue27TypeContext>),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Vue3TypeResolverCacheIdentity {
    typescript_version: String,
    module_resolution: Vue3TypeModuleResolutionKind,
    module: Vue3TypeModuleKind,
    package_json_features: Vue3PackageJsonResolutionFeatures,
    type_reference_package_json_features: Vue3PackageJsonResolutionFeatures,
    module_suffixes: std::sync::Arc<[String]>,
}

impl Vue3TypeResolverCacheIdentity {
    fn from_resolver(type_resolver: &Vue3TypeResolverContext) -> Self {
        Self {
            typescript_version: type_resolver.typescript_version.to_string(),
            module_resolution: type_resolver.module_resolution,
            module: type_resolver.effective_module(),
            package_json_features: type_resolver.package_json_features(),
            type_reference_package_json_features: type_resolver
                .package_json_features_for_type_reference(false),
            module_suffixes: type_resolver.module_suffixes.clone(),
        }
    }

    fn payload_weight(&self) -> usize {
        self.module_suffixes.iter().fold(
            self.typescript_version
                .len()
                .saturating_add(std::mem::size_of::<Vue3TypeModuleResolutionKind>())
                .saturating_add(std::mem::size_of::<Vue3TypeModuleKind>())
                .saturating_add(std::mem::size_of::<Vue3PackageJsonResolutionFeatures>() * 2),
            |weight, suffix| {
                weight
                    .saturating_add(std::mem::size_of::<String>())
                    .saturating_add(suffix.len())
            },
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Vue3ExternalTypeContextCacheKey {
    path: PathBuf,
    resolver: Vue3TypeResolverCacheIdentity,
}

impl Vue3ExternalTypeContextCacheKey {
    fn payload_weight(&self) -> usize {
        self.path
            .as_os_str()
            .as_encoded_bytes()
            .len()
            .saturating_add(self.resolver.payload_weight())
    }
}

#[derive(Debug)]
struct Vue3ExternalTypeLoadState {
    limits: Vue3ExternalTypeLoadLimits,
    source_cache:
        BTreeMap<Vue3ExternalTypeSourceCacheKey, Vue3ExternalTypeSourceCacheEntry>,
    context_cache:
        BTreeMap<Vue3ExternalTypeContextCacheKey, Vue3ExternalTypeContextCacheEntry>,
    resolution_cache:
        BTreeMap<Vue3TypeImportResolutionCacheKey, Vue3TypeImportResolutionCacheEntry>,
    metadata_source_cache: BTreeMap<PathBuf, Vue3MetadataSourceCacheEntry>,
    metadata_path_identities: BTreeMap<PathBuf, PathBuf>,
    tsconfig_cache: BTreeMap<PathBuf, Vue3TsconfigCacheEntry>,
    package_json_cache: BTreeMap<PathBuf, Vue3PackageJsonCacheEntry>,
    tsconfig_node_states: BTreeSet<(PathBuf, PathBuf, PathBuf)>,
    ancestor_search_dirs: BTreeSet<PathBuf>,
    active_package_resolutions:
        std::collections::HashMap<std::thread::ThreadId, Vec<PathBuf>>,
    active_package_import_resolutions: std::collections::HashMap<
        std::thread::ThreadId,
        Vec<Vue3PackageImportResolutionIdentity>,
    >,
    context_waits:
        std::collections::HashMap<std::thread::ThreadId, Vue3ExternalTypeContextWaitEdge>,
    next_source_flight_id: u64,
    next_context_flight_id: u64,
    reserved_import_bytes: usize,
    reserved_global_bytes: usize,
    next_metadata_flight_id: u64,
    reserved_metadata_bytes: usize,
    metadata_generation: u64,
    metadata_blocked: bool,
    stats: Vue3ExternalTypeLoadStats,
    // Parent contexts are cached only when every recursive load completed.
    failure_epoch: usize,
}

impl Vue3ExternalTypeLoadState {
    fn new(limits: Vue3ExternalTypeLoadLimits) -> Self {
        Self {
            limits,
            source_cache: BTreeMap::new(),
            context_cache: BTreeMap::new(),
            resolution_cache: BTreeMap::new(),
            metadata_source_cache: BTreeMap::new(),
            metadata_path_identities: BTreeMap::new(),
            tsconfig_cache: BTreeMap::new(),
            package_json_cache: BTreeMap::new(),
            tsconfig_node_states: BTreeSet::new(),
            ancestor_search_dirs: BTreeSet::new(),
            active_package_resolutions: std::collections::HashMap::new(),
            active_package_import_resolutions: std::collections::HashMap::new(),
            context_waits: std::collections::HashMap::new(),
            next_source_flight_id: 0,
            next_context_flight_id: 0,
            reserved_import_bytes: 0,
            reserved_global_bytes: 0,
            next_metadata_flight_id: 0,
            reserved_metadata_bytes: 0,
            metadata_generation: 0,
            metadata_blocked: false,
            stats: Vue3ExternalTypeLoadStats::default(),
            failure_epoch: 0,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Vue3ExternalTypeLoadSession {
    state: std::sync::Arc<std::sync::Mutex<Vue3ExternalTypeLoadState>>,
}

impl std::fmt::Debug for Vue3ExternalTypeLoadSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.lock();
        formatter
            .debug_struct("Vue3ExternalTypeLoadSession")
            .field("limits", &state.limits)
            .field("stats", &state.stats)
            .field("source_cache_entries", &state.source_cache.len())
            .field("context_cache_entries", &state.context_cache.len())
            .field("resolution_cache_entries", &state.resolution_cache.len())
            .field(
                "metadata_source_cache_entries",
                &state.metadata_source_cache.len(),
            )
            .field("tsconfig_cache_entries", &state.tsconfig_cache.len())
            .field("package_json_cache_entries", &state.package_json_cache.len())
            .field("metadata_blocked", &state.metadata_blocked)
            .finish()
    }
}

impl Vue3ExternalTypeLoadSession {
    pub(crate) fn with_limits(limits: Vue3ExternalTypeLoadLimits) -> Self {
        Self {
            state: std::sync::Arc::new(std::sync::Mutex::new(
                Vue3ExternalTypeLoadState::new(limits),
            )),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vue3ExternalTypeLoadState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[cfg(test)]
    pub(crate) fn stats(&self) -> Vue3ExternalTypeLoadStats {
        self.lock().stats
    }

    pub(crate) fn limits(&self) -> Vue3ExternalTypeLoadLimits {
        self.lock().limits
    }

    pub(crate) fn failure_epoch(&self) -> usize {
        self.lock().failure_epoch
    }

    fn source_from_path_with_resolver(
        &self,
        path: &Path,
        kind: Vue3ExternalTypeSourceKind,
        type_resolver: &Vue3TypeResolverContext,
    ) -> Option<std::sync::Arc<Vue3ExternalTypeSource>> {
        let format = vue3_external_type_format_with_resolver(path, type_resolver)?;
        let cache_key = vue3_external_type_source_cache_key(path, kind, format);
        match self.begin_source_load(cache_key) {
            Vue3ExternalTypeSourceLoad::Ready(source) => Some(source),
            Vue3ExternalTypeSourceLoad::Wait(waiter) => waiter.wait(),
            Vue3ExternalTypeSourceLoad::Start(mut owner) => {
                let source = read_vue3_external_type_source(path, format, &mut owner);
                owner.complete(source)
            }
            Vue3ExternalTypeSourceLoad::Failed => None,
        }
    }

    #[cfg(test)]
    fn source_from_path(
        &self,
        path: &Path,
        kind: Vue3ExternalTypeSourceKind,
    ) -> Option<std::sync::Arc<Vue3ExternalTypeSource>> {
        let type_resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            module: Some(Vue3TypeModuleKind::EcmaScript),
            external_type_session: self.clone(),
            ..Vue3TypeResolverContext::default()
        };
        self.source_from_path_with_resolver(path, kind, &type_resolver)
    }

    fn record_context_failure(&self) {
        self.lock().failure_epoch += 1;
    }

    fn record_resolution_failure(&self) {
        self.lock().failure_epoch += 1;
    }

    fn has_context_build_capacity(&self) -> bool {
        let state = self.lock();
        state.stats.context_lookups < state.limits.max_context_lookups
            && state.stats.context_builds < state.limits.max_context_builds
            && state.stats.context_build_weight < state.limits.max_context_build_weight
    }

    fn begin_uncached_context_load(&self, source_weight: usize) -> bool {
        let mut state = self.lock();
        if state.stats.context_lookups >= state.limits.max_context_lookups {
            state.failure_epoch += 1;
            return false;
        }
        state.stats.context_lookups += 1;
        if state.stats.context_builds >= state.limits.max_context_builds {
            state.failure_epoch += 1;
            return false;
        }
        let remaining = state
            .limits
            .max_context_build_weight
            .saturating_sub(state.stats.context_build_weight);
        if source_weight > remaining {
            state.stats.context_build_weight = state.limits.max_context_build_weight;
            state.failure_epoch += 1;
            return false;
        }
        state.stats.context_builds += 1;
        state.stats.context_build_weight += source_weight;
        true
    }

    fn finish_uncached_context_load(
        &self,
        context: Vue27TypeContext,
    ) -> Option<Vue27TypeContext> {
        let context_weight = vue3_external_type_context_cache_cost(&context);
        let mut state = self.lock();
        let remaining = state
            .limits
            .max_context_build_weight
            .saturating_sub(state.stats.context_build_weight);
        if context_weight > remaining {
            state.stats.context_build_weight = state.limits.max_context_build_weight;
            state.failure_epoch += 1;
            return None;
        }
        state.stats.context_build_weight += context_weight;
        Some(context)
    }

}

impl Default for Vue3ExternalTypeLoadSession {
    fn default() -> Self {
        Self::with_limits(Vue3ExternalTypeLoadLimits::default())
    }
}

include!("external_type_loading_parts/context_cost.rs");
include!("external_type_loading_parts/resolution.rs");
include!("external_type_loading_parts/metadata.rs");

pub(crate) fn vue3_external_type_path_identity(path: &Path) -> PathBuf {
    let identity = std::fs::canonicalize(path).unwrap_or_else(|_| {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|current| current.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        };
        normalize_path_components(absolute)
    });
    vue3_external_type_path_key(identity)
}

fn vue3_external_type_lexical_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    vue3_external_type_path_key(normalize_path_components(absolute))
}

fn vue3_external_type_path_key(path: PathBuf) -> PathBuf {
    if cfg!(windows) {
        let mut path = path.into_os_string();
        path.make_ascii_lowercase();
        PathBuf::from(path)
    } else {
        path
    }
}

fn vue3_external_type_context_cache_key(
    path: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3ExternalTypeContextCacheKey {
    Vue3ExternalTypeContextCacheKey {
        path: vue3_external_type_lexical_path(path),
        resolver: Vue3TypeResolverCacheIdentity::from_resolver(type_resolver),
    }
}

fn vue3_external_type_source_cache_key(
    path: &Path,
    kind: Vue3ExternalTypeSourceKind,
    format: Vue3ExternalTypeFormat,
) -> Vue3ExternalTypeSourceCacheKey {
    Vue3ExternalTypeSourceCacheKey {
        semantic: vue3_external_type_semantic_identity(path, format),
        kind,
    }
}

fn vue3_external_type_semantic_identity(
    path: &Path,
    format: Vue3ExternalTypeFormat,
) -> Vue3ExternalTypeSemanticIdentity {
    Vue3ExternalTypeSemanticIdentity {
        path: vue3_external_type_path_identity(path),
        mode: vue3_external_type_source_mode(path, format),
    }
}

fn vue3_external_type_source_semantic_identity(
    path: &Path,
    source: &Vue3ExternalTypeSource,
) -> Vue3ExternalTypeSemanticIdentity {
    vue3_external_type_semantic_identity(
        path,
        Vue3ExternalTypeFormat {
            source_type: source.source_type,
            resolution_mode: source.resolution_mode,
            dynamic_resolution_mode: source.dynamic_resolution_mode,
        },
    )
}

fn vue3_external_type_source_mode(path: &Path, format: Vue3ExternalTypeFormat) -> String {
    let resolution = match format.resolution_mode {
        Vue3TypeResolutionMode::Import => "import",
        Vue3TypeResolutionMode::Require => "require",
    };
    let dynamic_resolution = match format.dynamic_resolution_mode {
        Vue3TypeResolutionMode::Import => "dynamic-import",
        Vue3TypeResolutionMode::Require => "dynamic-require",
    };
    if vue3_path_has_vue_extension(path) {
        return format!("vue:{resolution}:{dynamic_resolution}");
    }
    let source_type = format.source_type;
    let language = if source_type.is_typescript_definition() {
        "dts"
    } else if source_type.is_typescript() {
        "ts"
    } else {
        "js"
    };
    let module = match source_type.module_kind() {
        oxc_span::ModuleKind::Script => "script",
        oxc_span::ModuleKind::Module => "module",
        oxc_span::ModuleKind::Unambiguous => "unambiguous",
        oxc_span::ModuleKind::CommonJS => "commonjs",
    };
    let variant = if source_type.is_jsx() { "jsx" } else { "plain" };
    format!("{language}:{module}:{variant}:{resolution}:{dynamic_resolution}")
}

pub(crate) fn vue3_external_type_source_from_path(
    path: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<std::sync::Arc<Vue3ExternalTypeSource>> {
    type_resolver
        .external_type_session
        .source_from_path_with_resolver(path, Vue3ExternalTypeSourceKind::Import, type_resolver)
}

pub(crate) fn vue3_external_global_type_source_from_path(
    path: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<std::sync::Arc<Vue3ExternalTypeSource>> {
    type_resolver
        .external_type_session
        .source_from_path_with_resolver(path, Vue3ExternalTypeSourceKind::Global, type_resolver)
}

fn read_vue3_external_type_source(
    path: &Path,
    format: Vue3ExternalTypeFormat,
    owner: &mut Vue3ExternalTypeSourceOwner,
) -> Option<Vue3ExternalTypeSource> {
    let mut file = std::fs::File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    let declared_len_u64 = metadata.len();
    let declared_len = usize::try_from(declared_len_u64).ok()?;
    if !owner.reserve_bytes(declared_len) {
        return None;
    }
    let mut bytes = Vec::with_capacity(declared_len.min(64 * 1024));
    let read_failed = {
        let mut limited = std::io::Read::take(&mut file, declared_len as u64);
        std::io::Read::read_to_end(&mut limited, &mut bytes).is_err()
    };
    if read_failed {
        owner.record_bytes_read(bytes.len());
        return None;
    }
    let bytes_read = bytes.len();
    owner.record_bytes_read(bytes_read);
    let length_changed = file
        .metadata()
        .map_or(true, |metadata| metadata.len() != declared_len_u64);
    if bytes_read != declared_len || length_changed {
        return None;
    }
    let source = String::from_utf8(bytes).ok()?;
    if vue3_path_has_vue_extension(path) {
        return Some(vue3_external_vue_type_source(path, &source, format));
    }
    Some(Vue3ExternalTypeSource {
        source,
        source_type: format.source_type,
        resolution_mode: format.resolution_mode,
        dynamic_resolution_mode: format.dynamic_resolution_mode,
    })
}

fn vue3_external_vue_type_source(
    path: &Path,
    source: &str,
    format: Vue3ExternalTypeFormat,
) -> Vue3ExternalTypeSource {
    let mut sources = SourceMap::default();
    let source_file = sources.add_file(Some(path.to_path_buf()), source.to_string());
    let options = Vue3SfcParseOptions::default();
    let extracted = extract_sfc_blocks(
        source,
        source_file,
        SfcBlockContentMode::Vue3 { options: &options },
    );
    let descriptor = vue3_descriptor_from_blocks(
        normalize_path_string(path),
        source,
        source_file,
        extracted.blocks,
        &options,
    )
    .descriptor;
    let mut blocks = Vec::new();
    let mut source_type = oxc_span::SourceType::ts().with_module(true);
    for block in [descriptor.script.as_ref(), descriptor.script_setup.as_ref()]
        .into_iter()
        .flatten()
    {
        if block.attrs.lang.as_deref() == Some("tsx") {
            source_type = oxc_span::SourceType::tsx().with_module(true);
        }
        blocks.push(block.content.as_str());
    }
    Vue3ExternalTypeSource {
        source: blocks.join("\n"),
        source_type,
        resolution_mode: format.resolution_mode,
        dynamic_resolution_mode: format.dynamic_resolution_mode,
    }
}

#[cfg(test)]
fn vue3_external_type_format(
    path: &Path,
    session: &Vue3ExternalTypeLoadSession,
) -> Option<Vue3ExternalTypeFormat> {
    let type_resolver = Vue3TypeResolverContext {
        module_resolution: Vue3TypeModuleResolutionKind::Bundler,
        module: Some(Vue3TypeModuleKind::EcmaScript),
        external_type_session: session.clone(),
        ..Vue3TypeResolverContext::default()
    };
    vue3_external_type_format_with_resolver(path, &type_resolver)
}

fn vue3_external_type_format_with_resolver(
    path: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3ExternalTypeFormat> {
    let lexical_path = normalize_path_components(path.to_path_buf());
    let mut source_type = vue3_type_source_type(&normalize_path_string(&lexical_path));
    let effective_module = type_resolver.effective_module();
    if vue3_path_has_vue_extension(&lexical_path) {
        let (resolution_mode, dynamic_resolution_mode) =
            vue3_inline_type_resolution_modes(source_type, type_resolver);
        return Some(Vue3ExternalTypeFormat {
            source_type,
            resolution_mode,
            dynamic_resolution_mode,
        });
    }
    if !vue3_path_has_ambiguous_module_extension(&lexical_path) {
        let resolution_mode = vue3_static_resolution_mode(source_type);
        return Some(Vue3ExternalTypeFormat {
            source_type,
            resolution_mode,
            dynamic_resolution_mode: if vue3_import_syntax_affects_module_resolution(
                type_resolver,
            ) {
                vue3_dynamic_resolution_mode(effective_module, resolution_mode)
            } else {
                Vue3TypeResolutionMode::Import
            },
        });
    }

    let should_lookup_package_scope = matches!(
        type_resolver.module_resolution,
        Vue3TypeModuleResolutionKind::Node16 | Vue3TypeModuleResolutionKind::NodeNext
    ) || vue3_path_contains_node_modules(&lexical_path);
    let package_module_type = if should_lookup_package_scope {
        vue3_package_module_type_for_path(
            &lexical_path,
            &type_resolver.external_type_session,
        )?
    } else {
        Vue3PackageModuleType::Unspecified
    };
    let package_implied_resolution_mode = match package_module_type {
        Vue3PackageModuleType::Module => {
            if !source_type.is_typescript_definition() {
                source_type = source_type.with_module(true);
            }
            Vue3TypeResolutionMode::Import
        }
        Vue3PackageModuleType::CommonJs => Vue3TypeResolutionMode::Require,
        Vue3PackageModuleType::Unspecified
            if matches!(
                effective_module,
                Vue3TypeModuleKind::Node16 | Vue3TypeModuleKind::NodeNext
            ) =>
        {
            Vue3TypeResolutionMode::Require
        }
        Vue3PackageModuleType::Unspecified => {
            vue3_module_fallback_resolution_mode(effective_module)
        }
    };
    let resolution_mode = if vue3_import_syntax_affects_module_resolution(type_resolver) {
        package_implied_resolution_mode
    } else {
        vue3_static_resolution_mode(source_type)
    };
    Some(Vue3ExternalTypeFormat {
        source_type,
        resolution_mode,
        dynamic_resolution_mode: if vue3_import_syntax_affects_module_resolution(type_resolver) {
            vue3_dynamic_resolution_mode(effective_module, resolution_mode)
        } else {
            Vue3TypeResolutionMode::Import
        },
    })
}

fn vue3_import_syntax_affects_module_resolution(
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    matches!(
        type_resolver.module_resolution,
        Vue3TypeModuleResolutionKind::Node16 | Vue3TypeModuleResolutionKind::NodeNext
    ) || {
        let features = type_resolver.package_json_features();
        features.exports || features.imports
    }
}

pub(crate) fn vue3_inline_type_resolution_modes(
    source_type: oxc_span::SourceType,
    type_resolver: &Vue3TypeResolverContext,
) -> (Vue3TypeResolutionMode, Vue3TypeResolutionMode) {
    if !vue3_import_syntax_affects_module_resolution(type_resolver) {
        return (
            vue3_static_resolution_mode(source_type),
            Vue3TypeResolutionMode::Import,
        );
    }
    let effective_module = type_resolver.effective_module();
    let static_resolution_mode = vue3_module_fallback_resolution_mode(effective_module);
    (
        static_resolution_mode,
        vue3_dynamic_resolution_mode(effective_module, static_resolution_mode),
    )
}

fn vue3_module_fallback_resolution_mode(
    module: Vue3TypeModuleKind,
) -> Vue3TypeResolutionMode {
    match module {
        Vue3TypeModuleKind::CommonJs
        | Vue3TypeModuleKind::Node16
        | Vue3TypeModuleKind::NodeNext => Vue3TypeResolutionMode::Require,
        Vue3TypeModuleKind::Classic
        | Vue3TypeModuleKind::EcmaScript
        | Vue3TypeModuleKind::Preserve => Vue3TypeResolutionMode::Import,
    }
}

fn vue3_dynamic_resolution_mode(
    module: Vue3TypeModuleKind,
    static_resolution_mode: Vue3TypeResolutionMode,
) -> Vue3TypeResolutionMode {
    match module {
        Vue3TypeModuleKind::Node16
        | Vue3TypeModuleKind::NodeNext
        | Vue3TypeModuleKind::Preserve => Vue3TypeResolutionMode::Import,
        Vue3TypeModuleKind::Classic => Vue3TypeResolutionMode::Require,
        Vue3TypeModuleKind::CommonJs | Vue3TypeModuleKind::EcmaScript => static_resolution_mode,
    }
}

fn vue3_path_has_ambiguous_module_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["ts", "tsx", "js", "jsx"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

fn vue3_path_has_vue_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vue"))
}

fn vue3_path_contains_node_modules(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(component, std::path::Component::Normal(name) if vue3_path_component_is_node_modules(name))
    })
}

fn vue3_path_component_is_node_modules(name: &std::ffi::OsStr) -> bool {
    if cfg!(windows) {
        name.to_str()
            .is_some_and(|name| name.eq_ignore_ascii_case("node_modules"))
    } else {
        name == std::ffi::OsStr::new("node_modules")
    }
}

pub(crate) fn vue3_type_source_type(filename: &str) -> oxc_span::SourceType {
    if let Ok(source_type) = oxc_span::SourceType::from_path(filename) {
        return source_type;
    }
    let lowercase_filename = Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_ascii_lowercase);
    lowercase_filename
        .as_deref()
        .and_then(|filename| oxc_span::SourceType::from_path(filename).ok())
        .unwrap_or_else(oxc_span::SourceType::ts)
}
