type Vue3MetadataSourceFlight = Vue3SingleFlight<String>;

#[derive(Clone, Debug)]
enum Vue3MetadataSourceCacheEntry {
    Loading(std::sync::Arc<Vue3MetadataSourceFlight>),
    Ready(std::sync::Arc<String>),
    Missing,
}

enum Vue3MetadataSourceOutcome {
    Ready(String),
    Missing,
    Blocked,
}

enum Vue3MetadataSourceLoad {
    Ready(std::sync::Arc<String>),
    Wait(Vue3MetadataSourceWaiter),
    Start(Vue3MetadataSourceOwner),
    Missing,
    Blocked,
}

enum Vue3MetadataSourceWaitResult {
    Ready(std::sync::Arc<String>),
    Missing,
    Blocked,
}

struct Vue3MetadataSourceWaiter {
    session: Vue3ExternalTypeLoadSession,
    flight: std::sync::Arc<Vue3MetadataSourceFlight>,
}

impl Vue3MetadataSourceWaiter {
    fn wait(self) -> Vue3MetadataSourceWaitResult {
        match self.flight.wait() {
            Vue3SingleFlightOutcome::Complete(result) => {
                let mut state = self.session.lock();
                if state.metadata_blocked
                    || state.metadata_generation != self.flight.generation
                {
                    state.failure_epoch += 1;
                    return Vue3MetadataSourceWaitResult::Blocked;
                }
                state.stats.metadata_source_cache_hits += 1;
                result.map_or(
                    Vue3MetadataSourceWaitResult::Missing,
                    Vue3MetadataSourceWaitResult::Ready,
                )
            }
            Vue3SingleFlightOutcome::Aborted => {
                if !self.session.metadata_is_blocked() {
                    self.session.block_metadata();
                }
                Vue3MetadataSourceWaitResult::Blocked
            }
        }
    }
}

struct Vue3MetadataSourceOwner {
    session: Vue3ExternalTypeLoadSession,
    cache_key: PathBuf,
    flight: std::sync::Arc<Vue3MetadataSourceFlight>,
    reserved_bytes: usize,
    bytes_read: usize,
    reservation_active: bool,
    active: bool,
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Vue3MetadataSourceOwner {
    fn reserve_bytes(&mut self, bytes: usize) -> bool {
        if self.reservation_active {
            return false;
        }
        let session = self.session.clone();
        let mut state = session.lock();
        if state.metadata_blocked
            || state.metadata_generation != self.flight.generation
            || !vue3_metadata_source_flight_matches(
                state.metadata_source_cache.get(&self.cache_key),
                self.flight.id,
            )
        {
            return false;
        }
        let remaining = state
            .limits
            .max_metadata_bytes
            .saturating_sub(state.stats.metadata_bytes)
            .saturating_sub(state.reserved_metadata_bytes);
        if bytes > state.limits.max_metadata_file_bytes || bytes > remaining {
            return false;
        }
        state.reserved_metadata_bytes += bytes;
        self.reserved_bytes = bytes;
        self.reservation_active = true;
        true
    }

    fn record_bytes_read(&mut self, bytes: usize) {
        assert!(self.reservation_active, "metadata bytes must be reserved first");
        assert!(
            bytes <= self.reserved_bytes,
            "metadata read exceeded its byte reservation"
        );
        self.bytes_read = bytes;
    }

    fn complete(
        mut self,
        outcome: Vue3MetadataSourceOutcome,
    ) -> Option<std::sync::Arc<String>> {
        let (result, blocked) = match outcome {
            Vue3MetadataSourceOutcome::Ready(source)
                if self.reservation_active && self.bytes_read == self.reserved_bytes =>
            {
                (Some(std::sync::Arc::new(source)), false)
            }
            Vue3MetadataSourceOutcome::Ready(_)
            | Vue3MetadataSourceOutcome::Missing
                if self.reservation_active =>
            {
                (None, true)
            }
            Vue3MetadataSourceOutcome::Ready(_) | Vue3MetadataSourceOutcome::Blocked => {
                (None, true)
            }
            Vue3MetadataSourceOutcome::Missing => (None, false),
        };
        let session = self.session.clone();
        let mut state = session.lock();
        let current = vue3_metadata_source_flight_matches(
            state.metadata_source_cache.get(&self.cache_key),
            self.flight.id,
        );
        let stale = state.metadata_blocked
            || state.metadata_generation != self.flight.generation
            || !current;
        self.settle_reservation(&mut state, false);
        let should_abort = blocked || stale;
        let flights = if should_abort {
            vue3_block_metadata_state(&mut state)
        } else {
            match &result {
                Some(source) => {
                    state.metadata_source_cache.insert(
                        self.cache_key.clone(),
                        Vue3MetadataSourceCacheEntry::Ready(source.clone()),
                    );
                }
                None => {
                    state.metadata_source_cache.insert(
                        self.cache_key.clone(),
                        Vue3MetadataSourceCacheEntry::Missing,
                    );
                }
            }
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

    fn settle_reservation(&mut self, state: &mut Vue3ExternalTypeLoadState, forfeit: bool) {
        if !self.reservation_active {
            return;
        }
        state.reserved_metadata_bytes = state
            .reserved_metadata_bytes
            .saturating_sub(self.reserved_bytes);
        let committed = if forfeit {
            self.reserved_bytes
        } else {
            self.bytes_read
        };
        state.stats.metadata_bytes = state.stats.metadata_bytes.saturating_add(committed);
        self.reserved_bytes = 0;
        self.bytes_read = 0;
        self.reservation_active = false;
    }
}

impl Drop for Vue3MetadataSourceOwner {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let session = self.session.clone();
        let mut state = session.lock();
        self.settle_reservation(&mut state, true);
        let flights = vue3_block_metadata_state(&mut state);
        drop(state);
        vue3_abort_metadata_flights(flights);
        self.flight.abort();
        self.active = false;
    }
}

impl Vue3ExternalTypeLoadSession {
    fn begin_metadata_source_load(&self, cache_key: PathBuf) -> Vue3MetadataSourceLoad {
        let owner = std::thread::current().id();
        let mut state = self.lock();
        if state.metadata_blocked {
            state.failure_epoch += 1;
            return Vue3MetadataSourceLoad::Blocked;
        }
        match state.metadata_source_cache.get(&cache_key).cloned() {
            Some(Vue3MetadataSourceCacheEntry::Ready(source)) => {
                state.stats.metadata_source_cache_hits += 1;
                return Vue3MetadataSourceLoad::Ready(source);
            }
            Some(Vue3MetadataSourceCacheEntry::Missing) => {
                state.stats.metadata_source_cache_hits += 1;
                return Vue3MetadataSourceLoad::Missing;
            }
            Some(Vue3MetadataSourceCacheEntry::Loading(flight)) => {
                if flight.owner == owner {
                    let flights = vue3_block_metadata_state(&mut state);
                    drop(state);
                    vue3_abort_metadata_flights(flights);
                    return Vue3MetadataSourceLoad::Blocked;
                }
                return Vue3MetadataSourceLoad::Wait(Vue3MetadataSourceWaiter {
                    session: self.clone(),
                    flight,
                });
            }
            None => {}
        }
        if state.stats.metadata_files_read >= state.limits.max_metadata_files {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            return Vue3MetadataSourceLoad::Blocked;
        }
        let flight_id = state.next_metadata_flight_id;
        let Some(next_flight_id) = flight_id.checked_add(1) else {
            let flights = vue3_block_metadata_state(&mut state);
            drop(state);
            vue3_abort_metadata_flights(flights);
            return Vue3MetadataSourceLoad::Blocked;
        };
        state.next_metadata_flight_id = next_flight_id;
        state.stats.metadata_files_read += 1;
        let flight = std::sync::Arc::new(Vue3MetadataSourceFlight::new(
            flight_id,
            owner,
            state.metadata_generation,
        ));
        state.metadata_source_cache.insert(
            cache_key.clone(),
            Vue3MetadataSourceCacheEntry::Loading(flight.clone()),
        );
        drop(state);
        Vue3MetadataSourceLoad::Start(Vue3MetadataSourceOwner {
            session: self.clone(),
            cache_key,
            flight,
            reserved_bytes: 0,
            bytes_read: 0,
            reservation_active: false,
            active: true,
            _not_send: std::marker::PhantomData,
        })
    }
}

fn vue3_metadata_source_flight_matches(
    entry: Option<&Vue3MetadataSourceCacheEntry>,
    flight_id: u64,
) -> bool {
    matches!(
        entry,
        Some(Vue3MetadataSourceCacheEntry::Loading(flight)) if flight.id == flight_id
    )
}

#[derive(Default)]
struct Vue3MetadataFlightsToAbort {
    sources: Vec<std::sync::Arc<Vue3MetadataSourceFlight>>,
    tsconfigs: Vec<std::sync::Arc<Vue3SingleFlight<serde_json::Value>>>,
    tsconfig_settings: Vec<std::sync::Arc<Vue3TsconfigModuleResolutionFlight>>,
    package_jsons: Vec<std::sync::Arc<Vue3SingleFlight<Vue3PackageJsonTypeManifest>>>,
}

fn vue3_block_metadata_state(
    state: &mut Vue3ExternalTypeLoadState,
) -> Vue3MetadataFlightsToAbort {
    state.failure_epoch += 1;
    if state.metadata_blocked {
        return Vue3MetadataFlightsToAbort::default();
    }
    state.metadata_blocked = true;
    state.metadata_generation = state.metadata_generation.saturating_add(1);

    let sources = vue3_take_metadata_source_flights(&mut state.metadata_source_cache);
    let tsconfigs = vue3_take_metadata_parse_flights(&mut state.tsconfig_cache);
    let tsconfig_settings =
        vue3_take_tsconfig_module_resolution_flights(&mut state.tsconfig_module_resolution_cache);
    let package_jsons = vue3_take_metadata_parse_flights(&mut state.package_json_cache);
    Vue3MetadataFlightsToAbort {
        sources,
        tsconfigs,
        tsconfig_settings,
        package_jsons,
    }
}

fn vue3_take_tsconfig_module_resolution_flights(
    cache: &mut BTreeMap<
        Vue3TsconfigModuleResolutionCacheKey,
        Vue3TsconfigModuleResolutionCacheEntry,
    >,
) -> Vec<std::sync::Arc<Vue3TsconfigModuleResolutionFlight>> {
    let mut flights = Vec::new();
    cache.retain(|_, entry| match entry {
        Vue3TsconfigModuleResolutionCacheEntry::Loading(flight) => {
            flights.push(flight.clone());
            false
        }
        Vue3TsconfigModuleResolutionCacheEntry::Ready(_) => true,
    });
    flights
}

fn vue3_take_metadata_source_flights(
    cache: &mut BTreeMap<PathBuf, Vue3MetadataSourceCacheEntry>,
) -> Vec<std::sync::Arc<Vue3MetadataSourceFlight>> {
    let mut flights = Vec::new();
    cache.retain(|_, entry| match entry {
        Vue3MetadataSourceCacheEntry::Loading(flight) => {
            flights.push(flight.clone());
            false
        }
        Vue3MetadataSourceCacheEntry::Ready(_) | Vue3MetadataSourceCacheEntry::Missing => true,
    });
    flights
}

fn vue3_take_metadata_parse_flights<T>(
    cache: &mut BTreeMap<PathBuf, Vue3MetadataParseCacheEntry<T>>,
) -> Vec<std::sync::Arc<Vue3SingleFlight<T>>> {
    let mut flights = Vec::new();
    cache.retain(|_, entry| match entry {
        Vue3MetadataParseCacheEntry::Loading(flight) => {
            flights.push(flight.clone());
            false
        }
        Vue3MetadataParseCacheEntry::Ready(_) => true,
    });
    flights
}

fn vue3_abort_metadata_flights(flights: Vue3MetadataFlightsToAbort) {
    for flight in flights.sources {
        flight.abort();
    }
    for flight in flights.tsconfigs {
        flight.abort();
    }
    for flight in flights.tsconfig_settings {
        flight.abort();
    }
    for flight in flights.package_jsons {
        flight.abort();
    }
}

#[cfg(test)]
mod metadata_source_single_flight_tests {
    use super::*;
    use std::time::Duration;

    fn start(session: &Vue3ExternalTypeLoadSession, key: &str) -> Vue3MetadataSourceOwner {
        match session.begin_metadata_source_load(PathBuf::from(key)) {
            Vue3MetadataSourceLoad::Start(owner) => owner,
            _ => panic!("expected metadata source owner"),
        }
    }

    fn budget(session: &Vue3ExternalTypeLoadSession) -> (usize, usize) {
        let state = session.lock();
        (state.stats.metadata_bytes, state.reserved_metadata_bytes)
    }

    #[test]
    fn metadata_flight_extraction_preserves_completed_entries() {
        let owner = std::thread::current().id();
        let source_flight = std::sync::Arc::new(Vue3MetadataSourceFlight::new(1, owner, 0));
        let ready_source = std::sync::Arc::new(String::from("cached"));
        let mut source_cache = BTreeMap::from([
            (
                PathBuf::from("loading-source"),
                Vue3MetadataSourceCacheEntry::Loading(source_flight.clone()),
            ),
            (
                PathBuf::from("ready-source"),
                Vue3MetadataSourceCacheEntry::Ready(ready_source.clone()),
            ),
            (
                PathBuf::from("missing-source"),
                Vue3MetadataSourceCacheEntry::Missing,
            ),
        ]);

        let source_flights = vue3_take_metadata_source_flights(&mut source_cache);

        assert_eq!(source_flights.len(), 1);
        assert!(std::sync::Arc::ptr_eq(
            &source_flights[0],
            &source_flight
        ));
        assert!(matches!(
            source_cache.get(Path::new("ready-source")),
            Some(Vue3MetadataSourceCacheEntry::Ready(source))
                if std::sync::Arc::ptr_eq(source, &ready_source)
        ));
        assert!(matches!(
            source_cache.get(Path::new("missing-source")),
            Some(Vue3MetadataSourceCacheEntry::Missing)
        ));
        assert!(!source_cache.contains_key(Path::new("loading-source")));

        let parse_flight = std::sync::Arc::new(Vue3SingleFlight::<serde_json::Value>::new(
            2, owner, 0,
        ));
        let ready_value = std::sync::Arc::new(serde_json::json!({ "cached": true }));
        let mut parse_cache = BTreeMap::from([
            (
                PathBuf::from("loading-parse"),
                Vue3MetadataParseCacheEntry::Loading(parse_flight.clone()),
            ),
            (
                PathBuf::from("ready-parse"),
                Vue3MetadataParseCacheEntry::Ready(ready_value.clone()),
            ),
        ]);

        let parse_flights = vue3_take_metadata_parse_flights(&mut parse_cache);

        assert_eq!(parse_flights.len(), 1);
        assert!(std::sync::Arc::ptr_eq(&parse_flights[0], &parse_flight));
        assert!(matches!(
            parse_cache.get(Path::new("ready-parse")),
            Some(Vue3MetadataParseCacheEntry::Ready(value))
                if std::sync::Arc::ptr_eq(value, &ready_value)
        ));
        assert!(!parse_cache.contains_key(Path::new("loading-parse")));
    }

    #[test]
    fn metadata_source_single_flight_shares_ready_results_and_cache_hits() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = "shared-metadata";
        let mut owner = start(&session, key);
        assert!(owner.reserve_bytes(8));
        let waiter_session = session.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3MetadataSourceLoad::Wait(waiter) =
                waiter_session.begin_metadata_source_load(PathBuf::from(key))
            else {
                panic!("expected metadata source waiter");
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
        let owned = owner
            .complete(Vue3MetadataSourceOutcome::Ready("metadata".into()))
            .expect("owner metadata");
        let Vue3MetadataSourceWaitResult::Ready(waited) = done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter completed")
        else {
            panic!("expected waiter metadata");
        };
        waiter.join().expect("join metadata waiter");
        assert!(std::sync::Arc::ptr_eq(&owned, &waited));

        let Vue3MetadataSourceLoad::Ready(cached) =
            session.begin_metadata_source_load(PathBuf::from(key))
        else {
            panic!("expected cached metadata");
        };
        assert!(std::sync::Arc::ptr_eq(&owned, &cached));
        let stats = session.stats();
        assert_eq!(stats.metadata_files_read, 1);
        assert_eq!(stats.metadata_bytes, 8);
        assert_eq!(stats.metadata_source_cache_hits, 2);
        assert_eq!(budget(&session), (8, 0));
        assert!(!session.metadata_is_blocked());
    }

    #[test]
    fn metadata_source_single_flight_shares_and_caches_missing_results() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = "missing-metadata";
        let owner = start(&session, key);
        let waiter_session = session.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3MetadataSourceLoad::Wait(waiter) =
                waiter_session.begin_metadata_source_load(PathBuf::from(key))
            else {
                panic!("expected metadata source waiter");
            };
            ready_tx.send(()).expect("signal waiter ready");
            done_tx
                .send(waiter.wait())
                .expect("signal waiter completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter registered");

        assert!(owner
            .complete(Vue3MetadataSourceOutcome::Missing)
            .is_none());
        assert!(matches!(
            done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("waiter completed"),
            Vue3MetadataSourceWaitResult::Missing
        ));
        waiter.join().expect("join metadata waiter");
        assert!(matches!(
            session.begin_metadata_source_load(PathBuf::from(key)),
            Vue3MetadataSourceLoad::Missing
        ));
        let stats = session.stats();
        assert_eq!(stats.metadata_files_read, 1);
        assert_eq!(stats.metadata_bytes, 0);
        assert_eq!(stats.metadata_source_cache_hits, 2);
        assert!(!session.metadata_is_blocked());
    }

    #[test]
    fn metadata_source_budget_block_aborts_other_generation_owners() {
        let session = Vue3ExternalTypeLoadSession::with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_file_bytes: 8,
            max_metadata_bytes: 12,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let mut first = start(&session, "first-metadata");
        assert!(first.reserve_bytes(8));
        let waiter_session = session.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3MetadataSourceLoad::Wait(waiter) = waiter_session
                .begin_metadata_source_load(PathBuf::from("first-metadata"))
            else {
                panic!("expected metadata source waiter");
            };
            ready_tx.send(()).expect("signal waiter ready");
            done_tx
                .send(waiter.wait())
                .expect("signal waiter completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter registered");

        let mut rejected = start(&session, "rejected-metadata");
        assert!(!rejected.reserve_bytes(8));
        assert!(rejected
            .complete(Vue3MetadataSourceOutcome::Blocked)
            .is_none());
        assert!(matches!(
            done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("waiter completed"),
            Vue3MetadataSourceWaitResult::Blocked
        ));
        waiter.join().expect("join metadata waiter");

        first.record_bytes_read(8);
        assert!(first
            .complete(Vue3MetadataSourceOutcome::Ready("stale".into()))
            .is_none());
        assert_eq!(budget(&session), (8, 0));
        let state = session.lock();
        assert!(state.metadata_blocked);
        assert_eq!(state.metadata_generation, 1);
        assert!(!state
            .metadata_source_cache
            .contains_key(Path::new("first-metadata")));
        assert!(!state
            .metadata_source_cache
            .contains_key(Path::new("rejected-metadata")));
        assert!(!state.metadata_source_cache.values().any(
            |entry| matches!(entry, Vue3MetadataSourceCacheEntry::Loading(_))
        ));
    }

    #[test]
    fn metadata_source_waiter_rechecks_generation_after_owner_completion() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = "completed-before-block";
        let mut owner = start(&session, key);
        assert!(owner.reserve_bytes(4));
        let waiter_session = session.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3MetadataSourceLoad::Wait(waiter) =
                waiter_session.begin_metadata_source_load(PathBuf::from(key))
            else {
                panic!("expected metadata source waiter");
            };
            ready_tx.send(()).expect("signal waiter ready");
            release_rx.recv().expect("release waiter");
            done_tx
                .send(waiter.wait())
                .expect("signal waiter completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter registered");

        owner.record_bytes_read(4);
        assert!(owner
            .complete(Vue3MetadataSourceOutcome::Ready("done".into()))
            .is_some());
        session.block_metadata();
        release_tx.send(()).expect("release waiter");
        assert!(matches!(
            done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("waiter completed"),
            Vue3MetadataSourceWaitResult::Blocked
        ));
        waiter.join().expect("join metadata waiter");
    }

    #[test]
    fn metadata_source_owner_unwind_forfeits_budget_and_blocks_waiters() {
        let session = Vue3ExternalTypeLoadSession::default();
        let key = "panicking-metadata";
        let mut owner = start(&session, key);
        assert!(owner.reserve_bytes(8));
        let waiter_session = session.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3MetadataSourceLoad::Wait(waiter) =
                waiter_session.begin_metadata_source_load(PathBuf::from(key))
            else {
                panic!("expected metadata source waiter");
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
            panic!("test metadata owner unwind");
        }));
        assert!(unwind.is_err());
        assert!(matches!(
            done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("waiter completed"),
            Vue3MetadataSourceWaitResult::Blocked
        ));
        waiter.join().expect("join metadata waiter");
        assert_eq!(budget(&session), (8, 0));
        assert!(session.metadata_is_blocked());
        assert!(!session
            .lock()
            .metadata_source_cache
            .contains_key(Path::new(key)));
    }

    #[test]
    fn metadata_source_same_thread_reentry_fails_closed() {
        let session = Vue3ExternalTypeLoadSession::default();
        let owner = start(&session, "recursive-metadata");

        assert!(matches!(
            session.begin_metadata_source_load(PathBuf::from("recursive-metadata")),
            Vue3MetadataSourceLoad::Blocked
        ));
        assert!(session.metadata_is_blocked());
        drop(owner);
    }

    #[test]
    fn metadata_source_reader_accepts_and_caches_an_empty_file() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("empty.json");
        std::fs::write(&path, "").expect("write empty metadata");
        let session = Vue3ExternalTypeLoadSession::with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_file_bytes: 0,
            max_metadata_bytes: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        let first = session
            .metadata_source_from_path(&path)
            .expect("empty metadata source");
        let second = session
            .metadata_source_from_path(&path)
            .expect("cached empty metadata source");

        assert!(first.is_empty());
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        let stats = session.stats();
        assert_eq!(stats.metadata_files_read, 1);
        assert_eq!(stats.metadata_bytes, 0);
        assert_eq!(stats.metadata_source_cache_hits, 1);
        assert_eq!(budget(&session), (0, 0));
    }
}
