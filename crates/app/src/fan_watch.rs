//! Keeps the fan service running (user decision, 30 October 2026), also in
//! Auto mode because the service applies the CCD selection too. The elevated
//! dashboard checks at start and every 30 s and starts the installed, verified
//! cooling task through the Task Scheduler when it does not run. It never
//! installs. A missing installation is shown once; failures go to
//! `fan-watch.json` in the data folder, with a 5-minute pause after repeats.
use std::{
    sync::Mutex,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const INTERVAL: Duration = Duration::from_secs(30);
const BACK_OFF: Duration = Duration::from_secs(300);
/// Failed starts in a row before the pause.
const FAILURES_BEFORE_PAUSE: u8 = 3;

static NOTICE: Mutex<Option<String>> = Mutex::new(None);

/// The pending one-time notice for the dashboard status line.
pub fn take_notice() -> Option<String> {
    NOTICE.lock().ok()?.take()
}

/// What one check leads to.
#[derive(Debug, PartialEq, Eq)]
enum Next {
    Check,
    Pause,
}

/// Counts failures; after a few in a row the watcher pauses once.
#[derive(Default)]
struct Failures(u8);
impl Failures {
    fn record(&mut self, failed: bool) -> Next {
        if !failed {
            self.0 = 0;
            return Next::Check;
        }
        self.0 = self.0.saturating_add(1);
        if self.0 >= FAILURES_BEFORE_PAUSE {
            self.0 = 0;
            Next::Pause
        } else {
            Next::Check
        }
    }
}

pub fn spawn() {
    if !crate::single_instance::elevated() {
        return;
    }
    thread::spawn(|| {
        let mut failures = Failures::default();
        let mut missing_shown = false;
        loop {
            let failed = if omen_platform::fan_service_present() {
                missing_shown = false;
                match omen_platform::ensure_fan_service_running() {
                    Ok(started) => {
                        if started {
                            record("started", None);
                        }
                        false
                    }
                    Err(error) => {
                        record("failed", Some(&error));
                        true
                    }
                }
            } else {
                if !missing_shown {
                    missing_shown = true;
                    if let Ok(mut notice) = NOTICE.lock() {
                        *notice = Some(crate::i18n::t("fan.watch.missing").into());
                    }
                }
                false
            };
            let wait = match failures.record(failed) {
                Next::Check => INTERVAL,
                Next::Pause => BACK_OFF,
            };
            thread::sleep(wait);
        }
    });
}

/// The last start or failure, for diagnosis only.
fn record(event: &str, error: Option<&str>) {
    let Ok(store) = omen_platform::profile_store::ProfileStore::user_data() else {
        return;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let _ = std::fs::write(
        store.root().join("fan-watch.json"),
        serde_json::json!({"schema":1,"event":event,"error":error,"recorded_unix_s":now})
            .to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_failures_pause_once_and_a_success_resets() {
        let mut failures = Failures::default();
        assert_eq!(failures.record(true), Next::Check);
        assert_eq!(failures.record(true), Next::Check);
        assert_eq!(failures.record(true), Next::Pause);
        // After the pause the watcher tries again with a fresh count.
        assert_eq!(failures.record(true), Next::Check);
        assert_eq!(failures.record(false), Next::Check);
        assert_eq!(failures.record(true), Next::Check);
    }
}
