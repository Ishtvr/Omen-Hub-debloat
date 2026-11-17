//! Named immutable-in-profile curve snapshots and a request-level recovery log.
//! The caller owns the hardware lease and checks protected cooling duties.
//! File persistence is not an acknowledgement of physical policy uptake.
use super::*;
use omen_core::{
    fan_profile::{FanPreset, validate_curve},
    message::msg,
    profiles::FanPoint,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum FanPolicyOwner {
    Profile { id: String, digest: String },
    Library { id: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        store: ProfileStore,
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "omen-fan-tests-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            let store = ProfileStore::at(root.clone()).unwrap();
            fs::write(root.join("fixture.marker"), b"owned").unwrap();
            Self { store, root }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if let (Ok(root), Ok(base)) = (
                fs::canonicalize(&self.root),
                fs::canonicalize(std::env::temp_dir()),
            ) {
                if root.parent() == Some(base.as_path())
                    && root
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("omen-fan-tests-"))
                    && root.join("fixture.marker").is_file()
                {
                    let _ = fs::remove_dir_all(root);
                }
            }
        }
    }
    fn points(duty: u8) -> Vec<FanPoint> {
        vec![
            FanPoint {
                temperature_c: 30,
                duty_percent: duty,
            },
            FanPoint {
                temperature_c: 90,
                duty_percent: 100,
            },
        ]
    }
    fn owner() -> FanPolicyOwner {
        FanPolicyOwner::Profile {
            id: "performance-test".into(),
            digest: "a".repeat(64),
        }
    }
    fn preset() -> FanPreset {
        FanPreset {
            schema: 1,
            id: "fan-test".into(),
            name: "Quiet curve".into(),
            board: "8E35".into(),
            bios: "F.13".into(),
            points: points(30),
        }
    }
    #[test]
    fn library_updates_are_optimistic_and_never_change_profile_snapshots_or_permissions() {
        let f = Fixture::new();
        let s = &f.store;
        let mut p = preset();
        s.save_fan_preset(&p, None).unwrap();
        let snapshot = omen_core::profiles::PerformanceProfile {
            schema: 1,
            id: "performance-test".into(),
            name: "Performance".into(),
            binding: omen_core::profiles::DeviceBinding {
                manufacturer: "HP".into(),
                board: "8E35".into(),
                bios: "F.13".into(),
                cpu_names: vec![],
                nvidia_uuid: None,
                nvidia_driver: None,
                nvidia_vbios: None,
            },
            settings: vec![
                Setting {
                    feature: omen_core::features::Feature::CpuFanCurve,
                    value: omen_core::profiles::Value::FanCurve(p.points.clone()),
                },
                Setting {
                    feature: omen_core::features::Feature::GpuFanCurve,
                    value: omen_core::profiles::Value::FanCurve(p.points.clone()),
                },
            ],
        };
        s.save(&snapshot).unwrap();
        let profile_bytes = read_bytes(&f.root.join("profiles/performance-test.json")).unwrap();
        fs::write(f.root.join("startup.json"), b"permission sentinel").unwrap();
        fs::write(f.root.join("control-journal.json"), b"recovery sentinel").unwrap();
        let old = p.clone();
        p.points = points(40);
        p.name = "Updated".into();
        s.save_fan_preset(&p, Some(&old)).unwrap();
        assert!(s.save_fan_preset(&old, Some(&old)).is_err());
        assert_eq!(s.list().unwrap().len(), 1);
        assert_eq!(s.fan_presets().unwrap(), vec![p.clone()]);
        s.delete_fan_preset(&p).unwrap();
        assert!(s.fan_presets().unwrap().is_empty());
        assert_eq!(
            read_bytes(&f.root.join("profiles/performance-test.json")).unwrap(),
            profile_bytes
        );
        assert_eq!(
            read_bytes(&f.root.join("startup.json")).unwrap(),
            b"permission sentinel"
        );
        assert_eq!(
            read_bytes(&f.root.join("control-journal.json")).unwrap(),
            b"recovery sentinel"
        );
        assert_eq!(
            fs::read_dir(f.root.join("fan-preset-history"))
                .unwrap()
                .count(),
            1
        );
    }
    #[test]
    fn request_and_restore_retain_exact_original_bytes_and_never_touch_core_journal() {
        let f = Fixture::new();
        let s = &f.store;
        let mode = b"  {\"schema\":1,\"mode\":\"auto\"}\n";
        fs::write(f.root.join("fan-mode.json"), mode).unwrap();
        s.save_active_fan_curve(&points(25)).unwrap();
        let curve = read_bytes(&f.root.join("active-fan-curve.json")).unwrap();
        fs::write(f.root.join("control-journal.json"), b"untouched").unwrap();
        let prepared = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(45))
            .unwrap();
        assert!(s.fan_policy_pending().unwrap());
        assert_eq!(read_bytes(&f.root.join("fan-mode.json")).unwrap(), mode);
        s.commit_fan_policy(&prepared).unwrap();
        assert!(!s.fan_policy_pending().unwrap());
        assert!(
            s.fan_policy_matches("performance-test", &"a".repeat(64))
                .unwrap()
        );
        assert!(s.restore_fan_policy(&prepared).is_err());
        s.restore_fan_policy(&s.fan_policy_request().unwrap().unwrap())
            .unwrap();
        assert_eq!(read_bytes(&f.root.join("fan-mode.json")).unwrap(), mode);
        assert_eq!(
            read_bytes(&f.root.join("active-fan-curve.json")).unwrap(),
            curve
        );
        assert_eq!(
            read_bytes(&f.root.join("control-journal.json")).unwrap(),
            b"untouched"
        );
    }
    #[test]
    fn crash_after_first_file_restores_absence_on_reopen() {
        let f = Fixture::new();
        let s = &f.store;
        let prepared = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(30))
            .unwrap();
        atomic_bytes(
            &f.root.join("active-fan-curve.json"),
            prepared.after.curve.as_ref().unwrap(),
        )
        .unwrap();
        let reopened = ProfileStore::at(f.root.clone()).unwrap();
        assert!(reopened.fan_policy_pending().unwrap());
        reopened
            .restore_fan_policy(&reopened.fan_policy_request().unwrap().unwrap())
            .unwrap();
        assert!(!f.root.join("active-fan-curve.json").exists());
        assert!(!f.root.join("fan-mode.json").exists());
        assert!(!reopened.fan_policy_pending().unwrap());
    }
    #[test]
    fn foreign_preferences_and_a_pending_record_are_never_overwritten() {
        let f = Fixture::new();
        let s = &f.store;
        let p = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(30))
            .unwrap();
        let foreign = b"{\"schema\":1,\"mode\":\"max\"}";
        fs::write(f.root.join("fan-mode.json"), foreign).unwrap();
        assert!(s.commit_fan_policy(&p).is_err());
        assert!(s.restore_fan_policy(&p).is_err());
        assert!(
            s.prepare_fan_policy(owner(), "8E35", "F.13", &points(40))
                .is_err()
        );
        assert_eq!(read_bytes(&f.root.join("fan-mode.json")).unwrap(), foreign);
        assert!(s.fan_policy_pending().unwrap());
        assert!(!f.root.join("active-fan-curve.json").exists());
    }
    #[test]
    fn corrupt_originals_and_changed_journal_fail_before_preference_writes() {
        let f = Fixture::new();
        let s = &f.store;
        fs::write(f.root.join("fan-mode.json"), b"{}").unwrap();
        assert!(
            s.prepare_fan_policy(owner(), "8E35", "F.13", &points(30))
                .is_err()
        );
        assert!(!s.fan_policy_path().exists());
        fs::remove_file(f.root.join("fan-mode.json")).unwrap();
        let p = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(30))
            .unwrap();
        let mut foreign = p.clone();
        foreign.owner = FanPolicyOwner::Library {
            id: "another-owner".into(),
        };
        atomic_json(&s.fan_policy_path(), &foreign).unwrap();
        assert!(s.commit_fan_policy(&p).is_err());
        assert!(!f.root.join("active-fan-curve.json").exists());
    }
    #[test]
    fn a_conflict_during_restoration_becomes_a_durable_pending_barrier() {
        let f = Fixture::new();
        let s = &f.store;
        let request = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(30))
            .unwrap();
        s.commit_fan_policy(&request).unwrap();
        let requested = s.fan_policy_request().unwrap().unwrap();
        let foreign = b"{\"schema\":1,\"mode\":\"max\"}";
        fs::write(f.root.join("fan-mode.json"), foreign).unwrap();
        assert!(s.restore_fan_policy(&requested).is_err());
        assert!(s.fan_policy_pending().unwrap());
        assert_eq!(
            s.fan_policy_request().unwrap().unwrap().phase,
            FanPolicyPhase::Recovering
        );
        assert!(
            s.prepare_fan_policy(owner(), "8E35", "F.13", &points(40))
                .is_err()
        );
        assert_eq!(read_bytes(&f.root.join("fan-mode.json")).unwrap(), foreign);
    }
    fn direct_checkpoint(store: &mut ProfileStore, original: i64, target: i64, digest: &str) {
        use omen_core::{
            features::Feature,
            profiles::{DeviceBinding, Value},
            transaction::{Captured, PendingTransaction},
        };
        let transaction = PendingTransaction {
            schema: 1,
            profile_id: "performance-test".into(),
            profile_digest: digest.into(),
            binding: DeviceBinding {
                manufacturer: "HP".into(),
                board: "8E35".into(),
                bios: "F.13".into(),
                cpu_names: vec![],
                nvidia_uuid: None,
                nvidia_driver: None,
                nvidia_vbios: None,
            },
            original: vec![Captured {
                feature: Feature::CpuTemperatureLimitC,
                value: Value::Integer(original),
                restore: Value::Integer(original),
            }],
            requested: vec![Setting {
                feature: Feature::CpuTemperatureLimitC,
                value: Value::Integer(target),
            }],
        };
        <ProfileStore as Journal<Value>>::begin(store, &transaction).unwrap();
        <ProfileStore as Journal<Value>>::finish(store, Completion::Applied).unwrap();
    }
    #[test]
    fn interruption_after_direct_applied_is_not_mistaken_for_a_fan_only_duty() {
        use omen_core::profiles::Value;
        let mut f = Fixture::new();
        let digest = "a".repeat(64);
        let s = &mut f.store;
        let request = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(30))
            .unwrap();
        assert!(!s.fan_pending_direct_applied().unwrap());
        direct_checkpoint(s, 100, 85, &digest);
        assert!(!s.pending().unwrap());
        assert!(s.fan_policy_pending().unwrap());
        assert!(s.fan_pending_direct_applied().unwrap());
        // The verified direct recovery completes before the preference duty.
        let saved = s.last_transaction::<Value>().unwrap();
        <ProfileStore as Journal<Value>>::resume_restore(s, &saved).unwrap();
        <ProfileStore as Journal<Value>>::finish(s, Completion::Restored).unwrap();
        assert!(!s.fan_pending_direct_applied().unwrap());
        s.restore_fan_policy(&request).unwrap();
        assert!(!s.fan_policy_pending().unwrap());
    }
    #[test]
    fn reapplying_the_same_revision_does_not_own_the_previous_checkpoint() {
        let mut f = Fixture::new();
        let digest = "a".repeat(64);
        let s = &mut f.store;
        direct_checkpoint(s, 85, 85, &digest);
        let previous_bytes = read_bytes(&s.journal_path()).unwrap();
        let request = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(30))
            .unwrap();
        assert!(
            !s.fan_pending_direct_applied().unwrap(),
            "Same id/digest is not a new direct operation"
        );
        direct_checkpoint(s, 85, 85, &digest);
        assert_eq!(read_bytes(&s.journal_path()).unwrap(), previous_bytes);
        assert!(
            s.fan_pending_direct_applied().unwrap(),
            "The actual new checkpoint transition is owned"
        );
        assert_eq!(request.owner, owner());
    }
    #[test]
    fn early_aborted_replacement_retains_the_previous_fan_owners_originals() {
        let mut f = Fixture::new();
        let s = &mut f.store;
        direct_checkpoint(s, 100, 85, &"a".repeat(64));
        let first = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(30))
            .unwrap();
        s.commit_fan_policy(&first).unwrap();
        let active = s.fan_policy_request().unwrap().unwrap();
        let next = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(45))
            .unwrap();
        // Helper/prelude failure before direct Begin retains the old owner.
        s.restore_fan_policy(&next).unwrap();
        assert_eq!(s.fan_policy_request().unwrap().unwrap(), active);
        assert!(
            s.fan_policy_matches("performance-test", &"a".repeat(64))
                .unwrap()
        );
        s.restore_fan_policy(&active).unwrap();
        assert!(!f.root.join("fan-mode.json").exists());
        assert!(!f.root.join("active-fan-curve.json").exists());
    }
    #[test]
    fn a_missing_previous_record_keeps_an_aborted_replacement_pending() {
        let f = Fixture::new();
        let s = &f.store;
        let first = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(30))
            .unwrap();
        s.commit_fan_policy(&first).unwrap();
        let next = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(45))
            .unwrap();
        let history = s.root.join("fan-policy-history").join(format!(
            "{}.json",
            next.previous_record_sha256.as_ref().unwrap()
        ));
        let bytes = read_bytes(&history).unwrap();
        fs::remove_file(&history).unwrap();
        assert!(s.restore_fan_policy(&next).is_err());
        assert!(s.fan_policy_pending().unwrap());
        assert_eq!(s.fan_images().unwrap(), next.before);
        fs::write(&history, b"corrupt archive").unwrap();
        assert!(s.restore_fan_policy(&next).is_err());
        assert!(s.fan_policy_pending().unwrap());
        // Retry the interrupted reattach after its exact archive is available.
        atomic_bytes(&history, &bytes).unwrap();
        s.restore_fan_policy(&next).unwrap();
        assert_eq!(
            s.fan_policy_request().unwrap().unwrap().phase,
            FanPolicyPhase::Requested
        );
    }
    #[test]
    fn an_external_fan_choice_is_not_reclaimed_by_previous_profile_metadata() {
        let f = Fixture::new();
        let s = &f.store;
        let first = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(30))
            .unwrap();
        s.commit_fan_policy(&first).unwrap();
        s.save_fan_mode(FanMode::Max).unwrap();
        let next = s
            .prepare_fan_policy(owner(), "8E35", "F.13", &points(45))
            .unwrap();
        s.restore_fan_policy(&next).unwrap();
        assert_eq!(s.fan_mode(), FanMode::Max);
        assert_eq!(
            s.fan_policy_request().unwrap().unwrap().phase,
            FanPolicyPhase::Restored
        );
        assert!(
            !s.fan_policy_matches("performance-test", &"a".repeat(64))
                .unwrap()
        );
    }
    #[test]
    fn edited_or_deleted_library_revisions_cannot_be_requested_from_a_stale_ui() {
        let f = Fixture::new();
        let s = &f.store;
        let old = preset();
        s.save_fan_preset(&old, None).unwrap();
        let mut new = old.clone();
        new.points = points(45);
        s.save_fan_preset(&new, Some(&old)).unwrap();
        assert!(s.prepare_library_fan_policy(&old).is_err());
        assert!(!s.fan_policy_path().exists());
        s.delete_fan_preset(&new).unwrap();
        assert!(s.prepare_library_fan_policy(&new).is_err());
        assert!(!s.fan_policy_path().exists());
        assert!(!f.root.join("fan-mode.json").exists());
        assert!(!f.root.join("active-fan-curve.json").exists());
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FanPolicyPhase {
    Prepared,
    Requested,
    Recovering,
    Restored,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Images {
    mode: Option<Vec<u8>>,
    curve: Option<Vec<u8>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanPolicyRequest {
    schema: u32,
    pub owner: FanPolicyOwner,
    pub board: String,
    pub bios: String,
    pub phase: FanPolicyPhase,
    direct_before_sha256: Option<String>,
    previous_record_sha256: Option<String>,
    before: Images,
    after: Images,
}
fn image(path: &Path) -> Result<Option<Vec<u8>>, String> {
    if !path.try_exists().map_err(|e| e.to_string())? {
        return Ok(None);
    }
    read_bytes(path).map(Some)
}
fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn validate_images(images: &Images) -> Result<(), String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Mode {
        schema: u32,
        mode: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Curve {
        schema: u32,
        points: Vec<FanPoint>,
    }
    if let Some(bytes) = &images.mode {
        let value: Mode =
            serde_json::from_slice(bytes).map_err(|_| msg("fan_profile.err.original", &[]))?;
        if value.schema != 1 || !matches!(value.mode.as_str(), "bios" | "curve" | "max" | "auto") {
            return Err(msg("fan_profile.err.original", &[]));
        }
    }
    if let Some(bytes) = &images.curve {
        let value: Curve =
            serde_json::from_slice(bytes).map_err(|_| msg("fan_profile.err.original", &[]))?;
        if value.schema != 1 {
            return Err(msg("fan_profile.err.original", &[]));
        }
        validate_curve(&value.points)?;
    }
    Ok(())
}
impl FanPolicyRequest {
    fn validate(&self) -> Result<(), String> {
        if [&self.direct_before_sha256, &self.previous_record_sha256]
            .into_iter()
            .flatten()
            .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(msg("fan_profile.err.journal", &[]));
        }
        match &self.owner {
            FanPolicyOwner::Profile { id, digest } => {
                validate_id(id)?;
                if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(msg("fan_profile.err.journal", &[]));
                }
            }
            FanPolicyOwner::Library { id } => validate_id(id)?,
        }
        if self.schema != 1 || self.board.is_empty() || self.bios.is_empty() {
            return Err(msg("fan_profile.err.journal", &[]));
        }
        validate_images(&self.before)?;
        validate_images(&self.after)?;
        let mode: serde_json::Value = serde_json::from_slice(
            self.after
                .mode
                .as_deref()
                .ok_or_else(|| msg("fan_profile.err.journal", &[]))?,
        )
        .map_err(|e| e.to_string())?;
        if mode["mode"] != "curve" || self.after.curve.is_none() {
            return Err(msg("fan_profile.err.journal", &[]));
        }
        Ok(())
    }
    fn owns(&self, current: &Images) -> bool {
        current == &self.before
            || current == &self.after
            || (current.mode == self.before.mode && current.curve == self.after.curve)
    }
    pub fn belongs_to(&self, id: &str, digest: &str) -> bool {
        matches!(&self.owner,FanPolicyOwner::Profile{id:owner_id,digest:owner_digest} if owner_id==id&&owner_digest==digest)
    }
}
impl ProfileStore {
    fn fan_policy_path(&self) -> PathBuf {
        self.root.join("fan-profile-journal.json")
    }
    fn fan_images(&self) -> Result<Images, String> {
        Ok(Images {
            mode: image(&self.root.join("fan-mode.json"))?,
            curve: image(&self.root.join("active-fan-curve.json"))?,
        })
    }
    pub fn fan_presets(&self) -> Result<Vec<FanPreset>, String> {
        let path = self.root.join("fan-presets");
        if !path.try_exists().map_err(|e| e.to_string())? {
            return Ok(vec![]);
        }
        let mut list = vec![];
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            if list.len() >= 128 {
                return Err(msg("fan_profile.err.library_limit", &[]));
            }
            let p: FanPreset = read_json(&path)?;
            p.validate()?;
            if path.file_stem().and_then(|s| s.to_str()) != Some(p.id.as_str()) {
                return Err(msg("fan_profile.err.library", &[]));
            }
            list.push(p);
        }
        list.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
        Ok(list)
    }
    pub fn save_fan_preset(
        &self,
        preset: &FanPreset,
        expected: Option<&FanPreset>,
    ) -> Result<(), String> {
        #[cfg(windows)]
        let _lock = lock_fan_preference(&self.root)?;
        preset.validate()?;
        let folder = self.root.join("fan-presets");
        fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let path = folder.join(format!("{}.json", preset.id));
        let old = if path.try_exists().map_err(|e| e.to_string())? {
            Some(read_json::<FanPreset>(&path)?)
        } else {
            None
        };
        if old.as_ref() != expected {
            return Err(msg("fan_profile.err.library_changed", &[]));
        }
        if old.is_none() && self.fan_presets()?.len() >= 128 {
            return Err(msg("fan_profile.err.library_limit", &[]));
        }
        atomic_json(&path, preset)
    }
    pub fn delete_fan_preset(&self, preset: &FanPreset) -> Result<(), String> {
        #[cfg(windows)]
        let _lock = lock_fan_preference(&self.root)?;
        preset.validate()?;
        let path = self
            .root
            .join("fan-presets")
            .join(format!("{}.json", preset.id));
        let current: FanPreset = read_json(&path)?;
        if &current != preset {
            return Err(msg("fan_profile.err.library_changed", &[]));
        }
        let bytes = read_bytes(&path)?;
        let history = self.root.join("fan-preset-history");
        fs::create_dir_all(&history).map_err(|e| e.to_string())?;
        atomic_bytes(
            &history.join(format!("{}-{:x}.json", preset.id, Sha256::digest(&bytes))),
            &bytes,
        )?;
        // Existing performance profiles retain their point snapshots.
        fs::remove_file(path).map_err(|e| e.to_string())
    }
    pub fn fan_policy_request(&self) -> Result<Option<FanPolicyRequest>, String> {
        let Some(bytes) = image(&self.fan_policy_path())? else {
            return Ok(None);
        };
        let request: FanPolicyRequest =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        request.validate()?;
        Ok(Some(request))
    }
    pub fn fan_policy_pending(&self) -> Result<bool, String> {
        Ok(self.fan_policy_request()?.is_some_and(|r| {
            matches!(
                r.phase,
                FanPolicyPhase::Prepared | FanPolicyPhase::Recovering
            )
        }))
    }
    /// Caller has checked helper/model/ownership before this durable capture,
    /// which must precede the non-recoverable HP prelude as well as direct SET.
    pub fn prepare_fan_policy(
        &self,
        owner: FanPolicyOwner,
        board: &str,
        bios: &str,
        points: &[FanPoint],
    ) -> Result<FanPolicyRequest, String> {
        #[cfg(windows)]
        let _lock = lock_fan_preference(&self.root)?;
        self.prepare_fan_policy_locked(owner, board, bios, points)
    }
    /// Saved library Apply must not silently activate an edited/deleted stale
    /// UI clone under the old name. Compare revision and capture in one lock.
    pub fn prepare_library_fan_policy(
        &self,
        preset: &FanPreset,
    ) -> Result<FanPolicyRequest, String> {
        #[cfg(windows)]
        let _lock = lock_fan_preference(&self.root)?;
        preset.validate()?;
        let current: FanPreset = read_json(
            &self
                .root
                .join("fan-presets")
                .join(format!("{}.json", preset.id)),
        )
        .map_err(|_| msg("fan_profile.err.library_changed", &[]))?;
        if &current != preset {
            return Err(msg("fan_profile.err.library_changed", &[]));
        }
        self.prepare_fan_policy_locked(
            FanPolicyOwner::Library {
                id: preset.id.clone(),
            },
            &preset.board,
            &preset.bios,
            &preset.points,
        )
    }
    fn prepare_fan_policy_locked(
        &self,
        owner: FanPolicyOwner,
        board: &str,
        bios: &str,
        points: &[FanPoint],
    ) -> Result<FanPolicyRequest, String> {
        validate_curve(points)?;
        let previous_record_sha256 = if let Some(previous) = self.fan_policy_request()? {
            if matches!(
                previous.phase,
                FanPolicyPhase::Prepared | FanPolicyPhase::Recovering
            ) {
                return Err(msg("fan_profile.err.pending", &[]));
            }
            let bytes = read_bytes(&self.fan_policy_path())?;
            let folder = self.root.join("fan-policy-history");
            fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            let hash = format!("{:x}", Sha256::digest(&bytes));
            atomic_bytes(&folder.join(format!("{hash}.json")), &bytes)?;
            Some(hash)
        } else {
            None
        };
        let request = FanPolicyRequest {
            schema: 1,
            owner,
            board: board.into(),
            bios: bios.into(),
            phase: FanPolicyPhase::Prepared,
            direct_before_sha256: self.direct_journal_fingerprint()?,
            previous_record_sha256,
            before: self.fan_images()?,
            after: Images {
                mode: Some(json_bytes(&serde_json::json!({"schema":1,"mode":"curve"}))?),
                curve: Some(json_bytes(
                    &serde_json::json!({"schema":1,"points":points}),
                )?),
            },
        };
        request.validate()?;
        atomic_json(&self.fan_policy_path(), &request)?;
        Ok(request)
    }
    fn direct_journal_fingerprint(&self) -> Result<Option<String>, String> {
        let file = match fs::File::open(self.journal_path()) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        (&file)
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(msg("store.err.size", &[]));
        }
        // Atomic replacement may produce byte-identical transaction JSON. Bind
        // the content to its file identity without adding a field that the
        // installed legacy startup/helper journal decoder would reject.
        let mut hash = Sha256::new();
        hash.update(&bytes);
        #[cfg(windows)]
        {
            use std::os::windows::{fs::MetadataExt, io::AsRawHandle};
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx,
            };
            let mut id = FILE_ID_INFO::default();
            // SAFETY: file owns this live handle, id is an exact sized writable
            // FILE_ID_INFO buffer for the documented FileIdInfo class.
            if unsafe {
                GetFileInformationByHandleEx(
                    file.as_raw_handle(),
                    FileIdInfo,
                    (&mut id as *mut FILE_ID_INFO).cast(),
                    std::mem::size_of::<FILE_ID_INFO>() as u32,
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            hash.update(id.VolumeSerialNumber.to_le_bytes());
            hash.update(id.FileId.Identifier);
            hash.update(
                file.metadata()
                    .map_err(|e| e.to_string())?
                    .creation_time()
                    .to_le_bytes(),
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = file.metadata().map_err(|e| e.to_string())?;
            hash.update(meta.dev().to_le_bytes());
            hash.update(meta.ino().to_le_bytes());
            hash.update(meta.ctime().to_le_bytes());
            hash.update(meta.ctime_nsec().to_le_bytes());
        }
        Ok(Some(format!("{:x}", hash.finalize())))
    }
    /// A completed direct checkpoint can still belong to an interrupted combined
    /// operation. ID/digest alone cannot distinguish reapplying the same revision:
    /// compare the actual checkpoint transition captured before the prelude.
    pub fn fan_pending_direct_applied(&self) -> Result<bool, String> {
        let Some(request) = self.fan_policy_request()?.filter(|r| {
            matches!(
                r.phase,
                FanPolicyPhase::Prepared | FanPolicyPhase::Recovering
            )
        }) else {
            return Ok(false);
        };
        if !matches!(&request.owner, FanPolicyOwner::Profile { .. })
            || self.direct_journal_fingerprint()? == request.direct_before_sha256
        {
            return Ok(false);
        }
        let Some((id, digest, _)) = self.applied_settings()? else {
            return Ok(false);
        };
        Ok(request.belongs_to(&id, &digest))
    }
    pub fn commit_fan_policy(&self, expected: &FanPolicyRequest) -> Result<(), String> {
        #[cfg(windows)]
        let _lock = lock_fan_preference(&self.root)?;
        if self.fan_policy_request()?.as_ref() != Some(expected)
            || expected.phase != FanPolicyPhase::Prepared
        {
            return Err(msg("fan_profile.err.journal_changed", &[]));
        }
        if self.fan_images()? != expected.before {
            return Err(msg("fan_profile.err.foreign", &[]));
        }
        atomic_bytes(
            &self.root.join("active-fan-curve.json"),
            expected.after.curve.as_ref().unwrap(),
        )?;
        let intermediate = Images {
            mode: expected.before.mode.clone(),
            curve: expected.after.curve.clone(),
        };
        if self.fan_images()? != intermediate {
            return Err(msg("fan_profile.err.foreign", &[]));
        }
        atomic_bytes(
            &self.root.join("fan-mode.json"),
            expected.after.mode.as_ref().unwrap(),
        )?;
        let mut next = expected.clone();
        next.phase = FanPolicyPhase::Requested;
        atomic_json(&self.fan_policy_path(), &next)
    }
    /// Restores exact owned preference images, not a claim of firmware BIOS
    /// ownership. Foreign bytes and a pending journal remain untouched.
    pub fn restore_fan_policy(&self, expected: &FanPolicyRequest) -> Result<(), String> {
        #[cfg(windows)]
        let _lock = lock_fan_preference(&self.root)?;
        if self.fan_policy_request()?.as_ref() != Some(expected) {
            return Err(msg("fan_profile.err.journal_changed", &[]));
        }
        if expected.phase == FanPolicyPhase::Restored {
            return Ok(());
        }
        let mut pending = expected.clone();
        if expected.phase == FanPolicyPhase::Requested {
            pending.phase = FanPolicyPhase::Recovering;
            atomic_json(&self.fan_policy_path(), &pending)?;
        }
        if !expected.owns(&self.fan_images()?) {
            return Err(msg("fan_profile.err.foreign", &[]));
        }
        for (name, bytes) in [
            ("fan-mode.json", &expected.before.mode),
            ("active-fan-curve.json", &expected.before.curve),
        ] {
            if self.fan_policy_request()?.as_ref() != Some(&pending) {
                return Err(msg("fan_profile.err.journal_changed", &[]));
            }
            if !expected.owns(&self.fan_images()?) {
                return Err(msg("fan_profile.err.foreign", &[]));
            }
            let path = self.root.join(name);
            if let Some(bytes) = bytes {
                atomic_bytes(&path, bytes)?;
            } else if path.try_exists().map_err(|e| e.to_string())? {
                fs::remove_file(path).map_err(|e| e.to_string())?;
            }
        }
        if self.fan_images()? != expected.before {
            return Err(msg("fan_profile.err.foreign", &[]));
        }
        if self.fan_policy_request()?.as_ref() != Some(&pending) {
            return Err(msg("fan_profile.err.journal_changed", &[]));
        }
        let mut next = expected.clone();
        next.phase = FanPolicyPhase::Restored;
        // A failed replacement before the direct checkpoint changed must not
        // discard the previous still-applied fan owner's original image. Keep
        // B pending until this single final journal swap to A is durable.
        if self.direct_journal_fingerprint()? == expected.direct_before_sha256
            && let Some(previous_hash) = &expected.previous_record_sha256
        {
            let path = self
                .root
                .join("fan-policy-history")
                .join(format!("{previous_hash}.json"));
            let bytes = read_bytes(&path)?;
            if format!("{:x}", Sha256::digest(&bytes)) != *previous_hash {
                return Err(msg("fan_profile.err.journal", &[]));
            }
            let previous: FanPolicyRequest =
                serde_json::from_slice(&bytes).map_err(|_| msg("fan_profile.err.journal", &[]))?;
            previous.validate()?;
            if previous.board != expected.board
                || previous.bios != expected.bios
                || !matches!(
                    previous.phase,
                    FanPolicyPhase::Requested | FanPolicyPhase::Restored
                )
            {
                return Err(msg("fan_profile.err.journal", &[]));
            }
            let previous_current = if previous.phase == FanPolicyPhase::Requested {
                &previous.after
            } else {
                &previous.before
            };
            if previous_current == &expected.before {
                let completed = json_bytes(&next)?;
                atomic_bytes(
                    &self
                        .root
                        .join("fan-policy-history")
                        .join(format!("{:x}.json", Sha256::digest(&completed))),
                    &completed,
                )?;
                return atomic_bytes(&self.fan_policy_path(), &bytes);
            }
        }
        atomic_json(&self.fan_policy_path(), &next)
    }
    pub fn fan_policy_matches(&self, id: &str, digest: &str) -> Result<bool, String> {
        let Some(request) = self.fan_policy_request()? else {
            return Ok(false);
        };
        Ok(request.phase == FanPolicyPhase::Requested
            && request.belongs_to(id, digest)
            && self.fan_images()? == request.after)
    }
}
