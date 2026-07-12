include!("metadata_source_single_flight.rs");
include!("metadata_parse_single_flight.rs");

struct Vue3PackageResolutionGuard<'a> {
    session: &'a Vue3ExternalTypeLoadSession,
    owner: std::thread::ThreadId,
    identity: PathBuf,
}

impl Drop for Vue3PackageResolutionGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.session.lock();
        let remove_owner = state
            .active_package_resolutions
            .get_mut(&self.owner)
            .is_some_and(|stack| {
                if let Some(index) = stack.iter().rposition(|path| path == &self.identity) {
                    stack.remove(index);
                }
                stack.is_empty()
            });
        if remove_owner {
            state.active_package_resolutions.remove(&self.owner);
        }
    }
}

impl Vue3ExternalTypeLoadSession {
    #[cfg(test)]
    fn metadata_source_from_path(&self, path: &Path) -> Option<std::sync::Arc<String>> {
        let cache_key = self.metadata_cache_key(path)?;
        self.metadata_source_from_path_with_key(path, cache_key)
    }

    fn metadata_source_from_path_with_key(
        &self,
        path: &Path,
        cache_key: PathBuf,
    ) -> Option<std::sync::Arc<String>> {
        match self.begin_metadata_source_load(cache_key) {
            Vue3MetadataSourceLoad::Ready(source) => Some(source),
            Vue3MetadataSourceLoad::Wait(waiter) => match waiter.wait() {
                Vue3MetadataSourceWaitResult::Ready(source) => Some(source),
                Vue3MetadataSourceWaitResult::Missing
                | Vue3MetadataSourceWaitResult::Blocked => None,
            },
            Vue3MetadataSourceLoad::Start(mut owner) => {
                let outcome = read_vue3_metadata_source(path, &mut owner);
                owner.complete(outcome)
            }
            Vue3MetadataSourceLoad::Missing | Vue3MetadataSourceLoad::Blocked => None,
        }
    }

    fn tsconfig_from_path(&self, path: &Path) -> Option<std::sync::Arc<serde_json::Value>> {
        self.parsed_metadata_from_path::<Vue3TsconfigMetadataKind>(path)
    }

    fn package_json_from_path(
        &self,
        path: &Path,
    ) -> Option<std::sync::Arc<Vue3PackageJsonTypeManifest>> {
        self.parsed_metadata_from_path::<Vue3PackageJsonMetadataKind>(path)
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
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
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

    fn max_ancestor_search_depth(&self) -> usize {
        self.lock().limits.max_ancestor_search_depth
    }

    fn claim_ancestor_search_dir(&self, dir: &Path, suffix: &str) -> bool {
        let raw_candidate_weight = vue3_ancestor_search_candidate_weight(dir, suffix);
        let path_limit = {
            let state = self.lock();
            if state.metadata_blocked {
                return false;
            }
            state.limits.max_ancestor_search_path_bytes
        };
        if raw_candidate_weight > path_limit {
            self.block_metadata();
            return false;
        }

        let identity = vue3_external_type_lexical_path(dir);
        let identity_weight = identity.as_os_str().as_encoded_bytes().len();
        let normalized_candidate_weight = vue3_ancestor_search_candidate_weight(&identity, suffix);
        let mut state = self.lock();
        if state.metadata_blocked {
            return false;
        }
        if normalized_candidate_weight > state.limits.max_ancestor_search_path_bytes {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            return false;
        }
        if state.ancestor_search_dirs.contains(&identity) {
            return true;
        }
        let Some(next_weight) = state.stats.ancestor_search_weight.checked_add(identity_weight)
        else {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            return false;
        };
        if state.ancestor_search_dirs.len() >= state.limits.max_ancestor_search_entries
            || next_weight > state.limits.max_ancestor_search_weight
        {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            return false;
        }
        state.ancestor_search_dirs.insert(identity);
        state.stats.ancestor_search_entries = state.ancestor_search_dirs.len();
        state.stats.ancestor_search_weight = next_weight;
        true
    }

    fn claim_tsconfig_discovery_entry(&self) -> bool {
        let mut state = self.lock();
        if state.metadata_blocked {
            return false;
        }
        if state.stats.tsconfig_discovery_entries
            >= state.limits.max_tsconfig_discovery_entries
        {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
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
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            return false;
        }
        state.stats.tsconfig_discovery_files += 1;
        true
    }

    pub(crate) fn metadata_is_blocked(&self) -> bool {
        let mut state = self.lock();
        if state.metadata_blocked {
            state.failure_epoch += 1;
            return true;
        }
        false
    }

    fn replace_metadata_path_pattern(
        &self,
        source: &str,
        pattern: &str,
        replacement: &str,
    ) -> Option<String> {
        let max_bytes = {
            let mut state = self.lock();
            if state.metadata_blocked {
                state.failure_epoch += 1;
                return None;
            }
            state.limits.max_generated_path_bytes
        };
        match vue3_bounded_replace(source, pattern, replacement, max_bytes) {
            Some(value) => Some(value),
            None => {
                self.block_metadata();
                None
            }
        }
    }

    fn metadata_path_is_within_limit(&self, path: &str) -> bool {
        let within_limit = {
            let mut state = self.lock();
            if state.metadata_blocked {
                state.failure_epoch += 1;
                return false;
            }
            path.len() <= state.limits.max_generated_path_bytes
        };
        if !within_limit {
            self.block_metadata();
        }
        within_limit
    }

    fn block_metadata(&self) {
        let mut state = self.lock();
        let flights = vue3_block_metadata_state(&mut state);
        drop(state);
        vue3_abort_metadata_flights(flights);
    }

    fn begin_package_resolution(&self, path: &Path) -> Option<Vue3PackageResolutionGuard<'_>> {
        if self.metadata_is_blocked() {
            return None;
        }
        let identity = vue3_external_type_path_identity(path);
        let owner = std::thread::current().id();
        let mut state = self.lock();
        let owner_stack = state.active_package_resolutions.get(&owner);
        let recursion_blocked = owner_stack.is_some_and(|stack| stack.contains(&identity))
            || owner_stack.map_or(0, Vec::len) >= state.limits.max_package_resolution_depth;
        if state.metadata_blocked || recursion_blocked {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            return None;
        }
        state
            .active_package_resolutions
            .entry(owner)
            .or_default()
            .push(identity.clone());
        drop(state);
        Some(Vue3PackageResolutionGuard {
            session: self,
            owner,
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
                let flights = vue3_block_metadata_state(&mut state);
                drop(state);
                vue3_abort_metadata_flights(flights);
                return None;
            }
        }
        let probed_identity = vue3_external_type_path_identity(path);
        let mut state = self.lock();
        if state.metadata_blocked {
            state.failure_epoch += 1;
            return None;
        }
        if let Some(identity) = state.metadata_path_identities.get(&lexical_key) {
            return Some(identity.clone());
        }
        if state.metadata_path_identities.len() >= state.limits.max_metadata_files {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            return None;
        }
        state
            .metadata_path_identities
            .insert(lexical_key, probed_identity.clone());
        Some(probed_identity)
    }
}

fn read_vue3_metadata_source(
    path: &Path,
    owner: &mut Vue3MetadataSourceOwner,
) -> Vue3MetadataSourceOutcome {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Vue3MetadataSourceOutcome::Missing;
        }
        Err(_) => return Vue3MetadataSourceOutcome::Blocked,
    };
    let metadata = match file.metadata() {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) | Err(_) => return Vue3MetadataSourceOutcome::Blocked,
    };
    let declared_len_u64 = metadata.len();
    let Ok(declared_len) = usize::try_from(declared_len_u64) else {
        return Vue3MetadataSourceOutcome::Blocked;
    };
    if !owner.reserve_bytes(declared_len) {
        return Vue3MetadataSourceOutcome::Blocked;
    }
    let mut bytes = Vec::with_capacity(declared_len.min(64 * 1024));
    let read_failed = {
        let mut limited = std::io::Read::take(&mut file, declared_len as u64);
        std::io::Read::read_to_end(&mut limited, &mut bytes).is_err()
    };
    if read_failed {
        owner.record_bytes_read(bytes.len());
        return Vue3MetadataSourceOutcome::Blocked;
    }
    let bytes_read = bytes.len();
    owner.record_bytes_read(bytes_read);
    let length_changed = file
        .metadata()
        .map_or(true, |metadata| metadata.len() != declared_len_u64);
    if bytes_read != declared_len || length_changed {
        return Vue3MetadataSourceOutcome::Blocked;
    }
    match String::from_utf8(bytes) {
        Ok(source) => Vue3MetadataSourceOutcome::Ready(source),
        Err(_) => Vue3MetadataSourceOutcome::Blocked,
    }
}

#[cfg(test)]
mod package_resolution_tests {
    use super::*;

    #[test]
    fn package_resolution_recursion_is_scoped_to_the_current_thread() {
        let session = Vue3ExternalTypeLoadSession::with_limits(Vue3ExternalTypeLoadLimits {
            max_package_resolution_depth: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let path = PathBuf::from("same-package");
        let _guard = session
            .begin_package_resolution(&path)
            .expect("first thread guard");
        let other_session = session.clone();
        let other_path = path.clone();

        let allowed = std::thread::spawn(move || {
            other_session
                .begin_package_resolution(&other_path)
                .is_some()
        })
        .join()
        .expect("join package resolution thread");

        assert!(allowed);
        assert!(!session.metadata_is_blocked());
    }

    #[test]
    fn package_resolution_rejects_same_thread_recursion() {
        let session = Vue3ExternalTypeLoadSession::default();
        let path = Path::new("recursive-package");
        let _guard = session
            .begin_package_resolution(path)
            .expect("outer package resolution");

        assert!(session.begin_package_resolution(path).is_none());
        assert!(session.metadata_is_blocked());
    }

    #[test]
    fn package_resolution_rejects_a_zero_depth_limit() {
        let session = Vue3ExternalTypeLoadSession::with_limits(Vue3ExternalTypeLoadLimits {
            max_package_resolution_depth: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert!(session
            .begin_package_resolution(Path::new("blocked-package"))
            .is_none());
        assert!(session.metadata_is_blocked());
    }

    #[test]
    fn package_resolution_guard_cleans_up_during_unwind() {
        let session = Vue3ExternalTypeLoadSession::default();
        let path = PathBuf::from("panicking-package");
        let unwind_session = session.clone();
        let unwind_path = path.clone();

        let result = std::panic::catch_unwind(move || {
            let _guard = unwind_session
                .begin_package_resolution(&unwind_path)
                .expect("package resolution before panic");
            panic!("test package resolution unwind");
        });

        assert!(result.is_err());
        assert!(session.begin_package_resolution(&path).is_some());
        assert!(!session.metadata_is_blocked());
    }
}
