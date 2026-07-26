#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Vue3TypeResolutionKind {
    Import,
    ReferencePath,
    ReferenceTypes,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Vue3TypeImportResolutionCacheKey {
    kind: Vue3TypeResolutionKind,
    importer: PathBuf,
    relative_current_dir: Option<PathBuf>,
    source: String,
    typescript_version: String,
}

impl Vue3TypeImportResolutionCacheKey {
    fn weight(&self, entry: &Vue3TypeImportResolutionCacheEntry) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.importer.as_os_str().as_encoded_bytes().len())
            .saturating_add(
                self.relative_current_dir
                    .as_ref()
                    .map_or(0, |path| path.as_os_str().as_encoded_bytes().len()),
            )
            .saturating_add(self.source.len())
            .saturating_add(self.typescript_version.len())
            .saturating_add(std::mem::size_of::<Vue3TypeImportResolutionCacheEntry>())
            .saturating_add(entry.path_weight())
    }
}

#[derive(Clone, Debug)]
enum Vue3TypeImportResolutionCacheEntry {
    Resolved(PathBuf),
    Missing,
}

impl Vue3TypeImportResolutionCacheEntry {
    fn from_resolution(resolution: &Option<PathBuf>) -> Self {
        match resolution {
            Some(path) => Self::Resolved(path.clone()),
            None => Self::Missing,
        }
    }

    fn resolution(&self) -> Option<PathBuf> {
        match self {
            Self::Resolved(path) => Some(path.clone()),
            Self::Missing => None,
        }
    }

    fn path_weight(&self) -> usize {
        match self {
            Self::Resolved(path) => path.as_os_str().as_encoded_bytes().len(),
            Self::Missing => 0,
        }
    }
}

enum Vue3TypeImportResolutionLoad {
    Ready(Option<PathBuf>),
    Failed,
    Start {
        cache_key: Option<Vue3TypeImportResolutionCacheKey>,
        failure_epoch: usize,
    },
}

impl Vue3ExternalTypeLoadSession {
    fn begin_type_import_resolution(
        &self,
        kind: Vue3TypeResolutionKind,
        filename: &str,
        source: &str,
        typescript_version: &nodejs_semver::Version,
        is_relative: bool,
    ) -> Vue3TypeImportResolutionLoad {
        let max_cache_entry_weight = {
            let mut state = self.lock();
            if state.stats.resolution_lookups >= state.limits.max_resolution_lookups {
                state.failure_epoch += 1;
                return Vue3TypeImportResolutionLoad::Failed;
            }
            state.stats.resolution_lookups += 1;
            if !is_relative && state.metadata_blocked {
                state.failure_epoch += 1;
                return Vue3TypeImportResolutionLoad::Failed;
            }
            state.limits.max_resolution_cache_entry_weight
        };
        let relative_current_dir = if Path::new(filename).is_relative() {
            std::env::current_dir().ok().map(|current_dir| {
                vue3_external_type_path_key(normalize_path_components(current_dir))
            })
        } else {
            None
        };
        let typescript_version = typescript_version.to_string();
        let minimum_weight = std::mem::size_of::<Vue3TypeImportResolutionCacheKey>()
            .saturating_add(filename.len())
            .saturating_add(
                relative_current_dir
                    .as_ref()
                    .map_or(0, |path| path.as_os_str().as_encoded_bytes().len()),
            )
            .saturating_add(source.len())
            .saturating_add(typescript_version.len())
            .saturating_add(std::mem::size_of::<Vue3TypeImportResolutionCacheEntry>());
        let cache_key = (minimum_weight <= max_cache_entry_weight).then(|| {
            Vue3TypeImportResolutionCacheKey {
                kind,
                importer: PathBuf::from(filename),
                relative_current_dir,
                source: source.to_string(),
                typescript_version,
            }
        });
        let mut state = self.lock();
        if !is_relative && state.metadata_blocked {
            state.failure_epoch += 1;
            return Vue3TypeImportResolutionLoad::Failed;
        }
        if let Some(cache_key) = cache_key.as_ref() {
            if let Some(entry) = state.resolution_cache.get(cache_key).cloned() {
                state.stats.resolution_cache_hits += 1;
                return Vue3TypeImportResolutionLoad::Ready(entry.resolution());
            }
        }
        Vue3TypeImportResolutionLoad::Start {
            cache_key,
            failure_epoch: state.failure_epoch,
        }
    }

    fn finish_type_import_resolution(
        &self,
        cache_key: Option<Vue3TypeImportResolutionCacheKey>,
        resolution: Option<PathBuf>,
        failure_epoch: usize,
        is_relative: bool,
    ) -> Option<PathBuf> {
        let mut state = self.lock();
        if !is_relative && state.metadata_blocked {
            state.failure_epoch += 1;
            return None;
        }
        let Some(cache_key) = cache_key else {
            return resolution;
        };
        if let Some(entry) = state.resolution_cache.get(&cache_key).cloned() {
            state.stats.resolution_cache_hits += 1;
            return entry.resolution();
        }
        if state.failure_epoch != failure_epoch {
            return resolution;
        }
        let entry = Vue3TypeImportResolutionCacheEntry::from_resolution(&resolution);
        let weight = cache_key.weight(&entry);
        if state.resolution_cache.len() < state.limits.max_resolution_cache_entries
            && weight <= state.limits.max_resolution_cache_entry_weight
            && state
                .stats
                .cached_resolution_weight
                .saturating_add(weight)
                <= state.limits.max_resolution_cache_weight
        {
            state.stats.cached_resolution_weight =
                state.stats.cached_resolution_weight.saturating_add(weight);
            state.resolution_cache.insert(cache_key, entry);
        }
        resolution
    }
}
