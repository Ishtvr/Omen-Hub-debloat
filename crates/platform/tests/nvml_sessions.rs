//! Device probe, read-only, ignored by default: does a retained NVML session
//! survive other sessions opening and closing in the same process? The tray
//! process crashed in `nvml.dll` on 22 November 2026 while "Keep preset" held
//! one session and the preset editor opened and closed others.
//!
//! Result on 8E35 with driver 32.0.16.2114 (22 November 2026): 20 rounds
//! pass, so other sessions closing is not the trigger. The crash site
//! (`nvml.dll+0xfd6e0`) reads device state in a five-try retry loop; what
//! invalidates that memory is unknown. Readings therefore run in disposable
//! child processes.
//!
//! Run on the reference device only:
//! `cargo test -p omen-platform --test nvml_sessions -- --ignored --nocapture`
#![cfg(windows)]

#[test]
#[ignore = "needs the NVIDIA driver on the reference device"]
fn a_retained_session_is_read_after_other_sessions_closed() {
    let retained = omen_platform::GpuLimitProbe::new().expect("NVML session");
    println!("retained first read: {:?}", retained.read());
    for round in 0..20 {
        // Same thread first: open and close a full control session.
        let report = omen_platform::read_gpu_controls(None).map(|r| r.identity.name);
        println!("round {round}: other session {report:?}");
        // Then from another thread, as the preset editor worker does.
        std::thread::spawn(|| {
            let _ = omen_platform::read_gpu_controls(None);
        })
        .join()
        .unwrap();
        println!("round {round}: retained read {:?}", retained.read());
    }
}
