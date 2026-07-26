enum Vue3MetadataParseCacheEntry<T> {
    Loading(std::sync::Arc<Vue3SingleFlight<T>>),
    Ready(std::sync::Arc<T>),
}

impl<T> Clone for Vue3MetadataParseCacheEntry<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Loading(flight) => Self::Loading(flight.clone()),
            Self::Ready(value) => Self::Ready(value.clone()),
        }
    }
}

impl<T> std::fmt::Debug for Vue3MetadataParseCacheEntry<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loading(flight) => formatter.debug_tuple("Loading").field(flight).finish(),
            Self::Ready(_) => formatter.write_str("Ready(..)"),
        }
    }
}

type Vue3TsconfigCacheEntry = Vue3MetadataParseCacheEntry<serde_json::Value>;
type Vue3PackageJsonCacheEntry =
    Vue3MetadataParseCacheEntry<Vue3PackageJsonTypeManifest>;

trait Vue3MetadataKind: Sized + 'static {
    type Output: Send + Sync + 'static;

    fn cache(
        state: &Vue3ExternalTypeLoadState,
    ) -> &BTreeMap<PathBuf, Vue3MetadataParseCacheEntry<Self::Output>>;

    fn cache_mut(
        state: &mut Vue3ExternalTypeLoadState,
    ) -> &mut BTreeMap<PathBuf, Vue3MetadataParseCacheEntry<Self::Output>>;

    fn parse(source: &str) -> Option<Self::Output>;
}

struct Vue3TsconfigMetadataKind;

impl Vue3MetadataKind for Vue3TsconfigMetadataKind {
    type Output = serde_json::Value;

    fn cache(
        state: &Vue3ExternalTypeLoadState,
    ) -> &BTreeMap<PathBuf, Vue3MetadataParseCacheEntry<Self::Output>> {
        &state.tsconfig_cache
    }

    fn cache_mut(
        state: &mut Vue3ExternalTypeLoadState,
    ) -> &mut BTreeMap<PathBuf, Vue3MetadataParseCacheEntry<Self::Output>> {
        &mut state.tsconfig_cache
    }

    fn parse(source: &str) -> Option<Self::Output> {
        vue3_parse_tsconfig_jsonc(source)
    }
}

struct Vue3PackageJsonMetadataKind;

impl Vue3MetadataKind for Vue3PackageJsonMetadataKind {
    type Output = Vue3PackageJsonTypeManifest;

    fn cache(
        state: &Vue3ExternalTypeLoadState,
    ) -> &BTreeMap<PathBuf, Vue3MetadataParseCacheEntry<Self::Output>> {
        &state.package_json_cache
    }

    fn cache_mut(
        state: &mut Vue3ExternalTypeLoadState,
    ) -> &mut BTreeMap<PathBuf, Vue3MetadataParseCacheEntry<Self::Output>> {
        &mut state.package_json_cache
    }

    fn parse(source: &str) -> Option<Self::Output> {
        serde_json::from_str(source).ok()
    }
}

enum Vue3MetadataParseLoad<K: Vue3MetadataKind> {
    Ready(std::sync::Arc<K::Output>),
    Wait(Vue3MetadataParseWaiter<K>),
    Start(Vue3MetadataParseOwner<K>),
    Vacant,
    Blocked,
}

struct Vue3MetadataParseWaiter<K: Vue3MetadataKind> {
    session: Vue3ExternalTypeLoadSession,
    flight: std::sync::Arc<Vue3SingleFlight<K::Output>>,
    _kind: std::marker::PhantomData<K>,
}

impl<K: Vue3MetadataKind> Vue3MetadataParseWaiter<K> {
    fn wait(self) -> Option<std::sync::Arc<K::Output>> {
        match self.flight.wait() {
            Vue3SingleFlightOutcome::Complete(Some(value)) => {
                let mut state = self.session.lock();
                if state.metadata_blocked
                    || state.metadata_generation != self.flight.generation
                {
                    state.failure_epoch += 1;
                    return None;
                }
                state.stats.metadata_parse_cache_hits += 1;
                Some(value)
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

struct Vue3MetadataParseOwner<K: Vue3MetadataKind> {
    session: Vue3ExternalTypeLoadSession,
    cache_key: PathBuf,
    flight: std::sync::Arc<Vue3SingleFlight<K::Output>>,
    active: bool,
    _kind: std::marker::PhantomData<(K, std::rc::Rc<()>)>,
}

impl<K: Vue3MetadataKind> Vue3MetadataParseOwner<K> {
    fn complete(mut self, output: Option<K::Output>) -> Option<std::sync::Arc<K::Output>> {
        let result = output.map(std::sync::Arc::new);
        let session = self.session.clone();
        let mut state = session.lock();
        let current = vue3_metadata_parse_flight_matches::<K>(
            K::cache(&state).get(&self.cache_key),
            self.flight.id,
        );
        let stale = state.metadata_blocked
            || state.metadata_generation != self.flight.generation
            || !current;
        let should_abort = result.is_none() || stale;
        let flights = if should_abort {
            vue3_block_metadata_state(&mut state)
        } else {
            K::cache_mut(&mut state).insert(
                self.cache_key.clone(),
                Vue3MetadataParseCacheEntry::Ready(
                    result.as_ref().expect("checked parse result").clone(),
                ),
            );
            Vue3MetadataFlightsToAbort::default()
        };
        drop(state);
        vue3_abort_metadata_flights(flights);
        if should_abort {
            self.flight.abort();
        } else {
            self.flight.complete(result.clone());
        }
        self.active = false;
        if should_abort { None } else { result }
    }
}

impl<K: Vue3MetadataKind> Drop for Vue3MetadataParseOwner<K> {
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
    fn parsed_metadata_from_path<K: Vue3MetadataKind>(
        &self,
        path: &Path,
    ) -> Option<std::sync::Arc<K::Output>> {
        let cache_key = self.metadata_cache_key(path)?;
        match self.begin_metadata_parse::<K>(cache_key.clone(), false) {
            Vue3MetadataParseLoad::Ready(value) => return Some(value),
            Vue3MetadataParseLoad::Wait(waiter) => return waiter.wait(),
            Vue3MetadataParseLoad::Blocked => return None,
            Vue3MetadataParseLoad::Vacant => {}
            Vue3MetadataParseLoad::Start(_) => unreachable!("parse ownership was not requested"),
        }
        let source = self.metadata_source_from_path_with_key(path, cache_key.clone())?;
        match self.begin_metadata_parse::<K>(cache_key, true) {
            Vue3MetadataParseLoad::Ready(value) => Some(value),
            Vue3MetadataParseLoad::Wait(waiter) => waiter.wait(),
            Vue3MetadataParseLoad::Start(owner) => owner.complete(K::parse(source.as_str())),
            Vue3MetadataParseLoad::Vacant => unreachable!("parse ownership was requested"),
            Vue3MetadataParseLoad::Blocked => None,
        }
    }

    fn begin_metadata_parse<K: Vue3MetadataKind>(
        &self,
        cache_key: PathBuf,
        claim: bool,
    ) -> Vue3MetadataParseLoad<K> {
        let owner = std::thread::current().id();
        let mut state = self.lock();
        if state.metadata_blocked {
            state.failure_epoch += 1;
            return Vue3MetadataParseLoad::Blocked;
        }
        match K::cache(&state).get(&cache_key).cloned() {
            Some(Vue3MetadataParseCacheEntry::Ready(value)) => {
                state.stats.metadata_parse_cache_hits += 1;
                return Vue3MetadataParseLoad::Ready(value);
            }
            Some(Vue3MetadataParseCacheEntry::Loading(flight)) => {
                if flight.owner == owner {
                    let flights = vue3_block_metadata_state(&mut state);
                    drop(state);
                    vue3_abort_metadata_flights(flights);
                    return Vue3MetadataParseLoad::Blocked;
                }
                return Vue3MetadataParseLoad::Wait(Vue3MetadataParseWaiter {
                    session: self.clone(),
                    flight,
                    _kind: std::marker::PhantomData,
                });
            }
            None if !claim => return Vue3MetadataParseLoad::Vacant,
            None => {}
        }
        let flight_id = state.next_metadata_flight_id;
        let Some(next_flight_id) = flight_id.checked_add(1) else {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            return Vue3MetadataParseLoad::Blocked;
        };
        state.next_metadata_flight_id = next_flight_id;
        let flight = std::sync::Arc::new(Vue3SingleFlight::new(
            flight_id,
            owner,
            state.metadata_generation,
        ));
        K::cache_mut(&mut state).insert(
            cache_key.clone(),
            Vue3MetadataParseCacheEntry::Loading(flight.clone()),
        );
        drop(state);
        Vue3MetadataParseLoad::Start(Vue3MetadataParseOwner {
            session: self.clone(),
            cache_key,
            flight,
            active: true,
            _kind: std::marker::PhantomData,
        })
    }
}

fn vue3_metadata_parse_flight_matches<K: Vue3MetadataKind>(
    entry: Option<&Vue3MetadataParseCacheEntry<K::Output>>,
    flight_id: u64,
) -> bool {
    matches!(
        entry,
        Some(Vue3MetadataParseCacheEntry::Loading(flight)) if flight.id == flight_id
    )
}

#[cfg(test)]
mod metadata_parse_single_flight_tests {
    use super::*;
    use std::time::Duration;

    fn start_parse<K: Vue3MetadataKind>(
        session: &Vue3ExternalTypeLoadSession,
        key: &Path,
    ) -> Vue3MetadataParseOwner<K> {
        match session.begin_metadata_parse::<K>(key.to_path_buf(), true) {
            Vue3MetadataParseLoad::Start(owner) => owner,
            _ => panic!("expected metadata parse owner"),
        }
    }

    #[test]
    fn metadata_parse_single_flight_isolated_per_kind_and_shares_results() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = PathBuf::from("shared-metadata-parse");
        let tsconfig_owner = start_parse::<Vue3TsconfigMetadataKind>(&session, &key);
        let package_owner = start_parse::<Vue3PackageJsonMetadataKind>(&session, &key);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (tsconfig_done_tx, tsconfig_done_rx) = std::sync::mpsc::channel();
        let tsconfig_session = session.clone();
        let tsconfig_key = key.clone();
        let tsconfig_ready_tx = ready_tx.clone();
        let tsconfig_waiter = std::thread::spawn(move || {
            let Vue3MetadataParseLoad::Wait(waiter) = tsconfig_session
                .begin_metadata_parse::<Vue3TsconfigMetadataKind>(tsconfig_key, true)
            else {
                panic!("expected tsconfig parse waiter");
            };
            tsconfig_ready_tx.send(()).expect("signal tsconfig waiter");
            tsconfig_done_tx
                .send(waiter.wait())
                .expect("signal tsconfig completion");
        });
        let (package_done_tx, package_done_rx) = std::sync::mpsc::channel();
        let package_session = session.clone();
        let package_key = key.clone();
        let package_waiter = std::thread::spawn(move || {
            let Vue3MetadataParseLoad::Wait(waiter) = package_session
                .begin_metadata_parse::<Vue3PackageJsonMetadataKind>(package_key, true)
            else {
                panic!("expected package parse waiter");
            };
            ready_tx.send(()).expect("signal package waiter");
            package_done_tx
                .send(waiter.wait())
                .expect("signal package completion");
        });
        for _ in 0..2 {
            ready_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("parse waiter registered");
        }

        let owned_tsconfig = tsconfig_owner
            .complete(Vue3TsconfigMetadataKind::parse(
                r#"{"compilerOptions":{"strict":true}}"#,
            ))
            .expect("parsed tsconfig");
        let owned_package = package_owner
            .complete(Vue3PackageJsonMetadataKind::parse(
                r#"{"types":"./index.d.ts"}"#,
            ))
            .expect("parsed package manifest");
        let waited_tsconfig = tsconfig_done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("tsconfig waiter completed")
            .expect("tsconfig waiter result");
        let waited_package = package_done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("package waiter completed")
            .expect("package waiter result");
        tsconfig_waiter.join().expect("join tsconfig waiter");
        package_waiter.join().expect("join package waiter");
        assert!(std::sync::Arc::ptr_eq(
            &owned_tsconfig,
            &waited_tsconfig
        ));
        assert!(std::sync::Arc::ptr_eq(&owned_package, &waited_package));

        let Vue3MetadataParseLoad::Ready(cached_tsconfig) = session
            .begin_metadata_parse::<Vue3TsconfigMetadataKind>(key.clone(), false)
        else {
            panic!("expected cached tsconfig");
        };
        let Vue3MetadataParseLoad::Ready(cached_package) = session
            .begin_metadata_parse::<Vue3PackageJsonMetadataKind>(key.clone(), false)
        else {
            panic!("expected cached package manifest");
        };
        assert!(std::sync::Arc::ptr_eq(
            &owned_tsconfig,
            &cached_tsconfig
        ));
        assert!(std::sync::Arc::ptr_eq(&owned_package, &cached_package));
        assert_eq!(session.stats().metadata_parse_cache_hits, 4);
        let state = session.lock();
        assert!(state.tsconfig_cache.contains_key(&key));
        assert!(state.package_json_cache.contains_key(&key));
        assert!(!state.metadata_blocked);
    }

    #[test]
    fn metadata_parse_single_flight_notifies_all_registered_waiters() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = PathBuf::from("metadata-parse-with-many-waiters");
        let owner = start_parse::<Vue3TsconfigMetadataKind>(&session, &key);
        let flight = owner.flight.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let mut waiters = Vec::new();
        for _ in 0..7 {
            let waiter_session = session.clone();
            let waiter_key = key.clone();
            let waiter_ready_tx = ready_tx.clone();
            let waiter_done_tx = done_tx.clone();
            waiters.push(std::thread::spawn(move || {
                let Vue3MetadataParseLoad::Wait(waiter) = waiter_session
                    .begin_metadata_parse::<Vue3TsconfigMetadataKind>(waiter_key, true)
                else {
                    panic!("expected metadata parse waiter");
                };
                waiter_ready_tx.send(()).expect("signal parse waiter");
                waiter_done_tx
                    .send(waiter.wait())
                    .expect("signal parse completion");
            }));
        }
        for _ in 0..7 {
            ready_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("parse waiter registered");
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while flight.waiting_count() != 7 {
            assert!(
                std::time::Instant::now() < deadline,
                "parse waiters did not enter the completion cell"
            );
            std::thread::yield_now();
        }

        let owned = owner
            .complete(Vue3TsconfigMetadataKind::parse("{}"))
            .expect("parsed tsconfig");
        for _ in 0..7 {
            let waited = done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("parse waiter completed")
                .expect("parse waiter result");
            assert!(std::sync::Arc::ptr_eq(&owned, &waited));
        }
        for waiter in waiters {
            waiter.join().expect("join parse waiter");
        }
        assert_eq!(flight.waiting_count(), 0);
        assert_eq!(session.stats().metadata_parse_cache_hits, 7);
        assert!(!session.metadata_is_blocked());
    }

    #[test]
    fn metadata_block_aborts_all_flight_kinds_and_rejects_stale_owners() {
        let session = Vue3ExternalTypeLoadSession::default();
        let source_key = PathBuf::from("blocked-source");
        let tsconfig_key = PathBuf::from("blocked-tsconfig");
        let package_key = PathBuf::from("blocked-package");
        let mut source_owner = match session.begin_metadata_source_load(source_key.clone()) {
            Vue3MetadataSourceLoad::Start(owner) => owner,
            _ => panic!("expected metadata source owner"),
        };
        assert!(source_owner.reserve_bytes(4));
        let tsconfig_owner =
            start_parse::<Vue3TsconfigMetadataKind>(&session, &tsconfig_key);
        let package_owner =
            start_parse::<Vue3PackageJsonMetadataKind>(&session, &package_key);

        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (source_done_tx, source_done_rx) = std::sync::mpsc::channel();
        let source_session = session.clone();
        let source_wait_key = source_key.clone();
        let source_ready_tx = ready_tx.clone();
        let source_waiter = std::thread::spawn(move || {
            let Vue3MetadataSourceLoad::Wait(waiter) =
                source_session.begin_metadata_source_load(source_wait_key)
            else {
                panic!("expected metadata source waiter");
            };
            source_ready_tx.send(()).expect("signal source waiter");
            source_done_tx
                .send(waiter.wait())
                .expect("signal source completion");
        });
        let (tsconfig_done_tx, tsconfig_done_rx) = std::sync::mpsc::channel();
        let tsconfig_session = session.clone();
        let tsconfig_wait_key = tsconfig_key.clone();
        let tsconfig_ready_tx = ready_tx.clone();
        let tsconfig_waiter = std::thread::spawn(move || {
            let Vue3MetadataParseLoad::Wait(waiter) = tsconfig_session
                .begin_metadata_parse::<Vue3TsconfigMetadataKind>(tsconfig_wait_key, true)
            else {
                panic!("expected tsconfig parse waiter");
            };
            tsconfig_ready_tx.send(()).expect("signal tsconfig waiter");
            tsconfig_done_tx
                .send(waiter.wait())
                .expect("signal tsconfig completion");
        });
        let (package_done_tx, package_done_rx) = std::sync::mpsc::channel();
        let package_session = session.clone();
        let package_wait_key = package_key.clone();
        let package_waiter = std::thread::spawn(move || {
            let Vue3MetadataParseLoad::Wait(waiter) = package_session
                .begin_metadata_parse::<Vue3PackageJsonMetadataKind>(package_wait_key, true)
            else {
                panic!("expected package parse waiter");
            };
            ready_tx.send(()).expect("signal package waiter");
            package_done_tx
                .send(waiter.wait())
                .expect("signal package completion");
        });
        for _ in 0..3 {
            ready_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("metadata waiter registered");
        }

        session.block_metadata();
        assert!(matches!(
            source_done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("source waiter completed"),
            Vue3MetadataSourceWaitResult::Blocked
        ));
        assert!(tsconfig_done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("tsconfig waiter completed")
            .is_none());
        assert!(package_done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("package waiter completed")
            .is_none());
        source_waiter.join().expect("join source waiter");
        tsconfig_waiter.join().expect("join tsconfig waiter");
        package_waiter.join().expect("join package waiter");

        source_owner.record_bytes_read(4);
        assert!(source_owner
            .complete(Vue3MetadataSourceOutcome::Ready("late".into()))
            .is_none());
        assert!(tsconfig_owner
            .complete(Vue3TsconfigMetadataKind::parse("{}"))
            .is_none());
        assert!(package_owner
            .complete(Vue3PackageJsonMetadataKind::parse("{}"))
            .is_none());
        let state = session.lock();
        assert_eq!(state.metadata_generation, 1);
        assert_eq!(state.reserved_metadata_bytes, 0);
        assert!(!state.metadata_source_cache.contains_key(&source_key));
        assert!(!state.tsconfig_cache.contains_key(&tsconfig_key));
        assert!(!state.package_json_cache.contains_key(&package_key));
    }

    #[test]
    fn metadata_parse_waiter_rechecks_generation_after_owner_completion() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = PathBuf::from("completed-parse-before-block");
        let owner = start_parse::<Vue3TsconfigMetadataKind>(&session, &key);
        let waiter_session = session.clone();
        let waiter_key = key.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3MetadataParseLoad::Wait(waiter) = waiter_session
                .begin_metadata_parse::<Vue3TsconfigMetadataKind>(waiter_key, true)
            else {
                panic!("expected metadata parse waiter");
            };
            ready_tx.send(()).expect("signal parse waiter");
            release_rx.recv().expect("release parse waiter");
            done_tx
                .send(waiter.wait())
                .expect("signal parse completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("parse waiter registered");

        assert!(owner
            .complete(Vue3TsconfigMetadataKind::parse("{}"))
            .is_some());
        session.block_metadata();
        release_tx.send(()).expect("release parse waiter");
        assert!(done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("parse waiter completed")
            .is_none());
        waiter.join().expect("join parse waiter");
        let state = session.lock();
        assert_eq!(state.metadata_generation, 1);
        assert_eq!(state.stats.metadata_parse_cache_hits, 0);
    }

    #[test]
    fn metadata_parse_failure_aborts_other_kind_and_rejects_its_owner() {
        let session = Vue3ExternalTypeLoadSession::default();
        let malformed_key = PathBuf::from("malformed-tsconfig");
        let package_key = PathBuf::from("package-blocked-by-malformed-tsconfig");
        let malformed_owner =
            start_parse::<Vue3TsconfigMetadataKind>(&session, &malformed_key);
        let package_owner =
            start_parse::<Vue3PackageJsonMetadataKind>(&session, &package_key);
        let waiter_session = session.clone();
        let waiter_key = package_key.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3MetadataParseLoad::Wait(waiter) = waiter_session
                .begin_metadata_parse::<Vue3PackageJsonMetadataKind>(waiter_key, true)
            else {
                panic!("expected package parse waiter");
            };
            ready_tx.send(()).expect("signal package waiter");
            done_tx
                .send(waiter.wait())
                .expect("signal package completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("package waiter registered");

        assert!(malformed_owner
            .complete(Vue3TsconfigMetadataKind::parse("{"))
            .is_none());
        assert!(done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("package waiter completed")
            .is_none());
        waiter.join().expect("join package waiter");
        assert!(package_owner
            .complete(Vue3PackageJsonMetadataKind::parse(
                r#"{"types":"./late.d.ts"}"#,
            ))
            .is_none());
        let state = session.lock();
        assert!(state.metadata_blocked);
        assert_eq!(state.metadata_generation, 1);
        assert!(!state.tsconfig_cache.contains_key(&malformed_key));
        assert!(!state.package_json_cache.contains_key(&package_key));
    }

    #[test]
    fn metadata_parse_owner_unwind_blocks_waiters_and_clears_loading_entry() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = PathBuf::from("panicking-package-parse");
        let owner = start_parse::<Vue3PackageJsonMetadataKind>(&session, &key);
        let waiter_session = session.clone();
        let waiter_key = key.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3MetadataParseLoad::Wait(waiter) = waiter_session
                .begin_metadata_parse::<Vue3PackageJsonMetadataKind>(waiter_key, true)
            else {
                panic!("expected package parse waiter");
            };
            ready_tx.send(()).expect("signal package waiter");
            done_tx
                .send(waiter.wait())
                .expect("signal package completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("package waiter registered");

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _owner = owner;
            panic!("test metadata parse owner unwind");
        }));
        assert!(unwind.is_err());
        assert!(done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("package waiter completed")
            .is_none());
        waiter.join().expect("join package waiter");
        let state = session.lock();
        assert!(state.metadata_blocked);
        assert_eq!(state.metadata_generation, 1);
        assert!(!state.package_json_cache.contains_key(&key));
    }

    #[test]
    fn metadata_parse_same_thread_reentry_fails_closed() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = PathBuf::from("recursive-tsconfig-parse");
        let owner = start_parse::<Vue3TsconfigMetadataKind>(&session, &key);

        assert!(matches!(
            session.begin_metadata_parse::<Vue3TsconfigMetadataKind>(key.clone(), true),
            Vue3MetadataParseLoad::Blocked
        ));
        drop(owner);
        let state = session.lock();
        assert!(state.metadata_blocked);
        assert_eq!(state.metadata_generation, 1);
        assert!(!state.tsconfig_cache.contains_key(&key));
    }

    #[test]
    fn package_metadata_loading_is_single_flight_across_threads() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("package.json");
        let source = r#"{"types":"./index.d.ts"}"#;
        std::fs::write(&path, source).expect("write package manifest");
        let session = Vue3ExternalTypeLoadSession::default();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(9));
        let mut workers = Vec::new();
        for _ in 0..8 {
            let worker_session = session.clone();
            let worker_path = path.clone();
            let worker_barrier = barrier.clone();
            workers.push(std::thread::spawn(move || {
                worker_barrier.wait();
                worker_session
                    .package_json_from_path(&worker_path)
                    .expect("package manifest")
            }));
        }
        barrier.wait();
        let manifests = workers
            .into_iter()
            .map(|worker| worker.join().expect("join package metadata worker"))
            .collect::<Vec<_>>();

        for manifest in &manifests[1..] {
            assert!(std::sync::Arc::ptr_eq(&manifests[0], manifest));
        }
        assert_eq!(
            manifests[0].types.as_ref().and_then(serde_json::Value::as_str),
            Some("./index.d.ts")
        );
        let stats = session.stats();
        assert_eq!(stats.metadata_files_read, 1);
        assert_eq!(stats.metadata_bytes, source.len());
        assert_eq!(stats.metadata_parse_cache_hits, 7);
        assert!(!session.metadata_is_blocked());
    }

    #[test]
    fn package_module_type_values_are_strict_and_non_fatal() {
        for (value, expected) in [
            (r#""module""#, Vue3PackageModuleType::Module),
            (r#""commonjs""#, Vue3PackageModuleType::CommonJs),
            (r#""MODULE""#, Vue3PackageModuleType::CommonJs),
            (r#""invalid""#, Vue3PackageModuleType::CommonJs),
            ("true", Vue3PackageModuleType::CommonJs),
            ("null", Vue3PackageModuleType::CommonJs),
            (r#"{"nested":true}"#, Vue3PackageModuleType::CommonJs),
        ] {
            let source = format!(r#"{{"type":{value},"types":"index.d.ts"}}"#);
            let manifest = serde_json::from_str::<Vue3PackageJsonTypeManifest>(&source)
                .expect("parse package manifest");
            assert_eq!(manifest.module_type, expected, "{value}");
            assert_eq!(
                manifest.types.as_ref().and_then(serde_json::Value::as_str),
                Some("index.d.ts"),
                "{value}"
            );
        }
    }

    #[test]
    fn package_name_values_are_strict_and_non_fatal() {
        for (value, expected) in [
            (r#""vuec-package""#, Some("vuec-package")),
            (r#""@vuec/package""#, Some("@vuec/package")),
            ("true", None),
            ("null", None),
            (r#"["vuec-package"]"#, None),
            (r#"{"nested":"vuec-package"}"#, None),
        ] {
            let source = format!(r#"{{"name":{value},"types":"index.d.ts"}}"#);
            let manifest = serde_json::from_str::<Vue3PackageJsonTypeManifest>(&source)
                .expect("parse package manifest");
            assert_eq!(manifest.name.as_deref(), expected, "{value}");
            assert_eq!(
                manifest.types.as_ref().and_then(serde_json::Value::as_str),
                Some("index.d.ts"),
                "{value}"
            );
        }
    }
}
