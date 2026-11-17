//! Latest intent wins; an in-flight firmware operation reaches its safe boundary
//! before the replacement starts. Never terminate a controller/WMI call.
use super::*;
use std::sync::Condvar;
#[derive(Clone)]
pub(super) enum Target {
    Preset(String),
    Mode(HpMode),
}
struct State {
    queue: omen_core::latest_request::Latest<Target>,
    paused: bool,
    stopped: bool,
}
pub(super) struct ResultData {
    pub generation: u64,
    pub result: Result<String, String>,
    pub pending: Option<bool>,
}
pub(super) struct Handle {
    state: Arc<(Mutex<State>, Condvar)>,
    pub results: mpsc::Receiver<ResultData>,
}
pub(super) const DONE: u32 = WM_APP + 93;
impl Handle {
    pub fn new() -> (Self, mpsc::Sender<ResultData>) {
        let (tx, rx) = mpsc::channel();
        (
            Self {
                state: Arc::new((
                    Mutex::new(State {
                        queue: Default::default(),
                        paused: false,
                        stopped: false,
                    }),
                    Condvar::new(),
                )),
                results: rx,
            },
            tx,
        )
    }
    pub fn submit(&self, target: Target) -> u64 {
        let (lock, cv) = &*self.state;
        let n = lock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .queue
            .submit(target);
        cv.notify_one();
        n
    }
    pub fn current(&self, n: u64) -> bool {
        self.state
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .queue
            .current(n)
    }
    pub fn pause(&self, value: bool) {
        let (lock, cv) = &*self.state;
        lock.lock().unwrap_or_else(|e| e.into_inner()).paused = value;
        cv.notify_one();
    }
    pub fn stop(&self) {
        let (lock, cv) = &*self.state;
        lock.lock().unwrap_or_else(|e| e.into_inner()).stopped = true;
        cv.notify_one();
    }
    pub fn spawn(&self, hwnd: usize, tx: mpsc::Sender<ResultData>) -> thread::JoinHandle<()> {
        self.spawn_with(
            tx,
            |target| match target {
                Target::Preset(id) => {
                    crate::elevation::run_serial(Some(id), crate::elevation::Action::Replace)
                        .map(|r| crate::elevation::applied_status(&r))
                }
                Target::Mode(m) => {
                    crate::elevation::run_serial(None, crate::elevation::Action::ReplaceMode(*m))
                        .map(|report| crate::elevation::mode_written_status(*m, &report))
                }
            },
            |target| {
                let selection = match target {
                    Target::Preset(id) => Selection::Preset(id.clone()),
                    Target::Mode(m) => Selection::Mode(*m),
                };
                crate::ui::record_manual(&selection);
            },
            || {
                ProfileStore::user_data()
                    .and_then(|s| Ok(s.pending()? || s.fan_policy_pending()?))
                    .ok()
            },
            move || unsafe {
                PostMessageW(hwnd as HWND, DONE, 0, 0);
            },
        )
    }
    fn spawn_with(
        &self,
        tx: mpsc::Sender<ResultData>,
        mut execute: impl FnMut(&Target) -> Result<String, String> + Send + 'static,
        mut record: impl FnMut(&Target) + Send + 'static,
        mut pending: impl FnMut() -> Option<bool> + Send + 'static,
        mut notify: impl FnMut() + Send + 'static,
    ) -> thread::JoinHandle<()> {
        let shared = self.state.clone();
        thread::spawn(move || {
            loop {
                let (generation, target) = {
                    let (lock, cv) = &*shared;
                    let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
                    while !s.stopped && (s.paused || !s.queue.pending()) {
                        s = cv.wait(s).unwrap_or_else(|e| e.into_inner());
                    }
                    if s.stopped {
                        break;
                    }
                    s.queue.take().expect("Pending latest intent")
                };
                let result = execute(&target);
                if result.is_ok()
                    && shared
                        .0
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .queue
                        .current(generation)
                {
                    record(&target);
                }
                if tx
                    .send(ResultData {
                        generation,
                        result,
                        pending: pending(),
                    })
                    .is_err()
                {
                    break;
                }
                notify();
            }
        })
    }
}
impl Host {
    pub(super) fn queue_apply(&self, target: Target) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        if d.get_confirm_kind() != 0 {
            return;
        }
        if self.test {
            self.status(t("ui.test_no_write"), true);
            return;
        }
        let replacing = d.get_apply_busy();
        self.exit_after.set(false);
        self.apply.submit(target);
        d.set_apply_busy(true);
        d.set_busy(true);
        if replacing {
            self.notice(t("notice.background"), t("apply.replacing"), false);
        }
        self.status(t("control.working"), false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn replacement_waits_for_active_work_and_only_latest_success_is_recorded() {
        // Exercise the actual worker loop, with a stalled executor in place of
        // a firmware call. Both success and failure of obsolete work must let
        // the latest intent run, never an intermediate target or second writer.
        for old_fails in [false, true] {
            let (handle, results) = Handle::new();
            let (started_tx, started_rx) = mpsc::channel();
            let (finish_tx, finish_rx) = mpsc::channel();
            let recorded = Arc::new(Mutex::new(Vec::new()));
            let saved = recorded.clone();
            let worker = handle.spawn_with(
                results,
                move |target| {
                    let Target::Preset(id) = target else {
                        unreachable!()
                    };
                    started_tx.send(id.clone()).unwrap();
                    if id == "old" {
                        finish_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                        if old_fails {
                            return Err("old failure".into());
                        }
                    }
                    Ok(id.clone())
                },
                move |target| {
                    let Target::Preset(id) = target else {
                        unreachable!()
                    };
                    saved.lock().unwrap().push(id.clone());
                },
                || Some(false),
                || {},
            );
            let old = handle.submit(Target::Preset("old".into()));
            assert_eq!(
                started_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
                "old"
            );
            handle.submit(Target::Preset("intermediate".into()));
            let latest = handle.submit(Target::Preset("latest".into()));
            assert!(
                started_rx.try_recv().is_err(),
                "The old call is still active"
            );
            finish_tx.send(()).unwrap();
            let old_result = handle.results.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(old_result.generation, old);
            assert_eq!(old_result.result.is_err(), old_fails);
            assert!(!handle.current(old_result.generation));
            assert_eq!(
                started_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
                "latest"
            );
            let current = handle.results.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(current.generation, latest);
            assert_eq!(current.result.unwrap(), "latest");
            assert!(handle.current(latest));
            assert_eq!(*recorded.lock().unwrap(), ["latest"]);
            handle.stop();
            worker.join().unwrap();
        }
    }

    #[test]
    fn queued_apply_cannot_start_during_an_existing_save_or_recovery() {
        let (handle, results) = Handle::new();
        handle.pause(true);
        let (started_tx, started_rx) = mpsc::channel();
        let worker = handle.spawn_with(
            results,
            move |_| {
                started_tx.send(()).unwrap();
                Ok("done".into())
            },
            |_| {},
            || None,
            || {},
        );
        handle.submit(Target::Preset("replaced".into()));
        let latest = handle.submit(Target::Mode(HpMode::Balanced));
        assert!(started_rx.try_recv().is_err());
        handle.pause(false);
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            handle
                .results
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .generation,
            latest
        );
        handle.stop();
        worker.join().unwrap();
    }
}
