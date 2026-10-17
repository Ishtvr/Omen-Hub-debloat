//! CCD (core complex) affinity for the program in the foreground. Pure policy:
//! the Windows background service supplies the topology and process access.
//!
//! A selection is a bit set: bit 0 = first CCD, bit 1 = second CCD. All bits
//! set means "no restriction": Windows schedules freely and every program the
//! service pinned gets its original affinity back.
use crate::message::msg;
use std::collections::BTreeMap;

/// Upper bound of programs whose original affinity is remembered at once.
const MAX_TRACKED: usize = 256;

/// The selection that leaves every CCD enabled.
pub fn all(ccds: usize) -> u8 {
    ((1u16 << ccds.min(8)) - 1) as u8
}

/// Checks a profile value against the CCD count of this processor.
pub fn validate(selection: i64, ccds: usize) -> Result<u8, String> {
    if !(2..=8).contains(&ccds) {
        return Err(msg("ccd.err.topology", &[&ccds]));
    }
    match u8::try_from(selection) {
        Ok(s) if (1..=all(ccds)).contains(&s) => Ok(s),
        _ => Err(msg("ccd.err.range", &[&selection, &all(ccds)])),
    }
}

/// The affinity mask for `selection`, or None when every CCD is selected.
/// `ccd_masks` lists the logical-processor mask of each CCD in order.
pub fn affinity(selection: u8, ccd_masks: &[usize]) -> Result<Option<usize>, String> {
    let selection = validate(i64::from(selection), ccd_masks.len())?;
    if selection == all(ccd_masks.len()) {
        return Ok(None);
    }
    Ok(Some(
        ccd_masks
            .iter()
            .enumerate()
            .filter(|(i, _)| selection & (1 << i) != 0)
            .fold(0, |mask, (_, ccd)| mask | ccd),
    ))
}

/// A process identity that survives PID reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessKey {
    pub pid: u32,
    /// Creation time in the platform's own unit.
    pub created: u64,
}

pub trait Processes {
    /// The program owning the foreground window, if it may be pinned.
    fn foreground(&mut self) -> Option<ProcessKey>;
    /// Current affinity; an error means the process is gone or protected.
    fn affinity(&mut self, key: ProcessKey) -> Result<usize, String>;
    fn set_affinity(&mut self, key: ProcessKey, mask: usize) -> Result<(), String>;
}

/// Pins each program that comes to the foreground to the target CCDs and
/// remembers its previous affinity, so "all CCDs" can put it back.
#[derive(Debug, Default)]
pub struct Enforcer {
    target: Option<usize>,
    originals: BTreeMap<ProcessKey, usize>,
}

impl Enforcer {
    /// Programs currently pinned by this enforcer.
    pub fn pinned(&self) -> usize {
        self.originals.len()
    }

    /// One service cycle. Returns the problems of this cycle (protected or
    /// vanished programs are skipped, never retried in a loop).
    pub fn tick(&mut self, processes: &mut impl Processes, target: Option<usize>) -> Vec<String> {
        let mut errors = vec![];
        if target != self.target {
            // A new selection moves every pinned program; "all" releases them.
            let keys: Vec<_> = self.originals.keys().copied().collect();
            for key in keys {
                let mask = target.unwrap_or(self.originals[&key]);
                if let Err(e) = processes.set_affinity(key, mask) {
                    errors.push(e);
                    self.originals.remove(&key);
                }
            }
            if target.is_none() {
                self.originals.clear();
            }
            self.target = target;
        }
        let Some(mask) = target else {
            return errors;
        };
        let Some(key) = processes.foreground() else {
            return errors;
        };
        match processes.affinity(key) {
            // Already inside the selection (pinned before, or restricted by
            // the program itself): leave it alone.
            Ok(current) if current & !mask == 0 => {}
            Ok(current) => {
                if self.originals.len() >= MAX_TRACKED {
                    self.prune(processes);
                }
                if self.originals.len() < MAX_TRACKED {
                    match processes.set_affinity(key, mask) {
                        Ok(()) => {
                            self.originals.entry(key).or_insert(current);
                        }
                        Err(e) => errors.push(e),
                    }
                }
            }
            Err(e) => errors.push(e),
        }
        errors
    }

    /// Puts every pinned program back (service shutdown).
    pub fn release(&mut self, processes: &mut impl Processes) {
        for (key, original) in std::mem::take(&mut self.originals) {
            let _ = processes.set_affinity(key, original);
        }
        self.target = None;
    }

    fn prune(&mut self, processes: &mut impl Processes) {
        self.originals
            .retain(|key, _| processes.affinity(*key).is_ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        foreground: Option<ProcessKey>,
        masks: BTreeMap<ProcessKey, usize>,
        protected: Vec<ProcessKey>,
    }
    impl Processes for Fake {
        fn foreground(&mut self) -> Option<ProcessKey> {
            self.foreground
        }
        fn affinity(&mut self, key: ProcessKey) -> Result<usize, String> {
            self.masks.get(&key).copied().ok_or("gone".into())
        }
        fn set_affinity(&mut self, key: ProcessKey, mask: usize) -> Result<(), String> {
            if self.protected.contains(&key) {
                return Err("denied".into());
            }
            self.masks
                .get_mut(&key)
                .map(|m| *m = mask)
                .ok_or("gone".into())
        }
    }
    const CCDS: [usize; 2] = [0x0000_ffff, 0xffff_0000];
    fn key(pid: u32) -> ProcessKey {
        ProcessKey { pid, created: 7 }
    }

    #[test]
    fn selection_maps_to_ccd_masks() {
        assert_eq!(affinity(1, &CCDS).unwrap(), Some(0x0000_ffff));
        assert_eq!(affinity(2, &CCDS).unwrap(), Some(0xffff_0000));
        assert_eq!(affinity(3, &CCDS).unwrap(), None);
        assert!(affinity(0, &CCDS).is_err());
        assert!(affinity(4, &CCDS).is_err());
        assert!(validate(1, 1).is_err());
        assert_eq!(all(2), 3);
    }

    #[test]
    fn foreground_programs_are_pinned_moved_and_released() {
        let mut fake = Fake::default();
        fake.masks.insert(key(1), 0xffff_ffff);
        fake.masks.insert(key(2), 0xffff_ffff);
        let mut enforcer = Enforcer::default();
        fake.foreground = Some(key(1));
        assert!(enforcer.tick(&mut fake, Some(CCDS[0])).is_empty());
        fake.foreground = Some(key(2));
        enforcer.tick(&mut fake, Some(CCDS[0]));
        assert_eq!(fake.masks[&key(1)], CCDS[0]);
        assert_eq!(fake.masks[&key(2)], CCDS[0]);
        assert_eq!(enforcer.pinned(), 2);
        // Switching to the second CCD moves both pinned programs.
        enforcer.tick(&mut fake, Some(CCDS[1]));
        assert_eq!(fake.masks[&key(1)], CCDS[1]);
        // "Both" restores the originals, not the earlier target.
        enforcer.tick(&mut fake, None);
        assert_eq!(fake.masks[&key(1)], 0xffff_ffff);
        assert_eq!(fake.masks[&key(2)], 0xffff_ffff);
        assert_eq!(enforcer.pinned(), 0);
    }

    #[test]
    fn self_restricted_and_protected_programs_are_left_alone() {
        let mut fake = Fake::default();
        fake.masks.insert(key(1), 0x0000_000f);
        fake.masks.insert(key(2), 0xffff_ffff);
        fake.protected.push(key(2));
        let mut enforcer = Enforcer::default();
        fake.foreground = Some(key(1));
        assert!(enforcer.tick(&mut fake, Some(CCDS[0])).is_empty());
        assert_eq!(fake.masks[&key(1)], 0x0000_000f);
        fake.foreground = Some(key(2));
        assert_eq!(enforcer.tick(&mut fake, Some(CCDS[0])).len(), 1);
        assert_eq!(fake.masks[&key(2)], 0xffff_ffff);
        assert_eq!(enforcer.pinned(), 0);
    }

    #[test]
    fn release_restores_and_vanished_programs_are_dropped() {
        let mut fake = Fake::default();
        fake.masks.insert(key(1), 0xffff_ffff);
        fake.masks.insert(key(2), 0xffff_ffff);
        let mut enforcer = Enforcer::default();
        for pid in [1, 2] {
            fake.foreground = Some(key(pid));
            enforcer.tick(&mut fake, Some(CCDS[1]));
        }
        fake.masks.remove(&key(2));
        fake.foreground = None;
        let errors = enforcer.tick(&mut fake, Some(CCDS[0]));
        assert_eq!(errors.len(), 1);
        assert_eq!(enforcer.pinned(), 1);
        enforcer.release(&mut fake);
        assert_eq!(fake.masks[&key(1)], 0xffff_ffff);
        assert_eq!(enforcer.pinned(), 0);
    }

    #[test]
    fn multi_ccd_topology_and_bounds_validation() {
        let three_ccds = [0x000f, 0x00f0, 0x0f00];
        assert_eq!(all(3), 7);
        assert_eq!(affinity(1, &three_ccds).unwrap(), Some(0x000f));
        assert_eq!(affinity(2, &three_ccds).unwrap(), Some(0x00f0));
        assert_eq!(affinity(4, &three_ccds).unwrap(), Some(0x0f00));
        assert_eq!(affinity(5, &three_ccds).unwrap(), Some(0x0f0f));
        assert_eq!(affinity(7, &three_ccds).unwrap(), None);

        assert!(validate(-1, 3).is_err());
        assert!(validate(0, 3).is_err());
        assert!(validate(8, 3).is_err());
        assert!(validate(1, 1).is_err());
        assert!(validate(1, 9).is_err());
        assert_eq!(validate(3, 2).unwrap(), 3);
    }

    #[test]
    fn prune_drops_dead_processes_when_capacity_reached() {
        let mut fake = Fake::default();
        let mut enforcer = Enforcer::default();
        for pid in 1..=(MAX_TRACKED as u32) {
            fake.masks.insert(key(pid), 0xffff_ffff);
            fake.foreground = Some(key(pid));
            assert!(enforcer.tick(&mut fake, Some(CCDS[0])).is_empty());
        }
        assert_eq!(enforcer.pinned(), MAX_TRACKED);

        // A new process while all MAX_TRACKED are alive cannot be tracked
        let extra_pid = (MAX_TRACKED as u32) + 1;
        fake.masks.insert(key(extra_pid), 0xffff_ffff);
        fake.foreground = Some(key(extra_pid));
        assert!(enforcer.tick(&mut fake, Some(CCDS[0])).is_empty());
        assert_eq!(enforcer.pinned(), MAX_TRACKED);
        assert_eq!(fake.masks[&key(extra_pid)], 0xffff_ffff);

        // Terminate process 1
        fake.masks.remove(&key(1));
        // Next tick triggers prune and allows extra_pid to be pinned
        assert!(enforcer.tick(&mut fake, Some(CCDS[0])).is_empty());
        assert_eq!(enforcer.pinned(), MAX_TRACKED);
        assert_eq!(fake.masks[&key(extra_pid)], CCDS[0]);
    }
}
