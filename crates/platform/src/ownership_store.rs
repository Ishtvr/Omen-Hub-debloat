//! Separate bounded recovery ledger. The Windows caller supplies the protected
//! installation directory and owns the machine-wide lifetime lease.
use omen_core::ownership::{Journal, Record};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::PathBuf};

const MAX_BYTES: u64 = 16 * 1024;
pub(crate) struct OwnershipJournal {
    root: PathBuf,
}
impl OwnershipJournal {
    pub(crate) fn at(root: PathBuf) -> Result<Self, String> {
        if !root.is_absolute() {
            return Err("Controller recovery requires an absolute protected directory".into());
        }
        Ok(Self { root })
    }
    fn path(&self) -> PathBuf {
        self.root.join("controller-ownership.json")
    }
    fn bytes(&self) -> Result<Option<Vec<u8>>, String> {
        let file = match File::open(self.path()) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("Controller recovery record exceeds its bound".into());
        }
        Ok(Some(bytes))
    }
}
impl Journal for OwnershipJournal {
    fn load(&self) -> Result<Option<Record>, String> {
        let Some(bytes) = self.bytes()? else {
            return Ok(None);
        };
        let record: Record = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        record.validate()?;
        Ok(Some(record))
    }
    fn save(&mut self, expected: Option<&Record>, next: &Record) -> Result<(), String> {
        next.validate()?;
        let current = self.load()?;
        if current.as_ref() != expected {
            return Err("Controller journal changed; it was not overwritten".into());
        }
        if let Some(previous) = current.as_ref() {
            if previous.owner == next.owner && previous.original != next.original {
                return Err("Controller recovery originals are immutable".into());
            }
            if previous.owner != next.owner {
                if !previous.phase.complete() {
                    return Err("Pending controller recovery cannot be replaced".into());
                }
                let bytes = self.bytes()?.ok_or("Controller journal disappeared")?;
                let archive = self
                    .root
                    .join("controller-history")
                    .join(format!("{:x}.json", Sha256::digest(&bytes)));
                if archive.try_exists().map_err(|e| e.to_string())? {
                    let mut prior = vec![];
                    File::open(&archive)
                        .map_err(|e| e.to_string())?
                        .take(MAX_BYTES + 1)
                        .read_to_end(&mut prior)
                        .map_err(|e| e.to_string())?;
                    if prior != bytes {
                        return Err("Controller history digest conflicts".into());
                    }
                } else {
                    std::fs::create_dir_all(self.root.join("controller-history"))
                        .map_err(|e| e.to_string())?;
                    crate::profile_store::atomic_bytes(&archive, &bytes)?;
                }
            }
        }
        crate::profile_store::atomic_json(&self.path(), next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omen_core::ownership::{Identity, Owner, Phase, Snapshot, State, Subject};

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "OmenHubPP-ownership-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            // Only individual files inside this freshly created test directory.
            let _ = std::fs::remove_file(self.0.join("controller-ownership.json"));
            let history = self.0.join("controller-history");
            if let Ok(files) = std::fs::read_dir(&history) {
                for file in files.flatten() {
                    let name = file.file_name().to_string_lossy().into_owned();
                    if name.len() == 69
                        && name.ends_with(".json")
                        && name.as_bytes()[..64].iter().all(|c| c.is_ascii_hexdigit())
                    {
                        let _ = std::fs::remove_file(file.path());
                    }
                }
            }
            let _ = std::fs::remove_dir(history);
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    fn record() -> Record {
        Record {
            schema: 1,
            owner: Owner {
                user: "user".into(),
                process_id: 10,
                process_started: 100,
                boot: 1,
                session: "a".repeat(64),
            },
            original: Snapshot {
                identity: Identity {
                    subject: Subject::HpOmenCap,
                    image_sha256: "b".repeat(64),
                    configuration_sha256: "c".repeat(64),
                },
                state: State::Running,
                process_id: 20,
                process_started: 200,
                can_stop: true,
                has_dependents: false,
            },
            phase: Phase::StopPending,
        }
    }
    #[test]
    fn disk_ledger_reopens_and_rejects_foreign_or_changed_originals() {
        let root = TestDirectory::new();
        let mut journal = OwnershipJournal::at(root.0.clone()).unwrap();
        let pending = record();
        journal.save(None, &pending).unwrap();
        let reopened = OwnershipJournal::at(root.0.clone()).unwrap();
        assert_eq!(reopened.load().unwrap(), Some(pending.clone()));
        let mut foreign = pending.clone();
        foreign.owner.session = "d".repeat(64);
        assert!(journal.save(Some(&pending), &foreign).is_err());
        let mut changed = pending.clone();
        changed.original.process_id += 1;
        assert!(journal.save(Some(&pending), &changed).is_err());
        assert_eq!(journal.load().unwrap(), Some(pending));
    }
    #[test]
    fn completed_history_is_retained_and_stale_writers_cannot_overwrite() {
        let root = TestDirectory::new();
        let mut journal = OwnershipJournal::at(root.0.clone()).unwrap();
        let pending = record();
        journal.save(None, &pending).unwrap();
        let mut finished = pending.clone();
        finished.phase = Phase::Restored;
        journal.save(Some(&pending), &finished).unwrap();
        let previous = std::fs::read(journal.path()).unwrap();
        let mut next = pending.clone();
        next.owner.session = "d".repeat(64);
        journal.save(Some(&finished), &next).unwrap();
        assert!(journal.save(Some(&finished), &pending).is_err());
        let archive = root
            .0
            .join("controller-history")
            .join(format!("{:x}.json", Sha256::digest(&previous)));
        assert_eq!(std::fs::read(archive).unwrap(), previous);
    }
    #[test]
    fn corrupt_oversized_and_unknown_fields_are_not_empty_ledgers() {
        let root = TestDirectory::new();
        let journal = OwnershipJournal::at(root.0.clone()).unwrap();
        for bytes in [b"{".to_vec(), vec![b'x'; MAX_BYTES as usize + 1]] {
            std::fs::write(journal.path(), bytes).unwrap();
            assert!(journal.load().is_err());
        }
        let mut raw = serde_json::to_value(record()).unwrap();
        raw["command"] = "unexpected".into();
        std::fs::write(journal.path(), serde_json::to_vec(&raw).unwrap()).unwrap();
        assert!(journal.load().is_err());
    }
}
