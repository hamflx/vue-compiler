type Vue3ExternalTypeSourceResult = Option<std::sync::Arc<Vue3ExternalTypeSource>>;
type Vue3ExternalTypeSourceFlight = Vue3SingleFlight<Vue3ExternalTypeSource>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Vue3ExternalTypeSourceKind {
    Import,
    Global,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Vue3ExternalTypeSemanticIdentity {
    path: PathBuf,
    mode: String,
}

impl Vue3ExternalTypeSemanticIdentity {
    fn work(&self) -> usize {
        self.path
            .as_os_str()
            .as_encoded_bytes()
            .len()
            .saturating_add(self.mode.len())
            .saturating_add(std::mem::size_of::<Self>())
            .saturating_add(1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Vue3ExternalTypeSourceCacheKey {
    semantic: Vue3ExternalTypeSemanticIdentity,
    kind: Vue3ExternalTypeSourceKind,
}

#[derive(Clone, Debug)]
enum Vue3ExternalTypeSourceCacheEntry {
    Loading(std::sync::Arc<Vue3ExternalTypeSourceFlight>),
    Ready(std::sync::Arc<Vue3ExternalTypeSource>),
    Failed,
}

enum Vue3ExternalTypeSourceLoad {
    Ready(std::sync::Arc<Vue3ExternalTypeSource>),
    Wait(Vue3ExternalTypeSourceWaiter),
    Start(Vue3ExternalTypeSourceOwner),
    Failed,
}

struct Vue3ExternalTypeSourceWaiter {
    session: Vue3ExternalTypeLoadSession,
    flight: std::sync::Arc<Vue3ExternalTypeSourceFlight>,
}

impl Vue3ExternalTypeSourceWaiter {
    fn wait(self) -> Vue3ExternalTypeSourceResult {
        match self.flight.wait() {
            Vue3SingleFlightOutcome::Complete(result) => {
                if result.is_some() {
                    self.session.lock().stats.source_cache_hits += 1;
                }
                result
            }
            Vue3SingleFlightOutcome::Aborted => None,
        }
    }
}

struct Vue3ExternalTypeSourceOwner {
    session: Vue3ExternalTypeLoadSession,
    cache_key: Vue3ExternalTypeSourceCacheKey,
    kind: Vue3ExternalTypeSourceKind,
    flight: std::sync::Arc<Vue3ExternalTypeSourceFlight>,
    reserved_bytes: usize,
    bytes_read: usize,
    reservation_active: bool,
    active: bool,
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Vue3ExternalTypeSourceOwner {
    fn reserve_bytes(&mut self, bytes: usize) -> bool {
        if self.reservation_active {
            return false;
        }
        let session = self.session.clone();
        let mut state = session.lock();
        if !vue3_source_flight_matches(
            state.source_cache.get(&self.cache_key),
            self.flight.id,
        ) {
            return false;
        }
        let (completed, reserved, max_total) = match self.kind {
            Vue3ExternalTypeSourceKind::Import => (
                state.stats.import_bytes,
                state.reserved_import_bytes,
                state.limits.max_import_bytes,
            ),
            Vue3ExternalTypeSourceKind::Global => (
                state.stats.global_bytes,
                state.reserved_global_bytes,
                state.limits.max_global_bytes,
            ),
        };
        let remaining = max_total
            .saturating_sub(completed)
            .saturating_sub(reserved);
        if bytes > state.limits.max_file_bytes || bytes > remaining {
            return false;
        }
        match self.kind {
            Vue3ExternalTypeSourceKind::Import => {
                state.reserved_import_bytes += bytes;
            }
            Vue3ExternalTypeSourceKind::Global => {
                state.reserved_global_bytes += bytes;
            }
        }
        self.reserved_bytes = bytes;
        self.reservation_active = true;
        true
    }

    fn record_bytes_read(&mut self, bytes: usize) {
        assert!(self.reservation_active, "source bytes must be reserved first");
        assert!(
            bytes <= self.reserved_bytes,
            "source read exceeded its byte reservation"
        );
        self.bytes_read = bytes;
    }

    fn complete(
        mut self,
        source: Option<Vue3ExternalTypeSource>,
    ) -> Vue3ExternalTypeSourceResult {
        let result = (self.reservation_active && self.bytes_read == self.reserved_bytes)
            .then(|| source.map(std::sync::Arc::new))
            .flatten();
        let session = self.session.clone();
        let mut state = session.lock();
        let current = vue3_source_flight_matches(
            state.source_cache.get(&self.cache_key),
            self.flight.id,
        );
        self.settle_reservation(&mut state, false);
        if current {
            match &result {
                Some(source) => {
                    state.source_cache.insert(
                        self.cache_key.clone(),
                        Vue3ExternalTypeSourceCacheEntry::Ready(source.clone()),
                    );
                }
                None => {
                    state.source_cache.insert(
                        self.cache_key.clone(),
                        Vue3ExternalTypeSourceCacheEntry::Failed,
                    );
                }
            }
        }
        if result.is_none() {
            state.failure_epoch += 1;
        }
        drop(state);
        self.flight.complete(result.clone());
        self.active = false;
        result
    }

    fn settle_reservation(&mut self, state: &mut Vue3ExternalTypeLoadState, forfeit: bool) {
        if !self.reservation_active {
            return;
        }
        let committed = if forfeit {
            self.reserved_bytes
        } else {
            self.bytes_read
        };
        match self.kind {
            Vue3ExternalTypeSourceKind::Import => {
                state.reserved_import_bytes = state
                    .reserved_import_bytes
                    .saturating_sub(self.reserved_bytes);
                state.stats.import_bytes = state.stats.import_bytes.saturating_add(committed);
            }
            Vue3ExternalTypeSourceKind::Global => {
                state.reserved_global_bytes = state
                    .reserved_global_bytes
                    .saturating_sub(self.reserved_bytes);
                state.stats.global_bytes = state.stats.global_bytes.saturating_add(committed);
            }
        }
        self.reservation_active = false;
        self.reserved_bytes = 0;
        self.bytes_read = 0;
    }
}

impl Drop for Vue3ExternalTypeSourceOwner {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let session = self.session.clone();
        let mut state = session.lock();
        let current = vue3_source_flight_matches(
            state.source_cache.get(&self.cache_key),
            self.flight.id,
        );
        // An unwind can occur after I/O but before its byte count is recorded.
        // Forfeit the whole reservation so repeated panics cannot bypass the budget.
        self.settle_reservation(&mut state, true);
        if current {
            state.source_cache.remove(&self.cache_key);
        }
        state.failure_epoch += 1;
        drop(state);
        self.flight.abort();
        self.active = false;
    }
}

impl Vue3ExternalTypeLoadSession {
    fn begin_source_load(
        &self,
        cache_key: Vue3ExternalTypeSourceCacheKey,
    ) -> Vue3ExternalTypeSourceLoad {
        let kind = cache_key.kind;
        let owner = std::thread::current().id();
        let mut state = self.lock();
        match state.source_cache.get(&cache_key).cloned() {
            Some(Vue3ExternalTypeSourceCacheEntry::Ready(source)) => {
                state.stats.source_cache_hits += 1;
                return Vue3ExternalTypeSourceLoad::Ready(source);
            }
            Some(Vue3ExternalTypeSourceCacheEntry::Failed) => {
                state.failure_epoch += 1;
                return Vue3ExternalTypeSourceLoad::Failed;
            }
            Some(Vue3ExternalTypeSourceCacheEntry::Loading(flight)) => {
                if flight.owner == owner {
                    state.failure_epoch += 1;
                    return Vue3ExternalTypeSourceLoad::Failed;
                }
                return Vue3ExternalTypeSourceLoad::Wait(Vue3ExternalTypeSourceWaiter {
                    session: self.clone(),
                    flight,
                });
            }
            None => {}
        }
        let (files_read, max_files) = match kind {
            Vue3ExternalTypeSourceKind::Import => {
                (state.stats.import_files_read, state.limits.max_import_files)
            }
            Vue3ExternalTypeSourceKind::Global => {
                (state.stats.global_files_read, state.limits.max_global_files)
            }
        };
        if files_read >= max_files {
            state.failure_epoch += 1;
            return Vue3ExternalTypeSourceLoad::Failed;
        }
        let flight_id = state.next_source_flight_id;
        let Some(next_flight_id) = flight_id.checked_add(1) else {
            state.failure_epoch += 1;
            return Vue3ExternalTypeSourceLoad::Failed;
        };
        state.next_source_flight_id = next_flight_id;
        match kind {
            Vue3ExternalTypeSourceKind::Import => state.stats.import_files_read += 1,
            Vue3ExternalTypeSourceKind::Global => state.stats.global_files_read += 1,
        }
        let flight = std::sync::Arc::new(Vue3ExternalTypeSourceFlight::new(flight_id, owner, 0));
        state.source_cache.insert(
            cache_key.clone(),
            Vue3ExternalTypeSourceCacheEntry::Loading(flight.clone()),
        );
        drop(state);
        Vue3ExternalTypeSourceLoad::Start(Vue3ExternalTypeSourceOwner {
            session: self.clone(),
            cache_key,
            kind,
            flight,
            reserved_bytes: 0,
            bytes_read: 0,
            reservation_active: false,
            active: true,
            _not_send: std::marker::PhantomData,
        })
    }
}

fn vue3_source_flight_matches(
    entry: Option<&Vue3ExternalTypeSourceCacheEntry>,
    flight_id: u64,
) -> bool {
    matches!(
        entry,
        Some(Vue3ExternalTypeSourceCacheEntry::Loading(flight)) if flight.id == flight_id
    )
}

#[cfg(test)]
mod source_single_flight_tests {
    use super::*;
    use std::time::Duration;

    fn source(value: &str) -> Vue3ExternalTypeSource {
        Vue3ExternalTypeSource {
            source: value.into(),
            source_type: oxc_span::SourceType::ts(),
            resolution_mode: Vue3TypeResolutionMode::Import,
        }
    }

    fn cache_key(
        value: &str,
        kind: Vue3ExternalTypeSourceKind,
    ) -> Vue3ExternalTypeSourceCacheKey {
        Vue3ExternalTypeSourceCacheKey {
            semantic: Vue3ExternalTypeSemanticIdentity {
                path: PathBuf::from(value),
                mode: "test".into(),
            },
            kind,
        }
    }

    #[cfg(windows)]
    #[test]
    fn external_type_cache_keys_preserve_native_windows_paths() {
        use std::os::windows::ffi::OsStringExt;

        fn path_with_surrogate(name: &str, surrogate: u16) -> PathBuf {
            let mut units = name.encode_utf16().collect::<Vec<_>>();
            units.insert(5, surrogate);
            PathBuf::from(std::ffi::OsString::from_wide(&units))
        }

        let first = path_with_surrogate("type-.ts", 0xd800);
        let uppercase = path_with_surrogate("TYPE-.TS", 0xd800);
        let second = path_with_surrogate("type-.ts", 0xd801);
        let type_resolver = Vue3TypeResolverContext::default();

        assert_eq!(first.to_string_lossy(), second.to_string_lossy());
        assert_ne!(
            vue3_external_type_path_identity(&first),
            vue3_external_type_path_identity(&second)
        );
        assert_ne!(
            vue3_external_type_context_cache_key(&first, &type_resolver),
            vue3_external_type_context_cache_key(&second, &type_resolver)
        );
        assert_ne!(
            vue3_external_type_source_cache_key(
                &first,
                Vue3ExternalTypeSourceKind::Import,
                Vue3ExternalTypeFormat {
                    source_type: oxc_span::SourceType::ts(),
                    resolution_mode: Vue3TypeResolutionMode::Import,
                },
            ),
            vue3_external_type_source_cache_key(
                &second,
                Vue3ExternalTypeSourceKind::Import,
                Vue3ExternalTypeFormat {
                    source_type: oxc_span::SourceType::ts(),
                    resolution_mode: Vue3TypeResolutionMode::Import,
                },
            )
        );
        assert_eq!(
            vue3_external_type_context_cache_key(&first, &type_resolver),
            vue3_external_type_context_cache_key(&uppercase, &type_resolver)
        );
        assert_eq!(
            vue3_external_type_source_cache_key(
                &first,
                Vue3ExternalTypeSourceKind::Import,
                Vue3ExternalTypeFormat {
                    source_type: oxc_span::SourceType::ts(),
                    resolution_mode: Vue3TypeResolutionMode::Import,
                },
            ),
            vue3_external_type_source_cache_key(
                &uppercase,
                Vue3ExternalTypeSourceKind::Import,
                Vue3ExternalTypeFormat {
                    source_type: oxc_span::SourceType::ts(),
                    resolution_mode: Vue3TypeResolutionMode::Import,
                },
            )
        );
    }

    #[test]
    fn package_scopes_separate_resolution_mode_from_declaration_scope() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let module_package = node_modules.join("module-package");
        let commonjs_package = node_modules.join("commonjs-package");
        let outer_package = node_modules.join("outer-package");
        let nested_package = outer_package.join("nested");
        let manifestless_package = node_modules.join("manifestless-package");
        std::fs::create_dir_all(&module_package).expect("create module package");
        std::fs::create_dir_all(&commonjs_package).expect("create CommonJS package");
        std::fs::create_dir_all(&nested_package).expect("create nested package");
        std::fs::create_dir_all(&manifestless_package).expect("create manifestless package");
        std::fs::write(dir.path().join("package.json"), r#"{"type":"module"}"#)
            .expect("write application manifest");
        std::fs::write(
            module_package.join("package.json"),
            r#"{"type":"module"}"#,
        )
        .expect("write module package manifest");
        std::fs::write(
            commonjs_package.join("package.json"),
            r#"{"type":"commonjs"}"#,
        )
        .expect("write CommonJS package manifest");
        std::fs::write(
            outer_package.join("package.json"),
            r#"{"type":"module"}"#,
        )
        .expect("write outer package manifest");
        std::fs::write(nested_package.join("package.json"), "{}")
            .expect("write nested package boundary");
        let session = Vue3ExternalTypeLoadSession::default();

        let module_ts = vue3_external_type_format(&module_package.join("index.ts"), &session)
            .expect("module TypeScript format");
        assert!(module_ts.source_type.is_module());
        assert_eq!(module_ts.resolution_mode, Vue3TypeResolutionMode::Import);

        let module_dts =
            vue3_external_type_format(&module_package.join("index.d.ts"), &session)
                .expect("module declaration format");
        assert!(module_dts.source_type.is_typescript_definition());
        assert!(module_dts.source_type.is_unambiguous());
        assert_eq!(module_dts.resolution_mode, Vue3TypeResolutionMode::Import);

        let commonjs_ts =
            vue3_external_type_format(&commonjs_package.join("index.ts"), &session)
                .expect("CommonJS TypeScript format");
        assert!(commonjs_ts.source_type.is_unambiguous());
        assert_eq!(
            commonjs_ts.resolution_mode,
            Vue3TypeResolutionMode::Require
        );

        let nested_ts = vue3_external_type_format(&nested_package.join("index.ts"), &session)
            .expect("nested default CommonJS format");
        assert!(nested_ts.source_type.is_unambiguous());
        assert_eq!(nested_ts.resolution_mode, Vue3TypeResolutionMode::Require);

        let explicit_commonjs =
            vue3_external_type_format(&module_package.join("index.d.cts"), &session)
                .expect("explicit CommonJS format");
        assert!(explicit_commonjs.source_type.is_typescript_definition());
        assert!(explicit_commonjs.source_type.is_commonjs());
        assert_eq!(
            explicit_commonjs.resolution_mode,
            Vue3TypeResolutionMode::Require
        );

        let local_package = dir.path().join("local-package");
        std::fs::create_dir_all(&local_package).expect("create local package");
        std::fs::write(
            local_package.join("package.json"),
            r#"{"type":"commonjs"}"#,
        )
        .expect("write local package manifest");
        let local_ts = vue3_external_type_format(&local_package.join("index.ts"), &session)
            .expect("local TypeScript format");
        assert!(local_ts.source_type.is_unambiguous());
        assert_eq!(local_ts.resolution_mode, Vue3TypeResolutionMode::Import);

        let manifestless_ts =
            vue3_external_type_format(&manifestless_package.join("index.ts"), &session)
                .expect("manifestless dependency format");
        assert!(manifestless_ts.source_type.is_unambiguous());
        assert_eq!(
            manifestless_ts.resolution_mode,
            Vue3TypeResolutionMode::Require
        );
    }

    #[test]
    fn package_scope_metadata_precedes_source_budget_and_is_cached() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("package");
        std::fs::create_dir_all(&package).expect("create package");
        std::fs::write(package.join("package.json"), r#"{"type":"module"}"#)
            .expect("write package manifest");
        std::fs::write(package.join("first.ts"), "export interface First {}")
            .expect("write first source");
        std::fs::write(package.join("second.ts"), "export interface Second {}")
            .expect("write second source");
        let session = Vue3ExternalTypeLoadSession::default();

        assert!(session
            .source_from_path(&package.join("first.ts"), Vue3ExternalTypeSourceKind::Import)
            .is_some());
        assert!(session
            .source_from_path(&package.join("second.ts"), Vue3ExternalTypeSourceKind::Import)
            .is_some());
        let stats = session.stats();
        assert_eq!(stats.metadata_files_read, 1);
        assert_eq!(stats.metadata_parse_cache_hits, 1);
        assert_eq!(stats.ancestor_search_entries, 1);
        assert_eq!(stats.import_files_read, 2);

        let blocked = Vue3ExternalTypeLoadSession::with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_files: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(blocked
            .source_from_path(&package.join("first.ts"), Vue3ExternalTypeSourceKind::Import)
            .is_none());
        assert_eq!(blocked.stats().import_files_read, 0);
        assert!(blocked.metadata_is_blocked());

        let explicit = package.join("explicit.mts");
        std::fs::write(&explicit, "export interface Explicit {}")
            .expect("write explicit source");
        let source = blocked
            .source_from_path(&explicit, Vue3ExternalTypeSourceKind::Import)
            .expect("explicit source bypasses package metadata");
        assert!(source.source_type.is_module());
        assert_eq!(source.resolution_mode, Vue3TypeResolutionMode::Import);
        assert_eq!(blocked.stats().import_files_read, 1);
    }

    #[test]
    fn package_scope_search_honors_exact_depth_and_malformed_boundaries() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("package");
        let deep = package.join("deep");
        std::fs::create_dir_all(&deep).expect("create deep package directory");
        std::fs::write(package.join("package.json"), r#"{"type":"commonjs"}"#)
            .expect("write package manifest");
        let source_path = deep.join("index.ts");
        std::fs::write(&source_path, "export interface Props {}")
            .expect("write package source");

        let exact = Vue3ExternalTypeLoadSession::with_limits(Vue3ExternalTypeLoadLimits {
            max_ancestor_search_depth: 2,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let source = exact
            .source_from_path(&source_path, Vue3ExternalTypeSourceKind::Import)
            .expect("find manifest at exact depth");
        assert_eq!(source.resolution_mode, Vue3TypeResolutionMode::Require);
        assert_eq!(exact.stats().ancestor_search_entries, 2);
        assert!(!exact.metadata_is_blocked());

        let short = Vue3ExternalTypeLoadSession::with_limits(Vue3ExternalTypeLoadLimits {
            max_ancestor_search_depth: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(short
            .source_from_path(&source_path, Vue3ExternalTypeSourceKind::Import)
            .is_none());
        assert_eq!(short.stats().ancestor_search_entries, 1);
        assert_eq!(short.stats().import_files_read, 0);
        assert!(short.metadata_is_blocked());

        let malformed = package.join("malformed");
        std::fs::create_dir_all(&malformed).expect("create malformed package boundary");
        std::fs::write(malformed.join("package.json"), "{")
            .expect("write malformed package manifest");
        let malformed_source = malformed.join("index.ts");
        std::fs::write(&malformed_source, "export interface Malformed {}")
            .expect("write malformed package source");
        let malformed_session = Vue3ExternalTypeLoadSession::default();
        assert!(malformed_session
            .source_from_path(&malformed_source, Vue3ExternalTypeSourceKind::Import)
            .is_none());
        assert_eq!(malformed_session.stats().ancestor_search_entries, 1);
        assert_eq!(malformed_session.stats().import_files_read, 0);
        assert!(malformed_session.metadata_is_blocked());

        let application = dir.path().join("application");
        let manifestless = application.join("node_modules").join("manifestless");
        std::fs::create_dir_all(&manifestless).expect("create manifestless dependency");
        std::fs::write(application.join("package.json"), r#"{"type":"module"}"#)
            .expect("write outer application manifest");
        let manifestless_source = manifestless.join("index.ts");
        std::fs::write(&manifestless_source, "interface ManifestlessGlobal {}")
            .expect("write manifestless source");
        let boundary = Vue3ExternalTypeLoadSession::with_limits(Vue3ExternalTypeLoadLimits {
            max_ancestor_search_depth: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let source = boundary
            .source_from_path(
                &manifestless_source,
                Vue3ExternalTypeSourceKind::Import,
            )
            .expect("stop before node_modules boundary");
        assert!(source.source_type.is_unambiguous());
        assert_eq!(source.resolution_mode, Vue3TypeResolutionMode::Require);
        assert_eq!(boundary.stats().ancestor_search_entries, 1);
        assert_eq!(boundary.stats().import_files_read, 1);
        assert!(!boundary.metadata_is_blocked());
    }

    #[cfg(unix)]
    #[test]
    fn source_cache_separates_symlinked_package_scope_modes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let shared = dir.path().join("shared.ts");
        std::fs::write(
            &shared,
            "import 'scope-conditional'; export interface Shared {}",
        )
        .expect("write shared source");
        let module_package = dir.path().join("node_modules").join("module-package");
        let commonjs_package = dir.path().join("node_modules").join("commonjs-package");
        std::fs::create_dir_all(&module_package).expect("create module package");
        std::fs::create_dir_all(&commonjs_package).expect("create CommonJS package");
        std::fs::write(
            module_package.join("package.json"),
            r#"{"type":"module"}"#,
        )
        .expect("write module package manifest");
        std::fs::write(
            commonjs_package.join("package.json"),
            r#"{"type":"commonjs"}"#,
        )
        .expect("write CommonJS package manifest");
        std::os::unix::fs::symlink(&shared, module_package.join("index.ts"))
            .expect("link module source");
        std::os::unix::fs::symlink(&shared, commonjs_package.join("index.ts"))
            .expect("link CommonJS source");
        let session = Vue3ExternalTypeLoadSession::default();

        let module_source = session
            .source_from_path(
                &module_package.join("index.ts"),
                Vue3ExternalTypeSourceKind::Import,
            )
            .expect("load module source");
        let commonjs_source = session
            .source_from_path(
                &commonjs_package.join("index.ts"),
                Vue3ExternalTypeSourceKind::Import,
            )
            .expect("load CommonJS source");

        assert!(module_source.source_type.is_module());
        assert_eq!(module_source.resolution_mode, Vue3TypeResolutionMode::Import);
        assert!(commonjs_source.source_type.is_unambiguous());
        assert_eq!(
            commonjs_source.resolution_mode,
            Vue3TypeResolutionMode::Require
        );
        assert!(!std::sync::Arc::ptr_eq(&module_source, &commonjs_source));
        assert_eq!(session.stats().import_files_read, 2);

        let conditional = dir.path().join("node_modules").join("scope-conditional");
        std::fs::create_dir_all(&conditional).expect("create conditional package");
        std::fs::write(
            conditional.join("package.json"),
            r#"{"exports":{".":{"types":{"import":"./import.d.mts","require":"./require.d.cts"}}}}"#,
        )
        .expect("write conditional package manifest");
        let import_entry = conditional.join("import.d.mts");
        let require_entry = conditional.join("require.d.cts");
        std::fs::write(
            &import_entry,
            "declare global { interface ImportScopeGlobal {} } export {}",
        )
        .expect("write import augmentation");
        std::fs::write(
            &require_entry,
            "declare global { interface RequireScopeGlobal {} } export {}",
        )
        .expect("write require augmentation");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let roots = [Vue3InlineModuleSource {
            filename: &filename,
            source: "import './node_modules/module-package/index'; import './node_modules/commonjs-package/index'",
            source_type: oxc_span::SourceType::ts(),
        }];
        let augmentations = vue3_reachable_global_augmentation_files(
            &filename,
            &[],
            &roots,
            &Vue3TypeResolverContext::default(),
        )
        .expect("scan both package scope modes")
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(
            augmentations,
            BTreeSet::from([import_entry, require_entry])
        );
    }

    fn start(
        session: &Vue3ExternalTypeLoadSession,
        key: &str,
        kind: Vue3ExternalTypeSourceKind,
    ) -> Vue3ExternalTypeSourceOwner {
        match session.begin_source_load(cache_key(key, kind)) {
            Vue3ExternalTypeSourceLoad::Start(owner) => owner,
            _ => panic!("expected source flight owner"),
        }
    }

    fn budget(
        session: &Vue3ExternalTypeLoadSession,
        kind: Vue3ExternalTypeSourceKind,
    ) -> (usize, usize) {
        let state = session.lock();
        match kind {
            Vue3ExternalTypeSourceKind::Import => {
                (state.stats.import_bytes, state.reserved_import_bytes)
            }
            Vue3ExternalTypeSourceKind::Global => {
                (state.stats.global_bytes, state.reserved_global_bytes)
            }
        }
    }

    #[test]
    fn source_single_flight_shares_owner_result_with_waiters_and_cache_hits() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = "shared-source";
        let mut owner = start(&session, key, Vue3ExternalTypeSourceKind::Import);
        assert!(owner.reserve_bytes(8));
        let waiter_session = session.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3ExternalTypeSourceLoad::Wait(waiter) = waiter_session.begin_source_load(
                cache_key(key, Vue3ExternalTypeSourceKind::Import),
            ) else {
                panic!("expected source flight waiter");
            };
            ready_tx.send(()).expect("signal waiter ready");
            done_tx
                .send(waiter.wait())
                .expect("signal waiter completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter registered");

        owner.record_bytes_read(8);
        let owned = owner.complete(Some(source("shared"))).expect("owner source");
        let waited = done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter completed")
            .expect("waiter source");
        waiter.join().expect("join source waiter");
        assert!(std::sync::Arc::ptr_eq(&owned, &waited));
        assert_eq!(session.stats().source_cache_hits, 1);

        let Vue3ExternalTypeSourceLoad::Ready(cached) = session.begin_source_load(
            cache_key(key, Vue3ExternalTypeSourceKind::Import),
        ) else {
            panic!("expected cached source");
        };
        assert!(std::sync::Arc::ptr_eq(&owned, &cached));
        let stats = session.stats();
        assert_eq!(stats.import_files_read, 1);
        assert_eq!(stats.import_bytes, 8);
        assert_eq!(stats.source_cache_hits, 2);
        assert_eq!(budget(&session, Vue3ExternalTypeSourceKind::Import).1, 0);
    }

    #[test]
    fn source_single_flight_rejects_same_thread_reentry() {
        let session = Vue3ExternalTypeLoadSession::default();
        let owner = start(
            &session,
            "recursive-source",
            Vue3ExternalTypeSourceKind::Import,
        );

        assert!(matches!(
            session.begin_source_load(
                cache_key(
                    "recursive-source",
                    Vue3ExternalTypeSourceKind::Import,
                ),
            ),
            Vue3ExternalTypeSourceLoad::Failed
        ));
        drop(owner);
        assert!(matches!(
            session.begin_source_load(
                cache_key(
                    "recursive-source",
                    Vue3ExternalTypeSourceKind::Import,
                ),
            ),
            Vue3ExternalTypeSourceLoad::Start(_)
        ));
    }

    #[test]
    fn source_single_flight_notifies_waiters_of_persistent_failure() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = "failed-source";
        let mut owner = start(&session, key, Vue3ExternalTypeSourceKind::Import);
        assert!(owner.reserve_bytes(4));
        let waiter_session = session.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3ExternalTypeSourceLoad::Wait(waiter) = waiter_session.begin_source_load(
                cache_key(key, Vue3ExternalTypeSourceKind::Import),
            ) else {
                panic!("expected source flight waiter");
            };
            ready_tx.send(()).expect("signal waiter ready");
            done_tx
                .send(waiter.wait())
                .expect("signal waiter completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter registered");

        owner.record_bytes_read(4);
        assert!(owner.complete(None).is_none());
        assert!(done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter completed")
            .is_none());
        waiter.join().expect("join source waiter");
        assert!(matches!(
            session.begin_source_load(
                cache_key(key, Vue3ExternalTypeSourceKind::Import),
            ),
            Vue3ExternalTypeSourceLoad::Failed
        ));
        let stats = session.stats();
        assert_eq!(stats.import_files_read, 1);
        assert_eq!(stats.import_bytes, 4);
        assert_eq!(stats.source_cache_hits, 0);
    }

    #[test]
    fn source_single_flight_owner_unwind_aborts_and_allows_retry() {
        let session = Vue3ExternalTypeLoadSession::with_limits(Vue3ExternalTypeLoadLimits {
            max_import_bytes: 16,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let key = "panicking-source";
        let mut owner = start(&session, key, Vue3ExternalTypeSourceKind::Import);
        assert!(owner.reserve_bytes(8));
        let waiter_session = session.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3ExternalTypeSourceLoad::Wait(waiter) = waiter_session.begin_source_load(
                cache_key(key, Vue3ExternalTypeSourceKind::Import),
            ) else {
                panic!("expected source flight waiter");
            };
            ready_tx.send(()).expect("signal waiter ready");
            done_tx
                .send(waiter.wait())
                .expect("signal waiter completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter registered");

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _owner = owner;
            panic!("test source owner unwind");
        }));
        assert!(unwind.is_err());
        assert!(done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter completed")
            .is_none());
        waiter.join().expect("join source waiter");
        assert_eq!(budget(&session, Vue3ExternalTypeSourceKind::Import), (8, 0));

        let mut retry = start(&session, key, Vue3ExternalTypeSourceKind::Import);
        assert!(retry.reserve_bytes(8));
        retry.record_bytes_read(8);
        assert!(retry.complete(Some(source("retry"))).is_some());
        assert_eq!(budget(&session, Vue3ExternalTypeSourceKind::Import), (16, 0));
    }

    #[test]
    fn source_single_flight_reserves_distinct_key_bytes_atomically() {
        for kind in [
            Vue3ExternalTypeSourceKind::Import,
            Vue3ExternalTypeSourceKind::Global,
        ] {
            let session = Vue3ExternalTypeLoadSession::with_limits(
                Vue3ExternalTypeLoadLimits {
                    max_file_bytes: 8,
                    max_import_bytes: 12,
                    max_global_bytes: 12,
                    ..Vue3ExternalTypeLoadLimits::default()
                },
            );
            let mut first = start(&session, "first", kind);
            assert!(first.reserve_bytes(8));
            assert_eq!(budget(&session, kind), (0, 8));

            let mut rejected = start(&session, "rejected", kind);
            assert!(!rejected.reserve_bytes(8));
            assert!(rejected.complete(None).is_none());
            assert_eq!(budget(&session, kind), (0, 8));

            first.record_bytes_read(8);
            let first_source = first
                .complete(Some(source("first")))
                .expect("first source");
            assert_eq!(budget(&session, kind), (8, 0));

            let mut exact = start(&session, "exact", kind);
            assert!(exact.reserve_bytes(4));
            exact.record_bytes_read(4);
            assert!(exact.complete(Some(source("exact"))).is_some());
            assert_eq!(budget(&session, kind), (12, 0));
            let Vue3ExternalTypeSourceLoad::Ready(cached) =
                session.begin_source_load(cache_key("first", kind))
            else {
                panic!("unrelated failure must not discard ready source");
            };
            assert!(std::sync::Arc::ptr_eq(&first_source, &cached));
        }
    }

    #[test]
    fn source_single_flight_continues_after_session_mutex_poisoning() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = "poisoned-session-source";
        let mut owner = start(&session, key, Vue3ExternalTypeSourceKind::Import);
        assert!(owner.reserve_bytes(4));
        let waiter_session = session.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3ExternalTypeSourceLoad::Wait(waiter) = waiter_session.begin_source_load(
                cache_key(key, Vue3ExternalTypeSourceKind::Import),
            ) else {
                panic!("expected source flight waiter");
            };
            ready_tx.send(()).expect("signal waiter ready");
            done_tx
                .send(waiter.wait())
                .expect("signal waiter completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter registered");

        let state = session.state.clone();
        let poisoned = std::panic::catch_unwind(move || {
            let _state = state.lock().expect("lock session state before poison");
            panic!("test session poison");
        });
        assert!(poisoned.is_err());

        owner.record_bytes_read(4);
        let owned = owner.complete(Some(source("safe"))).expect("owner source");
        let waited = done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter completed")
            .expect("waiter source");
        waiter.join().expect("join source waiter");
        assert!(std::sync::Arc::ptr_eq(&owned, &waited));
        assert_eq!(budget(&session, Vue3ExternalTypeSourceKind::Import), (4, 0));
    }

    #[test]
    fn source_reader_accepts_and_caches_a_zero_length_file() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("empty.ts");
        std::fs::write(&path, "").expect("write empty source");
        let resolver = Vue3TypeResolverContext {
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(
                Vue3ExternalTypeLoadLimits {
                    max_file_bytes: 0,
                    max_import_bytes: 0,
                    ..Vue3ExternalTypeLoadLimits::default()
                },
            ),
            ..Vue3TypeResolverContext::default()
        };

        let first = vue3_external_type_source_from_path(&path, &resolver).expect("empty source");
        let second =
            vue3_external_type_source_from_path(&path, &resolver).expect("cached empty source");

        assert!(first.source.is_empty());
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.import_files_read, 1);
        assert_eq!(stats.import_bytes, 0);
        assert_eq!(stats.source_cache_hits, 1);
        assert_eq!(
            budget(
                &resolver.external_type_session,
                Vue3ExternalTypeSourceKind::Import,
            ),
            (0, 0)
        );
    }
}
