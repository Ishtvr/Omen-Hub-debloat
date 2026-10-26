//! Durable cooling responsibility, separate from the service's STOP ledger.
//! Callers serialize mutations with the hardware lease. Paths never come from a record.
use omen_core::cooling_override::Checkpoint;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_RECORD: usize = 16 * 1024;
pub(crate) fn bounded_bytes(path: &Path, maximum: usize) -> Result<Option<Vec<u8>>, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = vec![];
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > maximum {
        return Err("Cooling record exceeds its bound".into());
    }
    Ok(Some(bytes))
}
pub(crate) struct CoolingJournal {
    root: PathBuf,
}
impl CoolingJournal {
    pub(crate) fn at(root: PathBuf) -> Result<Self, String> {
        if !root.is_absolute() {
            return Err("Cooling recovery requires an absolute protected directory".into());
        }
        Ok(Self { root })
    }
    fn path(&self) -> PathBuf {
        self.root.join("controller-cooling.json")
    }
    pub(crate) fn load(&self) -> Result<Option<Checkpoint>, String> {
        let Some(bytes) = bounded_bytes(&self.path(), MAX_RECORD)? else {
            return Ok(None);
        };
        let record: Checkpoint = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        record.validate()?;
        Ok(Some(record))
    }
    pub(crate) fn save(
        &self,
        expected: Option<&Checkpoint>,
        next: &Checkpoint,
    ) -> Result<(), String> {
        next.validate()?;
        let bytes = bounded_bytes(&self.path(), MAX_RECORD)?;
        let current = self.load()?;
        if current.as_ref() != expected {
            return Err("Cooling journal changed; it was not overwritten".into());
        }
        if let Some(previous) = current.as_ref() {
            if previous.owner == next.owner {
                let mut immutable = next.clone();
                immutable.phase = previous.phase;
                immutable.restore_requested_unix_ms = previous.restore_requested_unix_ms;
                if &immutable != previous {
                    return Err("Cooling recovery originals are immutable".into());
                }
                if previous.phase.complete() && previous != next {
                    return Err("A completed cooling duty cannot be reopened".into());
                }
            } else {
                if !previous.phase.complete() {
                    return Err("Pending cooling recovery cannot be replaced".into());
                }
                let bytes = bytes.ok_or("Cooling journal disappeared")?;
                let folder = self.root.join("controller-cooling-history");
                let archive = folder.join(format!("{:x}.json", Sha256::digest(&bytes)));
                if let Some(prior) = bounded_bytes(&archive, MAX_RECORD)? {
                    if prior != bytes {
                        return Err("Cooling history digest conflicts".into());
                    }
                } else {
                    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
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
    use omen_core::{
        cooling_override::{HelperIdentity, Phase},
        ownership::{Identity, Owner, Snapshot, State, Subject},
    };
    #[test]
    fn cooling_ledger_survives_reopen_and_retains_originals_on_conflict_or_corruption() {
        let root = std::env::temp_dir().join(format!(
            "OmenHubPP-cooling-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let journal = CoolingJournal::at(root.clone()).unwrap();
        let owner = Owner {
            user: "user".into(),
            process_id: 1,
            process_started: 10,
            boot: 1,
            session: "a".repeat(64),
        };
        let service = Snapshot {
            identity: Identity {
                subject: Subject::HpOmenCap,
                image_sha256: "b".repeat(64),
                configuration_sha256: "c".repeat(64),
            },
            state: State::Running,
            process_id: 2,
            process_started: 20,
            can_stop: true,
            has_dependents: false,
        };
        let original = Checkpoint::new(
            owner,
            service,
            HelperIdentity {
                binary_sha256: "d".repeat(64),
                process_id: 3,
                process_started: 30,
            },
            Some(b"{\"schema\":1,\"mode\":\"curve\"}".to_vec()),
            1000,
        )
        .unwrap();
        journal.save(None, &original).unwrap();
        assert_eq!(
            CoolingJournal::at(root.clone()).unwrap().load().unwrap(),
            Some(original.clone())
        );
        let mut changed = original.clone();
        changed.helper.process_started += 1;
        assert!(journal.save(Some(&original), &changed).is_err());
        changed = original.clone();
        changed.owner.session = "e".repeat(64);
        assert!(journal.save(Some(&original), &changed).is_err());
        let mut finished = original.clone();
        finished.phase = Phase::Restored;
        finished.restore_requested_unix_ms = Some(2000);
        journal.save(Some(&original), &finished).unwrap();
        let old = std::fs::read(journal.path()).unwrap();
        journal.save(Some(&finished), &changed).unwrap();
        assert!(journal.save(Some(&finished), &original).is_err());
        let archive = root
            .join("controller-cooling-history")
            .join(format!("{:x}.json", Sha256::digest(&old)));
        assert_eq!(std::fs::read(&archive).unwrap(), old);
        for bytes in [b"{".to_vec(), vec![b'x'; MAX_RECORD + 1]] {
            std::fs::write(journal.path(), bytes).unwrap();
            assert!(journal.load().is_err());
        }
        std::fs::remove_file(journal.path()).unwrap();
        std::fs::remove_file(archive).unwrap();
        std::fs::remove_dir(root.join("controller-cooling-history")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
