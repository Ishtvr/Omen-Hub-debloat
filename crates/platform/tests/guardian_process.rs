//! Standalone process fixtures. Only files in fresh test directories simulate
//! the service; no SCM, driver, firmware, power or installed-app action exists.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(not(windows))]
fn main() {}
#[cfg(windows)]
fn main() {
    if let Err(error) = fixture::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
#[cfg(windows)]
mod fixture {
    use omen_core::{
        guardian::{ParentState, Trigger},
        ownership::{
            self, Backend, Consent, Identity, Journal, Owner, Phase, Record, Snapshot, State,
            Subject,
        },
    };
    use omen_platform::guardian::{self, GuardianProcess, Outcome, ParentProcess, Request, Target};
    use serde::{Deserialize, Serialize};
    use std::{
        fs,
        io::Write,
        os::windows::{ffi::OsStrExt, process::CommandExt},
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
        System::{JobObjects::*, Threading::*},
    };
    type Result<T> = std::result::Result<T, String>;
    fn identity() -> Identity {
        Identity {
            subject: Subject::HpOmenCap,
            image_sha256: "a".repeat(64),
            configuration_sha256: "b".repeat(64),
        }
    }
    fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
        serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }
    fn save(path: &Path, value: &impl Serialize) -> Result<()> {
        let temp = path.with_extension(format!("{}-tmp", std::process::id()));
        let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec(value).map_err(|e| e.to_string())?)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        let src: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let dst: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe {
            MoveFileExW(
                src.as_ptr(),
                dst.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    struct Ledger(PathBuf);
    impl Journal for Ledger {
        fn load(&self) -> Result<Option<Record>> {
            let path = self.0.join("journal.json");
            if !path.exists() {
                return Ok(None);
            }
            let record: Record = read(&path)?;
            record.validate()?;
            Ok(Some(record))
        }
        fn save(&mut self, expected: Option<&Record>, next: &Record) -> Result<()> {
            if self.load()?.as_ref() != expected {
                return Err("Fixture CAS conflict".into());
            }
            next.validate()?;
            save(&self.0.join("journal.json"), next)
        }
    }
    #[derive(Debug, Serialize, Deserialize)]
    struct Service {
        running: bool,
        stops: u32,
        starts: u32,
        fail_start: bool,
    }
    struct Device(PathBuf);
    impl Backend for Device {
        fn inspect(&mut self) -> Result<Snapshot> {
            let s: Service = read(&self.0.join("service.json"))?;
            Ok(Snapshot {
                identity: identity(),
                state: if s.running {
                    State::Running
                } else {
                    State::Stopped
                },
                process_id: if s.running { 20 } else { 0 },
                process_started: if s.running { 200 } else { 0 },
                can_stop: true,
                has_dependents: false,
            })
        }
        fn stop(&mut self, expected: &Snapshot) -> Result<()> {
            if &self.inspect()? != expected {
                return Err("Fixture STOP identity differs".into());
            }
            let mut s: Service = read(&self.0.join("service.json"))?;
            s.running = false;
            s.stops += 1;
            save(&self.0.join("service.json"), &s)
        }
        fn start(&mut self, expected: &Identity) -> Result<()> {
            if &identity() != expected {
                return Err("Fixture START identity differs".into());
            }
            let mut s: Service = read(&self.0.join("service.json"))?;
            s.starts += 1;
            if !s.fail_start {
                s.running = true
            }
            let fail = s.fail_start;
            save(&self.0.join("service.json"), &s)?;
            if fail {
                Err("Simulated restart failure".into())
            } else {
                Ok(())
            }
        }
    }
    #[derive(Serialize, Deserialize)]
    struct Event {
        owner: Owner,
        guardian_pid: u32,
        guardian_started: u64,
    }
    struct ReturnTarget {
        root: PathBuf,
        owner: Owner,
    }
    impl Target for ReturnTarget {
        fn record(&mut self) -> Result<Option<Record>> {
            let record = Ledger(self.root.clone()).load()?;
            if record
                .as_ref()
                .is_some_and(|r| r.owner == self.owner && r.phase == Phase::Owned)
            {
                save(&self.root.join("seen.json"), &true)?;
            }
            Ok(record)
        }
        fn recover(&mut self, _trigger: Trigger, _parent: ParentState) -> Result<Phase> {
            ownership::release(
                &mut Device(self.root.clone()),
                &mut Ledger(self.root.clone()),
                &self.owner,
                &identity(),
            )
            .map(|r| r.phase)
            .map_err(|e| serde_json::to_string(&e).unwrap())
        }
    }
    fn peer(root: &Path, bad: bool) -> Result<()> {
        let input = std::io::stdin();
        let request = guardian::receive_request(&input)?;
        let parent = ParentProcess::authenticate(&request)?;
        let mut ready = guardian::ready(&request)?;
        if bad {
            ready.owner.session = "d".repeat(64)
        }
        guardian::write_frame(&mut std::io::stdout(), &ready)?;
        if bad {
            let began = Instant::now();
            while guardian::link_state(&input) == omen_core::guardian::LinkState::Open
                && began.elapsed() < Duration::from_secs(3)
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            return Ok(());
        }
        let result = guardian::monitor(
            &request,
            &parent,
            &input,
            &mut ReturnTarget {
                root: root.into(),
                owner: request.owner.clone(),
            },
        );
        save(&root.join("guardian-result.json"), &result)?;
        Ok(())
    }
    struct Job(HANDLE);
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn own_job(allow_breakaway: bool) -> Result<Job> {
        let job = Job(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) });
        if job.0.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = if allow_breakaway {
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK
        } else {
            0
        };
        if unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            )
        } == 0
            || unsafe { AssignProcessToJobObject(job.0, GetCurrentProcess()) } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(job)
    }
    fn wait_file(path: &Path) -> Result<()> {
        let end = Instant::now() + Duration::from_secs(8);
        while !path.exists() {
            if Instant::now() >= end {
                return Err(format!("Fixture timeout: {}", path.display()));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
    fn owner(root: &Path, mode: &str) -> Result<()> {
        let _job = if matches!(mode, "job-crash" | "restricted-job") {
            Some(own_job(mode == "job-crash")?)
        } else {
            None
        };
        let owner = omen_platform::hp_control::current_owner_identity()?;
        let request = Request {
            schema: 1,
            owner: owner.clone(),
            identity: identity(),
        };
        let args = vec![
            if mode == "bad-ready" {
                "--bad-peer".into()
            } else {
                "--peer".into()
            },
            root.as_os_str().to_owned(),
        ];
        let result = GuardianProcess::spawn_self(&request, &args);
        if matches!(mode, "bad-ready" | "restricted-job") {
            if result.is_ok() {
                return Err("Bad guardian readiness was accepted".into());
            }
            save(&root.join("owner-rejected.json"), &true)?;
            return Ok(());
        }
        let mut guard = result?;
        ownership::acquire(
            &mut Device(root.into()),
            &mut Ledger(root.into()),
            owner.clone(),
            &identity(),
            Consent::ExplicitSession,
        )
        .map_err(|e| serde_json::to_string(&e).unwrap())?;
        let mut in_job = 0;
        unsafe {
            IsProcessInJob(GetCurrentProcess(), std::ptr::null_mut(), &mut in_job);
        }
        let guardian_started = {
            let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, guard.id()) };
            if h.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let result = unsafe { guardian::creation(h) };
            unsafe {
                CloseHandle(h);
            }
            result?
        };
        guardian::write_frame(
            &mut std::io::stdout(),
            &Event {
                owner: owner.clone(),
                guardian_pid: guard.id(),
                guardian_started,
            },
        )?;
        match mode {
            "normal" => {
                ownership::release(
                    &mut Device(root.into()),
                    &mut Ledger(root.into()),
                    &owner,
                    &identity(),
                )
                .map_err(|e| serde_json::to_string(&e).unwrap())?;
            }
            "link-loss" => guard.disconnect(),
            "foreign" => {
                wait_file(&root.join("seen.json"))?;
                let mut ledger = Ledger(root.into());
                let prior = ledger.load()?.unwrap();
                let mut next = prior.clone();
                next.owner.session = "e".repeat(64);
                ledger.save(Some(&prior), &next)?;
                guard.disconnect();
            }
            "guard-loss" => {
                wait_file(&root.join("guard-killed.json"))?;
                if guard.ensure_running().is_ok() {
                    return Err("Dead guardian still appeared ready".into());
                }
                ownership::release(
                    &mut Device(root.into()),
                    &mut Ledger(root.into()),
                    &owner,
                    &identity(),
                )
                .map_err(|e| serde_json::to_string(&e).unwrap())?;
                return Ok(());
            }
            _ => {
                let end = Instant::now() + Duration::from_secs(30);
                while Instant::now() < end {
                    std::thread::sleep(Duration::from_millis(50));
                }
                return Err("Crash fixture owner was not terminated".into());
            }
        }
        if !guard.wait_for_exit(Duration::from_secs(5))? {
            return Err("Guardian remained after the fixture ended".into());
        }
        Ok(())
    }
    fn kill_guard(event: &Event) -> Result<()> {
        let h = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                0,
                event.guardian_pid,
            )
        };
        if h.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let result = (|| {
            if unsafe { guardian::creation(h) }? != event.guardian_started {
                return Err("Fixture guardian PID was reused".into());
            }
            if unsafe { TerminateProcess(h, 9) } == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            unsafe {
                WaitForSingleObject(h, 5000);
            }
            Ok(())
        })();
        unsafe {
            CloseHandle(h);
        }
        result
    }
    fn spawn_owner(root: &Path, mode: &str) -> Result<Child> {
        Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .args(["--owner", mode])
            .arg(root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())
    }
    fn wait_child(child: &mut Child) -> Result<()> {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!("Fixture owner exited {status}"))
                };
            }
            if Instant::now() >= end {
                return Err("Fixture owner did not exit".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn scenario(base: &Path, mode: &str) -> Result<()> {
        let root = base.join(format!(
            "OmenGuardian-{}-{}-{mode}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).map_err(|e| e.to_string())?;
        save(
            &root.join("service.json"),
            &Service {
                running: true,
                stops: 0,
                starts: 0,
                fail_start: mode == "restore-failure",
            },
        )?;
        let mut child = spawn_owner(&root, mode)?;
        if matches!(mode, "bad-ready" | "restricted-job") {
            wait_child(&mut child)?;
            assert!(root.join("owner-rejected.json").exists());
            assert!(!root.join("journal.json").exists());
        } else {
            let event: Event = guardian::read_frame(child.stdout.as_mut().unwrap())?;
            if matches!(mode, "crash" | "job-crash" | "restore-failure") {
                child.kill().map_err(|e| e.to_string())?;
                child.wait().map_err(|e| e.to_string())?;
                wait_file(&root.join("guardian-result.json"))?;
            } else if mode == "guard-loss" {
                kill_guard(&event)?;
                save(&root.join("guard-killed.json"), &true)?;
                wait_child(&mut child)?;
            } else {
                wait_child(&mut child)?;
            }
        }
        let service: Service = read(&root.join("service.json"))?;
        match mode {
            "bad-ready" | "restricted-job" => {
                assert!(service.running && service.stops == 0 && service.starts == 0)
            }
            "foreign" => assert!(!service.running && service.stops == 1 && service.starts == 0),
            "restore-failure" => {
                let report: std::result::Result<Outcome, String> =
                    read(&root.join("guardian-result.json"))?;
                assert!(report.is_err());
                assert!(!service.running && service.starts == 1);
                assert_eq!(
                    Ledger(root.clone()).load()?.unwrap().phase,
                    Phase::RestorePending
                );
            }
            _ => {
                assert!(
                    service.running && service.stops == 1 && service.starts == 1,
                    "{mode}: {service:?}"
                );
                assert_eq!(Ledger(root.clone()).load()?.unwrap().phase, Phase::Restored);
            }
        }
        // Flat, newly created fixture directory: individual files only, no
        // recursive deletion and no access to real control/installation data.
        for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_file() {
                fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
            }
        }
        fs::remove_dir(root).map_err(|e| e.to_string())?;
        println!("guardian process scenario passed: {mode}");
        Ok(())
    }
    fn suite(base: &Path) -> Result<()> {
        for mode in [
            "normal",
            "restricted-job",
            "crash",
            "job-crash",
            "link-loss",
            "foreign",
            "bad-ready",
            "guard-loss",
            "restore-failure",
        ] {
            scenario(base, mode)?;
        }
        Ok(())
    }
    fn independent_suite() -> Result<()> {
        // Test harness only. Codex/CI may place cargo in a restrictive cleanup
        // job. The product never falls back from failed BREAKAWAY. WMI starts
        // only this same no-harness fixture executable, without elevation or
        // shell, and its nine scenarios touch only this new temporary folder.
        #[derive(Serialize)]
        struct Input {
            #[serde(rename = "CommandLine")]
            command_line: String,
        }
        #[derive(Deserialize)]
        struct Output {
            #[serde(rename = "ReturnValue")]
            code: u32,
            #[serde(rename = "ProcessId")]
            process_id: u32,
        }
        #[derive(Deserialize)]
        #[serde(rename = "Win32_Process")]
        struct WmiProcess;
        let base = std::env::temp_dir().join(format!(
            "OmenGuardianSuite-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&base).map_err(|e| e.to_string())?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let command_line = format!(
            "\"{}\" --detached-suite \"{}\"",
            exe.display(),
            base.display()
        );
        let connection = wmi::WMIConnection::new().map_err(|e| e.to_string())?;
        let result: Output = connection
            .exec_class_method::<WmiProcess, _>("Create", Input { command_line })
            .map_err(|e| e.to_string())?;
        if result.code != 0 {
            return Err(format!(
                "Independent file-fixture host creation failed: {}",
                result.code
            ));
        }
        let result_path = base.join("suite-result.json");
        let end = Instant::now() + Duration::from_secs(50);
        while !result_path.exists() {
            if Instant::now() >= end {
                return Err(format!(
                    "File-fixture host {} did not report: {}",
                    result.process_id,
                    base.display()
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let result: std::result::Result<(), String> = read(&result_path)?;
        fs::remove_file(result_path).map_err(|e| e.to_string())?;
        if result.is_ok() {
            fs::remove_dir(&base).map_err(|e| e.to_string())?;
        }
        result.map_err(|e| format!("{e}; fixture diagnostics: {}", base.display()))?;
        println!(
            "9 guardian process scenarios passed outside the restrictive test job; real services/drivers were not addressed."
        );
        Ok(())
    }
    pub fn run() -> Result<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match args.first().map(String::as_str) {
            Some("--peer" | "--bad-peer") => peer(
                Path::new(args.get(1).ok_or("Missing fixture path")?),
                args[0] == "--bad-peer",
            )
            .inspect_err(|e| {
                let _ = save(&Path::new(&args[1]).join("peer-error.json"), e);
            }),
            Some("--owner") => owner(
                Path::new(args.get(2).ok_or("Missing fixture root")?),
                args.get(1).ok_or("Missing fixture mode")?,
            )
            .inspect_err(|e| {
                let _ = save(&Path::new(&args[2]).join("owner-error.json"), e);
            }),
            Some("--detached-suite") => {
                let base = Path::new(args.get(1).ok_or("Missing suite folder")?);
                let result = suite(base);
                save(&base.join("suite-result.json"), &result)?;
                result
            }
            _ => independent_suite(),
        }
    }
}
