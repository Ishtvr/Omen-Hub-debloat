//! Serialize visible GPU reads on a worker; each read owns a bounded child.
//! NVIDIA process-global allocations disappear with that child, even if the
//! driver's DLL unload retains state. No driver handle enters the dashboard.
use crate::i18n::{t, tf};
use omen_core::GpuSample;
use serde::{Deserialize, Serialize};
use std::{sync::mpsc, thread, time::Duration};
enum Request {
    Sample,
    Stop,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Readout {
    sample: Option<GpuSample>,
    warnings: Vec<String>,
    lost: bool,
}
pub(crate) fn read_in_child() -> Readout {
    let mut nvidia = omen_platform::Nvidia::new();
    let sample = nvidia.sample();
    Readout {
        sample,
        warnings: nvidia.warnings().to_vec(),
        lost: nvidia.needs_reconnect(),
    }
}
pub struct GpuTelemetry {
    tx: mpsc::Sender<Request>,
    rx: mpsc::Receiver<Readout>,
    thread: Option<thread::JoinHandle<()>>,
    warnings: Vec<String>,
    lost: bool,
}
impl GpuTelemetry {
    pub fn new() -> Self {
        let (tx, requests) = mpsc::channel();
        let (results, rx) = mpsc::channel();
        let thread = thread::spawn(move || {
            while let Ok(Request::Sample) = requests.recv() {
                let readout =
                    crate::read_process::capture(crate::read_process::ReadCommand::GpuTelemetry)
                        .unwrap_or_else(|error| Readout {
                            sample: None,
                            warnings: vec![tf("warning.gpu_reader", &[&error])],
                            lost: true,
                        });
                if results.send(readout).is_err() {
                    break;
                }
            }
        });
        Self {
            tx,
            rx,
            thread: Some(thread),
            warnings: vec![],
            lost: false,
        }
    }
    pub fn sample(&mut self) -> Option<GpuSample> {
        if self.tx.send(Request::Sample).is_err() {
            self.lost = true;
            self.warnings = vec![t("warning.nvidia_ended").into()];
            return None;
        }
        // The isolated child owns a five-second deadline; allow its result
        // to arrive before the worker itself is considered unresponsive.
        match self.rx.recv_timeout(Duration::from_secs(6)) {
            Ok(readout) => {
                self.warnings = readout.warnings;
                self.lost = readout.lost;
                readout.sample
            }
            Err(_) => {
                self.lost = true;
                self.warnings = vec![t("warning.nvidia_timeout").into()];
                None
            }
        }
    }
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
    pub fn needs_reconnect(&self) -> bool {
        self.lost
    }
}
impl Drop for GpuTelemetry {
    fn drop(&mut self) {
        let _ = self.tx.send(Request::Stop);
        if let Some(thread) = self.thread.take() {
            // No new reads after Stop. An in-flight read still has its own
            // five-second child/job deadline, including if this handle detaches.
            for _ in 0..20 {
                if thread.is_finished() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            if thread.is_finished() {
                let _ = thread.join();
            }
        }
    }
}
