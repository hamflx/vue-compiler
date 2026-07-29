#[derive(Clone, Debug)]
pub(crate) struct Vue3TsconfigPathMapping {
    pub(crate) pattern: String,
    pub(crate) targets: Vec<String>,
    pub(crate) target_base_dir: PathBuf,
    pub(crate) template_config_dir: PathBuf,
}

impl Vue3TsconfigPathMapping {
    fn payload_weight(&self) -> usize {
        self.targets.iter().fold(
            std::mem::size_of::<Self>()
                .saturating_add(self.pattern.len())
                .saturating_add(self.target_base_dir.as_os_str().as_encoded_bytes().len())
                .saturating_add(
                    self.template_config_dir
                        .as_os_str()
                        .as_encoded_bytes()
                        .len(),
                ),
            |weight, target| {
                weight
                    .saturating_add(std::mem::size_of::<String>())
                    .saturating_add(target.len())
            },
        )
    }
}

#[derive(Debug, Default)]
struct Vue3TsconfigModuleResolutionSettings {
    path_mappings: Option<Vec<Vue3TsconfigPathMapping>>,
    paths_base_dir: Option<PathBuf>,
    base_url: Option<PathBuf>,
    base_url_is_declared: bool,
}

impl Vue3TsconfigModuleResolutionSettings {
    fn inherit(&mut self, mut inherited: Self) {
        if inherited.path_mappings.is_some() {
            self.path_mappings = inherited.path_mappings.take();
            self.paths_base_dir = inherited.paths_base_dir.take();
        }
        if inherited.base_url_is_declared {
            self.base_url = inherited.base_url;
            self.base_url_is_declared = true;
        }
    }

    fn apply_effective_paths_base(
        &mut self,
        typescript_version: &nodejs_semver::Version,
        type_resolver: &Vue3TypeResolverContext,
    ) -> bool {
        let target_base_dir = if typescript_version < &(7, 0, 0).into() {
            self.base_url.as_ref()
        } else {
            None
        }
        .or(self.paths_base_dir.as_ref());
        let Some(target_base_dir) = target_base_dir else {
            return true;
        };
        for mapping in self.path_mappings.iter_mut().flatten() {
            if mapping.target_base_dir == *target_base_dir {
                continue;
            }
            let weight = std::mem::size_of::<PathBuf>()
                .saturating_add(target_base_dir.as_os_str().as_encoded_bytes().len());
            if !type_resolver
                .external_type_session
                .claim_tsconfig_materialization(weight)
            {
                return false;
            }
            mapping.target_base_dir.clone_from(target_base_dir);
        }
        true
    }

    fn path_mappings(&self) -> &[Vue3TsconfigPathMapping] {
        self.path_mappings.as_deref().unwrap_or_default()
    }

    fn payload_weight(&self) -> usize {
        let base_weight = std::mem::size_of::<Self>()
            .saturating_add(
                self.paths_base_dir
                    .as_ref()
                    .map_or(0, |path| path.as_os_str().as_encoded_bytes().len()),
            )
            .saturating_add(
                self.base_url
                    .as_ref()
                    .map_or(0, |path| path.as_os_str().as_encoded_bytes().len()),
            );
        self.path_mappings
            .iter()
            .flatten()
            .fold(base_weight, |weight, mapping| {
                weight.saturating_add(mapping.payload_weight())
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Vue3TsconfigTargetKind {
    Default,
    Legacy,
    Modern,
}

#[derive(Clone, Debug)]
struct Vue3TsconfigInheritedOption<T> {
    value: Option<T>,
    is_declared: bool,
}

impl<T> Vue3TsconfigInheritedOption<T> {
    fn inherit(&mut self, inherited: Self) {
        if inherited.is_declared {
            *self = inherited;
        }
    }

    fn set(&mut self, value: Option<T>) {
        self.value = value;
        self.is_declared = true;
    }
}

impl<T> Default for Vue3TsconfigInheritedOption<T> {
    fn default() -> Self {
        Self {
            value: None,
            is_declared: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Vue3TsconfigInheritedResolverOptions {
    module: Vue3TsconfigInheritedOption<Vue3TypeModuleKind>,
    module_resolution: Vue3TsconfigInheritedOption<Vue3TypeModuleResolutionKind>,
    module_suffixes: Vue3TsconfigInheritedOption<std::sync::Arc<[String]>>,
    root_dirs: Vue3TsconfigInheritedOption<std::sync::Arc<[PathBuf]>>,
    allow_js: Vue3TsconfigInheritedOption<bool>,
    check_js: Vue3TsconfigInheritedOption<bool>,
    custom_conditions: Vue3TsconfigInheritedOption<Vue3CustomConditionSet>,
    resolve_package_json_exports: Vue3TsconfigInheritedOption<bool>,
    resolve_package_json_imports: Vue3TsconfigInheritedOption<bool>,
    target: Vue3TsconfigInheritedOption<Vue3TsconfigTargetKind>,
}

impl Vue3TsconfigInheritedResolverOptions {
    fn inherit(&mut self, inherited: Self) {
        self.module.inherit(inherited.module);
        self.module_resolution.inherit(inherited.module_resolution);
        self.module_suffixes.inherit(inherited.module_suffixes);
        self.root_dirs.inherit(inherited.root_dirs);
        self.allow_js.inherit(inherited.allow_js);
        self.check_js.inherit(inherited.check_js);
        self.custom_conditions.inherit(inherited.custom_conditions);
        self.resolve_package_json_exports
            .inherit(inherited.resolve_package_json_exports);
        self.resolve_package_json_imports
            .inherit(inherited.resolve_package_json_imports);
        self.target.inherit(inherited.target);
    }

    fn effective_module(
        &self,
        typescript_version: &nodejs_semver::Version,
    ) -> Vue3TypeModuleKind {
        self.module.value.unwrap_or_else(|| match self.target.value {
            Some(Vue3TsconfigTargetKind::Legacy) => Vue3TypeModuleKind::CommonJs,
            Some(Vue3TsconfigTargetKind::Modern) => Vue3TypeModuleKind::EcmaScript,
            Some(Vue3TsconfigTargetKind::Default) | None
                if typescript_version >= &(6, 0, 0).into() =>
            {
                Vue3TypeModuleKind::EcmaScript
            }
            Some(Vue3TsconfigTargetKind::Default) | None => Vue3TypeModuleKind::CommonJs,
        })
    }

    fn effective_module_resolution(
        &self,
        typescript_version: &nodejs_semver::Version,
    ) -> Vue3TypeModuleResolutionKind {
        if let Some(module_resolution) = self.module_resolution.value {
            return module_resolution;
        }
        let module = self.effective_module(typescript_version);
        match module {
            Vue3TypeModuleKind::Classic => Vue3TypeModuleResolutionKind::Classic,
            Vue3TypeModuleKind::Node16 => Vue3TypeModuleResolutionKind::Node16,
            Vue3TypeModuleKind::NodeNext => Vue3TypeModuleResolutionKind::NodeNext,
            Vue3TypeModuleKind::Preserve => Vue3TypeModuleResolutionKind::Bundler,
            Vue3TypeModuleKind::CommonJs if typescript_version < &(6, 0, 0).into() =>
            {
                Vue3TypeModuleResolutionKind::Node10
            }
            Vue3TypeModuleKind::EcmaScript if typescript_version < &(6, 0, 0).into() =>
            {
                Vue3TypeModuleResolutionKind::Classic
            }
            Vue3TypeModuleKind::CommonJs | Vue3TypeModuleKind::EcmaScript => {
                Vue3TypeModuleResolutionKind::Bundler
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Vue3TsconfigTypeResolverOptions {
    pub(crate) module_resolution: Vue3TypeModuleResolutionKind,
    pub(crate) module: Vue3TypeModuleKind,
    pub(crate) module_suffixes: std::sync::Arc<[String]>,
    pub(crate) root_dirs: std::sync::Arc<[PathBuf]>,
    pub(crate) allow_js: bool,
    pub(crate) custom_conditions: Vue3CustomConditionSet,
    pub(crate) resolve_package_json_exports: Option<bool>,
    pub(crate) resolve_package_json_imports: Option<bool>,
}

type Vue3TsconfigGraphStateKey = (PathBuf, PathBuf, PathBuf);
type Vue3TsconfigTypeRootsOverride = Option<std::sync::Arc<[PathBuf]>>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Vue3TsconfigModuleResolutionCacheKey {
    state: Vue3TsconfigGraphStateKey,
    typescript_version: String,
}

impl Vue3TsconfigModuleResolutionCacheKey {
    fn new(
        config_path: &Path,
        template_config_dir: &Path,
        type_resolver: &Vue3TypeResolverContext,
    ) -> Self {
        Self {
            state: vue3_tsconfig_graph_state_key(config_path, template_config_dir),
            typescript_version: type_resolver.typescript_version.to_string(),
        }
    }

    fn payload_weight(&self) -> usize {
        [&self.state.0, &self.state.1, &self.state.2]
            .into_iter()
            .fold(
            std::mem::size_of::<Self>().saturating_add(self.typescript_version.len()),
            |weight, path| {
                weight.saturating_add(path.as_os_str().as_encoded_bytes().len())
            },
        )
    }
}

type Vue3TsconfigModuleResolutionFlight =
    Vue3SingleFlight<Vue3TsconfigModuleResolutionSettings>;

#[derive(Clone, Debug)]
enum Vue3TsconfigModuleResolutionCacheEntry {
    Loading(std::sync::Arc<Vue3TsconfigModuleResolutionFlight>),
    Ready(std::sync::Arc<Vue3TsconfigModuleResolutionSettings>),
}

enum Vue3TsconfigModuleResolutionLoad {
    Ready(std::sync::Arc<Vue3TsconfigModuleResolutionSettings>),
    Wait(Vue3TsconfigModuleResolutionWaiter),
    Start(Vue3TsconfigModuleResolutionOwner),
    Blocked,
}

struct Vue3TsconfigModuleResolutionWaiter {
    session: Vue3ExternalTypeLoadSession,
    flight: std::sync::Arc<Vue3TsconfigModuleResolutionFlight>,
}

impl Vue3TsconfigModuleResolutionWaiter {
    fn wait(self) -> Option<std::sync::Arc<Vue3TsconfigModuleResolutionSettings>> {
        match self.flight.wait() {
            Vue3SingleFlightOutcome::Complete(Some(settings)) => {
                let mut state = self.session.lock();
                if state.metadata_blocked
                    || state.metadata_generation != self.flight.generation
                {
                    state.failure_epoch += 1;
                    return None;
                }
                state.stats.tsconfig_settings_cache_hits += 1;
                Some(settings)
            }
            Vue3SingleFlightOutcome::Complete(None) | Vue3SingleFlightOutcome::Aborted => {
                if !self.session.metadata_is_blocked() {
                    self.session.block_metadata();
                }
                None
            }
        }
    }
}

struct Vue3TsconfigModuleResolutionOwner {
    session: Vue3ExternalTypeLoadSession,
    cache_key: Vue3TsconfigModuleResolutionCacheKey,
    flight: std::sync::Arc<Vue3TsconfigModuleResolutionFlight>,
    active: bool,
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Vue3TsconfigModuleResolutionOwner {
    fn complete(
        mut self,
        settings: Vue3TsconfigModuleResolutionSettings,
    ) -> Option<std::sync::Arc<Vue3TsconfigModuleResolutionSettings>> {
        let settings = std::sync::Arc::new(settings);
        let cache_weight = self
            .cache_key
            .payload_weight()
            .saturating_add(settings.payload_weight());
        let session = self.session.clone();
        let mut state = session.lock();
        let current = vue3_tsconfig_module_resolution_flight_matches(
            state.tsconfig_module_resolution_cache.get(&self.cache_key),
            self.flight.id,
        );
        let stale = state.metadata_blocked
            || state.metadata_generation != self.flight.generation
            || !current;
        if stale {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            self.flight.abort();
            self.active = false;
            return None;
        }

        let ready_entries = state
            .tsconfig_module_resolution_cache
            .values()
            .filter(|entry| matches!(entry, Vue3TsconfigModuleResolutionCacheEntry::Ready(_)))
            .count();
        let remaining_weight = state
            .limits
            .max_tsconfig_settings_cache_weight
            .saturating_sub(state.stats.cached_tsconfig_settings_weight);
        let retain = ready_entries < state.limits.max_tsconfig_settings_cache_entries
            && cache_weight <= state.limits.max_tsconfig_settings_cache_entry_weight
            && cache_weight <= remaining_weight;
        if retain {
            state.tsconfig_module_resolution_cache.insert(
                self.cache_key.clone(),
                Vue3TsconfigModuleResolutionCacheEntry::Ready(settings.clone()),
            );
            state.stats.cached_tsconfig_settings_weight += cache_weight;
        } else {
            state
                .tsconfig_module_resolution_cache
                .remove(&self.cache_key);
        }
        // Publish while holding the session lock so an unretained result still has
        // one linear completion point for both current waiters and later callers.
        self.flight.complete(Some(settings.clone()));
        drop(state);
        self.active = false;
        Some(settings)
    }
}

impl Drop for Vue3TsconfigModuleResolutionOwner {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let session = self.session.clone();
        let mut state = session.lock();
        let flights = vue3_block_metadata_state(&mut state);
        drop(state);
        vue3_abort_metadata_flights(flights);
        self.flight.abort();
        self.active = false;
    }
}

impl Vue3ExternalTypeLoadSession {
    fn begin_tsconfig_module_resolution_load(
        &self,
        cache_key: Vue3TsconfigModuleResolutionCacheKey,
    ) -> Vue3TsconfigModuleResolutionLoad {
        let owner = std::thread::current().id();
        let mut state = self.lock();
        if state.metadata_blocked {
            state.failure_epoch += 1;
            return Vue3TsconfigModuleResolutionLoad::Blocked;
        }
        match state
            .tsconfig_module_resolution_cache
            .get(&cache_key)
            .cloned()
        {
            Some(Vue3TsconfigModuleResolutionCacheEntry::Ready(settings)) => {
                state.stats.tsconfig_settings_cache_hits += 1;
                return Vue3TsconfigModuleResolutionLoad::Ready(settings);
            }
            Some(Vue3TsconfigModuleResolutionCacheEntry::Loading(flight)) => {
                if flight.owner == owner {
                    let flights = vue3_block_metadata_state(&mut state);
                    drop(state);
                    vue3_abort_metadata_flights(flights);
                    return Vue3TsconfigModuleResolutionLoad::Blocked;
                }
                return Vue3TsconfigModuleResolutionLoad::Wait(
                    Vue3TsconfigModuleResolutionWaiter {
                        session: self.clone(),
                        flight,
                    },
                );
            }
            None => {}
        }

        let loading_entries = state
            .tsconfig_module_resolution_cache
            .values()
            .filter(|entry| matches!(entry, Vue3TsconfigModuleResolutionCacheEntry::Loading(_)))
            .count();
        if loading_entries >= state.limits.max_tsconfig_nodes {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            return Vue3TsconfigModuleResolutionLoad::Blocked;
        }
        let flight_id = state.next_metadata_flight_id;
        let Some(next_flight_id) = flight_id.checked_add(1) else {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            return Vue3TsconfigModuleResolutionLoad::Blocked;
        };
        state.next_metadata_flight_id = next_flight_id;
        let flight = std::sync::Arc::new(Vue3TsconfigModuleResolutionFlight::new(
            flight_id,
            owner,
            state.metadata_generation,
        ));
        state.tsconfig_module_resolution_cache.insert(
            cache_key.clone(),
            Vue3TsconfigModuleResolutionCacheEntry::Loading(flight.clone()),
        );
        drop(state);
        Vue3TsconfigModuleResolutionLoad::Start(Vue3TsconfigModuleResolutionOwner {
            session: self.clone(),
            cache_key,
            flight,
            active: true,
            _not_send: std::marker::PhantomData,
        })
    }
}

fn vue3_tsconfig_module_resolution_flight_matches(
    entry: Option<&Vue3TsconfigModuleResolutionCacheEntry>,
    flight_id: u64,
) -> bool {
    matches!(
        entry,
        Some(Vue3TsconfigModuleResolutionCacheEntry::Loading(flight))
            if flight.id == flight_id
    )
}

#[cfg(test)]
mod vue3_tsconfig_module_resolution_single_flight_tests {
    use super::*;
    use std::time::Duration;

    fn cache_key(name: &str) -> Vue3TsconfigModuleResolutionCacheKey {
        Vue3TsconfigModuleResolutionCacheKey {
            state: (
                PathBuf::from(format!("/{name}/tsconfig.json")),
                PathBuf::from(format!("/{name}")),
                PathBuf::from(format!("/{name}")),
            ),
            typescript_version: "5.0.0".to_string(),
        }
    }

    fn settings(name: &str) -> Vue3TsconfigModuleResolutionSettings {
        Vue3TsconfigModuleResolutionSettings {
            base_url: Some(PathBuf::from(format!("/{name}/src"))),
            base_url_is_declared: true,
            ..Vue3TsconfigModuleResolutionSettings::default()
        }
    }

    fn start(
        session: &Vue3ExternalTypeLoadSession,
        key: Vue3TsconfigModuleResolutionCacheKey,
    ) -> Vue3TsconfigModuleResolutionOwner {
        match session.begin_tsconfig_module_resolution_load(key) {
            Vue3TsconfigModuleResolutionLoad::Start(owner) => owner,
            _ => panic!("expected tsconfig settings owner"),
        }
    }

    #[test]
    fn shares_owner_result_with_waiters_and_ready_cache_hits() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = cache_key("shared");
        let owner = start(&session, key.clone());
        let (waiting_tx, waiting_rx) = std::sync::mpsc::channel();
        let worker_session = session.clone();
        let worker_key = key.clone();
        let worker = std::thread::spawn(move || {
            let waiter = match worker_session.begin_tsconfig_module_resolution_load(worker_key) {
                Vue3TsconfigModuleResolutionLoad::Wait(waiter) => waiter,
                _ => panic!("expected tsconfig settings waiter"),
            };
            waiting_tx.send(()).expect("announce waiter");
            waiter.wait().expect("shared tsconfig settings")
        });
        waiting_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("waiter registered");

        let owned = owner.complete(settings("shared")).expect("owner result");
        let waited = worker.join().expect("join waiter");
        assert!(std::sync::Arc::ptr_eq(&owned, &waited));
        let cached = match session.begin_tsconfig_module_resolution_load(key) {
            Vue3TsconfigModuleResolutionLoad::Ready(cached) => cached,
            _ => panic!("expected ready tsconfig settings"),
        };
        assert!(std::sync::Arc::ptr_eq(&owned, &cached));
        assert_eq!(session.stats().tsconfig_settings_cache_hits, 2);
        assert!(!session.metadata_is_blocked());
    }

    #[test]
    fn shares_in_flight_result_when_cache_retention_is_disabled() {
        let session = Vue3ExternalTypeLoadSession::with_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_settings_cache_entries: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let key = cache_key("unretained");
        let owner = start(&session, key.clone());
        let (waiting_tx, waiting_rx) = std::sync::mpsc::channel();
        let worker_session = session.clone();
        let worker_key = key.clone();
        let worker = std::thread::spawn(move || {
            let waiter = match worker_session.begin_tsconfig_module_resolution_load(worker_key) {
                Vue3TsconfigModuleResolutionLoad::Wait(waiter) => waiter,
                _ => panic!("expected tsconfig settings waiter"),
            };
            waiting_tx.send(()).expect("announce waiter");
            waiter.wait().expect("shared unretained settings")
        });
        waiting_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("waiter registered");

        let owned = owner
            .complete(settings("unretained"))
            .expect("owner result");
        let waited = worker.join().expect("join waiter");
        assert!(std::sync::Arc::ptr_eq(&owned, &waited));
        assert!(session.lock().tsconfig_module_resolution_cache.is_empty());

        let rebuilt = start(&session, key)
            .complete(settings("rebuilt"))
            .expect("rebuilt result");
        assert!(!std::sync::Arc::ptr_eq(&owned, &rebuilt));
        assert_eq!(session.stats().tsconfig_settings_cache_hits, 1);
        assert!(!session.metadata_is_blocked());
    }

    #[test]
    fn same_thread_reentry_fails_closed_without_deadlocking() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = cache_key("reentrant");
        let owner = start(&session, key.clone());

        assert!(matches!(
            session.begin_tsconfig_module_resolution_load(key),
            Vue3TsconfigModuleResolutionLoad::Blocked
        ));
        assert!(owner.complete(settings("reentrant")).is_none());
        assert!(session.metadata_is_blocked());
    }

    #[test]
    fn owner_drop_aborts_waiters_and_blocks_partial_metadata_state() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = cache_key("aborted");
        let owner = start(&session, key.clone());
        let (waiting_tx, waiting_rx) = std::sync::mpsc::channel();
        let worker_session = session.clone();
        let worker = std::thread::spawn(move || {
            let waiter = match worker_session.begin_tsconfig_module_resolution_load(key) {
                Vue3TsconfigModuleResolutionLoad::Wait(waiter) => waiter,
                _ => panic!("expected tsconfig settings waiter"),
            };
            waiting_tx.send(()).expect("announce waiter");
            waiter.wait()
        });
        waiting_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("waiter registered");

        drop(owner);
        assert!(worker.join().expect("join waiter").is_none());
        assert!(session.metadata_is_blocked());
        assert!(session.lock().tsconfig_module_resolution_cache.is_empty());
    }
}

fn vue3_materialize_tsconfig_strings<'a>(
    values: impl IntoIterator<Item = &'a str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vec<String>> {
    let mut materialized = Vec::new();
    for value in values {
        let weight = std::mem::size_of::<String>().saturating_add(value.len());
        if !type_resolver
            .external_type_session
            .claim_tsconfig_materialization(weight)
        {
            return None;
        }
        materialized.push(value.to_string());
    }
    Some(materialized)
}

#[derive(Clone, Debug)]
struct Vue3TsconfigPathSpecList {
    targets: std::sync::Arc<[String]>,
    config_dir: std::sync::Arc<PathBuf>,
    template_config_dir: std::sync::Arc<PathBuf>,
}

impl Vue3TsconfigPathSpecList {
    fn from_array(
        value: &serde_json::Value,
        config_dir: &Path,
        template_config_dir: &Path,
        type_resolver: &Vue3TypeResolverContext,
    ) -> Option<Self> {
        let targets = vue3_materialize_tsconfig_strings(
            vue3_tsconfig_string_array(Some(value)),
            type_resolver,
        )?;
        Self::from_materialized_targets(targets, config_dir, template_config_dir, type_resolver)
    }

    fn from_string(
        value: &serde_json::Value,
        config_dir: &Path,
        template_config_dir: &Path,
        type_resolver: &Vue3TypeResolverContext,
    ) -> Option<Self> {
        let targets =
            vue3_materialize_tsconfig_strings(value.as_str(), type_resolver)?;
        Self::from_materialized_targets(targets, config_dir, template_config_dir, type_resolver)
    }

    fn from_materialized_targets(
        targets: Vec<String>,
        config_dir: &Path,
        template_config_dir: &Path,
        type_resolver: &Vue3TypeResolverContext,
    ) -> Option<Self> {
        let weight = std::mem::size_of::<Self>()
            .saturating_add(std::mem::size_of::<PathBuf>().saturating_mul(2))
            .saturating_add(config_dir.as_os_str().as_encoded_bytes().len())
            .saturating_add(template_config_dir.as_os_str().as_encoded_bytes().len());
        if !type_resolver
            .external_type_session
            .claim_tsconfig_materialization(weight)
        {
            return None;
        }
        Some(Self::from_targets(
            targets,
            config_dir,
            template_config_dir,
        ))
    }

    fn from_targets(
        targets: Vec<String>,
        config_dir: &Path,
        template_config_dir: &Path,
    ) -> Self {
        Self {
            targets: std::sync::Arc::from(targets),
            config_dir: std::sync::Arc::new(config_dir.to_path_buf()),
            template_config_dir: std::sync::Arc::new(template_config_dir.to_path_buf()),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Vue3TsconfigFileSpecs {
    files: Option<Vue3TsconfigPathSpecList>,
    include: Option<Vue3TsconfigPathSpecList>,
    exclude: Option<Vue3TsconfigPathSpecList>,
}

impl Vue3TsconfigFileSpecs {
    fn overlay(&mut self, overlay: Self) {
        if overlay.files.is_some() {
            self.files = overlay.files;
        }
        if overlay.include.is_some() {
            self.include = overlay.include;
        }
        if overlay.exclude.is_some() {
            self.exclude = overlay.exclude;
        }
    }

    fn apply_direct(
        &mut self,
        value: &serde_json::Value,
        config_dir: &Path,
        template_config_dir: &Path,
        type_resolver: &Vue3TypeResolverContext,
    ) -> bool {
        if let Some(files) = value.get("files") {
            let Some(files) = Vue3TsconfigPathSpecList::from_array(
                files,
                config_dir,
                template_config_dir,
                type_resolver,
            ) else {
                return false;
            };
            self.files = Some(files);
        }
        if let Some(include) = value.get("include") {
            let Some(include) = Vue3TsconfigPathSpecList::from_array(
                include,
                config_dir,
                template_config_dir,
                type_resolver,
            ) else {
                return false;
            };
            self.include = Some(include);
        }
        if let Some(exclude) = value.get("exclude") {
            let Some(exclude) = Vue3TsconfigPathSpecList::from_array(
                exclude,
                config_dir,
                template_config_dir,
                type_resolver,
            ) else {
                return false;
            };
            self.exclude = Some(exclude);
        }
        true
    }
}

#[derive(Clone, Debug, Default)]
struct Vue3TsconfigGlobalTypePackageSpecs {
    types: Option<std::sync::Arc<[String]>>,
    type_roots: Option<Vue3TsconfigPathSpecList>,
}

impl Vue3TsconfigGlobalTypePackageSpecs {
    fn overlay(&mut self, overlay: Self) {
        if overlay.types.is_some() {
            self.types = overlay.types;
        }
        if overlay.type_roots.is_some() {
            self.type_roots = overlay.type_roots;
        }
    }

    fn apply_direct(
        &mut self,
        value: &serde_json::Value,
        config_dir: &Path,
        template_config_dir: &Path,
        type_resolver: &Vue3TypeResolverContext,
    ) -> bool {
        let Some(compiler_options) = value
            .get("compilerOptions")
            .and_then(serde_json::Value::as_object)
        else {
            return true;
        };
        if let Some(types) = compiler_options.get("types") {
            let Some(types) = vue3_materialize_tsconfig_strings(
                vue3_tsconfig_string_array(Some(types)),
                type_resolver,
            ) else {
                return false;
            };
            self.types = Some(std::sync::Arc::from(types));
        }
        if let Some(type_roots) = compiler_options.get("typeRoots") {
            let Some(type_roots) = Vue3TsconfigPathSpecList::from_array(
                type_roots,
                config_dir,
                template_config_dir,
                type_resolver,
            ) else {
                return false;
            };
            self.type_roots = Some(type_roots);
        }
        true
    }
}

#[derive(Clone, Debug, Default)]
struct Vue3TsconfigOutputDirectorySpecs {
    out_dir: Option<Vue3TsconfigPathSpecList>,
    declaration_dir: Option<Vue3TsconfigPathSpecList>,
}

impl Vue3TsconfigOutputDirectorySpecs {
    fn overlay(&mut self, overlay: Self) {
        if overlay.out_dir.is_some() {
            self.out_dir = overlay.out_dir;
        }
        if overlay.declaration_dir.is_some() {
            self.declaration_dir = overlay.declaration_dir;
        }
    }

    fn apply_direct(
        &mut self,
        value: &serde_json::Value,
        config_dir: &Path,
        template_config_dir: &Path,
        type_resolver: &Vue3TypeResolverContext,
    ) -> bool {
        let Some(compiler_options) = value
            .get("compilerOptions")
            .and_then(serde_json::Value::as_object)
        else {
            return true;
        };
        if let Some(out_dir) = compiler_options.get("outDir") {
            let Some(out_dir) = Vue3TsconfigPathSpecList::from_string(
                out_dir,
                config_dir,
                template_config_dir,
                type_resolver,
            ) else {
                return false;
            };
            self.out_dir = Some(out_dir);
        }
        if let Some(declaration_dir) = compiler_options.get("declarationDir") {
            let Some(declaration_dir) = Vue3TsconfigPathSpecList::from_string(
                declaration_dir,
                config_dir,
                template_config_dir,
                type_resolver,
            ) else {
                return false;
            };
            self.declaration_dir = Some(declaration_dir);
        }
        true
    }
}

#[derive(Debug, Default)]
struct Vue3TsconfigExcludes {
    patterns: Vec<String>,
    directory_keys: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default)]
struct Vue3TsconfigGlobalSpecs {
    file_specs: Vue3TsconfigFileSpecs,
    type_package_specs: Vue3TsconfigGlobalTypePackageSpecs,
    output_directory_specs: Vue3TsconfigOutputDirectorySpecs,
}

impl Vue3TsconfigGlobalSpecs {
    fn overlay(&mut self, overlay: Self) {
        self.file_specs.overlay(overlay.file_specs);
        self.type_package_specs.overlay(overlay.type_package_specs);
        self.output_directory_specs
            .overlay(overlay.output_directory_specs);
    }

    fn apply_direct(
        &mut self,
        value: &serde_json::Value,
        config_dir: &Path,
        template_config_dir: &Path,
        type_resolver: &Vue3TypeResolverContext,
    ) -> bool {
        self.file_specs
            .apply_direct(value, config_dir, template_config_dir, type_resolver)
            && self.type_package_specs.apply_direct(
                value,
                config_dir,
                template_config_dir,
                type_resolver,
            )
            && self.output_directory_specs.apply_direct(
                value,
                config_dir,
                template_config_dir,
                type_resolver,
            )
    }
}

#[derive(Clone, Debug)]
struct Vue3TsconfigTypeRoots {
    paths: std::sync::Arc<[PathBuf]>,
    is_explicit: bool,
}

#[derive(Debug, Default)]
struct Vue3TsconfigGraphTraversal {
    seen_states: BTreeSet<Vue3TsconfigGraphStateKey>,
    active_identities: BTreeSet<PathBuf>,
    global_specs: BTreeMap<Vue3TsconfigGraphStateKey, Vue3TsconfigGlobalSpecs>,
    materialized_global_configs: BTreeSet<Vue3TsconfigGraphStateKey>,
    resolver_options:
        BTreeMap<Vue3TsconfigGraphStateKey, Vue3TsconfigInheritedResolverOptions>,
}

fn vue3_tsconfig_graph_state_key(
    config_path: &Path,
    template_config_dir: &Path,
) -> Vue3TsconfigGraphStateKey {
    (
        vue3_external_type_path_identity(config_path),
        vue3_external_type_lexical_path(config_path.parent().unwrap_or_else(|| Path::new(""))),
        vue3_external_type_lexical_path(template_config_dir),
    )
}

fn vue3_tsconfig_graph_enter(
    config_path: &Path,
    template_config_dir: &Path,
    depth: usize,
    traversal: &mut Vue3TsconfigGraphTraversal,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let state_key = vue3_tsconfig_graph_state_key(config_path, template_config_dir);
    let identity = state_key.0.clone();
    if traversal.active_identities.contains(&identity) {
        return None;
    }
    if traversal.seen_states.contains(&state_key) {
        return None;
    }
    if depth >= type_resolver.external_type_session.max_tsconfig_depth() {
        type_resolver.external_type_session.block_metadata();
        return None;
    }
    traversal.seen_states.insert(state_key.clone());
    if !type_resolver
        .external_type_session
        .claim_tsconfig_node(&state_key)
    {
        return None;
    }
    traversal.active_identities.insert(identity.clone());
    Some(identity)
}

#[cfg(test)]
pub(crate) fn resolve_vue3_tsconfig_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_tsconfig_type_import_with_mode(
        filename,
        source,
        Vue3TypeResolutionMode::Import,
        type_resolver,
    )
}

pub(crate) fn resolve_vue3_tsconfig_type_import_with_mode(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let config_path = vue3_tsconfig_search_paths(filename, type_resolver).next()?;
    let config_dir = config_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();
    let cache_key = Vue3TsconfigModuleResolutionCacheKey::new(
        &config_path,
        &config_dir,
        type_resolver,
    );
    let settings = match type_resolver
        .external_type_session
        .begin_tsconfig_module_resolution_load(cache_key)
    {
        Vue3TsconfigModuleResolutionLoad::Ready(settings) => settings,
        Vue3TsconfigModuleResolutionLoad::Wait(waiter) => waiter.wait()?,
        Vue3TsconfigModuleResolutionLoad::Start(owner) => {
            let mut traversal = Vue3TsconfigGraphTraversal::default();
            owner.complete(vue3_tsconfig_module_resolution_from_config(
                &config_path,
                &config_dir,
                &mut traversal,
                0,
                type_resolver,
            ))?
        }
        Vue3TsconfigModuleResolutionLoad::Blocked => return None,
    };
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let resolved = resolve_vue3_tsconfig_path_mappings_with_mode(
        settings.path_mappings(),
        source,
        resolution_mode,
        type_resolver,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if resolved.is_some() {
        return resolved;
    }
    if type_resolver.typescript_version < (7, 0, 0).into() {
        if let Some(base_url) = settings.base_url.as_ref() {
            let resolved = resolve_vue3_tsconfig_base_url_with_mode(
                base_url,
                source,
                resolution_mode,
                type_resolver,
            );
            if type_resolver.external_type_session.metadata_is_blocked() {
                return None;
            }
            if resolved.is_some() {
                return resolved;
            }
        }
    }
    None
}

pub(crate) fn vue3_tsconfig_search_paths<'a>(
    filename: &'a str,
    type_resolver: &'a Vue3TypeResolverContext,
) -> impl Iterator<Item = PathBuf> + 'a {
    Vue3AncestorSearchPaths::new(
        Path::new(filename).parent(),
        "tsconfig.json",
        &type_resolver.external_type_session,
    )
    .with_alternate_suffix("jsconfig.json")
    .project_config()
    .filter(|candidate| {
        type_resolver
            .external_type_session
            .metadata_path_is_file(candidate)
            .unwrap_or(false)
    })
}

#[derive(Debug, Default)]
struct Vue3TsconfigTypeRootsTraversal {
    active_identities: BTreeSet<PathBuf>,
    cached_overrides: BTreeMap<Vue3TsconfigGraphStateKey, Vue3TsconfigTypeRootsOverride>,
}

#[cfg(test)]
fn resolve_vue3_type_reference_directive(
    project_filename: &str,
    containing_filename: &str,
    type_name: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_reference_directive_with_mode(
        project_filename,
        containing_filename,
        type_name,
        None,
        type_resolver,
    )
}

fn resolve_vue3_type_reference_directive_with_mode(
    project_filename: &str,
    containing_filename: &str,
    type_name: &str,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_name.is_empty() {
        return None;
    }
    let mut request_resolver = type_resolver.clone();
    request_resolver.active_package_json_features = Some(
        type_resolver.package_json_features_for_type_reference(resolution_mode.is_some()),
    );
    let cache_source = format!(
        "{}:{containing_filename}{type_name}",
        containing_filename.len()
    );
    match request_resolver
        .external_type_session
        .begin_type_import_resolution(
            Vue3TypeResolutionKind::ReferenceTypes {
                mode: resolution_mode,
                package_json_features: request_resolver.package_json_features(),
            },
            project_filename,
            &cache_source,
            &request_resolver,
            false,
        ) {
        Vue3TypeImportResolutionLoad::Ready(resolution) => resolution,
        Vue3TypeImportResolutionLoad::Failed => None,
        Vue3TypeImportResolutionLoad::Start {
            cache_key,
            failure_epoch,
        } => {
            let resolution = resolve_vue3_type_reference_directive_uncached(
                project_filename,
                containing_filename,
                type_name,
                resolution_mode,
                &request_resolver,
            );
            request_resolver
                .external_type_session
                .finish_type_import_resolution(cache_key, resolution, failure_epoch, false)
        }
    }
}

fn resolve_vue3_type_reference_directive_uncached(
    project_filename: &str,
    containing_filename: &str,
    type_name: &str,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if !type_resolver
        .external_type_session
        .claim_metadata_target_steps(type_name.len())
    {
        return None;
    }
    let normalized_type_name =
        vue3_normalize_typescript_path_separators(type_name, type_resolver)?;
    let type_roots = vue3_tsconfig_effective_type_roots(project_filename, type_resolver)?;
    let primary = resolve_vue3_tsconfig_named_type_global_type_file(
        &type_roots,
        &normalized_type_name,
        resolution_mode,
        type_resolver,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if primary.is_some() {
        return primary;
    }
    let secondary = if vue3_type_reference_name_is_relative_or_rooted(&normalized_type_name) {
        let base = Path::new(containing_filename)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        let candidate = normalize_path_components(base.join(normalized_type_name));
        resolve_vue3_type_reference_package_candidate(
            &candidate,
            None,
            true,
            Vue3TypeReferenceLookupKind::Relative,
            resolution_mode,
            type_resolver,
        )
    } else {
        resolve_vue3_bare_type_reference(
            containing_filename,
            type_name,
            resolution_mode,
            type_resolver,
        )
    };
    if type_resolver.external_type_session.metadata_is_blocked() {
        None
    } else {
        secondary
    }
}

fn vue3_type_reference_name_is_relative_or_rooted(type_name: &str) -> bool {
    type_name == "."
        || type_name == ".."
        || vue3_type_import_source_is_relative(type_name)
        || Path::new(type_name).has_root()
}

fn vue3_tsconfig_effective_type_roots(
    project_filename: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigTypeRoots> {
    let config_path = vue3_tsconfig_search_paths(project_filename, type_resolver).next();
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let project_dir = Path::new(project_filename)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let Some(config_path) = config_path else {
        let roots = vue3_tsconfig_default_type_roots(project_dir, type_resolver);
        return (!type_resolver.external_type_session.metadata_is_blocked()).then(|| {
            Vue3TsconfigTypeRoots {
                paths: std::sync::Arc::from(roots),
                is_explicit: false,
            }
        });
    };
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    let mut traversal = Vue3TsconfigTypeRootsTraversal::default();
    let configured = vue3_tsconfig_type_roots_override_from_config(
        &config_path,
        config_dir,
        &mut traversal,
        0,
        type_resolver,
    )?;
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if let Some(configured) = configured {
        return Some(Vue3TsconfigTypeRoots {
            paths: configured,
            is_explicit: true,
        });
    }
    let roots = vue3_tsconfig_default_type_roots(config_dir, type_resolver);
    (!type_resolver.external_type_session.metadata_is_blocked()).then(|| Vue3TsconfigTypeRoots {
        paths: std::sync::Arc::from(roots),
        is_explicit: false,
    })
}

fn vue3_tsconfig_type_roots_override_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    traversal: &mut Vue3TsconfigTypeRootsTraversal,
    depth: usize,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigTypeRootsOverride> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let state_key = vue3_tsconfig_graph_state_key(config_path, template_config_dir);
    let identity = state_key.0.clone();
    if traversal.active_identities.contains(&identity) {
        return Some(None);
    }
    if let Some(cached) = traversal.cached_overrides.get(&state_key) {
        return Some(cached.clone());
    }
    if depth >= type_resolver.external_type_session.max_tsconfig_depth() {
        type_resolver.external_type_session.block_metadata();
        return None;
    }
    if !type_resolver
        .external_type_session
        .claim_tsconfig_node(&state_key)
    {
        return None;
    }
    traversal.active_identities.insert(identity.clone());
    let resolved = (|| {
        let value = type_resolver
            .external_type_session
            .tsconfig_from_path(config_path)?;
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        let extended_paths = vue3_tsconfig_extends_paths(&value, config_dir, type_resolver);
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        let mut effective = None;
        for extended in extended_paths {
            if let Some(extended_roots) = vue3_tsconfig_type_roots_override_from_config(
                &extended,
                template_config_dir,
                traversal,
                depth + 1,
                type_resolver,
            )? {
                effective = Some(extended_roots);
            }
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        let direct = value
            .get("compilerOptions")
            .and_then(serde_json::Value::as_object)
            .and_then(|options| options.get("typeRoots"));
        if let Some(direct) = direct {
            let mut roots = Vec::new();
            for target in vue3_tsconfig_string_array(Some(direct)) {
                if !type_resolver
                    .external_type_session
                    .claim_tsconfig_discovery_entry()
                {
                    return None;
                }
                let path = vue3_materialized_tsconfig_target_path(
                    config_dir,
                    template_config_dir,
                    target,
                    type_resolver,
                )?;
                roots.push(path);
            }
            effective = Some(std::sync::Arc::from(roots));
        }
        Some(effective)
    })();
    traversal.active_identities.remove(&identity);
    if let Some(effective) = &resolved {
        traversal
            .cached_overrides
            .insert(state_key, effective.clone());
    }
    resolved
}

fn resolve_vue3_tsconfig_named_type_global_type_file(
    type_roots: &Vue3TsconfigTypeRoots,
    type_name: &str,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_name.is_empty() {
        return None;
    }
    for type_root in type_roots.paths.iter() {
        if !type_resolver
            .external_type_session
            .claim_tsconfig_discovery_entry()
        {
            return None;
        }
        let path_bytes = vue3_ancestor_search_candidate_weight(type_root, type_name);
        if !type_resolver
            .external_type_session
            .claim_metadata_target_steps(path_bytes)
            || !vue3_claim_tsconfig_path_materialization(path_bytes, type_resolver)
        {
            return None;
        }
        let scoped_default_name = if type_resolver.typescript_version >= (5, 1, 0).into()
            && vue3_type_root_uses_scoped_package_mangling(type_root)
        {
            vue3_mangle_scoped_package_name(type_name)
        } else {
            None
        };
        let package_name = scoped_default_name.as_deref().unwrap_or(type_name);
        let package_dir = normalize_path_components(type_root.join(package_name));
        debug_assert!(package_dir.as_os_str().as_encoded_bytes().len() <= path_bytes);
        let file = resolve_vue3_type_reference_package_candidate(
            &package_dir,
            None,
            type_roots.is_explicit,
            Vue3TypeReferenceLookupKind::TypeRoot,
            resolution_mode,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        if let Some(file) = file {
            return type_resolver
                .external_type_session
                .claim_tsconfig_discovery_file()
                .then_some(file);
        }
    }
    None
}

fn vue3_type_root_uses_scoped_package_mangling(type_root: &Path) -> bool {
    let mut components = type_root.components().rev();
    matches!(
        (components.next(), components.next()),
        (
            Some(std::path::Component::Normal(at_types)),
            Some(std::path::Component::Normal(node_modules)),
        ) if at_types
            .to_str()
            .is_some_and(|name| name.eq_ignore_ascii_case("@types"))
            && node_modules
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("node_modules"))
    )
}

fn vue3_mangle_scoped_package_name(type_name: &str) -> Option<String> {
    let scoped = type_name.strip_prefix('@')?;
    let mangled = scoped.replacen('/', "__", 1);
    (mangled != scoped).then_some(mangled)
}

fn resolve_vue3_bare_type_reference(
    containing_filename: &str,
    type_name: &str,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let (package_name, subpath) = vue3_package_import_parts(type_name)?;
    for node_modules in vue3_node_modules_search_paths(containing_filename, type_resolver) {
        let package_dir = node_modules.join(package_name);
        let resolved = resolve_vue3_type_reference_package_candidate(
            &package_dir,
            subpath,
            true,
            Vue3TypeReferenceLookupKind::NodeModules,
            resolution_mode,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        if resolved.is_some() {
            return resolved;
        }
        let types_package_dir = node_modules.join(vue3_at_types_package_name(package_name));
        let resolved = resolve_vue3_type_reference_package_candidate(
            &types_package_dir,
            subpath,
            true,
            Vue3TypeReferenceLookupKind::NodeModules,
            resolution_mode,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        if resolved.is_some() {
            return resolved;
        }
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Vue3TypeReferenceLookupKind {
    TypeRoot,
    Relative,
    NodeModules,
}

fn resolve_vue3_type_reference_package_candidate(
    package_dir: &Path,
    subpath: Option<&str>,
    allow_direct_file: bool,
    lookup_kind: Vue3TypeReferenceLookupKind,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let uses_node_esm_specifier_rules = resolution_mode.is_some_and(|resolution_mode| {
        type_resolver
            .module_resolution
            .uses_node_esm_specifier_rules(
                resolution_mode,
                &type_resolver.typescript_version,
            )
    });
    let candidate = subpath
        .map(|subpath| package_dir.join(subpath))
        .unwrap_or_else(|| package_dir.to_path_buf());
    // Enabled exports precede the legacy root sibling-file probe in node_modules.
    let exports_precede_direct = subpath.is_none()
        && allow_direct_file
        && lookup_kind == Vue3TypeReferenceLookupKind::NodeModules
        && type_resolver.package_json_features().exports
        && vue3_package_json_has_truthy_exports(package_dir, type_resolver);
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if subpath.is_none() && allow_direct_file && !exports_precede_direct {
        let direct = resolve_vue3_type_reference_direct_file_with_mode(
            &candidate,
            uses_node_esm_specifier_rules,
            type_resolver,
        );
        if direct.is_some() || type_resolver.external_type_session.metadata_is_blocked() {
            return direct;
        }
    }
    if lookup_kind == Vue3TypeReferenceLookupKind::Relative
        && uses_node_esm_specifier_rules
    {
        return None;
    }
    if !type_resolver
        .external_type_session
        .metadata_path_is_dir(package_dir)?
    {
        return None;
    }
    let allow_candidate_manifest = if lookup_kind == Vue3TypeReferenceLookupKind::NodeModules {
        match resolve_vue3_package_json_type_reference_entry_phase(
            package_dir,
            subpath,
            resolution_mode,
            type_resolver,
        ) {
            Vue3PackageJsonPhaseResolution::Resolved(path) => return Some(path),
            Vue3PackageJsonPhaseResolution::Blocked => return None,
            Vue3PackageJsonPhaseResolution::NoPackageJson
                if subpath.is_none() && uses_node_esm_specifier_rules =>
            {
                return None;
            }
            Vue3PackageJsonPhaseResolution::NoPackageJson => true,
            Vue3PackageJsonPhaseResolution::Missing(fallback) => {
                if !fallback.allowed || !fallback.allow_index {
                    return None;
                }
                if subpath.is_none() && uses_node_esm_specifier_rules {
                    return resolve_vue3_metadata_type_reference_declaration_file(
                        &candidate.join("index.js"),
                        type_resolver,
                    );
                }
                fallback.allow_nested_manifest
            }
        }
    } else {
        true
    };
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if subpath.is_some() {
        let direct = resolve_vue3_type_reference_direct_file_with_mode(
            &candidate,
            uses_node_esm_specifier_rules,
            type_resolver,
        );
        if direct.is_some() || type_resolver.external_type_session.metadata_is_blocked() {
            return direct;
        }
    }
    if !type_resolver
        .external_type_session
        .metadata_path_is_dir(&candidate)?
    {
        return None;
    }
    let resolve_candidate_manifest = lookup_kind != Vue3TypeReferenceLookupKind::NodeModules
        || (subpath.is_some() && allow_candidate_manifest);
    if resolve_candidate_manifest {
        match resolve_vue3_package_json_type_reference_directory_entry_phase(
            &candidate,
            resolution_mode,
            type_resolver,
        ) {
            Vue3PackageJsonPhaseResolution::Resolved(path) => return Some(path),
            Vue3PackageJsonPhaseResolution::Blocked => return None,
            Vue3PackageJsonPhaseResolution::Missing(fallback) => {
                if !fallback.allowed || !fallback.allow_index {
                    return None;
                }
            }
            Vue3PackageJsonPhaseResolution::NoPackageJson => {}
        }
    }
    if type_resolver.external_type_session.metadata_is_blocked()
        || uses_node_esm_specifier_rules
    {
        return None;
    }
    resolve_vue3_metadata_type_reference_declaration_file(
        &candidate.join("index"),
        type_resolver,
    )
}

fn resolve_vue3_type_reference_direct_file_with_mode(
    candidate: &Path,
    uses_node_esm_specifier_rules: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if uses_node_esm_specifier_rules && vue3_typescript_path_extension(candidate).is_none() {
        return None;
    }
    resolve_vue3_metadata_type_reference_declaration_file(candidate, type_resolver)
}

fn vue3_tsconfig_module_resolution_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    traversal: &mut Vue3TsconfigGraphTraversal,
    depth: usize,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3TsconfigModuleResolutionSettings {
    let Some(identity) = vue3_tsconfig_graph_enter(
        config_path,
        template_config_dir,
        depth,
        traversal,
        type_resolver,
    ) else {
        return Vue3TsconfigModuleResolutionSettings::default();
    };
    let settings = (|| {
        let value = type_resolver
            .external_type_session
            .tsconfig_from_path(config_path)?;
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        let mut settings = Vue3TsconfigModuleResolutionSettings::default();
        for extended in vue3_tsconfig_extends_paths(&value, config_dir, type_resolver) {
            settings.inherit(vue3_tsconfig_module_resolution_from_config(
                &extended,
                template_config_dir,
                traversal,
                depth + 1,
                type_resolver,
            ));
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Some(Vue3TsconfigModuleResolutionSettings::default());
        }
        if vue3_tsconfig_declares_compiler_option(&value, "baseUrl") {
            settings.base_url = vue3_tsconfig_direct_base_url(
                &value,
                config_dir,
                template_config_dir,
                type_resolver,
            );
            settings.base_url_is_declared = true;
        }
        if vue3_tsconfig_declares_compiler_option(&value, "paths") {
            settings.path_mappings = Some(vue3_tsconfig_direct_path_mappings(
                &value,
                config_dir,
                template_config_dir,
                type_resolver,
            ));
            if type_resolver.external_type_session.metadata_is_blocked()
                || !type_resolver
                    .external_type_session
                    .claim_tsconfig_materialization(
                        std::mem::size_of::<PathBuf>().saturating_add(
                            config_dir.as_os_str().as_encoded_bytes().len(),
                        ),
                    )
            {
                return Some(Vue3TsconfigModuleResolutionSettings::default());
            }
            settings.paths_base_dir = Some(config_dir.to_path_buf());
        }
        if !settings.apply_effective_paths_base(&type_resolver.typescript_version, type_resolver) {
            return Some(Vue3TsconfigModuleResolutionSettings::default());
        }
        Some(settings)
    })()
    .unwrap_or_default();
    traversal.active_identities.remove(&identity);
    settings
}

pub(crate) fn vue3_tsconfig_type_resolver_options(
    filename: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigTypeResolverOptions> {
    let config_path = vue3_tsconfig_search_paths(filename, type_resolver).next();
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let configured = if let Some(config_path) = config_path {
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        let mut traversal = Vue3TsconfigGraphTraversal::default();
        vue3_tsconfig_type_resolver_options_from_config(
            &config_path,
            config_dir,
            &mut traversal,
            0,
            type_resolver,
        )?
    } else {
        Vue3TsconfigInheritedResolverOptions::default()
    };
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let module_resolution =
        configured.effective_module_resolution(&type_resolver.typescript_version);
    let module = configured.effective_module(&type_resolver.typescript_version);
    if !vue3_tsconfig_module_resolution_is_compatible(
        module,
        module_resolution,
        &type_resolver.typescript_version,
    ) || matches!(
        module_resolution,
        Vue3TypeModuleResolutionKind::Classic | Vue3TypeModuleResolutionKind::Node10
    ) && (configured.resolve_package_json_exports.value == Some(true)
        || configured.resolve_package_json_imports.value == Some(true)
        || configured.custom_conditions.value.is_some())
    {
        type_resolver.external_type_session.block_metadata();
        return None;
    }
    let resolve_package_json_exports = configured.resolve_package_json_exports.value;
    let resolve_package_json_imports = configured.resolve_package_json_imports.value;
    let allow_js = configured
        .allow_js
        .value
        .unwrap_or(configured.check_js.value.unwrap_or(false));
    let custom_conditions = configured.custom_conditions.value.unwrap_or_default();
    let module_suffixes = match configured.module_suffixes.value {
        Some(suffixes) if !suffixes.is_empty() => suffixes,
        Some(_) | None => vue3_default_module_suffixes(),
    };
    let root_dirs = configured
        .root_dirs
        .value
        .unwrap_or_else(|| std::sync::Arc::from(Vec::<PathBuf>::new()));
    Some(Vue3TsconfigTypeResolverOptions {
        module_resolution,
        module,
        module_suffixes,
        root_dirs,
        allow_js,
        custom_conditions,
        resolve_package_json_exports,
        resolve_package_json_imports,
    })
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_module_suffixes(
    filename: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<std::sync::Arc<[String]>> {
    vue3_tsconfig_type_resolver_options(filename, type_resolver)
        .map(|options| options.module_suffixes)
}

fn vue3_tsconfig_type_resolver_options_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    traversal: &mut Vue3TsconfigGraphTraversal,
    depth: usize,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigInheritedResolverOptions> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let state_key = vue3_tsconfig_graph_state_key(config_path, template_config_dir);
    if let Some(options) = traversal.resolver_options.get(&state_key) {
        return Some(options.clone());
    }
    let Some(identity) = vue3_tsconfig_graph_enter(
        config_path,
        template_config_dir,
        depth,
        traversal,
        type_resolver,
    ) else {
        return (!type_resolver.external_type_session.metadata_is_blocked())
            .then(Vue3TsconfigInheritedResolverOptions::default);
    };
    let configured = (|| {
        let Some(value) = type_resolver
            .external_type_session
            .tsconfig_from_path(config_path)
        else {
            return (!type_resolver.external_type_session.metadata_is_blocked())
                .then(Vue3TsconfigInheritedResolverOptions::default);
        };
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        let mut configured = Vue3TsconfigInheritedResolverOptions::default();
        for extended in vue3_tsconfig_extends_paths(&value, config_dir, type_resolver) {
            let inherited = vue3_tsconfig_type_resolver_options_from_config(
                &extended,
                template_config_dir,
                traversal,
                depth + 1,
                type_resolver,
            )?;
            configured.inherit(inherited);
        }
        if config_path.file_name() == Some(std::ffi::OsStr::new("jsconfig.json")) {
            configured.allow_js.set(Some(true));
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        let compiler_options = value
            .get("compilerOptions")
            .and_then(serde_json::Value::as_object);
        if let Some(module) = compiler_options.and_then(|options| options.get("module")) {
            configured.module.set(vue3_tsconfig_nullable_option(
                module,
                |value| vue3_tsconfig_direct_module_kind(value, type_resolver),
            )?);
        }
        if let Some(module_resolution) =
            compiler_options.and_then(|options| options.get("moduleResolution"))
        {
            configured
                .module_resolution
                .set(vue3_tsconfig_nullable_option(module_resolution, |value| {
                    vue3_tsconfig_direct_module_resolution_kind(value, type_resolver)
                })?);
        }
        if type_resolver.typescript_version >= (4, 7, 0).into() {
            if let Some(module_suffixes) =
                compiler_options.and_then(|options| options.get("moduleSuffixes"))
            {
                configured
                    .module_suffixes
                    .set(vue3_tsconfig_nullable_option(module_suffixes, |value| {
                        vue3_tsconfig_direct_module_suffixes(value, type_resolver)
                    })?);
            }
        }
        if let Some(root_dirs) = compiler_options.and_then(|options| options.get("rootDirs")) {
            configured
                .root_dirs
                .set(vue3_tsconfig_nullable_option(root_dirs, |value| {
                    vue3_tsconfig_direct_root_dirs(
                        value,
                        config_dir,
                        template_config_dir,
                        type_resolver,
                    )
                })?);
        }
        if let Some(value) = compiler_options.and_then(|options| options.get("allowJs")) {
            configured
                .allow_js
                .set(vue3_tsconfig_nullable_option(value, |value| {
                    vue3_tsconfig_direct_bool(value, type_resolver)
                })?);
        }
        if let Some(value) = compiler_options.and_then(|options| options.get("checkJs")) {
            configured
                .check_js
                .set(vue3_tsconfig_nullable_option(value, |value| {
                    vue3_tsconfig_direct_bool(value, type_resolver)
                })?);
        }
        if type_resolver.typescript_version >= (5, 0, 0).into() {
            if let Some(value) = compiler_options
                .and_then(|options| options.get("resolvePackageJsonExports"))
            {
                configured.resolve_package_json_exports.set(
                    vue3_tsconfig_nullable_option(value, |value| {
                        vue3_tsconfig_direct_bool(value, type_resolver)
                    })?,
                );
            }
            if let Some(value) = compiler_options
                .and_then(|options| options.get("resolvePackageJsonImports"))
            {
                configured.resolve_package_json_imports.set(
                    vue3_tsconfig_nullable_option(value, |value| {
                        vue3_tsconfig_direct_bool(value, type_resolver)
                    })?,
                );
            }
        }
        if let Some(value) = compiler_options.and_then(|options| options.get("customConditions")) {
            if type_resolver.typescript_version < (5, 0, 0).into() {
                type_resolver.external_type_session.block_metadata();
                return None;
            }
            configured
                .custom_conditions
                .set(vue3_tsconfig_direct_custom_conditions(value, type_resolver)?);
        }
        if let Some(target) = compiler_options.and_then(|options| options.get("target")) {
            configured.target.set(vue3_tsconfig_nullable_option(
                target,
                |value| vue3_tsconfig_direct_target_kind(value, type_resolver),
            )?);
        }
        traversal
            .resolver_options
            .insert(state_key, configured.clone());
        Some(configured)
    })();
    traversal.active_identities.remove(&identity);
    configured
}

fn vue3_tsconfig_nullable_option<T>(
    value: &serde_json::Value,
    parse: impl FnOnce(&serde_json::Value) -> Option<T>,
) -> Option<Option<T>> {
    if value.is_null() {
        Some(None)
    } else {
        parse(value).map(Some)
    }
}

fn vue3_tsconfig_direct_bool(
    value: &serde_json::Value,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<bool> {
    let Some(value) = value.as_bool() else {
        type_resolver.external_type_session.block_metadata();
        return None;
    };
    Some(value)
}

fn vue3_tsconfig_direct_module_suffixes(
    value: &serde_json::Value,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<std::sync::Arc<[String]>> {
    let Some(values) = value.as_array() else {
        type_resolver.external_type_session.block_metadata();
        return None;
    };
    let mut suffixes = Vec::new();
    for value in values {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return None;
        }
        let Some(suffix) = value.as_str() else {
            type_resolver.external_type_session.block_metadata();
            return None;
        };
        if !type_resolver
            .external_type_session
            .metadata_path_is_within_limit(suffix)
        {
            return None;
        }
        if !type_resolver
            .external_type_session
            .claim_tsconfig_materialization(
                std::mem::size_of::<String>().saturating_add(suffix.len()),
            )
        {
            return None;
        }
        suffixes.push(suffix.to_string());
    }
    Some(std::sync::Arc::from(suffixes))
}

fn vue3_tsconfig_direct_root_dirs(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<std::sync::Arc<[PathBuf]>> {
    let Some(values) = value.as_array() else {
        type_resolver.external_type_session.block_metadata();
        return None;
    };
    let mut root_dirs = Vec::new();
    for value in values {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return None;
        }
        let Some(root_dir) = value.as_str() else {
            type_resolver.external_type_session.block_metadata();
            return None;
        };
        root_dirs.push(vue3_materialized_tsconfig_target_path(
            config_dir,
            template_config_dir,
            root_dir,
            type_resolver,
        )?);
    }
    Some(std::sync::Arc::from(root_dirs))
}

fn vue3_tsconfig_direct_custom_conditions(
    value: &serde_json::Value,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Option<Vue3CustomConditionSet>> {
    if value.is_null() {
        return Some(None);
    }
    let Some(values) = value.as_array() else {
        type_resolver.external_type_session.block_metadata();
        return None;
    };
    let mut conditions = Vec::new();
    for value in values {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return None;
        }
        if value.is_null() {
            continue;
        }
        let Some(condition) = value.as_str() else {
            type_resolver.external_type_session.block_metadata();
            return None;
        };
        if !condition.is_empty() {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_materialization(
                    std::mem::size_of::<String>().saturating_add(condition.len()),
                )
            {
                return None;
            }
            conditions.push(condition.to_string());
        }
    }
    let normalization_steps = vue3_custom_condition_normalization_steps(&conditions);
    if normalization_steps > 0
        && !type_resolver
            .external_type_session
            .claim_tsconfig_normalization_steps(normalization_steps)
    {
        return None;
    }
    Some(Some(Vue3CustomConditionSet::from_strings(conditions)))
}

fn vue3_custom_condition_normalization_steps(conditions: &[String]) -> usize {
    let comparison_width = conditions
        .iter()
        .map(|condition| condition.len().max(1))
        .max()
        .unwrap_or(1);
    vue3_tsconfig_sort_normalization_steps(conditions.len(), comparison_width, 1)
}

fn vue3_tsconfig_sort_normalization_steps(
    entry_count: usize,
    comparison_width: usize,
    additional_linear_passes: usize,
) -> usize {
    if entry_count < 2 {
        return 0;
    }
    // Model each comparison at the widest key and account for caller-specific scans.
    let comparison_rounds =
        usize::BITS as usize - (entry_count - 1).leading_zeros() as usize;
    comparison_width
        .max(1)
        .saturating_mul(entry_count)
        .saturating_mul(comparison_rounds.saturating_add(additional_linear_passes))
}

fn vue3_tsconfig_direct_module_kind(
    value: &serde_json::Value,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TypeModuleKind> {
    let Some(value) = value.as_str() else {
        type_resolver.external_type_session.block_metadata();
        return None;
    };
    let version = &type_resolver.typescript_version;
    let parsed = if version < &(7, 0, 0).into()
        && ["none", "amd", "system", "umd"]
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
    {
        Some(Vue3TypeModuleKind::Classic)
    } else if value.eq_ignore_ascii_case("commonjs") {
        Some(Vue3TypeModuleKind::CommonJs)
    } else if ["es6", "es2015", "esnext"]
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
        || value.eq_ignore_ascii_case("es2020") && version >= &(3, 8, 0).into()
        || value.eq_ignore_ascii_case("es2022") && version >= &(4, 5, 0).into()
    {
        Some(Vue3TypeModuleKind::EcmaScript)
    } else if value.eq_ignore_ascii_case("node16") && version >= &(4, 7, 0).into()
        || value.eq_ignore_ascii_case("node18") && version >= &(5, 8, 0).into()
        || value.eq_ignore_ascii_case("node20") && version >= &(5, 9, 0).into()
    {
        Some(Vue3TypeModuleKind::Node16)
    } else if value.eq_ignore_ascii_case("nodenext") && version >= &(4, 7, 0).into() {
        Some(Vue3TypeModuleKind::NodeNext)
    } else if value.eq_ignore_ascii_case("preserve") && version >= &(5, 4, 0).into() {
        Some(Vue3TypeModuleKind::Preserve)
    } else {
        None
    };
    parsed.or_else(|| {
        type_resolver.external_type_session.block_metadata();
        None
    })
}

fn vue3_tsconfig_direct_module_resolution_kind(
    value: &serde_json::Value,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TypeModuleResolutionKind> {
    let Some(value) = value.as_str() else {
        type_resolver.external_type_session.block_metadata();
        return None;
    };
    let version = &type_resolver.typescript_version;
    let parsed = if value.eq_ignore_ascii_case("classic") && version < &(7, 0, 0).into() {
        Some(Vue3TypeModuleResolutionKind::Classic)
    } else if (value.eq_ignore_ascii_case("node") && version < &(7, 0, 0).into())
        || (value.eq_ignore_ascii_case("node10")
            && version >= &(5, 0, 0).into()
            && version < &(7, 0, 0).into())
    {
        Some(Vue3TypeModuleResolutionKind::Node10)
    } else if value.eq_ignore_ascii_case("node16") && version >= &(4, 7, 0).into() {
        Some(Vue3TypeModuleResolutionKind::Node16)
    } else if value.eq_ignore_ascii_case("nodenext") && version >= &(4, 7, 0).into() {
        Some(Vue3TypeModuleResolutionKind::NodeNext)
    } else if value.eq_ignore_ascii_case("bundler") && version >= &(5, 0, 0).into() {
        Some(Vue3TypeModuleResolutionKind::Bundler)
    } else {
        None
    };
    parsed.or_else(|| {
        type_resolver.external_type_session.block_metadata();
        None
    })
}

fn vue3_tsconfig_module_resolution_is_compatible(
    module: Vue3TypeModuleKind,
    module_resolution: Vue3TypeModuleResolutionKind,
    typescript_version: &nodejs_semver::Version,
) -> bool {
    let node_module = matches!(module, Vue3TypeModuleKind::Node16 | Vue3TypeModuleKind::NodeNext);
    let node_resolution = matches!(
        module_resolution,
        Vue3TypeModuleResolutionKind::Node16 | Vue3TypeModuleResolutionKind::NodeNext
    );
    if node_module != node_resolution {
        return false;
    }
    module_resolution != Vue3TypeModuleResolutionKind::Bundler
        || matches!(
            module,
            Vue3TypeModuleKind::EcmaScript | Vue3TypeModuleKind::Preserve
        )
        || module == Vue3TypeModuleKind::CommonJs
            && typescript_version >= &(6, 0, 0).into()
}

fn vue3_tsconfig_direct_target_kind(
    value: &serde_json::Value,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigTargetKind> {
    let Some(value) = value.as_str() else {
        type_resolver.external_type_session.block_metadata();
        return None;
    };
    let version = &type_resolver.typescript_version;
    let parsed = if value.eq_ignore_ascii_case("es3") && version < &(7, 0, 0).into() {
        Some(Vue3TsconfigTargetKind::Default)
    } else if value.eq_ignore_ascii_case("es5") && version < &(7, 0, 0).into() {
        Some(Vue3TsconfigTargetKind::Legacy)
    } else if ["es6", "es2015", "esnext"]
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
        || value.eq_ignore_ascii_case("es2016") && version >= &(2, 1, 0).into()
        || value.eq_ignore_ascii_case("es2017") && version >= &(2, 1, 0).into()
        || value.eq_ignore_ascii_case("es2018") && version >= &(2, 7, 0).into()
        || value.eq_ignore_ascii_case("es2019") && version >= &(3, 4, 0).into()
        || value.eq_ignore_ascii_case("es2020") && version >= &(3, 5, 0).into()
        || value.eq_ignore_ascii_case("es2021") && version >= &(4, 3, 0).into()
        || value.eq_ignore_ascii_case("es2022") && version >= &(4, 6, 0).into()
        || value.eq_ignore_ascii_case("es2023") && version >= &(5, 5, 0).into()
        || value.eq_ignore_ascii_case("es2024") && version >= &(5, 7, 0).into()
        || value.eq_ignore_ascii_case("es2025") && version >= &(6, 0, 0).into()
    {
        Some(Vue3TsconfigTargetKind::Modern)
    } else {
        None
    };
    parsed.or_else(|| {
        type_resolver.external_type_session.block_metadata();
        None
    })
}

pub(crate) fn vue3_tsconfig_global_type_files(
    filename: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vec::new();
    }
    let mut files = Vec::new();
    let mut traversal = Vue3TsconfigGraphTraversal::default();
    let mut seen_files = BTreeSet::new();
    if let Some(config_path) = vue3_tsconfig_search_paths(filename, type_resolver).next() {
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        vue3_tsconfig_global_type_files_from_config(
            &config_path,
            config_dir,
            true,
            &mut traversal,
            &mut seen_files,
            &mut files,
            type_resolver,
            0,
        );
    }
    if type_resolver.external_type_session.metadata_is_blocked() {
        Vec::new()
    } else {
        files
    }
}

fn vue3_tsconfig_global_type_files_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    materialize_global_specs: bool,
    traversal: &mut Vue3TsconfigGraphTraversal,
    seen_files: &mut BTreeSet<String>,
    files: &mut Vec<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
    depth: usize,
) -> Option<Vue3TsconfigGlobalSpecs> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let state_key = vue3_tsconfig_graph_state_key(config_path, template_config_dir);
    if let Some(global_specs) = traversal.global_specs.get(&state_key).cloned() {
        if materialize_global_specs {
            let value = type_resolver
                .external_type_session
                .tsconfig_from_path(config_path)?;
            if !vue3_materialize_tsconfig_global_specs(
                config_path,
                &value,
                &global_specs,
                &state_key,
                depth,
                traversal,
                seen_files,
                files,
                type_resolver,
            ) {
                return None;
            }
        }
        return Some(global_specs);
    }
    let identity = vue3_tsconfig_graph_enter(
        config_path,
        template_config_dir,
        depth,
        traversal,
        type_resolver,
    )?;
    let global_specs = (|| {
        let value = type_resolver
            .external_type_session
            .tsconfig_from_path(config_path)?;
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        let mut global_specs = Vue3TsconfigGlobalSpecs::default();
        for extended in vue3_tsconfig_extends_paths(&value, config_dir, type_resolver) {
            if let Some(extended_specs) = vue3_tsconfig_global_type_files_from_config(
                &extended,
                template_config_dir,
                false,
                traversal,
                seen_files,
                files,
                type_resolver,
                depth + 1,
            ) {
                global_specs.overlay(extended_specs);
            }
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        if !global_specs.apply_direct(
            &value,
            config_dir,
            template_config_dir,
            type_resolver,
        ) {
            return None;
        }
        traversal
            .global_specs
            .insert(state_key.clone(), global_specs.clone());
        if materialize_global_specs
            && !vue3_materialize_tsconfig_global_specs(
                config_path,
                &value,
                &global_specs,
                &state_key,
                depth,
                traversal,
                seen_files,
                files,
                type_resolver,
            )
        {
            return None;
        }
        Some(global_specs)
    })();
    traversal.active_identities.remove(&identity);
    global_specs
}

fn vue3_materialize_tsconfig_global_specs(
    config_path: &Path,
    value: &serde_json::Value,
    global_specs: &Vue3TsconfigGlobalSpecs,
    state_key: &Vue3TsconfigGraphStateKey,
    depth: usize,
    traversal: &mut Vue3TsconfigGraphTraversal,
    seen_files: &mut BTreeSet<String>,
    files: &mut Vec<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    if traversal
        .materialized_global_configs
        .contains(state_key)
    {
        return true;
    }
    if depth >= type_resolver.external_type_session.max_tsconfig_depth() {
        type_resolver.external_type_session.block_metadata();
        return false;
    }
    traversal
        .materialized_global_configs
        .insert(state_key.clone());
    vue3_append_tsconfig_global_type_files(
        vue3_tsconfig_file_spec_global_type_files(
            &global_specs.file_specs,
            &global_specs.output_directory_specs,
            config_dir,
            type_resolver,
        ),
        seen_files,
        files,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        return false;
    }
    vue3_append_tsconfig_global_type_files(
        vue3_tsconfig_global_type_package_files(
            &global_specs.type_package_specs,
            config_dir,
            type_resolver,
        ),
        seen_files,
        files,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        return false;
    }
    for reference in vue3_tsconfig_reference_paths(value, config_dir, type_resolver) {
        let reference_dir = reference.parent().unwrap_or_else(|| Path::new(""));
        vue3_tsconfig_global_type_files_from_config(
            &reference,
            reference_dir,
            true,
            traversal,
            seen_files,
            files,
            type_resolver,
            depth + 1,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return false;
        }
    }
    true
}

fn vue3_append_tsconfig_global_type_files(
    candidates: Vec<PathBuf>,
    seen_files: &mut BTreeSet<String>,
    files: &mut Vec<PathBuf>,
) {
    for file in candidates {
        let normalized = normalize_path_string(&file);
        if seen_files.insert(normalized) {
            files.push(file);
        }
    }
}

fn vue3_tsconfig_file_spec_global_type_files(
    file_specs: &Vue3TsconfigFileSpecs,
    output_directory_specs: &Vue3TsconfigOutputDirectorySpecs,
    config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(specs) = file_specs.files.as_ref() {
        for target in specs.targets.iter() {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_discovery_entry()
            {
                return Vec::new();
            }
            let Some(path) = vue3_tsconfig_target_path(
                &specs.config_dir,
                &specs.template_config_dir,
                target,
                type_resolver,
            ) else {
                return Vec::new();
            };
            if vue3_tsconfig_global_type_file_is_supported(&path) {
                if !type_resolver
                    .external_type_session
                    .claim_tsconfig_discovery_file()
                {
                    return Vec::new();
                }
                files.push(path);
            }
        }
    }
    let default_include = if file_specs.include.is_none() && file_specs.files.is_none() {
        let Some(targets) =
            vue3_materialize_tsconfig_strings(["**/*"], type_resolver)
        else {
            return Vec::new();
        };
        let Some(specs) = Vue3TsconfigPathSpecList::from_materialized_targets(
            targets,
            config_dir,
            config_dir,
            type_resolver,
        ) else {
            return Vec::new();
        };
        Some(specs)
    } else {
        None
    };
    let include_specs = file_specs.include.as_ref().or(default_include.as_ref());
    let Some(include_specs) = include_specs else {
        return files;
    };
    let excludes = if let Some(specs) = file_specs.exclude.as_ref() {
        let Some(excludes) = vue3_tsconfig_exclude_patterns(
            &specs.targets,
            &specs.config_dir,
            &specs.template_config_dir,
            type_resolver,
        ) else {
            return Vec::new();
        };
        excludes
    } else {
        let Some(excludes) = vue3_tsconfig_output_directory_exclude_patterns(
            output_directory_specs,
            type_resolver,
        ) else {
            return Vec::new();
        };
        excludes
    };
    let exclude_matchers = excludes
        .patterns
        .iter()
        .map(|pattern| Vue3CompiledTsconfigGlob::new(pattern))
        .collect::<Vec<_>>();
    for target in include_specs.targets.iter() {
        files.extend(vue3_tsconfig_include_global_type_files_with_excludes(
            &include_specs.config_dir,
            &include_specs.template_config_dir,
            target,
            &exclude_matchers,
            &excludes.directory_keys,
            type_resolver,
        ));
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Vec::new();
        }
    }
    files
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_direct_global_type_files(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let mut global_specs = Vue3TsconfigGlobalSpecs::default();
    if !global_specs.apply_direct(value, config_dir, template_config_dir, type_resolver) {
        return Vec::new();
    }
    let mut files = vue3_tsconfig_file_spec_global_type_files(
        &global_specs.file_specs,
        &global_specs.output_directory_specs,
        config_dir,
        type_resolver,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vec::new();
    }
    files.extend(vue3_tsconfig_global_type_package_files(
        &global_specs.type_package_specs,
        config_dir,
        type_resolver,
    ));
    if type_resolver.external_type_session.metadata_is_blocked() {
        Vec::new()
    } else {
        files
    }
}

pub(crate) fn vue3_tsconfig_string_array(
    value: Option<&serde_json::Value>,
) -> impl Iterator<Item = &str> {
    value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
}

fn vue3_tsconfig_global_type_package_files(
    specs: &Vue3TsconfigGlobalTypePackageSpecs,
    config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    if specs.types.as_ref().is_some_and(|types| types.is_empty()) {
        return Vec::new();
    }
    let type_roots = if let Some(type_roots) = specs.type_roots.as_ref() {
        let mut configured = Vec::new();
        for target in type_roots.targets.iter() {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_discovery_entry()
            {
                return Vec::new();
            }
            let Some(path) = vue3_materialized_tsconfig_target_path(
                &type_roots.config_dir,
                &type_roots.template_config_dir,
                target,
                type_resolver,
            ) else {
                return Vec::new();
            };
            if path.is_dir() {
                configured.push(path);
            }
        }
        configured
    } else {
        vue3_tsconfig_default_type_roots(config_dir, type_resolver)
    };
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vec::new();
    }
    if let Some(types) = specs.types.as_ref() {
        let mut files = Vec::new();
        for type_name in types.iter() {
            files.extend(vue3_tsconfig_named_type_global_type_files(
                &type_roots,
                type_name,
                type_resolver,
            ));
            if type_resolver.external_type_session.metadata_is_blocked() {
                return Vec::new();
            }
        }
        return files;
    }
    let mut files = Vec::new();
    for type_root in type_roots {
        files.extend(vue3_tsconfig_all_type_root_global_type_files(
            &type_root,
            type_resolver,
        ));
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Vec::new();
        }
    }
    files
}

pub(crate) fn vue3_tsconfig_default_type_roots(
    config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let mut type_roots = Vec::new();
    for node_modules in vue3_node_modules_search_paths_from_dir(config_dir, type_resolver) {
        if !type_resolver
            .external_type_session
            .claim_tsconfig_discovery_entry()
        {
            return Vec::new();
        }
        let path_bytes = vue3_ancestor_search_candidate_weight(&node_modules, "@types");
        if !vue3_claim_tsconfig_path_materialization(path_bytes, type_resolver) {
            return Vec::new();
        }
        let path = normalize_path_components(node_modules.join("@types"));
        debug_assert!(path.as_os_str().as_encoded_bytes().len() <= path_bytes);
        if path.is_dir() {
            type_roots.push(path);
        }
    }
    type_roots
}

pub(crate) fn vue3_tsconfig_named_type_global_type_files(
    type_roots: &[PathBuf],
    type_name: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    if !type_resolver
        .external_type_session
        .claim_metadata_target_steps(type_name.len())
    {
        return Vec::new();
    }
    if !vue3_tsconfig_type_name_is_safe(type_name) {
        return Vec::new();
    }
    let package_dir_count = vue3_tsconfig_type_name_package_dir_count(type_name);
    let mut files = Vec::new();
    for type_root in type_roots {
        let generated_steps = package_dir_count.saturating_mul(
            type_root
                .as_os_str()
                .as_encoded_bytes()
                .len()
                .saturating_add(1)
                .saturating_add(type_name.len()),
        );
        if !type_resolver
            .external_type_session
            .claim_metadata_target_steps(generated_steps)
        {
            return Vec::new();
        }
        for candidate in vue3_tsconfig_type_name_package_dir_candidates(type_name)
            .into_iter()
            .flatten()
        {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_discovery_entry()
            {
                return Vec::new();
            }
            let path_bytes = candidate.path_bytes(type_root);
            if !vue3_claim_tsconfig_path_materialization(path_bytes, type_resolver) {
                return Vec::new();
            }
            let package_dir = candidate.materialize(type_root);
            debug_assert!(package_dir.as_os_str().as_encoded_bytes().len() <= path_bytes);
            if let Some(file) =
                vue3_tsconfig_type_package_global_type_file(&package_dir, type_resolver)
            {
                files.push(file);
            }
            if type_resolver.external_type_session.metadata_is_blocked() {
                return Vec::new();
            }
        }
    }
    files
}

pub(crate) fn vue3_tsconfig_type_name_is_safe(type_name: &str) -> bool {
    !type_name.is_empty()
        && !type_name.contains(':')
        && !type_name.contains('\\')
        && !Path::new(type_name).is_absolute()
        && !type_name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}

#[derive(Clone, Copy)]
enum Vue3TsconfigTypeNamePackageDir<'a> {
    Literal(&'a str),
    Scoped {
        scope: &'a str,
        package: &'a str,
    },
    Mangled {
        scope: &'a str,
        package: &'a str,
    },
}

impl Vue3TsconfigTypeNamePackageDir<'_> {
    fn path_bytes(self, type_root: &Path) -> usize {
        match self {
            Self::Literal(type_name) => {
                vue3_tsconfig_joined_path_bytes(type_root, &[type_name])
            }
            Self::Scoped { scope, package } => {
                vue3_tsconfig_joined_path_bytes(type_root, &[scope, package])
            }
            Self::Mangled { scope, package } => type_root
                .as_os_str()
                .as_encoded_bytes()
                .len()
                .saturating_add(usize::from(!type_root.as_os_str().is_empty()))
                .saturating_add(scope.len())
                .saturating_add(2)
                .saturating_add(package.len()),
        }
    }

    fn materialize(self, type_root: &Path) -> PathBuf {
        match self {
            Self::Literal(type_name) => normalize_path_components(type_root.join(type_name)),
            Self::Scoped { scope, package } => {
                normalize_path_components(type_root.join(scope).join(package))
            }
            Self::Mangled { scope, package } => {
                normalize_path_components(type_root.join(format!("{scope}__{package}")))
            }
        }
    }
}

fn vue3_tsconfig_type_name_package_dir_candidates(
    type_name: &str,
) -> [Option<Vue3TsconfigTypeNamePackageDir<'_>>; 3] {
    if let Some(scoped) = type_name.strip_prefix('@') {
        if let Some((scope, package)) = scoped
            .split_once('/')
            .filter(|(_, package)| !package.contains('/'))
        {
            return [
                Some(Vue3TsconfigTypeNamePackageDir::Literal(type_name)),
                Some(Vue3TsconfigTypeNamePackageDir::Scoped { scope, package }),
                Some(Vue3TsconfigTypeNamePackageDir::Mangled { scope, package }),
            ];
        }
    }
    [
        Some(Vue3TsconfigTypeNamePackageDir::Literal(type_name)),
        None,
        None,
    ]
}

fn vue3_tsconfig_joined_path_bytes(type_root: &Path, components: &[&str]) -> usize {
    components.iter().fold(
        type_root.as_os_str().as_encoded_bytes().len(),
        |bytes, component| {
            bytes
                .saturating_add(usize::from(bytes != 0))
                .saturating_add(component.len())
        },
    )
}

fn vue3_tsconfig_type_name_package_dir_count(type_name: &str) -> usize {
    type_name
        .strip_prefix('@')
        .and_then(|scoped| scoped.split_once('/'))
        .filter(|(_, package)| !package.contains('/'))
        .map_or(1, |_| 3)
}

pub(crate) fn vue3_tsconfig_all_type_root_global_type_files(
    type_root: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let Some(entries) = vue3_tsconfig_bounded_sorted_dir_entries(type_root, type_resolver) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries {
        let name = entry
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !entry.is_dir() || name.is_empty() || name.starts_with('.') {
            continue;
        }
        if name.starts_with('@') {
            files.extend(vue3_tsconfig_all_scoped_type_root_global_type_files(
                &entry,
                type_resolver,
            ));
        } else if let Some(file) =
            vue3_tsconfig_type_package_global_type_file(&entry, type_resolver)
        {
            files.push(file);
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Vec::new();
        }
    }
    files
}

pub(crate) fn vue3_tsconfig_all_scoped_type_root_global_type_files(
    scope_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let Some(entries) = vue3_tsconfig_bounded_sorted_dir_entries(scope_dir, type_resolver) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries {
        if !entry.is_dir()
            || !entry
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.is_empty() && !name.starts_with('.'))
        {
            continue;
        }
        if let Some(file) = vue3_tsconfig_type_package_global_type_file(&entry, type_resolver) {
            files.push(file);
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Vec::new();
        }
    }
    files
}

pub(crate) fn vue3_tsconfig_type_package_global_type_file(
    package_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let path = resolve_vue3_package_entry_phase_with_mode(
        package_dir,
        None,
        Vue3TypeResolutionMode::Import,
        Vue3PackageResolutionPhase::Types,
        type_resolver,
    )?;
    if !vue3_tsconfig_global_type_file_is_supported(&path) {
        return None;
    }
    type_resolver
        .external_type_session
        .claim_tsconfig_discovery_file()
        .then_some(path)
}

fn vue3_tsconfig_bounded_sorted_dir_entries(
    dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vec<PathBuf>> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Some(Vec::new());
    };
    let max_path_bytes = type_resolver
        .external_type_session
        .limits()
        .max_generated_path_bytes;
    let mut paths = Vec::new();
    for entry in entries {
        if !type_resolver
            .external_type_session
            .claim_tsconfig_discovery_entry()
        {
            return None;
        }
        if let Ok(entry) = entry {
            let path_bytes = vue3_tsconfig_directory_entry_path_bytes(dir, &entry.file_name());
            if path_bytes > max_path_bytes {
                type_resolver.external_type_session.block_metadata();
                return None;
            }
            let materialization_weight =
                std::mem::size_of::<PathBuf>().saturating_add(path_bytes);
            if !type_resolver
                .external_type_session
                .claim_tsconfig_materialization(materialization_weight)
            {
                return None;
            }
            let path = entry.path();
            debug_assert!(path.as_os_str().as_encoded_bytes().len() <= path_bytes);
            paths.push(path);
        }
    }
    let normalization_steps = vue3_tsconfig_path_sort_normalization_steps(&paths);
    if normalization_steps > 0
        && !type_resolver
            .external_type_session
            .claim_tsconfig_normalization_steps(normalization_steps)
    {
        return None;
    }
    paths.sort();
    Some(paths)
}

fn vue3_tsconfig_directory_entry_path_bytes(dir: &Path, name: &std::ffi::OsStr) -> usize {
    dir.as_os_str()
        .as_encoded_bytes()
        .len()
        .saturating_add(usize::from(!dir.as_os_str().is_empty()))
        .saturating_add(name.as_encoded_bytes().len())
}

fn vue3_tsconfig_path_sort_normalization_steps(paths: &[PathBuf]) -> usize {
    let comparison_width = paths
        .iter()
        .map(|path| path.as_os_str().as_encoded_bytes().len().max(1))
        .max()
        .unwrap_or(1);
    vue3_tsconfig_sort_normalization_steps(paths.len(), comparison_width, 0)
}

#[cfg(test)]
mod vue3_tsconfig_directory_sort_tests {
    use super::*;

    fn resolver_with_limits(limits: Vue3ExternalTypeLoadLimits) -> Vue3TypeResolverContext {
        Vue3TypeResolverContext {
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(limits),
            ..Vue3TypeResolverContext::default()
        }
    }

    fn expected_path_weight(dir: &Path, names: &[&str]) -> usize {
        names.iter().fold(0usize, |weight, name| {
            weight.saturating_add(
                std::mem::size_of::<PathBuf>().saturating_add(
                    vue3_tsconfig_directory_entry_path_bytes(dir, std::ffi::OsStr::new(name)),
                ),
            )
        })
    }

    #[test]
    fn directory_paths_and_sorting_claim_exact_budgets_before_work() {
        let dir = tempfile::tempdir().expect("temp dir");
        let names = ["alpha", "omega"];
        for name in names {
            std::fs::write(dir.path().join(name), "").expect("write directory entry");
        }
        let expected = names
            .iter()
            .map(|name| dir.path().join(name))
            .collect::<Vec<_>>();
        let path_weight = expected_path_weight(dir.path(), &names);
        let path_bytes = vue3_tsconfig_directory_entry_path_bytes(
            dir.path(),
            std::ffi::OsStr::new(names[0]),
        );
        let normalization_steps = vue3_tsconfig_path_sort_normalization_steps(&expected);

        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: path_bytes,
            max_tsconfig_discovery_entries: names.len(),
            max_tsconfig_materialization_entries: names.len(),
            max_tsconfig_materialization_weight: path_weight,
            max_tsconfig_normalization_steps: normalization_steps,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            vue3_tsconfig_bounded_sorted_dir_entries(dir.path(), &exact),
            Some(expected.clone())
        );
        let stats = exact.external_type_session.stats();
        assert_eq!(stats.tsconfig_discovery_entries, names.len());
        assert_eq!(stats.tsconfig_materialization_entries, names.len());
        assert_eq!(stats.tsconfig_materialization_weight, path_weight);
        assert_eq!(stats.tsconfig_normalization_steps, normalization_steps);
        assert!(!exact.external_type_session.metadata_is_blocked());

        let short_path = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: path_bytes - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(vue3_tsconfig_bounded_sorted_dir_entries(dir.path(), &short_path).is_none());
        let stats = short_path.external_type_session.stats();
        assert_eq!(stats.tsconfig_discovery_entries, 1);
        assert_eq!(stats.tsconfig_materialization_entries, 0);
        assert_eq!(stats.tsconfig_materialization_weight, 0);
        assert!(short_path.external_type_session.metadata_is_blocked());

        let short_materialization = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_materialization_entries: names.len(),
            max_tsconfig_materialization_weight: path_weight - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(
            vue3_tsconfig_bounded_sorted_dir_entries(dir.path(), &short_materialization)
                .is_none()
        );
        assert_eq!(
            short_materialization
                .external_type_session
                .stats()
                .tsconfig_materialization_weight,
            path_weight - 1
        );
        assert!(short_materialization
            .external_type_session
            .metadata_is_blocked());

        let short_normalization = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_normalization_steps: normalization_steps - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(
            vue3_tsconfig_bounded_sorted_dir_entries(dir.path(), &short_normalization).is_none()
        );
        assert_eq!(
            short_normalization
                .external_type_session
                .stats()
                .tsconfig_normalization_steps,
            normalization_steps - 1
        );
        assert!(short_normalization
            .external_type_session
            .metadata_is_blocked());
    }

    #[test]
    fn singleton_directory_needs_no_sort_budget() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("only");
        std::fs::write(&path, "").expect("write directory entry");
        let resolver = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_normalization_steps: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert_eq!(
            vue3_tsconfig_bounded_sorted_dir_entries(dir.path(), &resolver),
            Some(vec![path])
        );
        assert_eq!(
            resolver
                .external_type_session
                .stats()
                .tsconfig_normalization_steps,
            0
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }
}

pub(crate) fn vue3_tsconfig_global_type_file_is_supported(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                [".d.ts", ".d.mts", ".d.cts"]
                    .iter()
                    .any(|extension| name.ends_with(extension))
            })
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_include_global_type_files(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    vue3_tsconfig_include_global_type_files_with_excludes(
        config_dir,
        template_config_dir,
        target,
        &[],
        &[],
        type_resolver,
    )
}

fn vue3_tsconfig_include_global_type_files_with_excludes(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    exclude_matchers: &[Vue3CompiledTsconfigGlob<'_>],
    excluded_directory_keys: &[PathBuf],
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    if !vue3_tsconfig_include_can_match_global_type_files(target) {
        return Vec::new();
    }
    if !type_resolver
        .external_type_session
        .claim_tsconfig_discovery_entry()
    {
        return Vec::new();
    }
    if !target.contains('*') && !target.contains('?') {
        let Some(path) =
            vue3_tsconfig_target_path(config_dir, template_config_dir, target, type_resolver)
        else {
            return Vec::new();
        };
        if vue3_tsconfig_global_type_file_is_supported(&path) {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_discovery_file()
            {
                return Vec::new();
            }
            return vue3_tsconfig_filter_global_type_files(
                vec![path],
                None,
                exclude_matchers,
                type_resolver,
            );
        }
        if path.is_dir() {
            let mut files = Vec::new();
            vue3_collect_global_type_files_from_dir_with_excludes(
                &path,
                &mut files,
                excluded_directory_keys,
                type_resolver,
            );
            return vue3_tsconfig_filter_global_type_files(
                files,
                None,
                exclude_matchers,
                type_resolver,
            );
        }
        return Vec::new();
    }
    let Some(glob) =
        vue3_tsconfig_include_glob(config_dir, template_config_dir, target, type_resolver)
    else {
        return Vec::new();
    };
    let mut files = Vec::new();
    vue3_collect_global_type_files_from_dir_with_excludes(
        &glob.root,
        &mut files,
        excluded_directory_keys,
        type_resolver,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vec::new();
    }
    let matcher = Vue3CompiledTsconfigGlob::new(&glob.pattern);
    vue3_tsconfig_filter_global_type_files(
        files,
        Some(&matcher),
        exclude_matchers,
        type_resolver,
    )
}

fn vue3_tsconfig_filter_global_type_files(
    files: Vec<PathBuf>,
    include_matcher: Option<&Vue3CompiledTsconfigGlob<'_>>,
    exclude_matchers: &[Vue3CompiledTsconfigGlob<'_>],
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    if include_matcher.is_none() && exclude_matchers.is_empty() {
        return files;
    }
    let mut budget = type_resolver
        .external_type_session
        .tsconfig_glob_match_budget();
    let mut matched = Vec::new();
    for file in files {
        let path = normalize_path_string(&file);
        if let Some(matcher) = include_matcher {
            match matcher.matches(&path, &mut || budget.claim_step()) {
                Some(true) => {}
                Some(false) => continue,
                None => break,
            }
        }
        let mut excluded = false;
        for matcher in exclude_matchers {
            match matcher.matches(&path, &mut || budget.claim_step()) {
                Some(true) => {
                    excluded = true;
                    break;
                }
                Some(false) => {}
                None => break,
            }
        }
        if budget.is_exhausted() {
            break;
        }
        if !excluded {
            matched.push(file);
        }
    }
    if !budget.finish() || type_resolver.external_type_session.metadata_is_blocked() {
        Vec::new()
    } else {
        matched
    }
}

fn vue3_tsconfig_exclude_patterns(
    targets: &[String],
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigExcludes> {
    let mut excludes = Vue3TsconfigExcludes::default();
    for target in targets {
        if !type_resolver
            .external_type_session
            .claim_tsconfig_discovery_entry()
        {
            return None;
        }
        let path = vue3_tsconfig_include_path(
            config_dir,
            template_config_dir,
            target,
            type_resolver,
        )?;
        let final_segment = target.rsplit(['/', '\\']).next().unwrap_or(target);
        let is_directory_pattern = path.is_dir()
            || target.ends_with('/')
            || target.ends_with('\\')
            || !final_segment.contains('.');
        let mut pattern = normalize_path_string(&path);
        if is_directory_pattern {
            if !target.contains(['*', '?']) {
                excludes
                    .directory_keys
                    .push(vue3_external_type_path_key(path));
            }
            pattern = type_resolver.external_type_session.concat_metadata_path(
                pattern.trim_end_matches('/'),
                "/**",
            )?;
        }
        excludes.patterns.push(pattern);
    }
    Some(excludes)
}

fn vue3_tsconfig_output_directory_exclude_patterns(
    specs: &Vue3TsconfigOutputDirectorySpecs,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigExcludes> {
    let mut excludes = Vue3TsconfigExcludes::default();
    for specs in [specs.out_dir.as_ref(), specs.declaration_dir.as_ref()]
        .into_iter()
        .flatten()
    {
        for target in specs.targets.iter() {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_discovery_entry()
            {
                return None;
            }
            let path = vue3_tsconfig_target_path(
                &specs.config_dir,
                &specs.template_config_dir,
                target,
                type_resolver,
            )?;
            let pattern = type_resolver.external_type_session.concat_metadata_path(
                normalize_path_string(&path).trim_end_matches('/'),
                "/**",
            )?;
            excludes
                .directory_keys
                .push(vue3_external_type_path_key(path));
            excludes.patterns.push(pattern);
        }
    }
    Some(excludes)
}

pub(crate) fn vue3_tsconfig_include_can_match_global_type_files(target: &str) -> bool {
    let file_pattern = target
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(target);
    if !file_pattern.contains('.') {
        return true;
    }
    [".d.ts", ".d.mts", ".d.cts", ".ts", ".mts", ".cts"]
        .iter()
        .any(|extension| file_pattern.ends_with(extension))
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_include_pattern(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<String> {
    let path = vue3_tsconfig_include_path(
        config_dir,
        template_config_dir,
        target,
        type_resolver,
    )?;
    Some(normalize_path_string(&path))
}

fn vue3_tsconfig_include_path(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    vue3_tsconfig_target_path(config_dir, template_config_dir, target, type_resolver)
}

struct Vue3TsconfigIncludeGlob {
    pattern: String,
    root: PathBuf,
}

fn vue3_tsconfig_include_glob(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigIncludeGlob> {
    let path = vue3_tsconfig_include_path(
        config_dir,
        template_config_dir,
        target,
        type_resolver,
    )?;
    let root = vue3_tsconfig_include_root_from_pattern(&path)?;
    Some(Vue3TsconfigIncludeGlob {
        pattern: normalize_path_string(&path),
        root,
    })
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_include_root_path(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let pattern = vue3_tsconfig_include_path(
        config_dir,
        template_config_dir,
        target,
        type_resolver,
    )?;
    vue3_tsconfig_include_root_from_pattern(&pattern)
}

fn vue3_tsconfig_include_root_from_pattern(pattern: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    for component in pattern.components() {
        let contains_wildcard = matches!(
            component,
            std::path::Component::Normal(segment)
                if segment.to_string_lossy().contains(['*', '?'])
        );
        if contains_wildcard {
            break;
        }
        root.push(component.as_os_str());
    }
    if root.as_os_str().is_empty() {
        root.push(".");
    }
    root.is_dir().then_some(root)
}

#[cfg(test)]
pub(crate) fn vue3_collect_global_type_files_from_dir(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) {
    vue3_collect_global_type_files_from_dir_with_excludes(dir, files, &[], type_resolver);
}

fn vue3_collect_global_type_files_from_dir_with_excludes(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    excluded_directory_keys: &[PathBuf],
    type_resolver: &Vue3TypeResolverContext,
) {
    let initial_len = files.len();
    let mut seen_dirs = BTreeSet::new();
    let max_depth = type_resolver
        .external_type_session
        .max_tsconfig_discovery_depth();
    vue3_collect_global_type_files_from_dir_inner(
        dir,
        files,
        type_resolver,
        &mut seen_dirs,
        excluded_directory_keys,
        0,
        max_depth,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        files.truncate(initial_len);
    }
}

fn vue3_collect_global_type_files_from_dir_inner(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
    seen_dirs: &mut BTreeSet<PathBuf>,
    excluded_directory_keys: &[PathBuf],
    depth: usize,
    max_depth: usize,
) {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return;
    }
    let directory_key = vue3_external_type_path_key(normalize_path_components(dir.to_path_buf()));
    if excluded_directory_keys
        .iter()
        .any(|excluded| directory_key == *excluded || directory_key.starts_with(excluded))
    {
        return;
    }
    let canonical_dir =
        std::fs::canonicalize(dir).unwrap_or_else(|_| normalize_path_components(dir.to_path_buf()));
    if !seen_dirs.insert(canonical_dir) {
        return;
    }
    let Some(entries) = vue3_tsconfig_bounded_sorted_dir_entries(dir, type_resolver) else {
        return;
    };
    for path in entries {
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if vue3_tsconfig_directory_is_implicitly_excluded(name) {
            continue;
        }
        if file_type.is_dir() {
            if depth < max_depth {
                vue3_collect_global_type_files_from_dir_inner(
                    &path,
                    files,
                    type_resolver,
                    seen_dirs,
                    excluded_directory_keys,
                    depth + 1,
                    max_depth,
                );
            }
        } else if file_type.is_file() && vue3_tsconfig_global_type_file_is_supported(&path) {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_discovery_file()
            {
                return;
            }
            files.push(normalize_path_components(path));
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return;
        }
    }
}

fn vue3_tsconfig_directory_is_implicitly_excluded(name: &str) -> bool {
    name.starts_with('.')
        || ["node_modules", "bower_components", "jspm_packages"]
            .iter()
            .any(|excluded| {
                if cfg!(windows) {
                    name.eq_ignore_ascii_case(excluded)
                } else {
                    name == *excluded
                }
            })
}

struct Vue3CompiledTsconfigGlob<'a> {
    parts: Vec<&'a str>,
}

impl<'a> Vue3CompiledTsconfigGlob<'a> {
    fn new(pattern: &'a str) -> Self {
        Self::from_parts(pattern.split('/'))
    }

    fn from_parts(parts: impl IntoIterator<Item = &'a str>) -> Self {
        let mut compiled = Vec::new();
        for part in parts {
            if part == "**" && compiled.last().copied() == Some("**") {
                continue;
            }
            compiled.push(part);
        }
        Self { parts: compiled }
    }

    fn matches(
        &self,
        path: &str,
        claim_step: &mut impl FnMut() -> bool,
    ) -> Option<bool> {
        let path_parts = path.split('/').collect::<Vec<_>>();
        self.matches_parts(&path_parts, claim_step)
    }

    fn matches_parts(
        &self,
        path: &[&str],
        claim_step: &mut impl FnMut() -> bool,
    ) -> Option<bool> {
        if !claim_step() {
            return None;
        }
        let mut pattern_index = 0;
        let mut path_index = 0;
        let mut double_star_pattern_index = None;
        let mut double_star_path_index = 0;
        while path_index < path.len() {
            if !claim_step() {
                return None;
            }
            if self.parts.get(pattern_index).copied() == Some("**") {
                double_star_pattern_index = Some(pattern_index);
                double_star_path_index = path_index;
                pattern_index += 1;
                continue;
            }
            let segment_matches = match self.parts.get(pattern_index) {
                Some(pattern) => vue3_tsconfig_glob_segment_match_bounded(
                    pattern,
                    path[path_index],
                    claim_step,
                )?,
                None => false,
            };
            if segment_matches {
                pattern_index += 1;
                path_index += 1;
                continue;
            }
            let Some(double_star_index) = double_star_pattern_index else {
                return Some(false);
            };
            if !claim_step() {
                return None;
            }
            double_star_path_index += 1;
            path_index = double_star_path_index;
            pattern_index = double_star_index + 1;
        }
        while self.parts.get(pattern_index).copied() == Some("**") {
            if !claim_step() {
                return None;
            }
            pattern_index += 1;
        }
        Some(pattern_index == self.parts.len())
    }
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_glob_matches(pattern: &str, path: &str) -> bool {
    let pattern: std::borrow::Cow<'_, str> = if pattern.contains('\\') {
        std::borrow::Cow::Owned(pattern.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(pattern)
    };
    let path: std::borrow::Cow<'_, str> = if path.contains('\\') {
        std::borrow::Cow::Owned(path.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(path)
    };
    Vue3CompiledTsconfigGlob::new(&pattern)
        .matches(&path, &mut || true)
        .unwrap_or(false)
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_glob_matches_with_session(
    pattern: &str,
    path: &str,
    session: &Vue3ExternalTypeLoadSession,
) -> Option<bool> {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    let matcher = Vue3CompiledTsconfigGlob::new(&pattern);
    let mut budget = session.tsconfig_glob_match_budget();
    let result = matcher.matches(&path, &mut || budget.claim_step());
    if budget.finish() {
        result
    } else {
        None
    }
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_glob_parts_match(pattern: &[&str], path: &[&str]) -> bool {
    Vue3CompiledTsconfigGlob::from_parts(pattern.iter().copied())
        .matches_parts(path, &mut || true)
        .unwrap_or(false)
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_glob_segment_match(pattern: &str, text: &str) -> bool {
    vue3_tsconfig_glob_segment_match_bounded(pattern, text, &mut || true).unwrap_or(false)
}

fn vue3_tsconfig_glob_segment_match_bounded(
    pattern: &str,
    text: &str,
    claim_step: &mut impl FnMut() -> bool,
) -> Option<bool> {
    if !claim_step() {
        return None;
    }
    let mut pattern_index = 0;
    let mut text_index = 0;
    let mut star_pattern_index = None;
    let mut star_text_index = 0;
    while text_index < text.len() {
        if !claim_step() {
            return None;
        }
        let pattern_char = vue3_tsconfig_glob_next_char(pattern, pattern_index);
        let (text_char, next_text_index) =
            vue3_tsconfig_glob_next_char(text, text_index).expect("valid glob text index");
        match pattern_char {
            Some(('*', mut next_pattern_index)) => {
                while let Some(('*', next_index)) =
                    vue3_tsconfig_glob_next_char(pattern, next_pattern_index)
                {
                    if !claim_step() {
                        return None;
                    }
                    next_pattern_index = next_index;
                }
                star_pattern_index = Some(next_pattern_index);
                star_text_index = text_index;
                pattern_index = next_pattern_index;
                if pattern_index == pattern.len() {
                    return Some(true);
                }
            }
            Some(('?', next_pattern_index)) => {
                pattern_index = next_pattern_index;
                text_index = next_text_index;
            }
            Some((pattern_char, next_pattern_index))
                if pattern_char == text_char
                    || (cfg!(windows) && pattern_char.eq_ignore_ascii_case(&text_char)) =>
            {
                pattern_index = next_pattern_index;
                text_index = next_text_index;
            }
            _ => {
                let Some(retry_pattern_index) = star_pattern_index else {
                    return Some(false);
                };
                if !claim_step() {
                    return None;
                }
                let Some((_, next_star_text_index)) =
                    vue3_tsconfig_glob_next_char(text, star_text_index)
                else {
                    return Some(false);
                };
                star_text_index = next_star_text_index;
                text_index = next_star_text_index;
                pattern_index = retry_pattern_index;
            }
        }
    }
    while let Some(('*', next_pattern_index)) =
        vue3_tsconfig_glob_next_char(pattern, pattern_index)
    {
        if !claim_step() {
            return None;
        }
        pattern_index = next_pattern_index;
    }
    Some(pattern_index == pattern.len())
}

fn vue3_tsconfig_glob_next_char(source: &str, index: usize) -> Option<(char, usize)> {
    let ch = source.get(index..)?.chars().next()?;
    Some((ch, index + ch.len_utf8()))
}

#[cfg(test)]
mod vue3_type_reference_directive_tests {
    use super::*;

    fn write_type_package(type_root: &Path, name: &str) -> PathBuf {
        write_type_package_with_entry(type_root, name, "index.d.ts")
    }

    fn write_type_package_with_entry(
        type_root: &Path,
        name: &str,
        entry_name: &str,
    ) -> PathBuf {
        let package_dir = type_root.join(name);
        std::fs::create_dir_all(&package_dir).expect("create type package directory");
        std::fs::write(
            package_dir.join("package.json"),
            format!(r#"{{"types":"{entry_name}"}}"#),
        )
        .expect("write type package manifest");
        let entry = package_dir.join(entry_name);
        std::fs::write(&entry, "declare interface ReferencedGlobal {}")
            .expect("write type package entry");
        entry
    }

    fn write_conditional_type_package(
        node_modules: &Path,
        name: &str,
        manifest: &str,
        entries: &[&str],
    ) -> PathBuf {
        let package_dir = node_modules.join(name);
        std::fs::create_dir_all(&package_dir).expect("create conditional type package");
        std::fs::write(package_dir.join("package.json"), manifest)
            .expect("write conditional package manifest");
        for entry in entries {
            std::fs::write(
                package_dir.join(entry),
                format!("interface {} {{}}", entry.replace('.', "_")),
            )
            .expect("write conditional package entry");
        }
        package_dir
    }

    fn resolver_with_limits(limits: Vue3ExternalTypeLoadLimits) -> Vue3TypeResolverContext {
        Vue3TypeResolverContext {
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(limits),
            ..Vue3TypeResolverContext::default()
        }
    }

    fn assert_node_type_reference_modes(
        project: &Path,
        containing: &Path,
        type_name: &str,
        import_expected: Option<&Path>,
        require_expected: Option<&Path>,
    ) {
        let project = project.to_string_lossy();
        let containing = containing.to_string_lossy();
        for module_resolution in [
            Vue3TypeModuleResolutionKind::Node16,
            Vue3TypeModuleResolutionKind::NodeNext,
        ] {
            for (mode, expected) in [
                (Vue3TypeResolutionMode::Import, import_expected),
                (Vue3TypeResolutionMode::Require, require_expected),
            ] {
                let resolver = Vue3TypeResolverContext {
                    typescript_version: (6, 0, 3).into(),
                    module_resolution,
                    ..Vue3TypeResolverContext::default()
                };
                let actual = resolve_vue3_type_reference_directive_with_mode(
                    &project,
                    &containing,
                    type_name,
                    Some(mode),
                    &resolver,
                );
                assert_eq!(
                    actual.as_deref(),
                    expected,
                    "{module_resolution:?} {mode:?} type reference {type_name}"
                );
            }
        }
    }

    #[test]
    fn reference_type_root_candidates_claim_resources_before_path_construction() {
        let dir = tempfile::tempdir().expect("temp dir");
        let type_root = dir.path().join("types");
        std::fs::create_dir_all(&type_root).expect("create type root");
        let type_name = "missing";
        let path_bytes = vue3_ancestor_search_candidate_weight(&type_root, type_name);
        let materialization_weight = std::mem::size_of::<PathBuf>() + path_bytes;
        let type_roots = Vue3TsconfigTypeRoots {
            paths: std::sync::Arc::from(vec![type_root]),
            is_explicit: true,
        };
        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_target_steps: path_bytes,
            max_tsconfig_materialization_entries: 1,
            max_tsconfig_materialization_weight: materialization_weight,
            max_tsconfig_discovery_entries: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert!(resolve_vue3_tsconfig_named_type_global_type_file(
            &type_roots,
            type_name,
            None,
            &exact,
        )
        .is_none());
        let exact_stats = exact.external_type_session.stats();
        assert_eq!(exact_stats.metadata_target_steps, path_bytes);
        assert_eq!(exact_stats.tsconfig_discovery_entries, 1);
        assert_eq!(exact_stats.tsconfig_materialization_entries, 1);
        assert_eq!(
            exact_stats.tsconfig_materialization_weight,
            materialization_weight
        );
        assert!(!exact.external_type_session.metadata_is_blocked());

        for limits in [
            Vue3ExternalTypeLoadLimits {
                max_metadata_target_steps: path_bytes - 1,
                max_tsconfig_materialization_entries: 1,
                max_tsconfig_materialization_weight: materialization_weight,
                max_tsconfig_discovery_entries: 1,
                ..Vue3ExternalTypeLoadLimits::default()
            },
            Vue3ExternalTypeLoadLimits {
                max_metadata_target_steps: path_bytes,
                max_generated_path_bytes: path_bytes - 1,
                max_tsconfig_materialization_entries: 1,
                max_tsconfig_materialization_weight: materialization_weight,
                max_tsconfig_discovery_entries: 1,
                ..Vue3ExternalTypeLoadLimits::default()
            },
            Vue3ExternalTypeLoadLimits {
                max_metadata_target_steps: path_bytes,
                max_tsconfig_materialization_entries: 1,
                max_tsconfig_materialization_weight: materialization_weight - 1,
                max_tsconfig_discovery_entries: 1,
                ..Vue3ExternalTypeLoadLimits::default()
            },
        ] {
            let resolver = resolver_with_limits(limits);
            assert!(resolve_vue3_tsconfig_named_type_global_type_file(
                &type_roots,
                type_name,
                None,
                &resolver,
            )
            .is_none());
            let stats = resolver.external_type_session.stats();
            assert_eq!(stats.metadata_resolution_path_probes, 0);
            assert_eq!(stats.tsconfig_materialization_entries, 0);
            assert!(resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn reference_type_roots_are_materialization_bounded_and_resolution_cached() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        std::fs::create_dir_all(&project_dir).expect("create project directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./first","./second"]}}"#,
        )
        .expect("write project config");
        let filename = project_dir.join("Comp.vue").to_string_lossy().to_string();
        let measuring = Vue3TypeResolverContext::default();

        assert!(resolve_vue3_type_reference_directive(
            &filename,
            &filename,
            "missing",
            &measuring,
        )
        .is_none());
        let measured = measuring.external_type_session.stats();
        assert!(measured.metadata_target_steps > 0);
        assert!(measured.metadata_resolution_path_probes > 0);
        assert!(measured.tsconfig_discovery_entries > 0);
        assert!(measured.tsconfig_materialization_entries >= 4);
        assert!(measured.tsconfig_materialization_weight > 0);
        assert!(!measuring.external_type_session.metadata_is_blocked());

        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_target_steps: measured.metadata_target_steps,
            max_metadata_resolution_path_probes: measured.metadata_resolution_path_probes,
            max_tsconfig_materialization_entries: measured.tsconfig_materialization_entries,
            max_tsconfig_materialization_weight: measured.tsconfig_materialization_weight,
            max_tsconfig_discovery_entries: measured.tsconfig_discovery_entries,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve_vue3_type_reference_directive(
            &filename,
            &filename,
            "missing",
            &exact,
        )
        .is_none());
        let exact_stats = exact.external_type_session.stats();
        assert_eq!(exact_stats.metadata_target_steps, measured.metadata_target_steps);
        assert_eq!(
            exact_stats.metadata_resolution_path_probes,
            measured.metadata_resolution_path_probes
        );
        assert_eq!(
            exact_stats.tsconfig_discovery_entries,
            measured.tsconfig_discovery_entries
        );
        assert_eq!(
            exact_stats.tsconfig_materialization_entries,
            measured.tsconfig_materialization_entries
        );
        assert_eq!(
            exact_stats.tsconfig_materialization_weight,
            measured.tsconfig_materialization_weight
        );
        assert!(!exact.external_type_session.metadata_is_blocked());

        assert!(resolve_vue3_type_reference_directive(
            &filename,
            &filename,
            "missing",
            &exact,
        )
        .is_none());
        let cached_stats = exact.external_type_session.stats();
        assert_eq!(cached_stats.resolution_cache_hits, 1);
        assert_eq!(
            cached_stats.metadata_target_steps,
            exact_stats.metadata_target_steps
        );
        assert_eq!(
            cached_stats.metadata_resolution_path_probes,
            exact_stats.metadata_resolution_path_probes
        );
        assert_eq!(
            cached_stats.tsconfig_discovery_entries,
            exact_stats.tsconfig_discovery_entries
        );
        assert_eq!(
            cached_stats.tsconfig_materialization_entries,
            exact_stats.tsconfig_materialization_entries
        );
        assert_eq!(
            cached_stats.tsconfig_materialization_weight,
            exact_stats.tsconfig_materialization_weight
        );

        for limits in [
            Vue3ExternalTypeLoadLimits {
                max_metadata_target_steps: measured.metadata_target_steps,
                max_tsconfig_materialization_entries:
                    measured.tsconfig_materialization_entries - 1,
                max_tsconfig_materialization_weight: measured.tsconfig_materialization_weight,
                ..Vue3ExternalTypeLoadLimits::default()
            },
            Vue3ExternalTypeLoadLimits {
                max_metadata_target_steps: measured.metadata_target_steps,
                max_tsconfig_materialization_entries: measured.tsconfig_materialization_entries,
                max_tsconfig_materialization_weight:
                    measured.tsconfig_materialization_weight - 1,
                ..Vue3ExternalTypeLoadLimits::default()
            },
        ] {
            let resolver = resolver_with_limits(limits);
            assert!(resolve_vue3_type_reference_directive(
                &filename,
                &filename,
                "missing",
                &resolver,
            )
            .is_none());
            assert!(resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn reference_types_uses_effective_extended_type_roots() {
        let dir = tempfile::tempdir().expect("temp dir");
        let base_dir = dir.path().join("configs").join("base");
        let project_dir = dir.path().join("project");
        std::fs::create_dir_all(project_dir.join("src")).expect("create project source dir");
        std::fs::create_dir_all(&base_dir).expect("create base config dir");
        let expected = write_type_package(&base_dir.join("types"), "referenced");
        let decoy = write_type_package(&project_dir.join("types"), "referenced");
        std::fs::write(
            base_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./types"]}}"#,
        )
        .expect("write base config");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{
                "extends":"../configs/base/tsconfig.json",
                "compilerOptions":{"types":[]}
            }"#,
        )
        .expect("write project config");
        let project = project_dir.join("src").join("Comp.vue");
        let containing = project_dir.join("src").join("ambient.d.ts");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "referenced",
                &resolver,
            ),
            Some(expected)
        );
        assert_ne!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "referenced",
                &Vue3TypeResolverContext::default(),
            ),
            Some(decoy)
        );
    }

    #[test]
    fn reference_types_applies_later_extends_and_direct_overrides() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let first_dir = dir.path().join("first");
        let second_dir = dir.path().join("second");
        std::fs::create_dir_all(project_dir.join("src")).expect("create project source dir");
        std::fs::create_dir_all(&first_dir).expect("create first config dir");
        std::fs::create_dir_all(&second_dir).expect("create second config dir");
        let _first = write_type_package(&first_dir.join("types"), "ordered");
        let second = write_type_package(&second_dir.join("types"), "ordered");
        let direct = write_type_package(&project_dir.join("types"), "direct");
        std::fs::write(
            first_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./types"]}}"#,
        )
        .expect("write first config");
        std::fs::write(
            second_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./types"]}}"#,
        )
        .expect("write second config");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{
                "extends":["../first/tsconfig.json","../second/tsconfig.json"],
                "compilerOptions":{"typeRoots":["./types"]}
            }"#,
        )
        .expect("write project config");
        let project = project_dir.join("src").join("Comp.vue");
        let containing = project_dir.join("src").join("ambient.d.ts");

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "direct",
                &Vue3TypeResolverContext::default(),
            ),
            Some(direct)
        );

        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"extends":["../first/tsconfig.json","../second/tsconfig.json"]}"#,
        )
        .expect("replace project config");
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "ordered",
                &Vue3TypeResolverContext::default(),
            ),
            Some(second)
        );
    }

    #[test]
    fn reference_types_use_the_nearest_project_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("packages").join("component");
        let source_dir = project_dir.join("src");
        std::fs::create_dir_all(&source_dir).expect("create project source dir");
        let _outer = write_type_package(&dir.path().join("outer-types"), "nearest");
        let inner = write_type_package(&project_dir.join("inner-types"), "nearest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./outer-types"]}}"#,
        )
        .expect("write outer config");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./inner-types"]}}"#,
        )
        .expect("write nearest config");
        let filename = source_dir.join("Comp.vue");

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "nearest",
                &Vue3TypeResolverContext::default(),
            ),
            Some(inner)
        );
    }

    #[test]
    fn reference_types_prefers_default_type_roots_then_uses_containing_file_fallback() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let containing_dir = dir.path().join("dependencies").join("consumer");
        std::fs::create_dir_all(&project_dir).expect("create project dir");
        std::fs::create_dir_all(&containing_dir).expect("create containing dir");
        let primary = write_type_package(
            &project_dir.join("node_modules").join("@types"),
            "preferred",
        );
        let _secondary_decoy =
            write_type_package(&containing_dir.join("node_modules"), "preferred");
        let secondary = write_type_package(&containing_dir.join("node_modules"), "secondary");
        let project = project_dir.join("Comp.vue");
        let containing = containing_dir.join("index.d.ts");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "preferred",
                &resolver,
            ),
            Some(primary)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "secondary",
                &resolver,
            ),
            Some(secondary)
        );
    }

    #[test]
    fn reference_types_empty_type_roots_still_use_containing_file_fallback() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let containing_dir = dir.path().join("dependency");
        std::fs::create_dir_all(&project_dir).expect("create project dir");
        std::fs::create_dir_all(&containing_dir).expect("create containing dir");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        let expected = write_type_package(&containing_dir.join("node_modules"), "fallback");
        let project = project_dir.join("Comp.vue");
        let containing = containing_dir.join("ambient.d.ts");

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "fallback",
                &Vue3TypeResolverContext::default(),
            ),
            Some(expected)
        );
    }

    #[test]
    fn reference_types_accept_backslash_relative_and_absolute_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let nested_dir = project_dir.join("types");
        std::fs::create_dir_all(&nested_dir).expect("create type directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        let backslash = nested_dir.join("backslash.d.ts");
        let absolute = nested_dir.join("absolute.d.ts");
        std::fs::write(&backslash, "interface BackslashReference {}")
            .expect("write backslash declaration");
        std::fs::write(&absolute, "interface AbsoluteReference {}")
            .expect("write absolute declaration");
        let project = project_dir.join("Comp.vue");
        let containing = project_dir.join("ambient.d.ts");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                r#".\types\backslash"#,
                &resolver,
            ),
            Some(backslash)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                &absolute.to_string_lossy(),
                &resolver,
            ),
            Some(absolute)
        );
    }

    #[test]
    fn reference_types_explicit_roots_precede_relative_secondary_lookup() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let source_dir = project_dir.join("src");
        let type_root = project_dir.join("types");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::create_dir_all(&type_root).expect("create type root");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":["./types"]}}"#,
        )
        .expect("write project config");
        let primary = type_root.join("local.d.ts");
        let secondary = source_dir.join("local.d.ts");
        std::fs::write(&primary, "interface PrimaryReference {}")
            .expect("write primary declaration");
        std::fs::write(&secondary, "interface SecondaryReference {}")
            .expect("write secondary declaration");
        let project = source_dir.join("Comp.vue");
        let containing = source_dir.join("ambient.d.ts");

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                r#".\local"#,
                &Vue3TypeResolverContext::default(),
            ),
            Some(primary)
        );
    }

    #[test]
    fn reference_types_secondary_lookup_is_declaration_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let node_modules = project_dir.join("node_modules");
        std::fs::create_dir_all(&node_modules).expect("create node_modules");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        let runtime = project_dir.join("runtime.ts");
        let declaration = project_dir.join("declaration.d.ts");
        std::fs::write(&runtime, "interface RuntimeOnly {}")
            .expect("write runtime source");
        std::fs::write(&declaration, "interface DeclarationOnly {}")
            .expect("write declaration source");
        let implicit_package = node_modules.join("implicit-runtime");
        std::fs::create_dir_all(&implicit_package).expect("create implicit runtime package");
        std::fs::write(
            implicit_package.join("index.ts"),
            "interface ImplicitRuntime {}",
        )
        .expect("write implicit runtime package entry");
        let explicit = write_type_package_with_entry(
            &node_modules,
            "explicit-runtime",
            "index.ts",
        );
        let exports_package = node_modules.join("exports-do-not-block-types");
        std::fs::create_dir_all(&exports_package).expect("create exports package");
        std::fs::write(
            exports_package.join("package.json"),
            r#"{"types":"index.d.ts","exports":{}}"#,
        )
        .expect("write exports package metadata");
        let exports_declaration = exports_package.join("index.d.ts");
        std::fs::write(
            &exports_declaration,
            "interface ExportsDoNotBlockTypes {}",
        )
        .expect("write exports package declaration");
        let main_package = node_modules.join("main-entry");
        let main_dist = main_package.join("dist");
        std::fs::create_dir_all(&main_dist).expect("create main package");
        std::fs::write(
            main_package.join("package.json"),
            r#"{"main":"dist/index.js"}"#,
        )
        .expect("write main package metadata");
        let main_declaration = main_dist.join("index.d.ts");
        std::fs::write(&main_declaration, "interface MainEntryTypes {}")
            .expect("write main package declaration");
        let dotted_decoy = node_modules.join("dotted.d.ts");
        let dotted_appended = node_modules.join("dotted.package.d.ts");
        let dotted_arbitrary = node_modules.join("dotted.d.package.ts");
        let hidden_appended = project_dir.join(".hidden.d.ts");
        let hidden_arbitrary = project_dir.join(".d.hidden.ts");
        std::fs::write(&dotted_decoy, "interface DottedDecoy {}")
            .expect("write dotted package decoy");
        std::fs::write(&dotted_appended, "interface DottedAppended {}")
            .expect("write appended dotted package declaration");
        std::fs::write(&dotted_arbitrary, "interface DottedArbitrary {}")
            .expect("write arbitrary-extension dotted package declaration");
        std::fs::write(&hidden_appended, "interface HiddenAppended {}")
            .expect("write appended hidden declaration");
        std::fs::write(&hidden_arbitrary, "interface HiddenArbitrary {}")
            .expect("write arbitrary-extension hidden declaration");
        let filename = project_dir.join("Comp.vue");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "./runtime",
                &resolver,
            ),
            None
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "./declaration",
                &resolver,
            ),
            Some(declaration)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "implicit-runtime",
                &resolver,
            ),
            None
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "explicit-runtime",
                &resolver,
            ),
            Some(explicit)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "exports-do-not-block-types",
                &resolver,
            ),
            Some(exports_declaration)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "main-entry",
                &resolver,
            ),
            Some(main_declaration)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "dotted.package",
                &resolver,
            ),
            Some(dotted_arbitrary)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "./.hidden",
                &resolver,
            ),
            Some(hidden_arbitrary)
        );
    }

    #[test]
    fn reference_types_types_versions_match_suppresses_package_fallbacks() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let node_modules = project_dir.join("node_modules");
        std::fs::create_dir_all(&node_modules).expect("create node_modules");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");

        for (package_name, target, write_target) in [
            ("missing-mapping", "missing.d.ts", false),
            ("raw-javascript-mapping", "mapped.js", true),
        ] {
            let package_dir = node_modules.join(package_name);
            std::fs::create_dir_all(&package_dir).expect("create package directory");
            std::fs::write(
                package_dir.join("package.json"),
                format!(
                    r#"{{"types":"source.d.ts","typesVersions":{{"*":{{"source.d.ts":["{target}"]}}}}}}"#
                ),
            )
            .expect("write package manifest");
            std::fs::write(
                package_dir.join("source.d.ts"),
                "interface TypeFieldDecoy {}",
            )
            .expect("write package type field decoy");
            std::fs::write(
                package_dir.join("index.d.ts"),
                "interface IndexFallbackDecoy {}",
            )
            .expect("write package index decoy");
            if write_target {
                std::fs::write(
                    package_dir.join(target),
                    "export const implementation = true;",
                )
                .expect("write raw JavaScript mapping target");
            }
        }

        let filename = project_dir.join("Comp.vue");
        for package_name in ["missing-mapping", "raw-javascript-mapping"] {
            let resolver = Vue3TypeResolverContext::default();
            assert_eq!(
                resolve_vue3_type_reference_directive(
                    &filename.to_string_lossy(),
                    &filename.to_string_lossy(),
                    package_name,
                    &resolver,
                ),
                None,
                "package {package_name}",
            );
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn reference_types_package_targets_honor_exact_generated_path_limit() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let package_dir = project_dir.join("node_modules").join("limited-main");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&dist_dir).expect("create package directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"main":"dist/index.js"}"#,
        )
        .expect("write package metadata");
        let target = dist_dir.join("index.d.ts");
        std::fs::write(&target, "interface LimitedMainReference {}")
            .expect("write package declaration");
        let filename = project_dir.join("Comp.vue");
        let required = target.as_os_str().as_encoded_bytes().len();
        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: required,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "limited-main",
                &exact,
            ),
            Some(target)
        );
        assert!(!exact.external_type_session.metadata_is_blocked());

        let short = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: required - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "limited-main",
                &short,
            ),
            None
        );
        assert!(short.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn reference_types_match_typescript_scoped_package_locations() {
        let dir = tempfile::tempdir().expect("temp dir");
        let configured_project = dir.path().join("configured");
        let default_project = dir.path().join("default");
        std::fs::create_dir_all(&configured_project).expect("create configured project");
        std::fs::create_dir_all(&default_project).expect("create default project");
        let custom_root = configured_project.join("custom-types");
        let configured = write_type_package(&custom_root.join("@scope"), "package");
        let _invalid_mangled = write_type_package(&custom_root, "scope__invalid");
        std::fs::write(
            configured_project.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":["./custom-types"]}}"#,
        )
        .expect("write configured project config");
        let secondary = write_type_package(
            &default_project.join("node_modules").join("@types"),
            "scope__secondary",
        );
        std::fs::write(
            default_project.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[]}}"#,
        )
        .expect("write default project config");

        let resolver = Vue3TypeResolverContext {
            typescript_version: (5, 1, 0).into(),
            ..Default::default()
        };
        let configured_filename = configured_project.join("Comp.vue");
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &configured_filename.to_string_lossy(),
                &configured_filename.to_string_lossy(),
                "@scope/package",
                &resolver,
            ),
            Some(configured)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &configured_filename.to_string_lossy(),
                &configured_filename.to_string_lossy(),
                "@scope/invalid",
                &resolver,
            ),
            None
        );

        let default_filename = default_project.join("Comp.vue");
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &default_filename.to_string_lossy(),
                &default_filename.to_string_lossy(),
                "@scope/secondary",
                &resolver,
            ),
            Some(secondary)
        );
    }

    #[test]
    fn reference_types_mangle_default_scoped_packages_from_typescript_5_1() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let configured_project_dir = dir.path().join("configured-project");
        let containing_dir = dir.path().join("external");
        std::fs::create_dir_all(&project_dir).expect("create project");
        std::fs::create_dir_all(&configured_project_dir).expect("create configured project");
        std::fs::create_dir_all(&containing_dir).expect("create containing directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[]}}"#,
        )
        .expect("write project config");
        let expected = write_type_package(
            &project_dir.join("node_modules").join("@types"),
            "scope__versioned",
        );
        let subpath_dir = project_dir
            .join("node_modules")
            .join("@types")
            .join("scope__versioned")
            .join("subpath");
        std::fs::create_dir_all(&subpath_dir).expect("create scoped package subpath");
        let expected_subpath = subpath_dir.join("index.d.ts");
        std::fs::write(&expected_subpath, "interface ScopedSubpath {}")
            .expect("write scoped package subpath");
        std::fs::write(
            configured_project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":["./node_modules/@types"]}}"#,
        )
        .expect("write configured project config");
        let configured_expected = write_type_package(
            &configured_project_dir.join("node_modules").join("@types"),
            "scope__configured",
        );
        let project = project_dir.join("Comp.vue");
        let configured_project = configured_project_dir.join("Comp.vue");
        let containing = containing_dir.join("ambient.d.ts");
        let baseline = Vue3TypeResolverContext::default();
        let current = Vue3TypeResolverContext {
            typescript_version: (5, 1, 0).into(),
            ..Default::default()
        };

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "@scope/versioned",
                &baseline,
            ),
            None
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "@scope/versioned",
                &current,
            ),
            Some(expected)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "@scope/versioned/subpath",
                &current,
            ),
            Some(expected_subpath)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &configured_project.to_string_lossy(),
                &containing.to_string_lossy(),
                "@scope/configured",
                &current,
            ),
            Some(configured_expected)
        );
    }

    #[test]
    fn reference_types_flat_files_only_precede_explicit_type_roots() {
        let dir = tempfile::tempdir().expect("temp dir");
        let default_project = dir.path().join("default-project");
        let explicit_project = dir.path().join("explicit-project");
        let containing_dir = dir.path().join("external");
        std::fs::create_dir_all(&default_project).expect("create default project");
        std::fs::create_dir_all(&explicit_project).expect("create explicit project");
        std::fs::create_dir_all(&containing_dir).expect("create containing directory");

        let mut expected = Vec::new();
        for (project, config) in [
            (
                &default_project,
                r#"{"compilerOptions":{"types":[]}}"#,
            ),
            (
                &explicit_project,
                r#"{"compilerOptions":{"types":[],"typeRoots":["./node_modules/@types"]}}"#,
            ),
        ] {
            std::fs::write(project.join("tsconfig.json"), config).expect("write project config");
            let type_root = project.join("node_modules").join("@types");
            std::fs::create_dir_all(&type_root).expect("create type root");
            let flat = type_root.join("priority.d.ts");
            std::fs::write(&flat, "interface FlatPriority {}")
                .expect("write flat type declaration");
            let directory = write_type_package(&type_root, "priority");
            expected.push((flat, directory));
        }

        let containing = containing_dir.join("ambient.d.ts");
        let resolver = Vue3TypeResolverContext::default();
        let default_filename = default_project.join("Comp.vue");
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &default_filename.to_string_lossy(),
                &containing.to_string_lossy(),
                "priority",
                &resolver,
            ),
            Some(expected[0].1.clone())
        );
        let explicit_filename = explicit_project.join("Comp.vue");
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &explicit_filename.to_string_lossy(),
                &containing.to_string_lossy(),
                "priority",
                &resolver,
            ),
            Some(expected[1].0.clone())
        );
    }

    #[test]
    fn reference_types_accept_modern_declaration_extensions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        std::fs::create_dir_all(&project_dir).expect("create project");
        let type_root = project_dir.join("types");
        let esm = write_type_package_with_entry(&type_root, "esm", "index.d.mts");
        let commonjs = write_type_package_with_entry(&type_root, "commonjs", "index.d.cts");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":["./types"]}}"#,
        )
        .expect("write project config");
        let filename = project_dir.join("Comp.vue");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "esm",
                &resolver,
            ),
            Some(esm)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "commonjs",
                &resolver,
            ),
            Some(commonjs)
        );
    }

    #[test]
    fn reference_types_cache_is_project_scoped() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first_project = dir.path().join("first");
        let second_project = dir.path().join("second");
        let containing = dir.path().join("shared").join("ambient.d.ts");
        std::fs::create_dir_all(&first_project).expect("create first project");
        std::fs::create_dir_all(&second_project).expect("create second project");
        let first = write_type_package(&first_project.join("types"), "cached");
        let second = write_type_package(&second_project.join("types"), "cached");
        for project in [&first_project, &second_project] {
            std::fs::write(
                project.join("tsconfig.json"),
                r#"{"compilerOptions":{"typeRoots":["./types"]}}"#,
            )
            .expect("write project config");
        }
        let first_filename = first_project.join("Comp.vue");
        let second_filename = second_project.join("Comp.vue");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &first_filename.to_string_lossy(),
                &containing.to_string_lossy(),
                "cached",
                &resolver,
            ),
            Some(first.clone())
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &second_filename.to_string_lossy(),
                &containing.to_string_lossy(),
                "cached",
                &resolver,
            ),
            Some(second)
        );
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.resolution_cache_hits, 0);
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &first_filename.to_string_lossy(),
                &containing.to_string_lossy(),
                "cached",
                &resolver,
            ),
            Some(first)
        );
        assert_eq!(
            resolver.external_type_session.stats().resolution_cache_hits,
            1
        );
    }

    #[test]
    fn reference_types_package_features_honor_options_modes_versions_and_cache() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let package_dir = project_dir
            .join("node_modules")
            .join("conditional-reference");
        std::fs::create_dir_all(&package_dir).expect("create package directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":[]}}"#,
        )
        .expect("write project config");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{
                "types": "./legacy.d.ts",
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write package manifest");
        let import_entry = package_dir.join("import.d.mts");
        let require_entry = package_dir.join("require.d.cts");
        let legacy_entry = package_dir.join("legacy.d.ts");
        std::fs::write(&import_entry, "interface ImportReference {}")
            .expect("write import declaration");
        std::fs::write(&require_entry, "interface RequireReference {}")
            .expect("write require declaration");
        std::fs::write(&legacy_entry, "interface LegacyReference {}")
            .expect("write legacy declaration");
        let filename = project_dir.join("Comp.vue");
        let filename = filename.to_string_lossy();
        let resolver = Vue3TypeResolverContext {
            typescript_version: (5, 3, 0).into(),
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            resolve_package_json_exports: Some(false),
            ..Vue3TypeResolverContext::default()
        };

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename,
                &filename,
                "conditional-reference",
                &resolver,
            ),
            Some(legacy_entry.clone()),
        );
        assert_eq!(
            resolve_vue3_type_reference_directive_with_mode(
                &filename,
                &filename,
                "conditional-reference",
                Some(Vue3TypeResolutionMode::Import),
                &resolver,
            ),
            Some(import_entry.clone()),
        );
        assert_eq!(
            resolve_vue3_type_reference_directive_with_mode(
                &filename,
                &filename,
                "conditional-reference",
                Some(Vue3TypeResolutionMode::Require),
                &resolver,
            ),
            Some(require_entry.clone()),
        );
        assert_eq!(resolver.external_type_session.stats().resolution_cache_hits, 0);

        for (mode, expected) in [
            (None, legacy_entry.clone()),
            (Some(Vue3TypeResolutionMode::Import), import_entry.clone()),
            (Some(Vue3TypeResolutionMode::Require), require_entry.clone()),
        ] {
            assert_eq!(
                resolve_vue3_type_reference_directive_with_mode(
                    &filename,
                    &filename,
                    "conditional-reference",
                    mode,
                    &resolver,
                ),
                Some(expected),
            );
        }
        assert_eq!(resolver.external_type_session.stats().resolution_cache_hits, 3);

        let node16_exports_disabled = Vue3TypeResolverContext {
            typescript_version: (5, 3, 0).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Node16,
            resolve_package_json_exports: Some(false),
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename,
                &filename,
                "conditional-reference",
                &node16_exports_disabled,
            ),
            Some(legacy_entry.clone()),
        );

        let node_next_default = Vue3TypeResolverContext {
            typescript_version: (5, 3, 0).into(),
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename,
                &filename,
                "conditional-reference",
                &node_next_default,
            ),
            Some(require_entry.clone()),
        );

        let classic_explicit = Vue3TypeResolverContext {
            typescript_version: (5, 3, 0).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Classic,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_reference_directive_with_mode(
                &filename,
                &filename,
                "conditional-reference",
                Some(Vue3TypeResolutionMode::Import),
                &classic_explicit,
            ),
            Some(import_entry),
        );

        let typescript_5_2 = Vue3TypeResolverContext {
            typescript_version: (5, 2, 2).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Classic,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_reference_directive_with_mode(
                &filename,
                &filename,
                "conditional-reference",
                Some(Vue3TypeResolutionMode::Import),
                &typescript_5_2,
            ),
            Some(legacy_entry),
        );
    }

    #[test]
    fn reference_types_truthy_exports_precede_flat_secondary_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let node_modules = project_dir.join("node_modules");
        let package_name = "flat-shadow";
        std::fs::create_dir_all(&project_dir).expect("create project directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        let package = write_conditional_type_package(
            &node_modules,
            package_name,
            r#"{
                "types": "./legacy.d.ts",
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
            &["import.d.mts", "require.d.cts", "legacy.d.ts"],
        );
        let flat = node_modules.join(format!("{package_name}.d.ts"));
        let import_entry = package.join("import.d.mts");
        let require_entry = package.join("require.d.cts");
        std::fs::write(&flat, "interface FlatExportPrecedence {}")
            .expect("write flat declaration");
        let filename = project_dir.join("Comp.vue");
        let filename = filename.to_string_lossy();

        for (module_resolution, mode, expected) in [
            (
                Vue3TypeModuleResolutionKind::NodeNext,
                None,
                &require_entry,
            ),
            (
                Vue3TypeModuleResolutionKind::NodeNext,
                Some(Vue3TypeResolutionMode::Import),
                &import_entry,
            ),
            (
                Vue3TypeModuleResolutionKind::NodeNext,
                Some(Vue3TypeResolutionMode::Require),
                &require_entry,
            ),
            (
                Vue3TypeModuleResolutionKind::Bundler,
                None,
                &import_entry,
            ),
            (
                Vue3TypeModuleResolutionKind::Classic,
                Some(Vue3TypeResolutionMode::Import),
                &import_entry,
            ),
        ] {
            let resolver = Vue3TypeResolverContext {
                typescript_version: (6, 0, 3).into(),
                module_resolution,
                ..Vue3TypeResolverContext::default()
            };
            assert_eq!(
                resolve_vue3_type_reference_directive_with_mode(
                    &filename,
                    &filename,
                    package_name,
                    mode,
                    &resolver,
                ),
                Some(expected.clone()),
                "{module_resolution:?} {mode:?}",
            );
        }

        for (version, mode, expected) in [
            ((5, 2, 2), Some(Vue3TypeResolutionMode::Require), &flat),
            ((6, 0, 3), None, &flat),
            (
                (6, 0, 3),
                Some(Vue3TypeResolutionMode::Require),
                &require_entry,
            ),
        ] {
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
                resolve_package_json_exports: Some(false),
                ..Vue3TypeResolverContext::default()
            };
            assert_eq!(
                resolve_vue3_type_reference_directive_with_mode(
                    &filename,
                    &filename,
                    package_name,
                    mode,
                    &resolver,
                ),
                Some(expected.clone()),
                "TypeScript {version:?} {mode:?}",
            );
        }

        let blocked_name = "flat-blocked";
        write_conditional_type_package(
            &node_modules,
            blocked_name,
            r#"{"exports":{}}"#,
            &[],
        );
        std::fs::write(
            node_modules.join(format!("{blocked_name}.d.ts")),
            "interface BlockedFlatDecoy {}",
        )
        .expect("write blocked flat decoy");
        let types_fallback = write_type_package(&node_modules.join("@types"), blocked_name);
        let resolver = Vue3TypeResolverContext {
            typescript_version: (6, 0, 3).into(),
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename,
                &filename,
                blocked_name,
                &resolver,
            ),
            Some(types_fallback),
        );
    }

    #[test]
    fn reference_types_node_esm_primary_type_roots_match_typescript() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let type_root = project_dir.join("types");
        let package_dir = type_root.join("primary-reference");
        std::fs::create_dir_all(&package_dir).expect("create package directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":["./types"]}}"#,
        )
        .expect("write project config");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{
                "types": "./legacy.d.ts",
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write package manifest");
        let legacy_entry = package_dir.join("legacy.d.ts");
        std::fs::write(&legacy_entry, "interface LegacyPrimaryReference {}")
            .expect("write legacy declaration");
        std::fs::write(package_dir.join("import.d.mts"), "interface ImportDecoy {}")
            .expect("write import decoy");
        std::fs::write(package_dir.join("require.d.cts"), "interface RequireDecoy {}")
            .expect("write require decoy");
        let filename = project_dir.join("Comp.vue");
        assert_node_type_reference_modes(
            &filename,
            &filename,
            "primary-reference",
            Some(legacy_entry.as_path()),
            Some(legacy_entry.as_path()),
        );

        let index_package = type_root.join("index-only");
        std::fs::create_dir_all(&index_package).expect("create index package");
        let index = index_package.join("index.d.ts");
        std::fs::write(&index, "interface PrimaryIndex {}").expect("write primary index");
        assert_node_type_reference_modes(
            &filename,
            &filename,
            "index-only",
            None,
            Some(index.as_path()),
        );
    }

    #[test]
    fn reference_types_node_esm_secondary_root_fallbacks_match_typescript() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let source_dir = project_dir.join("src");
        let node_modules = project_dir.join("node_modules");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        let project = project_dir.join("Comp.vue");
        let containing = source_dir.join("ambient.d.ts");
        std::fs::write(&containing, "export {};").expect("write containing declaration");

        let cases = [
            ("no-manifest", None, false),
            (
                "missing-exports",
                Some(r#"{"types":"./missing.d.ts"}"#),
                true,
            ),
            (
                "null-exports",
                Some(r#"{"types":"./missing.d.ts","exports":null}"#),
                true,
            ),
            (
                "false-exports",
                Some(r#"{"types":"./missing.d.ts","exports":false}"#),
                false,
            ),
            (
                "zero-exports",
                Some(r#"{"types":"./missing.d.ts","exports":0}"#),
                false,
            ),
            (
                "empty-string-exports",
                Some(r#"{"types":"./missing.d.ts","exports":""}"#),
                false,
            ),
        ];
        for (case, manifest, import_uses_index) in cases {
            let package_name = format!("vuec-reference-root-{case}");
            let package_dir = node_modules.join(&package_name);
            std::fs::create_dir_all(&package_dir).expect("create package directory");
            if let Some(manifest) = manifest {
                std::fs::write(package_dir.join("package.json"), manifest)
                    .expect("write package manifest");
            }
            let index = package_dir.join("index.d.ts");
            std::fs::write(&index, "interface RootReference {}").expect("write package index");

            assert_node_type_reference_modes(
                &project,
                &containing,
                &package_name,
                import_uses_index.then_some(index.as_path()),
                Some(index.as_path()),
            );
        }
    }

    #[test]
    fn reference_types_node_esm_secondary_subpaths_match_typescript() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let source_dir = project_dir.join("src");
        let node_modules = project_dir.join("node_modules");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        let project = project_dir.join("Comp.vue");
        let containing = source_dir.join("ambient.d.ts");
        std::fs::write(&containing, "export {};").expect("write containing declaration");

        let package_name = "vuec-reference-subpaths";
        let package_dir = node_modules.join(package_name);
        let folder = package_dir.join("folder");
        let nested = package_dir.join("nested");
        for directory in [&package_dir, &folder, &nested] {
            std::fs::create_dir_all(directory).expect("create package directory");
        }
        std::fs::write(package_dir.join("package.json"), "{}")
            .expect("write package manifest");
        let extensionless = package_dir.join("extensionless.d.ts");
        let explicit = package_dir.join("explicit.d.ts");
        let folder_index = folder.join("index.d.ts");
        for entry in [&extensionless, &explicit, &folder_index] {
            std::fs::write(entry, "interface SubpathReference {}").expect("write type entry");
        }
        std::fs::write(
            nested.join("package.json"),
            r#"{"types":"./entry.d.ts"}"#,
        )
        .expect("write nested package manifest");
        let nested_entry = nested.join("entry.d.ts");
        std::fs::write(&nested_entry, "interface NestedReference {}")
            .expect("write nested type entry");

        for (subpath, import_expected, require_expected) in [
            ("extensionless", None, Some(extensionless.as_path())),
            (
                "explicit.js",
                Some(explicit.as_path()),
                Some(explicit.as_path()),
            ),
            ("folder", None, Some(folder_index.as_path())),
            (
                "nested",
                Some(nested_entry.as_path()),
                Some(nested_entry.as_path()),
            ),
        ] {
            assert_node_type_reference_modes(
                &project,
                &containing,
                &format!("{package_name}/{subpath}"),
                import_expected,
                require_expected,
            );
        }

        for (case, exports) in [("null", "null"), ("false", "false")] {
            let package_name = format!("vuec-reference-subpath-{case}-exports");
            let package_dir = node_modules.join(&package_name);
            let nested = package_dir.join("nested");
            std::fs::create_dir_all(&nested).expect("create nested package directory");
            std::fs::write(
                package_dir.join("package.json"),
                format!(r#"{{"exports":{exports}}}"#),
            )
            .expect("write root package manifest");
            std::fs::write(
                nested.join("package.json"),
                r#"{"types":"./entry.d.ts"}"#,
            )
            .expect("write nested package manifest");
            std::fs::write(nested.join("entry.d.ts"), "interface NestedEntry {}")
                .expect("write nested package entry");
            let nested_index = nested.join("index.d.ts");
            std::fs::write(&nested_index, "interface NestedIndex {}").expect("write nested index");

            assert_node_type_reference_modes(
                &project,
                &containing,
                &format!("{package_name}/nested"),
                None,
                Some(nested_index.as_path()),
            );
        }
    }

    #[test]
    fn reference_types_node_esm_relative_names_match_typescript() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let source_dir = project_dir.join("src");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        let project = project_dir.join("Comp.vue");
        let containing = source_dir.join("ambient.d.ts");
        std::fs::write(&containing, "export {};").expect("write containing declaration");

        let extensionless = source_dir.join("extensionless.d.ts");
        let explicit = source_dir.join("explicit.d.ts");
        let folder = source_dir.join("folder");
        let nested = source_dir.join("nested");
        std::fs::create_dir_all(&folder).expect("create relative folder");
        std::fs::create_dir_all(&nested).expect("create relative nested package");
        std::fs::write(&extensionless, "interface RelativeExtensionless {}")
            .expect("write relative extensionless entry");
        std::fs::write(&explicit, "interface RelativeExplicit {}")
            .expect("write relative explicit entry");
        let folder_index = folder.join("index.d.ts");
        std::fs::write(&folder_index, "interface RelativeIndex {}")
            .expect("write relative index");
        std::fs::write(
            nested.join("package.json"),
            r#"{"types":"./entry.d.ts"}"#,
        )
        .expect("write relative package manifest");
        let nested_entry = nested.join("entry.d.ts");
        std::fs::write(&nested_entry, "interface RelativeNested {}")
            .expect("write relative package entry");

        for (type_name, import_expected, require_expected) in [
            (
                "./extensionless",
                None,
                Some(extensionless.as_path()),
            ),
            (
                "./explicit.js",
                Some(explicit.as_path()),
                Some(explicit.as_path()),
            ),
            ("./folder", None, Some(folder_index.as_path())),
            ("./nested", None, Some(nested_entry.as_path())),
        ] {
            assert_node_type_reference_modes(
                &project,
                &containing,
                type_name,
                import_expected,
                require_expected,
            );
        }
    }

    #[test]
    fn reference_types_conditional_exports_preserve_order_and_declaration_space() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let ordered = write_conditional_type_package(
            &node_modules,
            "ordered",
            r#"{
                "exports": {
                    ".": {
                        "default": "./default.d.ts",
                        "types": "./types.d.ts"
                    }
                }
            }"#,
            &["default.d.ts", "types.d.ts"],
        );
        let fallback = write_conditional_type_package(
            &node_modules,
            "fallback",
            r#"{
                "exports": {
                    ".": {
                        "types": "./missing.d.ts",
                        "import": "./import.d.mts",
                        "require": "./require.d.cts",
                        "default": "./default.d.ts"
                    }
                }
            }"#,
            &["import.d.mts", "require.d.cts", "default.d.ts"],
        );
        let declaration_only = write_conditional_type_package(
            &node_modules,
            "declaration-only",
            r#"{
                "exports": {
                    ".": {
                        "import": "./runtime.ts",
                        "default": "./fallback.d.ts"
                    }
                }
            }"#,
            &["runtime.ts", "fallback.d.ts"],
        );
        let require_only = write_conditional_type_package(
            &node_modules,
            "require-only",
            r#"{"exports":{".":{"require":"./require.d.cts"}}}"#,
            &["require.d.cts"],
        );
        let resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };

        for mode in [
            Vue3TypeResolutionMode::Import,
            Vue3TypeResolutionMode::Require,
        ] {
            assert_eq!(
                resolve_vue3_package_json_type_reference_entry(
                    &ordered,
                    None,
                    Some(mode),
                    &resolver,
                ),
                Vue3PackageJsonTypeResolution::Resolved(ordered.join("default.d.ts")),
            );
        }
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &fallback,
                None,
                Some(Vue3TypeResolutionMode::Import),
                &resolver,
            ),
            Vue3PackageJsonTypeResolution::Resolved(fallback.join("import.d.mts")),
        );
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &fallback,
                None,
                Some(Vue3TypeResolutionMode::Require),
                &resolver,
            ),
            Vue3PackageJsonTypeResolution::Resolved(fallback.join("require.d.cts")),
        );
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &declaration_only,
                None,
                Some(Vue3TypeResolutionMode::Import),
                &resolver,
            ),
            Vue3PackageJsonTypeResolution::Resolved(declaration_only.join("fallback.d.ts")),
        );
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &require_only,
                None,
                Some(Vue3TypeResolutionMode::Import),
                &resolver,
            ),
            Vue3PackageJsonTypeResolution::Blocked,
        );
    }

    #[test]
    fn reference_types_package_targets_do_not_add_declaration_suffixes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("package");
        std::fs::create_dir_all(&package).expect("create package directory");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "exports": {
                    "./extensionless": {
                        "types": ["./extensionless", "./fallback.d.ts"]
                    },
                    "./arbitrary": {
                        "types": ["./arbitrary.custom", "./fallback.d.ts"]
                    },
                    "./arbitrary-valid": { "types": "./valid.custom" },
                    "./javascript": { "types": "./javascript.js" },
                    "./explicit": { "types": "./explicit.d.ts" }
                }
            }"#,
        )
        .expect("write package manifest");
        for file in [
            "extensionless.d.ts",
            "arbitrary.custom.d.ts",
            "fallback.d.ts",
            "valid.d.custom.ts",
            "javascript.d.ts",
            "explicit.d.ts",
        ] {
            std::fs::write(package.join(file), "interface PackageTargetType {}")
                .expect("write declaration target");
        }
        let resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };

        for (subpath, expected) in [
            ("extensionless", "fallback.d.ts"),
            ("arbitrary", "fallback.d.ts"),
            ("arbitrary-valid", "valid.d.custom.ts"),
            ("javascript", "javascript.d.ts"),
            ("explicit", "explicit.d.ts"),
        ] {
            assert_eq!(
                resolve_vue3_package_json_type_reference_entry(
                    &package,
                    Some(subpath),
                    Some(Vue3TypeResolutionMode::Import),
                    &resolver,
                ),
                Vue3PackageJsonTypeResolution::Resolved(package.join(expected)),
                "subpath {subpath}",
            );
        }
    }

    #[test]
    fn reference_types_conditional_export_fanout_is_bounded() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = write_conditional_type_package(
            dir.path(),
            "bounded",
            r#"{
                "exports": {
                    ".": {
                        "types": "./missing.d.ts",
                        "import": "./hit.d.mts"
                    }
                }
            }"#,
            &["hit.d.mts"],
        );
        let exact = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..resolver_with_limits(Vue3ExternalTypeLoadLimits {
                max_metadata_fanout_entries: 2,
                ..Vue3ExternalTypeLoadLimits::default()
            })
        };
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &package,
                None,
                Some(Vue3TypeResolutionMode::Import),
                &exact,
            ),
            Vue3PackageJsonTypeResolution::Resolved(package.join("hit.d.mts")),
        );
        assert_eq!(exact.external_type_session.stats().metadata_fanout_entries, 2);

        let short = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..resolver_with_limits(Vue3ExternalTypeLoadLimits {
                max_metadata_fanout_entries: 1,
                ..Vue3ExternalTypeLoadLimits::default()
            })
        };
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &package,
                None,
                Some(Vue3TypeResolutionMode::Import),
                &short,
            ),
            Vue3PackageJsonTypeResolution::Blocked,
        );
        assert_eq!(short.external_type_session.stats().metadata_fanout_entries, 1);
        assert!(short.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn reference_types_metadata_exhaustion_prevents_secondary_fallback() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let containing_dir = project_dir.join("dependencies");
        std::fs::create_dir_all(&containing_dir).expect("create containing dir");
        let _secondary = write_type_package(&containing_dir.join("node_modules"), "blocked");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"extends":"./base.json"}"#,
        )
        .expect("write project config");
        std::fs::write(project_dir.join("base.json"), "{}").expect("write base config");
        let project = project_dir.join("Comp.vue");
        let containing = containing_dir.join("ambient.d.ts");
        let resolver = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert!(resolve_vue3_type_reference_directive(
            &project.to_string_lossy(),
            &containing.to_string_lossy(),
            "blocked",
            &resolver,
        )
        .is_none());
        assert!(resolver.external_type_session.metadata_is_blocked());
        assert_eq!(
            resolver
                .external_type_session
                .stats()
                .metadata_fanout_entries,
            0
        );
    }
}

#[cfg(test)]
mod vue3_module_suffix_config_tests {
    use super::*;

    fn resolver_with_limits(limits: Vue3ExternalTypeLoadLimits) -> Vue3TypeResolverContext {
        Vue3TypeResolverContext {
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(limits),
            ..Vue3TypeResolverContext::default()
        }
    }

    fn write_config(root: &Path, source: &str) -> String {
        std::fs::write(root.join("tsconfig.json"), source).expect("write suffix config");
        root.join("Comp.vue").to_string_lossy().to_string()
    }

    #[test]
    fn module_suffix_config_fanout_is_exact_and_fail_closed() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("base.json"),
            r#"{"compilerOptions":{"moduleSuffixes":[".native",""]}}"#,
        )
        .expect("write inherited suffix config");
        let filename = write_config(dir.path(), r#"{"extends":"./base.json"}"#);
        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 3,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            vue3_tsconfig_module_suffixes(&filename, &exact).as_deref(),
            Some([".native".to_string(), String::new()].as_slice())
        );
        assert_eq!(
            exact
                .external_type_session
                .stats()
                .metadata_fanout_entries,
            3
        );
        assert!(!exact.external_type_session.metadata_is_blocked());

        let short = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 2,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(vue3_tsconfig_module_suffixes(&filename, &short).is_none());
        assert_eq!(
            short
                .external_type_session
                .stats()
                .metadata_fanout_entries,
            2
        );
        assert!(short.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn module_suffix_config_empty_lists_clear_inheritance() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("base.json"),
            r#"{"compilerOptions":{"moduleSuffixes":[".base",""]}}"#,
        )
        .expect("write inherited suffix config");
        let filename = write_config(
            dir.path(),
            r#"{
                "extends":"./base.json",
                "compilerOptions":{"moduleSuffixes":[]}
            }"#,
        );
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            vue3_tsconfig_module_suffixes(&filename, &resolver).as_deref(),
            Some([String::new()].as_slice())
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn invalid_module_suffix_configs_do_not_keep_partial_values() {
        for source in [
            r#"{"compilerOptions":{"moduleSuffixes":".native"}}"#,
            r#"{"compilerOptions":{"moduleSuffixes":[".native",1]}}"#,
        ] {
            let dir = tempfile::tempdir().expect("temp dir");
            let filename = write_config(dir.path(), source);
            let resolver = Vue3TypeResolverContext::default();

            assert!(vue3_tsconfig_module_suffixes(&filename, &resolver).is_none());
            assert!(resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn module_suffix_config_and_generated_paths_are_bounded() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = write_config(
            dir.path(),
            r#"{"compilerOptions":{"moduleSuffixes":[".native"]}}"#,
        );
        let resolver = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: ".native".len() - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert!(vue3_tsconfig_module_suffixes(&filename, &resolver).is_none());
        assert!(resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn module_resolution_defaults_follow_typescript_version_and_target() {
        for (version, source, expected_resolution, expected_module) in [
            (
                (5, 9, 0),
                r#"{}"#,
                Vue3TypeModuleResolutionKind::Node10,
                Vue3TypeModuleKind::CommonJs,
            ),
            (
                (5, 9, 0),
                r#"{"compilerOptions":{"target":"ESNext"}}"#,
                Vue3TypeModuleResolutionKind::Classic,
                Vue3TypeModuleKind::EcmaScript,
            ),
            (
                (6, 0, 0),
                r#"{}"#,
                Vue3TypeModuleResolutionKind::Bundler,
                Vue3TypeModuleKind::EcmaScript,
            ),
            (
                (6, 0, 0),
                r#"{"compilerOptions":{"module":"CommonJS"}}"#,
                Vue3TypeModuleResolutionKind::Bundler,
                Vue3TypeModuleKind::CommonJs,
            ),
            (
                (6, 0, 0),
                r#"{"compilerOptions":{"target":"ES5"}}"#,
                Vue3TypeModuleResolutionKind::Bundler,
                Vue3TypeModuleKind::CommonJs,
            ),
        ] {
            let dir = tempfile::tempdir().expect("temp dir");
            let filename = write_config(dir.path(), source);
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                ..Vue3TypeResolverContext::default()
            };

            let options = vue3_tsconfig_type_resolver_options(&filename, &resolver)
                .expect("resolve compiler options");
            assert_eq!(
                options.module_resolution, expected_resolution,
                "TypeScript {version:?}: {source}"
            );
            assert_eq!(
                options.module, expected_module,
                "TypeScript {version:?}: {source}"
            );
        }
    }

    #[test]
    fn module_resolution_options_inherit_independently_and_cache_diamonds() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("base.json"),
            r#"{
                "compilerOptions": {
                    "module": "Node16",
                    "moduleResolution": "Node16"
                }
            }"#,
        )
        .expect("write base resolver config");
        let inherited = dir.path().join("inherited");
        let module_override = dir.path().join("module-override");
        let resolution_override = dir.path().join("resolution-override");
        for project in [&inherited, &module_override, &resolution_override] {
            std::fs::create_dir_all(project).expect("create inherited project");
        }
        std::fs::write(
            inherited.join("tsconfig.json"),
            r#"{"extends":"../base.json"}"#,
        )
        .expect("write inherited project config");
        std::fs::write(
            module_override.join("tsconfig.json"),
            r#"{
                "extends":"../base.json",
                "compilerOptions":{"module":"NodeNext"}
            }"#,
        )
        .expect("write module override config");
        std::fs::write(
            resolution_override.join("tsconfig.json"),
            r#"{
                "extends":"../base.json",
                "compilerOptions":{"moduleResolution":"NodeNext"}
            }"#,
        )
        .expect("write resolution override config");

        for (project, expected_resolution, expected_module) in [
            (
                &inherited,
                Vue3TypeModuleResolutionKind::Node16,
                Vue3TypeModuleKind::Node16,
            ),
            (
                &module_override,
                Vue3TypeModuleResolutionKind::Node16,
                Vue3TypeModuleKind::NodeNext,
            ),
            (
                &resolution_override,
                Vue3TypeModuleResolutionKind::NodeNext,
                Vue3TypeModuleKind::Node16,
            ),
        ] {
            let resolver = vue3_type_resolver_context_for_filename(
                &project.join("Comp.vue").to_string_lossy(),
            );
            assert_eq!(resolver.module_resolution, expected_resolution);
            assert_eq!(resolver.effective_module(), expected_module);
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }

        let diamond = dir.path().join("diamond");
        std::fs::create_dir_all(&diamond).expect("create diamond project");
        std::fs::write(
            diamond.join("shared.json"),
            r#"{"compilerOptions":{"module":"CommonJS"}}"#,
        )
        .expect("write shared config");
        std::fs::write(
            diamond.join("left.json"),
            r#"{
                "extends":"./shared.json",
                "compilerOptions":{"module":"NodeNext"}
            }"#,
        )
        .expect("write left config");
        std::fs::write(
            diamond.join("right.json"),
            r#"{"extends":"./shared.json"}"#,
        )
        .expect("write right config");
        std::fs::write(
            diamond.join("tsconfig.json"),
            r#"{"extends":["./left.json","./right.json"]}"#,
        )
        .expect("write diamond root config");
        let resolver = vue3_type_resolver_context_for_filename(
            &diamond.join("Comp.vue").to_string_lossy(),
        );

        assert_eq!(
            resolver.module_resolution,
            Vue3TypeModuleResolutionKind::Node10
        );
        assert_eq!(resolver.effective_module(), Vue3TypeModuleKind::CommonJs);
        assert_eq!(resolver.external_type_session.stats().tsconfig_nodes, 4);
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn allow_js_and_check_js_inherit_independently_with_explicit_precedence() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("base.json"),
            r#"{"compilerOptions":{"allowJs":true,"checkJs":false}}"#,
        )
        .expect("write base JavaScript config");
        let cases = [
            (
                "inherited",
                r#"{"extends":"../base.json"}"#,
                true,
            ),
            (
                "check-override",
                r#"{
                    "extends":"../base.json",
                    "compilerOptions":{"checkJs":true}
                }"#,
                true,
            ),
            (
                "allow-override",
                r#"{
                    "extends":"../base.json",
                    "compilerOptions":{"allowJs":false,"checkJs":true}
                }"#,
                false,
            ),
            (
                "check-only",
                r#"{"compilerOptions":{"checkJs":true}}"#,
                true,
            ),
        ];

        for (name, source, expected) in cases {
            let project = dir.path().join(name);
            std::fs::create_dir_all(&project).expect("create JavaScript option project");
            std::fs::write(project.join("tsconfig.json"), source)
                .expect("write JavaScript option config");
            let filename = project.join("Comp.vue").to_string_lossy().to_string();
            let resolver = vue3_type_resolver_context_for_filename(&filename);

            assert_eq!(resolver.allow_js, expected, "{name}");
            assert!(!resolver.external_type_session.metadata_is_blocked(), "{name}");
        }
    }

    #[test]
    fn invalid_allow_js_and_check_js_values_fail_closed() {
        for source in [
            r#"{"compilerOptions":{"allowJs":"true"}}"#,
            r#"{"compilerOptions":{"checkJs":1}}"#,
        ] {
            let dir = tempfile::tempdir().expect("temp dir");
            let filename = write_config(dir.path(), source);
            let resolver = Vue3TypeResolverContext::default();

            assert!(vue3_tsconfig_type_resolver_options(&filename, &resolver).is_none());
            assert!(resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn project_config_search_prefers_nearest_files_and_stops_at_node_modules() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("tsconfig.json"), "{}")
            .expect("write root TypeScript config");

        let nested = dir.path().join("nested");
        std::fs::create_dir_all(&nested).expect("create nested project");
        let nested_jsconfig = nested.join("jsconfig.json");
        std::fs::write(&nested_jsconfig, "{}").expect("write nested JavaScript config");
        let nested_resolver = Vue3TypeResolverContext::default();
        assert_eq!(
            vue3_tsconfig_search_paths(
                &nested.join("Comp.vue").to_string_lossy(),
                &nested_resolver,
            )
            .next(),
            Some(nested_jsconfig),
        );
        assert_eq!(
            nested_resolver
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            2,
        );

        let same_dir = dir.path().join("same-dir");
        std::fs::create_dir_all(&same_dir).expect("create same-directory project");
        let same_tsconfig = same_dir.join("tsconfig.json");
        std::fs::write(&same_tsconfig, "{}").expect("write same-directory TypeScript config");
        std::fs::write(same_dir.join("jsconfig.json"), "{}")
            .expect("write same-directory JavaScript config");
        let same_dir_resolver = Vue3TypeResolverContext::default();
        assert_eq!(
            vue3_tsconfig_search_paths(
                &same_dir.join("Comp.vue").to_string_lossy(),
                &same_dir_resolver,
            )
            .next(),
            Some(same_tsconfig),
        );
        assert_eq!(
            same_dir_resolver
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            1,
        );

        let dependency = dir.path().join("node_modules").join("package").join("src");
        std::fs::create_dir_all(&dependency).expect("create dependency source directory");
        let dependency_resolver = Vue3TypeResolverContext::default();
        assert!(vue3_tsconfig_search_paths(
            &dependency.join("Comp.vue").to_string_lossy(),
            &dependency_resolver,
        )
        .next()
        .is_none());
        assert_eq!(
            dependency_resolver
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            6,
        );
        assert!(!dependency_resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn jsconfig_defaults_allow_js_per_config_and_obeys_probe_budgets() {
        let dir = tempfile::tempdir().expect("temp dir");
        let default_project = dir.path().join("default");
        std::fs::create_dir_all(&default_project).expect("create default JavaScript project");
        std::fs::write(default_project.join("jsconfig.json"), "{}")
            .expect("write default JavaScript config");
        let filename = default_project
            .join("Comp.vue")
            .to_string_lossy()
            .to_string();
        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_resolution_path_probes: 2,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(
            vue3_tsconfig_type_resolver_options(&filename, &exact)
                .expect("resolve JavaScript config at exact probe limit")
                .allow_js
        );
        assert_eq!(
            exact
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            2,
        );
        assert!(!exact.external_type_session.metadata_is_blocked());

        let short = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_resolution_path_probes: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(vue3_tsconfig_type_resolver_options(&filename, &short).is_none());
        assert_eq!(
            short
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            1,
        );
        assert!(short.external_type_session.metadata_is_blocked());

        let inherited_project = dir.path().join("inherited");
        std::fs::create_dir_all(&inherited_project)
            .expect("create inherited JavaScript project");
        std::fs::write(
            inherited_project.join("base.json"),
            r#"{"compilerOptions":{"allowJs":false}}"#,
        )
        .expect("write inherited JavaScript base config");
        std::fs::write(
            inherited_project.join("jsconfig.json"),
            r#"{"extends":"./base.json"}"#,
        )
        .expect("write inherited JavaScript config");
        let inherited = vue3_tsconfig_type_resolver_options(
            &inherited_project.join("Comp.vue").to_string_lossy(),
            &Vue3TypeResolverContext::default(),
        )
        .expect("resolve inherited JavaScript config");
        assert!(inherited.allow_js);

        let explicit_project = dir.path().join("explicit");
        std::fs::create_dir_all(&explicit_project).expect("create explicit JavaScript project");
        std::fs::write(
            explicit_project.join("jsconfig.json"),
            r#"{"compilerOptions":{"allowJs":false}}"#,
        )
        .expect("write explicit JavaScript config");
        let explicit = vue3_tsconfig_type_resolver_options(
            &explicit_project.join("Comp.vue").to_string_lossy(),
            &Vue3TypeResolverContext::default(),
        )
        .expect("resolve explicit JavaScript config");
        assert!(!explicit.allow_js);

        let null_project = dir.path().join("null");
        std::fs::create_dir_all(&null_project).expect("create null JavaScript project");
        std::fs::write(
            null_project.join("jsconfig.json"),
            r#"{"compilerOptions":{"allowJs":null}}"#,
        )
        .expect("write null JavaScript config");
        let null = vue3_tsconfig_type_resolver_options(
            &null_project.join("Comp.vue").to_string_lossy(),
            &Vue3TypeResolverContext::default(),
        )
        .expect("resolve null JavaScript config");
        assert!(!null.allow_js);

        let extended_project = dir.path().join("extended");
        std::fs::create_dir_all(&extended_project).expect("create extended project");
        std::fs::write(extended_project.join("jsconfig.json"), "{}")
            .expect("write extended JavaScript config");
        let child = extended_project.join("child");
        std::fs::create_dir_all(&child).expect("create TypeScript child project");
        std::fs::write(
            child.join("tsconfig.json"),
            r#"{"extends":"../jsconfig.json"}"#,
        )
        .expect("write TypeScript child config");
        let extended = vue3_tsconfig_type_resolver_options(
            &child.join("Comp.vue").to_string_lossy(),
            &Vue3TypeResolverContext::default(),
        )
        .expect("resolve JavaScript config defaults through extends");
        assert!(extended.allow_js);
    }

    #[test]
    fn jsconfig_drives_path_resolution_and_resolver_options() {
        let dir = tempfile::tempdir().expect("temp dir");
        let target = dir.path().join("src").join("aliased.d.ts");
        std::fs::create_dir_all(target.parent().expect("target parent"))
            .expect("create JavaScript project source directory");
        std::fs::write(&target, "export interface AliasedProps { value: string }")
            .expect("write aliased declaration");
        std::fs::write(
            dir.path().join("jsconfig.json"),
            r#"{
                "compilerOptions":{
                    "baseUrl":".",
                    "paths":{"project-alias":["./src/aliased"]}
                }
            }"#,
        )
        .expect("write JavaScript project config");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = vue3_type_resolver_context_for_filename(&filename);

        assert!(resolver.allow_js);
        assert_eq!(
            resolve_vue3_type_import(&filename, "project-alias", &resolver),
            Some(target),
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn nullable_resolver_options_clear_inherited_values() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("base.json"),
            r#"{
                "compilerOptions":{
                    "module":"Node16",
                    "moduleResolution":"Node16",
                    "moduleSuffixes":[".base",""],
                    "allowJs":true,
                    "checkJs":true,
                    "customConditions":["browser"],
                    "resolvePackageJsonExports":true,
                    "resolvePackageJsonImports":true,
                    "target":"ES5"
                }
            }"#,
        )
        .expect("write nullable option base config");
        std::fs::write(
            dir.path().join("clear.json"),
            r#"{
                "compilerOptions":{
                    "module":null,
                    "moduleResolution":null,
                    "moduleSuffixes":null,
                    "allowJs":null,
                    "checkJs":null,
                    "customConditions":null,
                    "resolvePackageJsonExports":null,
                    "resolvePackageJsonImports":null,
                    "target":null
                }
            }"#,
        )
        .expect("write nullable option clearing config");
        let direct = dir.path().join("direct");
        let multiple = dir.path().join("multiple");
        std::fs::create_dir_all(&direct).expect("create direct nullable project");
        std::fs::create_dir_all(&multiple).expect("create multiple nullable project");
        std::fs::write(
            direct.join("tsconfig.json"),
            r#"{
                "extends":"../base.json",
                "compilerOptions":{
                    "module":null,
                    "moduleResolution":null,
                    "moduleSuffixes":null,
                    "allowJs":null,
                    "checkJs":null,
                    "customConditions":null,
                    "resolvePackageJsonExports":null,
                    "resolvePackageJsonImports":null,
                    "target":null
                }
            }"#,
        )
        .expect("write direct nullable project config");
        std::fs::write(
            multiple.join("tsconfig.json"),
            r#"{"extends":["../base.json","../clear.json"]}"#,
        )
        .expect("write multiple nullable project config");

        for project in [&direct, &multiple] {
            let resolver = Vue3TypeResolverContext {
                typescript_version: (6, 0, 3).into(),
                ..Vue3TypeResolverContext::default()
            };
            let options = vue3_tsconfig_type_resolver_options(
                &project.join("Comp.vue").to_string_lossy(),
                &resolver,
            )
            .expect("resolve nullable compiler options");

            assert_eq!(options.module, Vue3TypeModuleKind::EcmaScript);
            assert_eq!(
                options.module_resolution,
                Vue3TypeModuleResolutionKind::Bundler
            );
            assert_eq!(options.module_suffixes.as_ref(), [String::new()].as_slice());
            assert!(!options.allow_js);
            assert!(options.custom_conditions.iter().next().is_none());
            assert_eq!(options.resolve_package_json_exports, None);
            assert_eq!(options.resolve_package_json_imports, None);
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn nullable_lists_consume_no_fanout_and_clear_invalid_mode_features() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = write_config(
            dir.path(),
            r#"{
                "compilerOptions":{
                    "module":"CommonJS",
                    "moduleResolution":"Node10",
                    "moduleSuffixes":null,
                    "customConditions":null,
                    "resolvePackageJsonExports":null,
                    "resolvePackageJsonImports":null
                }
            }"#,
        );
        let resolver = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let options = vue3_tsconfig_type_resolver_options(&filename, &resolver)
            .expect("resolve null package features in Node10 mode");

        assert_eq!(
            options.module_resolution,
            Vue3TypeModuleResolutionKind::Node10
        );
        assert_eq!(options.module, Vue3TypeModuleKind::CommonJs);
        assert_eq!(options.module_suffixes.as_ref(), [String::new()].as_slice());
        assert!(options.custom_conditions.iter().next().is_none());
        assert_eq!(options.resolve_package_json_exports, None);
        assert_eq!(options.resolve_package_json_imports, None);
        assert_eq!(
            resolver
                .external_type_session
                .stats()
                .metadata_fanout_entries,
            0,
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn custom_conditions_inherit_replace_empty_and_null_values() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("base.json"),
            r#"{
                "compilerOptions":{
                    "module":"ESNext",
                    "customConditions":["browser","development"]
                }
            }"#,
        )
        .expect("write inherited custom conditions");
        let cases = [
            (
                "inherited",
                r#"{"extends":"../base.json","compilerOptions":{"moduleResolution":"Bundler"}}"#,
                vec!["browser".to_string(), "development".to_string()],
            ),
            (
                "replaced",
                r#"{
                    "extends":"../base.json",
                    "compilerOptions":{
                        "moduleResolution":"Bundler",
                        "customConditions":["worker"]
                    }
                }"#,
                vec!["worker".to_string()],
            ),
            (
                "empty-array",
                r#"{
                    "extends":"../base.json",
                    "compilerOptions":{
                        "moduleResolution":"Bundler",
                        "customConditions":[]
                    }
                }"#,
                Vec::new(),
            ),
            (
                "cleared-with-null",
                r#"{
                    "extends":"../base.json",
                    "compilerOptions":{
                        "moduleResolution":"Node10",
                        "customConditions":null
                    }
                }"#,
                Vec::new(),
            ),
        ];

        for (name, source, expected) in cases {
            let project = dir.path().join(name);
            std::fs::create_dir_all(&project).expect("create custom condition project");
            std::fs::write(project.join("tsconfig.json"), source)
                .expect("write custom condition config");
            let resolver = vue3_type_resolver_context_for_filename(
                &project.join("Comp.vue").to_string_lossy(),
            );

            assert_eq!(
                resolver.custom_conditions.iter().cloned().collect::<Vec<_>>(),
                expected,
                "{name}"
            );
            assert!(!resolver.external_type_session.metadata_is_blocked(), "{name}");
        }
    }

    #[test]
    fn custom_condition_config_is_versioned_validated_and_bounded() {
        let invalid_sources = [
            r#"{"compilerOptions":{"module":"ESNext","moduleResolution":"Bundler","customConditions":"browser"}}"#,
            r#"{"compilerOptions":{"module":"ESNext","moduleResolution":"Bundler","customConditions":["browser",1]}}"#,
        ];
        for source in invalid_sources {
            let dir = tempfile::tempdir().expect("temp dir");
            let filename = write_config(dir.path(), source);
            let resolver = Vue3TypeResolverContext::default();

            assert!(vue3_tsconfig_type_resolver_options(&filename, &resolver).is_none());
            assert!(resolver.external_type_session.metadata_is_blocked());
        }

        let dir = tempfile::tempdir().expect("temp dir");
        let filename = write_config(
            dir.path(),
            r#"{"compilerOptions":{"module":"ESNext","moduleResolution":"Bundler","customConditions":["worker",null,""," ","browser","worker"]}}"#,
        );
        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 6,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            vue3_tsconfig_type_resolver_options(&filename, &exact)
                .expect("resolve bounded custom conditions")
                .custom_conditions
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            [
                " ".to_string(),
                "browser".to_string(),
                "worker".to_string(),
            ],
        );
        assert_eq!(exact.external_type_session.stats().metadata_fanout_entries, 6);
        assert!(!exact.external_type_session.metadata_is_blocked());

        let short = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 5,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(vue3_tsconfig_type_resolver_options(&filename, &short).is_none());
        assert!(short.external_type_session.metadata_is_blocked());

        let version_cases = [
            (
                (4, 9, 5),
                r#"{"compilerOptions":{"module":"Node16","moduleResolution":"Node16","customConditions":[]}}"#,
                true,
            ),
            (
                (5, 0, 4),
                r#"{"compilerOptions":{"module":"CommonJS","moduleResolution":"Node10","customConditions":[]}}"#,
                true,
            ),
            (
                (5, 0, 4),
                r#"{"compilerOptions":{"module":"CommonJS","moduleResolution":"Node10","customConditions":null}}"#,
                false,
            ),
        ];
        for (version, source, expected_blocked) in version_cases {
            let dir = tempfile::tempdir().expect("temp dir");
            let filename = write_config(dir.path(), source);
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                ..Vue3TypeResolverContext::default()
            };
            let options = vue3_tsconfig_type_resolver_options(&filename, &resolver);

            assert_eq!(options.is_none(), expected_blocked, "TypeScript {version:?}");
            assert_eq!(
                resolver.external_type_session.metadata_is_blocked(),
                expected_blocked,
                "TypeScript {version:?}",
            );
            if let Some(options) = options {
                assert!(
                    options.custom_conditions.iter().next().is_none(),
                    "TypeScript {version:?}"
                );
            }
        }
    }

    #[test]
    fn custom_condition_normalization_work_is_bounded() {
        assert_eq!(
            VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_NORMALIZATION_STEPS,
            16 * 1024 * 1024
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = write_config(
            dir.path(),
            r#"{
                "compilerOptions":{
                    "module":"ESNext",
                    "moduleResolution":"Bundler",
                    "customConditions":["alpha","beta","alpha"]
                }
            }"#,
        );
        let normalization_steps = "alpha".len() * 3 * 3;

        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_normalization_steps: normalization_steps,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            vue3_tsconfig_type_resolver_options(&filename, &exact)
                .expect("normalize exact custom conditions")
                .custom_conditions
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            ["alpha".to_string(), "beta".to_string()]
        );
        assert_eq!(
            exact
                .external_type_session
                .stats()
                .tsconfig_normalization_steps,
            normalization_steps
        );
        assert!(!exact.external_type_session.metadata_is_blocked());

        let short = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_normalization_steps: normalization_steps - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(vue3_tsconfig_type_resolver_options(&filename, &short).is_none());
        assert_eq!(
            short
                .external_type_session
                .stats()
                .tsconfig_normalization_steps,
            normalization_steps - 1
        );
        assert!(short.external_type_session.metadata_is_blocked());

        for conditions in ["[]", r#"["single"]"#] {
            let dir = tempfile::tempdir().expect("temp dir");
            let filename = write_config(
                dir.path(),
                &format!(
                    r#"{{"compilerOptions":{{"module":"ESNext","moduleResolution":"Bundler","customConditions":{conditions}}}}}"#
                ),
            );
            let resolver = resolver_with_limits(Vue3ExternalTypeLoadLimits {
                max_tsconfig_normalization_steps: 0,
                ..Vue3ExternalTypeLoadLimits::default()
            });
            assert!(vue3_tsconfig_type_resolver_options(&filename, &resolver).is_some());
            assert_eq!(
                resolver
                    .external_type_session
                    .stats()
                    .tsconfig_normalization_steps,
                0
            );
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn custom_conditions_select_package_branches_and_isolate_caches() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir
            .path()
            .join("node_modules")
            .join("custom-condition-package");
        std::fs::create_dir_all(&package).expect("create conditional package");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "name":"custom-condition-package",
                "exports":{
                    ".":{
                        "browser":"./browser.d.ts",
                        "default":"./default.d.ts"
                    }
                }
            }"#,
        )
        .expect("write conditional package manifest");
        let browser = package.join("browser.d.ts");
        let fallback = package.join("default.d.ts");
        std::fs::write(&browser, "export interface Props { browser: string }")
            .expect("write browser declaration");
        std::fs::write(&fallback, "export interface Props { fallback: string }")
            .expect("write fallback declaration");
        let import_browser = dir.path().join("import-browser.d.ts");
        let import_fallback = dir.path().join("import-default.d.ts");
        std::fs::write(&import_browser, "export interface Imported { browser: string }")
            .expect("write browser package import declaration");
        std::fs::write(
            &import_fallback,
            "export interface Imported { fallback: string }",
        )
        .expect("write fallback package import declaration");
        std::fs::write(
            dir.path().join("package.json"),
            r##"{
                "name":"custom-condition-project",
                "imports":{
                    "#custom":{
                        "browser":"./import-browser.d.ts",
                        "default":"./import-default.d.ts"
                    }
                }
            }"##,
        )
        .expect("write conditional package imports manifest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions":{
                    "module":"ESNext",
                    "moduleResolution":"Bundler",
                    "customConditions":["browser"]
                }
            }"#,
        )
        .expect("write project config");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let configured = vue3_type_resolver_context_for_filename(&filename);
        let mut without_custom = configured.clone();
        without_custom.custom_conditions = Vue3CustomConditionSet::default();

        for _ in 0..2 {
            assert_eq!(
                resolve_vue3_type_import(
                    &filename,
                    "custom-condition-package",
                    &configured,
                ),
                Some(browser.clone()),
            );
            assert_eq!(
                resolve_vue3_type_import(
                    &filename,
                    "custom-condition-package",
                    &without_custom,
                ),
                Some(fallback.clone()),
            );
            assert_eq!(
                resolve_vue3_type_import(&filename, "#custom", &configured),
                Some(import_browser.clone()),
            );
            assert_eq!(
                resolve_vue3_type_import(&filename, "#custom", &without_custom),
                Some(import_fallback.clone()),
            );
        }
        assert_ne!(configured, without_custom);
        assert_eq!(
            configured
                .external_type_session
                .stats()
                .resolution_cache_hits,
            4,
        );

        let root = dir.path().join("root.ts");
        std::fs::write(
            &root,
            "export { Props } from 'custom-condition-package'",
        )
        .expect("write condition-sensitive root");
        let configured_context = vue3_external_type_context_from_path(
            &root,
            &mut BTreeSet::new(),
            &configured,
        )
        .expect("load configured custom condition context");
        let fallback_context = vue3_external_type_context_from_path(
            &root,
            &mut BTreeSet::new(),
            &without_custom,
        )
        .expect("load fallback condition context");
        assert_eq!(
            configured_context.type_sources.get("Props"),
            Some(&normalize_path_string(&browser)),
        );
        assert_eq!(
            fallback_context.type_sources.get("Props"),
            Some(&normalize_path_string(&fallback)),
        );
        assert!(!std::sync::Arc::ptr_eq(
            &configured_context,
            &fallback_context,
        ));
    }

    #[test]
    fn custom_condition_membership_overrides_builtin_condition_gates() {
        let resolver = Vue3TypeResolverContext {
            typescript_version: (6, 0, 3).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            custom_conditions: Vue3CustomConditionSet::from_strings(vec![
                "node".to_string(),
                "require".to_string(),
                "types@>=999".to_string(),
            ]),
            ..Vue3TypeResolverContext::default()
        };

        for condition in ["node", "require", "types@>=999"] {
            assert!(vue3_package_export_condition_is_active(
                condition,
                Vue3TypeResolutionMode::Import,
                &resolver,
            ));
        }
    }

    #[test]
    fn package_json_resolution_options_inherit_independently_and_validate_versions() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("base.json"),
            r#"{
                "compilerOptions": {
                    "module":"ESNext",
                    "moduleResolution":"Bundler",
                    "resolvePackageJsonExports":false,
                    "resolvePackageJsonImports":true
                }
            }"#,
        )
        .expect("write base package map config");
        let inherited = dir.path().join("inherited");
        let overridden = dir.path().join("overridden");
        for project in [&inherited, &overridden] {
            std::fs::create_dir_all(project).expect("create package map project");
        }
        std::fs::write(
            inherited.join("tsconfig.json"),
            r#"{"extends":"../base.json"}"#,
        )
        .expect("write inherited package map config");
        std::fs::write(
            overridden.join("tsconfig.json"),
            r#"{
                "extends":"../base.json",
                "compilerOptions":{"resolvePackageJsonImports":false}
            }"#,
        )
        .expect("write overridden package map config");

        for (project, expected_imports) in [(&inherited, Some(true)), (&overridden, Some(false))] {
            let resolver = vue3_type_resolver_context_for_filename(
                &project.join("Comp.vue").to_string_lossy(),
            );
            assert_eq!(
                resolver.module_resolution,
                Vue3TypeModuleResolutionKind::Bundler
            );
            assert_eq!(resolver.effective_module(), Vue3TypeModuleKind::EcmaScript);
            assert_eq!(resolver.resolve_package_json_exports, Some(false));
            assert_eq!(resolver.resolve_package_json_imports, expected_imports);
            assert!(!resolver.package_json_features().exports);
            assert_eq!(
                resolver.package_json_features().imports,
                expected_imports == Some(true)
            );
            assert!(resolver.package_json_features().self_name);
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }

        let unsupported = tempfile::tempdir().expect("unsupported option dir");
        let filename = write_config(
            unsupported.path(),
            r#"{
                "compilerOptions": {
                    "moduleResolution":"Node",
                    "resolvePackageJsonExports":"invalid"
                }
            }"#,
        );
        let typescript_4_9 = Vue3TypeResolverContext {
            typescript_version: (4, 9, 0).into(),
            ..Vue3TypeResolverContext::default()
        };
        let options = vue3_tsconfig_type_resolver_options(&filename, &typescript_4_9)
            .expect("ignore unsupported package map option");
        assert_eq!(options.resolve_package_json_exports, None);
        assert!(!typescript_4_9.external_type_session.metadata_is_blocked());

        for source in [
            r#"{
                "compilerOptions": {
                    "module":"ESNext",
                    "moduleResolution":"Bundler",
                    "resolvePackageJsonExports":"invalid"
                }
            }"#,
            r#"{
                "compilerOptions": {
                    "moduleResolution":"Node10",
                    "resolvePackageJsonImports":true
                }
            }"#,
        ] {
            let invalid = tempfile::tempdir().expect("invalid option dir");
            let filename = write_config(invalid.path(), source);
            let resolver = Vue3TypeResolverContext::default();
            assert!(vue3_tsconfig_type_resolver_options(&filename, &resolver).is_none());
            assert!(resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn invalid_and_version_unsupported_module_options_block_metadata() {
        for (version, source) in [
            ((5, 9, 0), r#"{"compilerOptions":{"module":1}}"#),
            (
                (5, 9, 0),
                r#"{"compilerOptions":{"moduleResolution":"unknown"}}"#,
            ),
            (
                (5, 9, 0),
                r#"{"compilerOptions":{"target":"future"}}"#,
            ),
            (
                (4, 6, 0),
                r#"{"compilerOptions":{"moduleResolution":"NodeNext"}}"#,
            ),
            (
                (4, 9, 0),
                r#"{"compilerOptions":{"moduleResolution":"Bundler"}}"#,
            ),
            (
                (5, 3, 0),
                r#"{"compilerOptions":{"module":"Preserve"}}"#,
            ),
            ((5, 7, 0), r#"{"compilerOptions":{"module":"Node18"}}"#),
            ((5, 8, 0), r#"{"compilerOptions":{"module":"Node20"}}"#),
        ] {
            let dir = tempfile::tempdir().expect("temp dir");
            let filename = write_config(dir.path(), source);
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                ..Vue3TypeResolverContext::default()
            };

            assert!(
                vue3_tsconfig_type_resolver_options(&filename, &resolver).is_none(),
                "TypeScript {version:?}: {source}"
            );
            assert!(resolver.external_type_session.metadata_is_blocked());
        }

        for (version, source, expected) in [
            (
                (4, 7, 0),
                r#"{"compilerOptions":{"module":"Node16","moduleResolution":"NodeNext"}}"#,
                Vue3TypeModuleResolutionKind::NodeNext,
            ),
            (
                (5, 0, 0),
                r#"{"compilerOptions":{"module":"ESNext","moduleResolution":"Bundler"}}"#,
                Vue3TypeModuleResolutionKind::Bundler,
            ),
            (
                (5, 4, 0),
                r#"{"compilerOptions":{"module":"Preserve"}}"#,
                Vue3TypeModuleResolutionKind::Bundler,
            ),
            (
                (5, 8, 0),
                r#"{"compilerOptions":{"module":"Node18"}}"#,
                Vue3TypeModuleResolutionKind::Node16,
            ),
            (
                (5, 9, 0),
                r#"{"compilerOptions":{"module":"Node20"}}"#,
                Vue3TypeModuleResolutionKind::Node16,
            ),
        ] {
            let dir = tempfile::tempdir().expect("temp dir");
            let filename = write_config(dir.path(), source);
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                ..Vue3TypeResolverContext::default()
            };

            assert_eq!(
                vue3_tsconfig_type_resolver_options(&filename, &resolver)
                    .expect("resolve supported compiler options")
                    .module_resolution,
                expected,
                "TypeScript {version:?}: {source}"
            );
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn module_and_module_resolution_combinations_match_typescript() {
        for (version, module, module_resolution) in [
            ((5, 9, 0), "Node16", "NodeNext"),
            ((5, 9, 0), "NodeNext", "Node16"),
            ((5, 9, 0), "CommonJS", "Node10"),
            ((5, 9, 0), "ESNext", "Node10"),
            ((5, 9, 0), "ESNext", "Bundler"),
            ((5, 9, 0), "Preserve", "Bundler"),
            ((6, 0, 0), "CommonJS", "Bundler"),
        ] {
            let dir = tempfile::tempdir().expect("valid module combination dir");
            let source = format!(
                r#"{{"compilerOptions":{{"module":"{module}","moduleResolution":"{module_resolution}"}}}}"#
            );
            let filename = write_config(dir.path(), &source);
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                ..Vue3TypeResolverContext::default()
            };

            assert!(
                vue3_tsconfig_type_resolver_options(&filename, &resolver).is_some(),
                "TypeScript {version:?}: {module} + {module_resolution}"
            );
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }

        for (version, module, module_resolution) in [
            ((5, 9, 0), "Node16", "Bundler"),
            ((5, 9, 0), "NodeNext", "Node10"),
            ((5, 9, 0), "ESNext", "Node16"),
            ((5, 9, 0), "Preserve", "NodeNext"),
            ((5, 9, 0), "CommonJS", "Bundler"),
            ((5, 9, 0), "AMD", "Bundler"),
        ] {
            let dir = tempfile::tempdir().expect("invalid module combination dir");
            let source = format!(
                r#"{{"compilerOptions":{{"module":"{module}","moduleResolution":"{module_resolution}"}}}}"#
            );
            let filename = write_config(dir.path(), &source);
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                ..Vue3TypeResolverContext::default()
            };

            assert!(
                vue3_tsconfig_type_resolver_options(&filename, &resolver).is_none(),
                "TypeScript {version:?}: {module} + {module_resolution}"
            );
            assert!(resolver.external_type_session.metadata_is_blocked());
        }

        for (version, module_resolution, should_resolve) in [
            ((5, 9, 0), "NodeNext", false),
            ((5, 9, 0), "Bundler", false),
            ((6, 0, 0), "Bundler", true),
        ] {
            let dir = tempfile::tempdir().expect("implicit module combination dir");
            let source = format!(
                r#"{{"compilerOptions":{{"moduleResolution":"{module_resolution}"}}}}"#
            );
            let filename = write_config(dir.path(), &source);
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                ..Vue3TypeResolverContext::default()
            };

            assert_eq!(
                vue3_tsconfig_type_resolver_options(&filename, &resolver).is_some(),
                should_resolve,
                "TypeScript {version:?}: implicit module + {module_resolution}"
            );
            assert_eq!(
                resolver.external_type_session.metadata_is_blocked(),
                !should_resolve
            );
        }
    }

    #[test]
    fn resolver_option_version_boundaries_match_typescript() {
        for (version, option, value) in [
            ((2, 0, 0), "target", "ES2017"),
            ((3, 3, 0), "target", "ES2019"),
            ((3, 4, 0), "target", "ES2020"),
            ((4, 5, 0), "target", "ES2022"),
            ((5, 4, 0), "target", "ES2023"),
            ((5, 6, 0), "target", "ES2024"),
            ((5, 9, 0), "target", "ES2025"),
            ((4, 9, 0), "moduleResolution", "Node10"),
            ((7, 0, 0), "target", "ES3"),
            ((7, 0, 0), "target", "ES5"),
            ((7, 0, 0), "moduleResolution", "Node"),
            ((7, 0, 0), "moduleResolution", "Node10"),
            ((7, 0, 0), "moduleResolution", "Classic"),
            ((7, 0, 0), "module", "None"),
            ((7, 0, 0), "module", "AMD"),
            ((7, 0, 0), "module", "System"),
            ((7, 0, 0), "module", "UMD"),
        ] {
            let dir = tempfile::tempdir().expect("temp dir");
            let source = format!(r#"{{"compilerOptions":{{"{option}":"{value}"}}}}"#);
            let filename = write_config(dir.path(), &source);
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                ..Vue3TypeResolverContext::default()
            };

            assert!(
                vue3_tsconfig_type_resolver_options(&filename, &resolver).is_none(),
                "TypeScript {version:?}: {option}={value}"
            );
            assert!(resolver.external_type_session.metadata_is_blocked());
        }

        for (version, option, value, expected) in [
            (
                (2, 1, 0),
                "target",
                "ES2017",
                Vue3TypeModuleResolutionKind::Classic,
            ),
            (
                (3, 4, 0),
                "target",
                "ES2019",
                Vue3TypeModuleResolutionKind::Classic,
            ),
            (
                (3, 5, 0),
                "target",
                "ES2020",
                Vue3TypeModuleResolutionKind::Classic,
            ),
            (
                (4, 6, 0),
                "target",
                "ES2022",
                Vue3TypeModuleResolutionKind::Classic,
            ),
            (
                (5, 5, 0),
                "target",
                "ES2023",
                Vue3TypeModuleResolutionKind::Classic,
            ),
            (
                (5, 7, 0),
                "target",
                "ES2024",
                Vue3TypeModuleResolutionKind::Classic,
            ),
            (
                (6, 0, 0),
                "target",
                "ES2025",
                Vue3TypeModuleResolutionKind::Bundler,
            ),
            (
                (5, 0, 0),
                "moduleResolution",
                "Node10",
                Vue3TypeModuleResolutionKind::Node10,
            ),
            (
                (6, 0, 0),
                "moduleResolution",
                "Classic",
                Vue3TypeModuleResolutionKind::Classic,
            ),
            (
                (6, 0, 0),
                "module",
                "AMD",
                Vue3TypeModuleResolutionKind::Classic,
            ),
            (
                (6, 0, 0),
                "target",
                "ES5",
                Vue3TypeModuleResolutionKind::Bundler,
            ),
        ] {
            let dir = tempfile::tempdir().expect("temp dir");
            let source = format!(r#"{{"compilerOptions":{{"{option}":"{value}"}}}}"#);
            let filename = write_config(dir.path(), &source);
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                ..Vue3TypeResolverContext::default()
            };

            assert_eq!(
                vue3_tsconfig_type_resolver_options(&filename, &resolver)
                    .expect("resolve supported compiler option")
                    .module_resolution,
                expected,
                "TypeScript {version:?}: {option}={value}"
            );
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn reference_paths_ignore_module_suffixes_but_declaration_probes_use_them() {
        let dir = tempfile::tempdir().expect("temp dir");
        let plain = dir.path().join("referenced.d.ts");
        let suffixed = dir.path().join("referenced.native.d.ts");
        std::fs::write(&plain, "interface PlainReference {}").expect("write plain reference");
        std::fs::write(&suffixed, "interface NativeReference {}").expect("write native reference");
        let filename = dir.path().join("root.d.ts").to_string_lossy().to_string();
        let resolver = Vue3TypeResolverContext {
            module_suffixes: std::sync::Arc::from([".native".to_string()]),
            ..Vue3TypeResolverContext::default()
        };

        assert_eq!(
            resolve_vue3_type_reference_path(&filename, "./referenced.d.ts", &resolver),
            Some(plain.clone())
        );
        assert_eq!(
            resolve_vue3_metadata_type_reference_declaration_file(&plain, &resolver),
            Some(suffixed)
        );
    }
}
