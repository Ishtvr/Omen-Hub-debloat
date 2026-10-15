//! Bounded, versioned profile persistence and durable hardware transaction journal.
use omen_core::{
    profiles::{PerformanceProfile, validate_id},
    transaction::{Completion, Journal, PendingTransaction},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_FILE_BYTES: u64 = 1024 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub fn profile_digest(profile: &PerformanceProfile) -> Result<String, String> {
    profile.validate()?;
    let bytes = serde_json::to_vec(profile).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalFile<S> {
    schema: u32,
    pending: bool,
    transaction: PendingTransaction<S>,
    completion: Option<Completion>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartupSelection {
    pub schema: u32,
    pub enabled: bool,
    pub profile_id: Option<String>,
    pub profile_digest: Option<String>,
}

pub struct ProfileStore {
    root: PathBuf,
}
/// A scoped qualification must not discard the last user transaction/approval.
pub struct JournalCheckpoint {
    root: PathBuf,
    previous: Option<Vec<u8>>,
}
impl ProfileStore {
    pub fn user_data() -> Result<Self, String> {
        #[cfg(windows)]
        return Self::at(crate::windows::profile_directory()?);
        #[cfg(not(windows))]
        {
            let local = std::env::var_os("LOCALAPPDATA")
                .ok_or("Windows user data directory is unavailable")?;
            Self::at(PathBuf::from(local).join("OmenHubPlusPlus"))
        }
    }
    pub fn at(root: PathBuf) -> Result<Self, String> {
        if !root.is_absolute() {
            return Err("Profile storage requires an absolute directory".into());
        }
        fs::create_dir_all(root.join("profiles")).map_err(|e| e.to_string())?;
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn checkpoint_completed_journal(&self) -> Result<JournalCheckpoint, String> {
        if self.pending()? {
            return Err("Pending recovery blocks a qualification checkpoint".into());
        }
        let previous = if self
            .journal_path()
            .try_exists()
            .map_err(|e| e.to_string())?
        {
            let bytes = read_bytes(&self.journal_path())?;
            let file: JournalFile<serde_json::Value> =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if file.pending || file.schema != 1 {
                return Err("Changed/pending checkpoint journal".into());
            }
            file.transaction.validate()?;
            let path = self
                .root
                .join("control-history")
                .join(format!("{:x}.json", Sha256::digest(&bytes)));
            fs::create_dir_all(self.root.join("control-history")).map_err(|e| e.to_string())?;
            atomic_bytes(&path, &bytes)?;
            Some(bytes)
        } else {
            None
        };
        Ok(JournalCheckpoint {
            root: self.root.clone(),
            previous,
        })
    }
    pub fn restore_completed_trial_checkpoint(
        &self,
        checkpoint: JournalCheckpoint,
        id: &str,
        digest: &str,
    ) -> Result<(), String> {
        if checkpoint.root != self.root {
            return Err("Qualification checkpoint belongs to another store".into());
        }
        let current: JournalFile<serde_json::Value> = read_json(&self.journal_path())?;
        current.transaction.validate()?;
        if current.schema != 1
            || current.pending
            || current.completion != Some(Completion::Restored)
            || current.transaction.profile_id != id
            || current.transaction.profile_digest != digest
        {
            return Err(
                "Only the exact completely restored qualification can return to its checkpoint"
                    .into(),
            );
        }
        if let Some(previous) = checkpoint.previous {
            atomic_bytes(&self.journal_path(), &previous)?;
        }
        Ok(())
    }
    /// Explicit migration: never overwrite an existing store or enable startup.
    /// Keep the old files; a pending original journal is copied before profiles.
    pub fn migrate_legacy(&self) -> Result<usize, String> {
        let source = std::env::var_os("LOCALAPPDATA").ok_or("Legacy AppData unavailable")?;
        self.migrate_from(&PathBuf::from(source).join("OmenHubPlusPlus"))
    }
    fn migrate_from(&self, source: &Path) -> Result<usize, String> {
        if !source.is_absolute() || !source.join("profiles").is_dir() {
            return Err("Legacy profile store does not exist".into());
        }
        if !self.list()?.is_empty()
            || self
                .journal_path()
                .try_exists()
                .map_err(|e| e.to_string())?
        {
            return Err(
                "Current store already has profiles or a journal; migration did not overwrite it"
                    .into(),
            );
        }
        let old = Self {
            root: source.to_owned(),
        };
        let profiles = old.list()?;
        let journal = if old.journal_path().try_exists().map_err(|e| e.to_string())? {
            old.pending()?;
            Some(read_json::<JournalFile<serde_json::Value>>(
                &old.journal_path(),
            )?)
        } else {
            None
        };
        self.disable_startup()?;
        if let Some(journal) = journal {
            atomic_json(&self.journal_path(), &journal)?;
        }
        for profile in &profiles {
            self.save(profile)?;
        }
        atomic_json(
            &self.root.join("migration.json"),
            &serde_json::json!({"schema":1,"source":fs::canonicalize(source).map_err(|e|e.to_string())?,"copied_profiles":profiles.len(),"old_files_retained":true,"startup_enabled":false}),
        )?;
        Ok(profiles.len())
    }
    fn path(&self, id: &str) -> Result<PathBuf, String> {
        validate_id(id)?;
        Ok(self.root.join("profiles").join(format!("{id}.json")))
    }
    pub fn save(&self, profile: &PerformanceProfile) -> Result<PathBuf, String> {
        profile.validate()?;
        let path = self.path(&profile.id)?;
        atomic_json(&path, profile)?;
        Ok(path)
    }
    pub fn load(&self, id: &str) -> Result<PerformanceProfile, String> {
        let profile: PerformanceProfile = read_json(&self.path(id)?)?;
        profile.validate()?;
        if profile.id != id {
            return Err("Stored profile ID does not match its filename".into());
        }
        Ok(profile)
    }
    pub fn import(&self, path: &Path) -> Result<PathBuf, String> {
        self.save(&read_json::<PerformanceProfile>(path)?)
    }
    pub fn list(&self) -> Result<Vec<PerformanceProfile>, String> {
        let mut result = vec![];
        for entry in fs::read_dir(self.root.join("profiles")).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().extension().is_some_and(|e| e == "json") {
                let id = entry
                    .path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or("Profile filename is not Unicode")?
                    .to_owned();
                result.push(self.load(&id)?);
            }
            if result.len() > 256 {
                return Err("Profile count exceeds the bounded store size".into());
            }
        }
        result.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(result)
    }
    fn journal_path(&self) -> PathBuf {
        self.root.join("control-journal.json")
    }
    pub fn pending(&self) -> Result<bool, String> {
        let path = self.journal_path();
        if !path.try_exists().map_err(|e| e.to_string())? {
            return Ok(false);
        }
        let file: JournalFile<serde_json::Value> = read_json(&path)?;
        if file.schema != 1 || file.transaction.schema != 1 {
            return Err("Unknown control-journal version; automatic application is blocked".into());
        }
        file.transaction.validate()?;
        if file.pending == file.completion.is_some() {
            return Err("Inconsistent control journal completion state".into());
        }
        Ok(file.pending)
    }
    pub fn last_transaction<S: DeserializeOwned>(&self) -> Result<PendingTransaction<S>, String> {
        let file: JournalFile<S> = read_json(&self.journal_path())?;
        if file.schema != 1 || file.transaction.schema != 1 {
            return Err("Unknown control journal schema".into());
        }
        file.transaction.validate()?;
        Ok(file.transaction)
    }
    pub fn startup(&self) -> Result<StartupSelection, String> {
        let path = self.root.join("startup.json");
        if !path.try_exists().map_err(|e| e.to_string())? {
            return Ok(StartupSelection {
                schema: 1,
                ..Default::default()
            });
        }
        let selection: StartupSelection = read_json(&path)?;
        if selection.schema != 1 {
            return Err("Unknown startup selection schema".into());
        }
        Ok(selection)
    }
    pub fn disable_startup(&self) -> Result<(), String> {
        atomic_json(
            &self.root.join("startup.json"),
            &StartupSelection {
                schema: 1,
                ..Default::default()
            },
        )
    }
    /// Startup can select only the exact, successfully applied profile revision.
    pub fn startup_eligibility(&self, id: &str) -> Result<StartupSelection, String> {
        let profile = self.load(id)?;
        let digest = profile_digest(&profile)?;
        let journal: JournalFile<serde_json::Value> = read_json(&self.journal_path())?;
        journal.transaction.validate()?;
        if journal.schema != 1
            || journal.transaction.schema != 1
            || journal.pending
            || journal.completion != Some(Completion::Applied)
            || journal.transaction.profile_id != id
            || journal.transaction.profile_digest != digest
            || journal.transaction.binding != profile.binding
            || journal.transaction.requested != profile.settings
        {
            return Err(
                "Startup requires a completed application of this exact profile/device revision"
                    .into(),
            );
        }
        Ok(StartupSelection {
            schema: 1,
            enabled: true,
            profile_id: Some(id.into()),
            profile_digest: Some(digest),
        })
    }
    pub fn enable_startup(&self, id: &str) -> Result<(), String> {
        atomic_json(
            &self.root.join("startup.json"),
            &self.startup_eligibility(id)?,
        )
    }
    pub fn startup_profile(&self) -> Result<Option<PerformanceProfile>, String> {
        let selection = self.startup()?;
        if !selection.enabled {
            return Ok(None);
        }
        if self.pending()? {
            return Err(
                "Previous transaction is incomplete; automatic retries are disabled".into(),
            );
        }
        let id = selection
            .profile_id
            .ok_or("Startup profile ID is missing")?;
        let profile = self.load(&id)?;
        if Some(profile_digest(&profile)?) != selection.profile_digest {
            return Err("Startup profile changed after verification; no settings applied".into());
        }
        Ok(Some(profile))
    }
    pub fn save_startup_result<T: Serialize>(&self, value: &T) -> Result<(), String> {
        atomic_json(&self.root.join("last-startup-result.json"), value)
    }
    pub fn save_control_result<T: Serialize>(&self, value: &T) -> Result<(), String> {
        atomic_json(&self.root.join("last-control-result.json"), value)
    }
    pub fn control_result(&self) -> Result<serde_json::Value, String> {
        read_json(&self.root.join("last-control-result.json"))
    }
    pub fn save_cpu_error<T: Serialize>(&self, value: &T) -> Result<(), String> {
        atomic_json(&self.root.join("last-cpu-error.json"), value)
    }
    pub fn cpu_error(&self) -> Result<Option<serde_json::Value>, String> {
        let path = self.root.join("last-cpu-error.json");
        if path.try_exists().map_err(|e| e.to_string())? {
            read_json(&path).map(Some)
        } else {
            Ok(None)
        }
    }
}

impl<S: Serialize + DeserializeOwned> Journal<S> for ProfileStore {
    fn begin(&mut self, transaction: &PendingTransaction<S>) -> Result<(), String> {
        transaction.validate()?;
        if self.pending()? {
            return Err(
                "A previous control transaction needs recovery; no new writes were sent".into(),
            );
        }
        #[derive(Serialize)]
        struct NewJournal<'a, S> {
            schema: u32,
            pending: bool,
            transaction: &'a PendingTransaction<S>,
            completion: Option<Completion>,
        }
        atomic_json(
            &self.journal_path(),
            &NewJournal {
                schema: 1,
                pending: true,
                transaction,
                completion: None,
            },
        )
    }
    fn finish(&mut self, completion: Completion) -> Result<(), String> {
        let mut file: JournalFile<S> = read_json(&self.journal_path())?;
        if file.schema != 1 || !file.pending {
            return Err("Control journal is not an active transaction".into());
        }
        file.pending = false;
        file.completion = Some(completion);
        // Retain the complete original image for later audited manual recovery.
        atomic_json(&self.journal_path(), &file)
    }
    fn resume_restore(&mut self, transaction: &PendingTransaction<S>) -> Result<(), String> {
        transaction.validate()?;
        let mut file: JournalFile<S> = read_json(&self.journal_path())?;
        if file.schema != 1
            || file.transaction.schema != 1
            || serde_json::to_value(&file.transaction).map_err(|e| e.to_string())?
                != serde_json::to_value(transaction).map_err(|e| e.to_string())?
        {
            return Err("Stored recovery image changed; no restore writes were sent".into());
        }
        file.pending = true;
        file.completion = None;
        atomic_json(&self.journal_path(), &file)
    }
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut data = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > MAX_FILE_BYTES {
        return Err("Profile/control file exceeds 1 MiB".into());
    }
    Ok(data)
}
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    serde_json::from_slice(&read_bytes(path)?).map_err(|e| format!("{}: {e}", path.display()))
}

pub(crate) fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut data = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    data.push(b'\n');
    atomic_bytes(path, &data)
}
fn atomic_bytes(path: &Path, data: &[u8]) -> Result<(), String> {
    if data.len() as u64 > MAX_FILE_BYTES {
        return Err("Profile/control file exceeds 1 MiB".into());
    }
    let parent = path.parent().ok_or("No parent directory")?;
    let temp = parent.join(format!(
        ".omen-{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(data).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        replace(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(windows)]
fn replace(temp: &Path, path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let source: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: Both terminated paths refer to the same directory; fixed rename flags.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}
#[cfg(not(windows))]
fn replace(temp: &Path, path: &Path) -> Result<(), String> {
    fs::rename(temp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omen_core::{
        features::Feature,
        profiles::{DeviceBinding, PROFILE_SCHEMA, Setting, Value},
        transaction::Captured,
    };

    struct Fixture(ProfileStore);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "omen-profile-test-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            Self(ProfileStore::at(root).unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Remove only files created in this isolated test fixture; no recursive deletion.
            let root = self.0.root();
            for directory in [
                root.join("control-history"),
                root.join("profiles"),
                root.to_owned(),
            ] {
                if let Ok(entries) = fs::read_dir(&directory) {
                    for entry in entries.flatten() {
                        if entry.file_type().is_ok_and(|t| t.is_file()) {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
            }
            let _ = fs::remove_dir(root.join("profiles"));
            let _ = fs::remove_dir(root.join("control-history"));
            let _ = fs::remove_dir(root);
        }
    }
    fn profile() -> PerformanceProfile {
        PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "test".into(),
            name: "Test".into(),
            binding: DeviceBinding {
                manufacturer: "HP".into(),
                board: "test".into(),
                bios: "F.test".into(),
                cpu_names: vec![],
                nvidia_uuid: Some("GPU-test".into()),
                nvidia_driver: Some("driver".into()),
                nvidia_vbios: Some("vbios".into()),
            },
            settings: vec![Setting {
                feature: Feature::GpuCoreOffsetMhz,
                value: Value::Integer(0),
            }],
        }
    }
    fn transaction(profile: &PerformanceProfile) -> PendingTransaction<Value> {
        PendingTransaction {
            schema: 1,
            profile_id: profile.id.clone(),
            profile_digest: profile_digest(profile).unwrap(),
            binding: profile.binding.clone(),
            original: vec![Captured {
                feature: Feature::GpuCoreOffsetMhz,
                value: Value::Integer(0),
                restore: Value::Integer(0),
            }],
            requested: profile.settings.clone(),
        }
    }
    fn finish(store: &mut ProfileStore, completion: Completion) {
        <ProfileStore as Journal<Value>>::finish(store, completion).unwrap();
    }
    #[test]
    fn restored_qualification_retains_the_previous_journal_and_startup_selection() {
        let mut fixture = Fixture::new();
        let store = &mut fixture.0;
        let original = profile();
        store.save(&original).unwrap();
        let original_tx = transaction(&original);
        store.begin(&original_tx).unwrap();
        finish(store, Completion::Applied);
        store.enable_startup(&original.id).unwrap();
        let startup = serde_json::to_value(store.startup().unwrap()).unwrap();
        let original_bytes = fs::read(store.journal_path()).unwrap();
        let mut noncanonical = b"  ".to_vec();
        noncanonical.extend_from_slice(&original_bytes);
        noncanonical.extend_from_slice(b"\n\n");
        fs::write(store.journal_path(), &noncanonical).unwrap();
        let checkpoint = store.checkpoint_completed_journal().unwrap();
        let mut trial_profile = profile();
        trial_profile.id = "trial".into();
        let trial = transaction(&trial_profile);
        store.begin(&trial).unwrap();
        assert!(store.checkpoint_completed_journal().is_err());
        finish(store, Completion::Restored);
        store
            .restore_completed_trial_checkpoint(
                checkpoint,
                &trial.profile_id,
                &trial.profile_digest,
            )
            .unwrap();
        assert_eq!(fs::read(store.journal_path()).unwrap(), noncanonical);
        assert_eq!(store.last_transaction::<Value>().unwrap(), original_tx);
        assert_eq!(
            serde_json::to_value(store.startup().unwrap()).unwrap(),
            startup
        );
        assert_eq!(store.startup_profile().unwrap().unwrap(), original);
        assert_eq!(
            fs::read_dir(store.root.join("control-history"))
                .unwrap()
                .count(),
            1
        );
    }
    #[test]
    fn cpu_diagnostic_retains_exact_detail_without_changing_control_approval() {
        let mut fixture = Fixture::new();
        let store = &mut fixture.0;
        let p = profile();
        store.save(&p).unwrap();
        let original = transaction(&p);
        store.begin(&original).unwrap();
        finish(store, Completion::Applied);
        store.enable_startup(&p.id).unwrap();
        let selection = serde_json::to_value(store.startup().unwrap()).unwrap();
        assert!(store.cpu_error().unwrap().is_none());
        let detail = serde_json::json!({"schema":1,"detail":"exact technical cause","user_message":"German recovery"});
        store.save_cpu_error(&detail).unwrap();
        assert_eq!(store.cpu_error().unwrap(), Some(detail));
        assert_eq!(store.last_transaction::<Value>().unwrap(), original);
        assert_eq!(
            serde_json::to_value(store.startup().unwrap()).unwrap(),
            selection
        );
    }
    #[test]
    fn startup_requires_applied_exact_revision_and_rejects_edits() {
        let mut fixture = Fixture::new();
        let store = &mut fixture.0;
        let mut profile = profile();
        store.save(&profile).unwrap();
        assert!(store.enable_startup("test").is_err());
        store.begin(&transaction(&profile)).unwrap();
        assert!(store.enable_startup("test").is_err());
        finish(store, Completion::Restored);
        assert!(store.enable_startup("test").is_err());
        store.begin(&transaction(&profile)).unwrap();
        finish(store, Completion::Applied);
        store.enable_startup("test").unwrap();
        assert_eq!(store.startup_profile().unwrap().unwrap(), profile);
        profile.settings[0].value = Value::Integer(15);
        store.save(&profile).unwrap();
        assert!(store.startup_profile().is_err());
        store.disable_startup().unwrap();
        assert!(store.startup_profile().unwrap().is_none());
    }
    #[test]
    fn pending_transaction_survives_reopening_and_blocks_new_writes_and_startup() {
        let mut fixture = Fixture::new();
        let profile = profile();
        fixture.0.save(&profile).unwrap();
        fixture.0.begin(&transaction(&profile)).unwrap();
        finish(&mut fixture.0, Completion::Applied);
        fixture.0.enable_startup("test").unwrap();
        fixture.0.begin(&transaction(&profile)).unwrap();
        let mut reopened = ProfileStore::at(fixture.0.root().to_owned()).unwrap();
        assert!(reopened.pending().unwrap());
        assert!(reopened.begin(&transaction(&profile)).is_err());
        assert!(reopened.startup_profile().is_err());
        assert_eq!(
            reopened.last_transaction::<Value>().unwrap(),
            transaction(&profile)
        );
    }
    #[test]
    fn startup_rejects_a_completed_journal_with_different_requested_values() {
        let mut fixture = Fixture::new();
        let profile = profile();
        fixture.0.save(&profile).unwrap();
        let mut saved = transaction(&profile);
        saved.requested[0].value = Value::Integer(15);
        fixture.0.begin(&saved).unwrap();
        finish(&mut fixture.0, Completion::Applied);
        assert!(fixture.0.startup_eligibility(&profile.id).is_err());
    }
    #[test]
    fn corrupt_unknown_or_oversized_files_are_failures() {
        let mut fixture = Fixture::new();
        let store = &mut fixture.0;
        fs::write(store.journal_path(), b"{bad").unwrap();
        assert!(store.pending().is_err());
        assert!(store.begin(&transaction(&profile())).is_err());
        let mut wrong = profile();
        wrong.schema = 99;
        assert!(store.save(&wrong).is_err());
        fs::write(
            store.path("test").unwrap(),
            vec![b' '; MAX_FILE_BYTES as usize + 1],
        )
        .unwrap();
        assert!(store.load("test").is_err());
        store.save(&profile()).unwrap();
        assert!(store.load("TEST").is_err());
        assert!(store.load("../test").is_err());
    }
    #[test]
    fn recovery_retains_original_image_and_checks_journal_has_not_changed() {
        let mut fixture = Fixture::new();
        let store = &mut fixture.0;
        let saved = transaction(&profile());
        store.begin(&saved).unwrap();
        finish(store, Completion::Applied);
        let mut changed = saved.clone();
        changed.original[0].value = Value::Integer(30);
        assert!(store.resume_restore(&changed).is_err());
        store.resume_restore(&saved).unwrap();
        assert!(store.pending().unwrap());
        finish(store, Completion::Restored);
        assert_eq!(store.last_transaction::<Value>().unwrap(), saved);
    }
    #[test]
    fn migration_retains_pending_barrier_and_never_overwrites_existing_data() {
        let mut source = Fixture::new();
        source.0.save(&profile()).unwrap();
        source.0.begin(&transaction(&profile())).unwrap();
        let original_bytes = fs::read(source.0.journal_path()).unwrap();
        let target = Fixture::new();
        assert_eq!(target.0.migrate_from(source.0.root()).unwrap(), 1);
        assert!(target.0.pending().unwrap());
        assert!(!target.0.startup().unwrap().enabled);
        assert_eq!(target.0.load("test").unwrap(), profile());
        assert_eq!(fs::read(source.0.journal_path()).unwrap(), original_bytes);
        assert!(target.0.migrate_from(source.0.root()).is_err());
        assert_eq!(target.0.load("test").unwrap(), profile());
    }
}
