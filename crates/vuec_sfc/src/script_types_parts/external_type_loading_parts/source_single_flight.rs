type Vue3ExternalTypeSourceResult = Option<std::sync::Arc<Vue3ExternalTypeSource>>;
type Vue3ExternalTypeSourceFlight = Vue3SingleFlight<Vue3ExternalTypeSource>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Vue3ExternalTypeSourceKind {
    Import,
    Global,
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
    cache_key: String,
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
        cache_key: String,
        kind: Vue3ExternalTypeSourceKind,
    ) -> Vue3ExternalTypeSourceLoad {
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
        }
    }

    fn start(
        session: &Vue3ExternalTypeLoadSession,
        key: &str,
        kind: Vue3ExternalTypeSourceKind,
    ) -> Vue3ExternalTypeSourceOwner {
        match session.begin_source_load(key.into(), kind) {
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
                key.into(),
                Vue3ExternalTypeSourceKind::Import,
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
            key.into(),
            Vue3ExternalTypeSourceKind::Import,
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
                "recursive-source".into(),
                Vue3ExternalTypeSourceKind::Import,
            ),
            Vue3ExternalTypeSourceLoad::Failed
        ));
        drop(owner);
        assert!(matches!(
            session.begin_source_load(
                "recursive-source".into(),
                Vue3ExternalTypeSourceKind::Import,
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
                key.into(),
                Vue3ExternalTypeSourceKind::Import,
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
            session.begin_source_load(key.into(), Vue3ExternalTypeSourceKind::Import),
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
                key.into(),
                Vue3ExternalTypeSourceKind::Import,
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
                session.begin_source_load("first".into(), kind)
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
                key.into(),
                Vue3ExternalTypeSourceKind::Import,
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
