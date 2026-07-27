type Vue3ExternalTypeContextResult = Option<std::sync::Arc<Vue27TypeContext>>;

struct Vue3ExternalTypeContextFlightValue {
    context: std::sync::Arc<Vue27TypeContext>,
    failure_free: bool,
}

type Vue3ExternalTypeContextFlight = Vue3SingleFlight<Vue3ExternalTypeContextFlightValue>;

#[derive(Clone, Copy, Debug)]
struct Vue3ExternalTypeContextWaitEdge {
    owner: std::thread::ThreadId,
    flight_id: u64,
}

enum Vue3ExternalTypeContextLoad {
    Ready(std::sync::Arc<Vue27TypeContext>),
    Wait(Vue3ExternalTypeContextWaiter),
    Start(Vue3ExternalTypeContextOwner),
    Failed,
}

struct Vue3ExternalTypeContextWaiter {
    session: Vue3ExternalTypeLoadSession,
    flight: std::sync::Arc<Vue3ExternalTypeContextFlight>,
    waiter: std::thread::ThreadId,
    observed_failure_epoch: usize,
    active: bool,
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Vue3ExternalTypeContextWaiter {
    fn wait(mut self) -> Vue3ExternalTypeContextResult {
        let outcome = self.flight.wait();
        let mut state = self.session.lock();
        vue3_remove_context_wait_edge(&mut state, self.waiter, self.flight.id);
        let result = match outcome {
            Vue3SingleFlightOutcome::Complete(Some(value)) => {
                if !value.failure_free && state.failure_epoch == self.observed_failure_epoch {
                    state.failure_epoch += 1;
                }
                state.stats.context_cache_hits += 1;
                Some(value.context.clone())
            }
            Vue3SingleFlightOutcome::Complete(None) | Vue3SingleFlightOutcome::Aborted => None,
        };
        drop(state);
        self.active = false;
        result
    }
}

impl Drop for Vue3ExternalTypeContextWaiter {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut state = self.session.lock();
        vue3_remove_context_wait_edge(&mut state, self.waiter, self.flight.id);
        self.active = false;
    }
}

struct Vue3ExternalTypeContextOwner {
    session: Vue3ExternalTypeLoadSession,
    cache_key: Vue3ExternalTypeContextCacheKey,
    flight: std::sync::Arc<Vue3ExternalTypeContextFlight>,
    failure_epoch: usize,
    active: bool,
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Vue3ExternalTypeContextOwner {
    fn reserve_build_weight(&mut self, weight: usize) -> bool {
        let session = self.session.clone();
        let mut state = session.lock();
        if !vue3_context_flight_matches(
            state.context_cache.get(&self.cache_key),
            self.flight.id,
        ) {
            vue3_remove_context_wait_edges_for_flight(&mut state, self.flight.id);
            state.failure_epoch += 1;
            drop(state);
            self.flight.abort();
            self.active = false;
            return false;
        }
        let remaining = state
            .limits
            .max_context_build_weight
            .saturating_sub(state.stats.context_build_weight);
        if weight <= remaining {
            state.stats.context_build_weight += weight;
            return true;
        }
        state.stats.context_build_weight = state.limits.max_context_build_weight;
        state.context_cache.remove(&self.cache_key);
        vue3_remove_context_wait_edges_for_flight(&mut state, self.flight.id);
        state.failure_epoch += 1;
        drop(state);
        self.flight.complete(None);
        self.active = false;
        false
    }

    fn complete(mut self, context: Option<Vue27TypeContext>) -> Vue3ExternalTypeContextResult {
        let context_weight = context
            .as_ref()
            .map(vue3_external_type_context_cache_cost);
        let cache_entry_weight = context_weight.map(|context_weight| {
            self.cache_key
                .payload_weight()
                .saturating_add(context_weight)
        });
        let result = context.map(std::sync::Arc::new);
        let session = self.session.clone();
        let mut state = session.lock();
        let current = vue3_context_flight_matches(
            state.context_cache.get(&self.cache_key),
            self.flight.id,
        );
        let remaining = state
            .limits
            .max_context_build_weight
            .saturating_sub(state.stats.context_build_weight);
        let weight_exceeded = context_weight.is_some_and(|weight| weight > remaining);
        if weight_exceeded {
            state.stats.context_build_weight = state.limits.max_context_build_weight;
        } else {
            state.stats.context_build_weight += context_weight.unwrap_or_default();
        }
        if !current {
            vue3_remove_context_wait_edges_for_flight(&mut state, self.flight.id);
            state.failure_epoch += 1;
            drop(state);
            self.flight.abort();
            self.active = false;
            return None;
        }
        if weight_exceeded || result.is_none() {
            state.context_cache.remove(&self.cache_key);
            vue3_remove_context_wait_edges_for_flight(&mut state, self.flight.id);
            state.failure_epoch += 1;
            drop(state);
            self.flight.complete(None);
            self.active = false;
            return None;
        }
        let failure_free = state.failure_epoch == self.failure_epoch;
        let cacheable = failure_free
            && cache_entry_weight.is_some_and(|weight| {
                weight <= state.limits.max_context_cache_entry_weight
                    && state.stats.cached_context_weight.saturating_add(weight)
                        <= state.limits.max_context_cache_weight
            });
        let context = result.as_ref().expect("checked context result").clone();
        if cacheable {
            state.stats.cached_context_weight += cache_entry_weight.unwrap_or_default();
            state.context_cache.insert(
                self.cache_key.clone(),
                Vue3ExternalTypeContextCacheEntry::Ready(context.clone()),
            );
        } else {
            state.context_cache.remove(&self.cache_key);
        }
        vue3_remove_context_wait_edges_for_flight(&mut state, self.flight.id);
        drop(state);
        self.flight
            .complete(Some(std::sync::Arc::new(Vue3ExternalTypeContextFlightValue {
                context,
                failure_free,
            })));
        self.active = false;
        result
    }
}

impl Drop for Vue3ExternalTypeContextOwner {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let session = self.session.clone();
        let mut state = session.lock();
        if vue3_context_flight_matches(
            state.context_cache.get(&self.cache_key),
            self.flight.id,
        ) {
            state.context_cache.remove(&self.cache_key);
        }
        vue3_remove_context_wait_edges_for_flight(&mut state, self.flight.id);
        state.failure_epoch += 1;
        drop(state);
        self.flight.abort();
        self.active = false;
    }
}

impl Vue3ExternalTypeLoadSession {
    fn begin_context_load(
        &self,
        cache_key: &Vue3ExternalTypeContextCacheKey,
    ) -> Vue3ExternalTypeContextLoad {
        let thread = std::thread::current().id();
        let mut state = self.lock();
        if state.stats.context_lookups >= state.limits.max_context_lookups {
            state.failure_epoch += 1;
            return Vue3ExternalTypeContextLoad::Failed;
        }
        state.stats.context_lookups += 1;
        match state.context_cache.get(cache_key).cloned() {
            Some(Vue3ExternalTypeContextCacheEntry::Ready(context)) => {
                state.stats.context_cache_hits += 1;
                return Vue3ExternalTypeContextLoad::Ready(context);
            }
            Some(Vue3ExternalTypeContextCacheEntry::Loading(flight)) => {
                if flight.owner == thread
                    || state.context_waits.contains_key(&thread)
                    || vue3_context_wait_would_cycle(&state, thread, flight.owner)
                {
                    state.failure_epoch += 1;
                    return Vue3ExternalTypeContextLoad::Failed;
                }
                let observed_failure_epoch = state.failure_epoch;
                state.context_waits.insert(
                    thread,
                    Vue3ExternalTypeContextWaitEdge {
                        owner: flight.owner,
                        flight_id: flight.id,
                    },
                );
                return Vue3ExternalTypeContextLoad::Wait(Vue3ExternalTypeContextWaiter {
                    session: self.clone(),
                    flight,
                    waiter: thread,
                    observed_failure_epoch,
                    active: true,
                    _not_send: std::marker::PhantomData,
                });
            }
            None => {}
        }
        if state.stats.context_builds >= state.limits.max_context_builds {
            state.failure_epoch += 1;
            return Vue3ExternalTypeContextLoad::Failed;
        }
        let key_weight = cache_key.payload_weight();
        let remaining = state
            .limits
            .max_context_build_weight
            .saturating_sub(state.stats.context_build_weight);
        if key_weight > remaining {
            state.stats.context_build_weight = state.limits.max_context_build_weight;
            state.failure_epoch += 1;
            return Vue3ExternalTypeContextLoad::Failed;
        }
        let flight_id = state.next_context_flight_id;
        let Some(next_flight_id) = flight_id.checked_add(1) else {
            state.failure_epoch += 1;
            return Vue3ExternalTypeContextLoad::Failed;
        };
        state.next_context_flight_id = next_flight_id;
        state.stats.context_builds += 1;
        state.stats.context_build_weight += key_weight;
        let failure_epoch = state.failure_epoch;
        let flight = std::sync::Arc::new(Vue3ExternalTypeContextFlight::new(
            flight_id, thread, 0,
        ));
        state.context_cache.insert(
            cache_key.clone(),
            Vue3ExternalTypeContextCacheEntry::Loading(flight.clone()),
        );
        drop(state);
        Vue3ExternalTypeContextLoad::Start(Vue3ExternalTypeContextOwner {
            session: self.clone(),
            cache_key: cache_key.clone(),
            flight,
            failure_epoch,
            active: true,
            _not_send: std::marker::PhantomData,
        })
    }
}

fn vue3_context_flight_matches(
    entry: Option<&Vue3ExternalTypeContextCacheEntry>,
    flight_id: u64,
) -> bool {
    matches!(
        entry,
        Some(Vue3ExternalTypeContextCacheEntry::Loading(flight)) if flight.id == flight_id
    )
}

fn vue3_context_wait_would_cycle(
    state: &Vue3ExternalTypeLoadState,
    waiter: std::thread::ThreadId,
    mut owner: std::thread::ThreadId,
) -> bool {
    for _ in 0..=state.context_waits.len() {
        if owner == waiter {
            return true;
        }
        let Some(edge) = state.context_waits.get(&owner) else {
            return false;
        };
        owner = edge.owner;
    }
    true
}

fn vue3_remove_context_wait_edge(
    state: &mut Vue3ExternalTypeLoadState,
    waiter: std::thread::ThreadId,
    flight_id: u64,
) {
    if state
        .context_waits
        .get(&waiter)
        .is_some_and(|edge| edge.flight_id == flight_id)
    {
        state.context_waits.remove(&waiter);
    }
}

fn vue3_remove_context_wait_edges_for_flight(
    state: &mut Vue3ExternalTypeLoadState,
    flight_id: u64,
) {
    state
        .context_waits
        .retain(|_, edge| edge.flight_id != flight_id);
}

#[cfg(test)]
mod context_single_flight_tests {
    use super::*;
    use std::time::Duration;

    fn key(value: &str) -> Vue3ExternalTypeContextCacheKey {
        Vue3ExternalTypeContextCacheKey {
            path: PathBuf::from(value),
            resolver: Vue3TypeResolverCacheIdentity {
                typescript_version: "5.0.0".into(),
                module_resolution: Vue3TypeModuleResolutionKind::Node10,
                module: Vue3TypeModuleKind::CommonJs,
                package_json_features: Vue3PackageJsonResolutionFeatures::default(),
                type_reference_package_json_features:
                    Vue3PackageJsonResolutionFeatures::default(),
                module_suffixes: vue3_default_module_suffixes(),
            },
        }
    }

    fn context(marker: &str) -> Vue27TypeContext {
        let mut context = Vue27TypeContext::default();
        context
            .declared_types
            .insert(marker.into(), vec!["string".into()]);
        context
    }

    fn start(
        session: &Vue3ExternalTypeLoadSession,
        cache_key: &Vue3ExternalTypeContextCacheKey,
    ) -> Vue3ExternalTypeContextOwner {
        match session.begin_context_load(cache_key) {
            Vue3ExternalTypeContextLoad::Start(owner) => owner,
            _ => panic!("expected context flight owner"),
        }
    }

    fn wait_until_registered(flight: &Vue3ExternalTypeContextFlight) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while flight.waiting_count() == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "context waiter did not enter the completion cell"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    fn context_single_flight_shares_owner_result_with_waiters_and_cache_hits() {
        let session = Vue3ExternalTypeLoadSession::default();
        let cache_key = key("shared-context");
        let mut owner = start(&session, &cache_key);
        assert!(owner.reserve_build_weight(8));
        let flight = owner.flight.clone();
        let waiter_session = session.clone();
        let waiter_key = cache_key.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3ExternalTypeContextLoad::Wait(waiter) =
                waiter_session.begin_context_load(&waiter_key)
            else {
                panic!("expected context flight waiter");
            };
            ready_tx.send(()).expect("signal context waiter");
            done_tx
                .send(waiter.wait())
                .expect("signal context completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("context waiter registered");
        wait_until_registered(&flight);

        let owned = owner
            .complete(Some(context("Shared")))
            .expect("owner context");
        let waited = done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("context waiter completed")
            .expect("waiter context");
        waiter.join().expect("join context waiter");
        assert!(std::sync::Arc::ptr_eq(&owned, &waited));

        let Vue3ExternalTypeContextLoad::Ready(cached) =
            session.begin_context_load(&cache_key)
        else {
            panic!("expected cached context");
        };
        assert!(std::sync::Arc::ptr_eq(&owned, &cached));
        let stats = session.stats();
        assert_eq!(stats.context_lookups, 3);
        assert_eq!(stats.context_builds, 1);
        assert_eq!(stats.context_cache_hits, 2);
        assert_eq!(flight.waiting_count(), 0);
    }

    #[test]
    fn context_single_flight_same_thread_reentry_fails_without_caching_parent() {
        let session = Vue3ExternalTypeLoadSession::default();
        let cache_key = key("recursive-context");
        let owner = start(&session, &cache_key);

        assert!(matches!(
            session.begin_context_load(&cache_key),
            Vue3ExternalTypeContextLoad::Failed
        ));
        assert!(owner.complete(Some(context("Partial"))).is_some());
        assert!(!session.lock().context_cache.contains_key(&cache_key));

        let retry = start(&session, &cache_key);
        drop(retry);
        assert!(!session.lock().context_cache.contains_key(&cache_key));
    }

    #[test]
    fn context_single_flight_owner_unwind_wakes_waiter_and_allows_retry() {
        let session = Vue3ExternalTypeLoadSession::default();
        let cache_key = key("panicking-context");
        let mut owner = start(&session, &cache_key);
        assert!(owner.reserve_build_weight(8));
        let flight = owner.flight.clone();
        let waiter_session = session.clone();
        let waiter_key = cache_key.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3ExternalTypeContextLoad::Wait(waiter) =
                waiter_session.begin_context_load(&waiter_key)
            else {
                panic!("expected context flight waiter");
            };
            ready_tx.send(()).expect("signal context waiter");
            done_tx
                .send(waiter.wait())
                .expect("signal context completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("context waiter registered");
        wait_until_registered(&flight);

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _owner = owner;
            panic!("test context owner unwind");
        }));
        assert!(unwind.is_err());
        assert!(done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("context waiter completed")
            .is_none());
        waiter.join().expect("join context waiter");
        assert!(!session.lock().context_cache.contains_key(&cache_key));

        let retry = start(&session, &cache_key);
        let retried = retry
            .complete(Some(context("Retry")))
            .expect("retry context");
        let Vue3ExternalTypeContextLoad::Ready(cached) =
            session.begin_context_load(&cache_key)
        else {
            panic!("expected cached retry context");
        };
        assert!(std::sync::Arc::ptr_eq(&retried, &cached));
    }

    #[test]
    fn context_single_flight_budget_failure_wakes_waiter_and_clears_loading() {
        let cache_key = key("bounded-context");
        let source_budget = 4;
        let session = Vue3ExternalTypeLoadSession::with_limits(Vue3ExternalTypeLoadLimits {
            max_context_build_weight: cache_key.payload_weight() + source_budget,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let mut owner = start(&session, &cache_key);
        let flight = owner.flight.clone();
        let waiter_session = session.clone();
        let waiter_key = cache_key.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3ExternalTypeContextLoad::Wait(waiter) =
                waiter_session.begin_context_load(&waiter_key)
            else {
                panic!("expected context flight waiter");
            };
            ready_tx.send(()).expect("signal context waiter");
            done_tx
                .send(waiter.wait())
                .expect("signal context completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("context waiter registered");
        wait_until_registered(&flight);

        assert!(!owner.reserve_build_weight(source_budget + 1));
        assert!(done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("context waiter completed")
            .is_none());
        waiter.join().expect("join context waiter");
        let state = session.lock();
        assert_eq!(
            state.stats.context_build_weight,
            state.limits.max_context_build_weight
        );
        assert_eq!(state.failure_epoch, 1);
        assert!(state.context_waits.is_empty());
        assert!(!state.context_cache.contains_key(&cache_key));
    }

    #[test]
    fn context_single_flight_shares_uncached_result_after_epoch_change() {
        let session = Vue3ExternalTypeLoadSession::default();
        let cache_key = key("uncached-shared-context");
        let owner = start(&session, &cache_key);
        let flight = owner.flight.clone();
        let waiter_session = session.clone();
        let waiter_key = cache_key.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let Vue3ExternalTypeContextLoad::Wait(waiter) =
                waiter_session.begin_context_load(&waiter_key)
            else {
                panic!("expected context flight waiter");
            };
            ready_tx.send(()).expect("signal context waiter");
            done_tx
                .send(waiter.wait())
                .expect("signal context completion");
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("context waiter registered");
        wait_until_registered(&flight);

        session.record_context_failure();
        let owned = owner
            .complete(Some(context("Uncached")))
            .expect("owner context");
        let waited = done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("context waiter completed")
            .expect("waiter context");
        waiter.join().expect("join context waiter");
        assert!(std::sync::Arc::ptr_eq(&owned, &waited));
        let state = session.lock();
        assert_eq!(state.failure_epoch, 1);
        assert!(state.context_waits.is_empty());
        assert!(!state.context_cache.contains_key(&cache_key));
    }

    #[test]
    fn context_single_flight_dirty_late_waiter_invalidates_parent() {
        let session = Vue3ExternalTypeLoadSession::default();
        let child_key = key("dirty-child");
        let parent_key = key("late-parent");
        let child_session = session.clone();
        let child_thread_key = child_key.clone();
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(0);
        let (complete_tx, complete_rx) = std::sync::mpsc::sync_channel(0);
        let (child_done_tx, child_done_rx) = std::sync::mpsc::channel();
        let child_thread = std::thread::spawn(move || {
            let owner = start(&child_session, &child_thread_key);
            started_tx.send(()).expect("signal child owner");
            complete_rx.recv().expect("release child owner");
            child_done_tx
                .send(owner.complete(Some(context("Child"))))
                .expect("signal child completion");
        });
        started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("child owner started");

        session.record_context_failure();
        assert_eq!(session.lock().failure_epoch, 1);
        let parent_owner = start(&session, &parent_key);
        let Vue3ExternalTypeContextLoad::Wait(child_waiter) =
            session.begin_context_load(&child_key)
        else {
            panic!("expected dirty child waiter");
        };
        complete_tx.send(()).expect("release child owner");
        let waited_child = child_waiter.wait().expect("dirty child context");
        let owned_child = child_done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("child owner completed")
            .expect("child owner context");
        child_thread.join().expect("join child owner");
        assert!(std::sync::Arc::ptr_eq(&owned_child, &waited_child));
        assert_eq!(session.lock().failure_epoch, 2);

        assert!(parent_owner
            .complete(Some(context("Parent")))
            .is_some());
        let state = session.lock();
        assert_eq!(state.failure_epoch, 2);
        assert_eq!(state.stats.context_lookups, 3);
        assert_eq!(state.stats.context_builds, 2);
        assert_eq!(state.stats.context_cache_hits, 1);
        assert!(state.context_waits.is_empty());
        assert!(!state.context_cache.contains_key(&child_key));
        assert!(!state.context_cache.contains_key(&parent_key));
    }

    #[test]
    fn context_single_flight_cross_thread_wait_cycle_fails_closed() {
        let session = Vue3ExternalTypeLoadSession::default();
        let first_key = key("cycle-first");
        let second_key = key("cycle-second");
        let (first_started_tx, first_started_rx) = std::sync::mpsc::sync_channel(0);
        let (second_started_tx, second_started_rx) = std::sync::mpsc::sync_channel(0);
        let (first_waiting_tx, first_waiting_rx) = std::sync::mpsc::sync_channel(0);
        let (done_tx, done_rx) = std::sync::mpsc::channel();

        let first_session = session.clone();
        let first_owner_key = first_key.clone();
        let first_wait_key = second_key.clone();
        let first_done_tx = done_tx.clone();
        let first_thread = std::thread::spawn(move || {
            let owner = start(&first_session, &first_owner_key);
            first_started_tx.send(()).expect("signal first owner");
            second_started_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("second owner started");
            let Vue3ExternalTypeContextLoad::Wait(waiter) =
                first_session.begin_context_load(&first_wait_key)
            else {
                panic!("expected first wait edge");
            };
            first_waiting_tx.send(()).expect("signal first wait edge");
            assert!(waiter.wait().is_some());
            assert!(owner.complete(Some(context("First"))).is_some());
            first_done_tx.send(()).expect("signal first completion");
        });

        let second_session = session.clone();
        let second_owner_key = second_key.clone();
        let second_wait_key = first_key.clone();
        let second_thread = std::thread::spawn(move || {
            first_started_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("first owner started");
            let owner = start(&second_session, &second_owner_key);
            second_started_tx.send(()).expect("signal second owner");
            first_waiting_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("first wait edge registered");
            assert!(matches!(
                second_session.begin_context_load(&second_wait_key),
                Vue3ExternalTypeContextLoad::Failed
            ));
            assert!(owner.complete(Some(context("Second"))).is_some());
            done_tx.send(()).expect("signal second completion");
        });

        for _ in 0..2 {
            done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("cycle participant completed");
        }
        first_thread.join().expect("join first cycle thread");
        second_thread.join().expect("join second cycle thread");
        let state = session.lock();
        assert_eq!(state.failure_epoch, 1);
        assert_eq!(state.stats.context_lookups, 4);
        assert_eq!(state.stats.context_builds, 2);
        assert_eq!(state.stats.context_cache_hits, 1);
        assert!(state.context_waits.is_empty());
        assert!(!state.context_cache.contains_key(&first_key));
        assert!(!state.context_cache.contains_key(&second_key));
    }

    #[test]
    fn context_single_flight_stale_owner_cannot_replace_new_flight() {
        let session = Vue3ExternalTypeLoadSession::default();
        let cache_key = key("stale-context");
        let old_owner = start(&session, &cache_key);
        let old_flight = old_owner.flight.clone();
        session.lock().context_cache.remove(&cache_key);
        let replacement = start(&session, &cache_key);
        let replacement_id = replacement.flight.id;

        assert!(old_owner.complete(Some(context("Stale"))).is_none());
        assert!(matches!(
            old_flight.wait(),
            Vue3SingleFlightOutcome::Aborted
        ));
        assert!(vue3_context_flight_matches(
            session.lock().context_cache.get(&cache_key),
            replacement_id,
        ));

        assert!(replacement
            .complete(Some(context("Replacement")))
            .is_some());
        assert!(!session.lock().context_cache.contains_key(&cache_key));
        let retry = start(&session, &cache_key);
        let retried = retry
            .complete(Some(context("Retry")))
            .expect("retry context");
        let Vue3ExternalTypeContextLoad::Ready(cached) =
            session.begin_context_load(&cache_key)
        else {
            panic!("expected cached retry context");
        };
        assert!(std::sync::Arc::ptr_eq(&retried, &cached));
    }
}
