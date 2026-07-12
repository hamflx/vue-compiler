enum Vue3SingleFlightOutcome<T> {
    Complete(Option<std::sync::Arc<T>>),
    Aborted,
}

enum Vue3SingleFlightState<T> {
    Running,
    Complete(Option<std::sync::Arc<T>>),
    Aborted,
}

struct Vue3SingleFlight<T> {
    id: u64,
    owner: std::thread::ThreadId,
    generation: u64,
    state: std::sync::Mutex<Vue3SingleFlightState<T>>,
    ready: std::sync::Condvar,
    #[cfg(test)]
    waiting: std::sync::atomic::AtomicUsize,
}

impl<T> std::fmt::Debug for Vue3SingleFlight<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Vue3SingleFlight")
            .field("id", &self.id)
            .field("owner", &self.owner)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl<T> Vue3SingleFlight<T> {
    fn new(id: u64, owner: std::thread::ThreadId, generation: u64) -> Self {
        Self {
            id,
            owner,
            generation,
            state: std::sync::Mutex::new(Vue3SingleFlightState::Running),
            ready: std::sync::Condvar::new(),
            #[cfg(test)]
            waiting: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn wait(&self) -> Vue3SingleFlightOutcome<T> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            match &*state {
                Vue3SingleFlightState::Running => {
                    #[cfg(test)]
                    self.waiting
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    state = self
                        .ready
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    #[cfg(test)]
                    self.waiting
                        .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                }
                Vue3SingleFlightState::Complete(result) => {
                    return Vue3SingleFlightOutcome::Complete(result.clone());
                }
                Vue3SingleFlightState::Aborted => return Vue3SingleFlightOutcome::Aborted,
            }
        }
    }

    fn complete(&self, result: Option<std::sync::Arc<T>>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if matches!(*state, Vue3SingleFlightState::Running) {
            *state = Vue3SingleFlightState::Complete(result);
            self.ready.notify_all();
        }
    }

    fn abort(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if matches!(*state, Vue3SingleFlightState::Running) {
            *state = Vue3SingleFlightState::Aborted;
            self.ready.notify_all();
        }
    }

    #[cfg(test)]
    fn waiting_count(&self) -> usize {
        self.waiting.load(std::sync::atomic::Ordering::SeqCst)
    }
}
