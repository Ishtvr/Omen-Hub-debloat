//! Own the NVIDIA session on a disposable read-only thread. Ending the visible
//! session also releases driver thread-local resources when entering the tray.
use omen_core::GpuSample;
use std::{sync::mpsc, thread, time::Duration};
enum Request {
    Sample,
    Stop,
}
struct Readout {
    sample: Option<GpuSample>,
    warnings: Vec<String>,
    lost: bool,
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
            // No FFI handle or Library crosses a thread. Only owned sensor
            // values leave this session, through the existing public adapter.
            let mut nvidia = omen_platform::Nvidia::new();
            while let Ok(Request::Sample) = requests.recv() {
                let sample = nvidia.sample();
                if results
                    .send(Readout {
                        sample,
                        warnings: nvidia.warnings().to_vec(),
                        lost: nvidia.needs_reconnect(),
                    })
                    .is_err()
                {
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
            self.warnings = vec!["NVIDIA-Lesesitzung beendet; alte Werte verworfen.".into()];
            return None;
        }
        match self.rx.recv_timeout(Duration::from_secs(5)) {
            Ok(readout) => {
                self.warnings = readout.warnings;
                self.lost = readout.lost;
                readout.sample
            }
            Err(_) => {
                self.lost = true;
                self.warnings = vec![
                    "NVIDIA-Messung hat das Zeitlimit überschritten; alte Werte verworfen.".into(),
                ];
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
            // Never wait indefinitely on a vendor call. A stopped, healthy
            // session exits and runs its FFI cleanup on the original thread.
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
