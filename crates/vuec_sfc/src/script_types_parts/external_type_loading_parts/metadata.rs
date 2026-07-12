#[derive(Clone, Debug)]
enum Vue3MetadataSourceCacheEntry {
    Loading,
    Ready(std::sync::Arc<String>),
    Failed,
}

#[derive(Clone, Debug)]
enum Vue3TsconfigCacheEntry {
    Loading,
    Ready(std::sync::Arc<serde_json::Value>),
    Failed,
}

#[derive(Clone, Debug)]
enum Vue3PackageJsonCacheEntry {
    Loading,
    Ready(std::sync::Arc<Vue3PackageJsonTypeManifest>),
    Failed,
}

struct Vue3PackageResolutionGuard<'a> {
    session: &'a Vue3ExternalTypeLoadSession,
    identity: PathBuf,
}

impl Drop for Vue3PackageResolutionGuard<'_> {
    fn drop(&mut self) {
        self.session
            .lock()
            .active_package_resolutions
            .remove(&self.identity);
    }
}

impl Vue3ExternalTypeLoadSession {
    fn metadata_source_from_path(&self, path: &Path) -> Option<std::sync::Arc<String>> {
        let cache_key = self.metadata_cache_key(path)?;
        let max_bytes = {
            let mut state = self.lock();
            if state.metadata_blocked {
                state.failure_epoch += 1;
                return None;
            }
            match state.metadata_source_cache.get(&cache_key).cloned() {
                Some(Vue3MetadataSourceCacheEntry::Ready(source)) => {
                    state.stats.metadata_source_cache_hits += 1;
                    return Some(source);
                }
                Some(Vue3MetadataSourceCacheEntry::Failed) => {
                    state.stats.metadata_source_cache_hits += 1;
                    return None;
                }
                Some(Vue3MetadataSourceCacheEntry::Loading) => return None,
                None => {}
            }
            if state.stats.metadata_files_read >= state.limits.max_metadata_files {
                state.metadata_blocked = true;
                state.failure_epoch += 1;
                return None;
            }
            let remaining = state
                .limits
                .max_metadata_bytes
                .saturating_sub(state.stats.metadata_bytes);
            state.stats.metadata_files_read += 1;
            state
                .metadata_source_cache
                .insert(cache_key.clone(), Vue3MetadataSourceCacheEntry::Loading);
            state.limits.max_metadata_file_bytes.min(remaining)
        };

        let (source, bytes_read, read_blocked) = read_vue3_metadata_source(path, max_bytes);
        let mut state = self.lock();
        state.stats.metadata_bytes = state.stats.metadata_bytes.saturating_add(bytes_read);
        let total_exceeded = state.stats.metadata_bytes > state.limits.max_metadata_bytes;
        match source {
            Some(source) if !total_exceeded => {
                let source = std::sync::Arc::new(source);
                state.metadata_source_cache.insert(
                    cache_key,
                    Vue3MetadataSourceCacheEntry::Ready(source.clone()),
                );
                Some(source)
            }
            _ => {
                state
                    .metadata_source_cache
                    .insert(cache_key, Vue3MetadataSourceCacheEntry::Failed);
                if read_blocked || total_exceeded {
                    state.metadata_blocked = true;
                    state.failure_epoch += 1;
                }
                None
            }
        }
    }

    fn tsconfig_from_path(&self, path: &Path) -> Option<std::sync::Arc<serde_json::Value>> {
        let cache_key = self.metadata_cache_key(path)?;
        {
            let mut state = self.lock();
            if state.metadata_blocked {
                state.failure_epoch += 1;
                return None;
            }
            match state.tsconfig_cache.get(&cache_key).cloned() {
                Some(Vue3TsconfigCacheEntry::Ready(value)) => {
                    state.stats.metadata_parse_cache_hits += 1;
                    return Some(value);
                }
                Some(Vue3TsconfigCacheEntry::Failed) => {
                    state.stats.metadata_parse_cache_hits += 1;
                    return None;
                }
                Some(Vue3TsconfigCacheEntry::Loading) => return None,
                None => {}
            }
        }
        let source = self.metadata_source_from_path(path)?;
        let cache_key = self.metadata_cache_key(path)?;
        {
            let mut state = self.lock();
            match state.tsconfig_cache.get(&cache_key).cloned() {
                Some(Vue3TsconfigCacheEntry::Ready(value)) => {
                    state.stats.metadata_parse_cache_hits += 1;
                    return Some(value);
                }
                Some(Vue3TsconfigCacheEntry::Failed) => {
                    state.stats.metadata_parse_cache_hits += 1;
                    return None;
                }
                Some(Vue3TsconfigCacheEntry::Loading) => return None,
                None => {
                    state
                        .tsconfig_cache
                        .insert(cache_key.clone(), Vue3TsconfigCacheEntry::Loading);
                }
            }
        }
        let parsed = vue3_parse_tsconfig_jsonc(&source).map(std::sync::Arc::new);
        let mut state = self.lock();
        match parsed {
            Some(value) => {
                state.tsconfig_cache.insert(
                    cache_key,
                    Vue3TsconfigCacheEntry::Ready(value.clone()),
                );
                Some(value)
            }
            None => {
                state
                    .tsconfig_cache
                    .insert(cache_key, Vue3TsconfigCacheEntry::Failed);
                state.metadata_blocked = true;
                state.failure_epoch += 1;
                None
            }
        }
    }

    fn package_json_from_path(
        &self,
        path: &Path,
    ) -> Option<std::sync::Arc<Vue3PackageJsonTypeManifest>> {
        let cache_key = self.metadata_cache_key(path)?;
        {
            let mut state = self.lock();
            if state.metadata_blocked {
                state.failure_epoch += 1;
                return None;
            }
            match state.package_json_cache.get(&cache_key).cloned() {
                Some(Vue3PackageJsonCacheEntry::Ready(value)) => {
                    state.stats.metadata_parse_cache_hits += 1;
                    return Some(value);
                }
                Some(Vue3PackageJsonCacheEntry::Failed) => {
                    state.stats.metadata_parse_cache_hits += 1;
                    return None;
                }
                Some(Vue3PackageJsonCacheEntry::Loading) => return None,
                None => {}
            }
        }
        let source = self.metadata_source_from_path(path)?;
        let cache_key = self.metadata_cache_key(path)?;
        {
            let mut state = self.lock();
            match state.package_json_cache.get(&cache_key).cloned() {
                Some(Vue3PackageJsonCacheEntry::Ready(value)) => {
                    state.stats.metadata_parse_cache_hits += 1;
                    return Some(value);
                }
                Some(Vue3PackageJsonCacheEntry::Failed) => {
                    state.stats.metadata_parse_cache_hits += 1;
                    return None;
                }
                Some(Vue3PackageJsonCacheEntry::Loading) => return None,
                None => {
                    state
                        .package_json_cache
                        .insert(cache_key.clone(), Vue3PackageJsonCacheEntry::Loading);
                }
            }
        }
        let parsed = serde_json::from_str::<Vue3PackageJsonTypeManifest>(&source)
            .ok()
            .map(std::sync::Arc::new);
        let mut state = self.lock();
        match parsed {
            Some(value) => {
                state.package_json_cache.insert(
                    cache_key,
                    Vue3PackageJsonCacheEntry::Ready(value.clone()),
                );
                Some(value)
            }
            None => {
                state
                    .package_json_cache
                    .insert(cache_key, Vue3PackageJsonCacheEntry::Failed);
                state.metadata_blocked = true;
                state.failure_epoch += 1;
                None
            }
        }
    }

    fn claim_tsconfig_node(&self, state_key: &(PathBuf, PathBuf, PathBuf)) -> bool {
        let mut state = self.lock();
        if state.metadata_blocked {
            return false;
        }
        if state.tsconfig_node_states.contains(state_key) {
            return true;
        }
        if state.tsconfig_node_states.len() >= state.limits.max_tsconfig_nodes {
            state.metadata_blocked = true;
            state.failure_epoch += 1;
            return false;
        }
        state.tsconfig_node_states.insert(state_key.clone());
        state.stats.tsconfig_nodes = state.tsconfig_node_states.len();
        true
    }

    fn max_tsconfig_depth(&self) -> usize {
        self.lock().limits.max_tsconfig_depth
    }

    fn max_tsconfig_discovery_depth(&self) -> usize {
        self.lock().limits.max_tsconfig_discovery_depth
    }

    fn claim_tsconfig_discovery_entry(&self) -> bool {
        let mut state = self.lock();
        if state.metadata_blocked {
            return false;
        }
        if state.stats.tsconfig_discovery_entries
            >= state.limits.max_tsconfig_discovery_entries
        {
            state.metadata_blocked = true;
            state.failure_epoch += 1;
            return false;
        }
        state.stats.tsconfig_discovery_entries += 1;
        true
    }

    fn claim_tsconfig_discovery_file(&self) -> bool {
        let mut state = self.lock();
        if state.metadata_blocked {
            return false;
        }
        if state.stats.tsconfig_discovery_files >= state.limits.max_tsconfig_discovery_files {
            state.metadata_blocked = true;
            state.failure_epoch += 1;
            return false;
        }
        state.stats.tsconfig_discovery_files += 1;
        true
    }

    fn metadata_is_blocked(&self) -> bool {
        let mut state = self.lock();
        if state.metadata_blocked {
            state.failure_epoch += 1;
            return true;
        }
        false
    }

    fn block_metadata(&self) {
        let mut state = self.lock();
        state.metadata_blocked = true;
        state.failure_epoch += 1;
    }

    fn begin_package_resolution(&self, path: &Path) -> Option<Vue3PackageResolutionGuard<'_>> {
        if self.metadata_is_blocked() {
            return None;
        }
        let identity = vue3_external_type_path_identity_path(path);
        let mut state = self.lock();
        if state.metadata_blocked
            || state.active_package_resolutions.contains(&identity)
            || state.active_package_resolutions.len() >= state.limits.max_package_resolution_depth
        {
            state.metadata_blocked = true;
            state.failure_epoch += 1;
            return None;
        }
        state.active_package_resolutions.insert(identity.clone());
        drop(state);
        Some(Vue3PackageResolutionGuard {
            session: self,
            identity,
        })
    }

    fn metadata_cache_key(&self, path: &Path) -> Option<PathBuf> {
        let lexical_key = vue3_external_type_lexical_path(path);
        {
            let mut state = self.lock();
            if state.metadata_blocked {
                state.failure_epoch += 1;
                return None;
            }
            if let Some(identity) = state.metadata_path_identities.get(&lexical_key) {
                return Some(identity.clone());
            }
            if state.metadata_path_identities.len() >= state.limits.max_metadata_files {
                state.metadata_blocked = true;
                state.failure_epoch += 1;
                return None;
            }
        }
        let probed_identity = vue3_external_type_path_identity_path(path);
        let mut state = self.lock();
        if state.metadata_blocked {
            state.failure_epoch += 1;
            return None;
        }
        if let Some(identity) = state.metadata_path_identities.get(&lexical_key) {
            return Some(identity.clone());
        }
        if state.metadata_path_identities.len() >= state.limits.max_metadata_files {
            state.metadata_blocked = true;
            state.failure_epoch += 1;
            return None;
        }
        state
            .metadata_path_identities
            .insert(lexical_key, probed_identity.clone());
        Some(probed_identity)
    }
}

fn read_vue3_metadata_source(path: &Path, max_bytes: usize) -> (Option<String>, usize, bool) {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return (None, 0, true),
        Err(error) => return (None, 0, error.kind() != std::io::ErrorKind::NotFound),
    };
    if metadata.len() > max_bytes as u64 {
        return (None, 0, true);
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) => return (None, 0, error.kind() != std::io::ErrorKind::NotFound),
    };
    let mut bytes = Vec::with_capacity(max_bytes.min(64 * 1024));
    let mut limited = std::io::Read::take(file, max_bytes.saturating_add(1) as u64);
    if std::io::Read::read_to_end(&mut limited, &mut bytes).is_err() {
        let bytes_read = bytes.len();
        return (None, bytes_read, true);
    }
    let bytes_read = bytes.len();
    if bytes_read > max_bytes {
        return (None, bytes_read, true);
    }
    match String::from_utf8(bytes) {
        Ok(source) => (Some(source), bytes_read, false),
        Err(_) => (None, bytes_read, true),
    }
}
