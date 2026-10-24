//! One boot/session identity, disposable device readers. Sampling never performs
//! inventory or initialization; pause drops the device and all counter baselines.
pub(super) struct ReaderSession<I, R> {
    inventory: Option<I>,
    reader: Option<R>,
}

impl<I, R> ReaderSession<I, R> {
    pub(super) fn new() -> Self {
        Self {
            inventory: None,
            reader: None,
        }
    }

    pub(super) fn prepare(
        &mut self,
        inventory: impl FnOnce() -> Result<I, String>,
        open: impl FnOnce(&I) -> Result<R, String>,
    ) -> Result<(), String> {
        if self.reader.is_none() {
            let identity = match &self.inventory {
                Some(identity) => identity,
                None => self.inventory.insert(inventory()?),
            };
            // Reopen revalidates the driver/hash/ABI against the qualified
            // identity; retaining that identity does not retain device handles.
            self.reader = Some(open(identity)?);
        }
        Ok(())
    }

    pub(super) fn sample<T>(
        &mut self,
        sample: impl FnOnce(&mut R) -> Result<T, String>,
    ) -> Result<T, String> {
        let result = sample(self.reader.as_mut().ok_or("CPU reader is not prepared")?);
        if result.is_err() {
            self.pause();
        }
        result
    }

    pub(super) fn pause(&mut self) {
        self.reader = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Device<'a> {
        drops: &'a Cell<u32>,
        baseline: bool,
    }
    impl Drop for Device<'_> {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[test]
    fn repeated_tray_cycles_reuse_identity_but_release_devices_and_baselines() {
        let inventories = Cell::new(0);
        let opens = Cell::new(0);
        let drops = Cell::new(0);
        let mut session = ReaderSession::new();
        for _ in 0..5 {
            session
                .prepare(
                    || {
                        inventories.set(inventories.get() + 1);
                        Ok("qualified board/BIOS")
                    },
                    |identity| {
                        assert_eq!(*identity, "qualified board/BIOS");
                        opens.set(opens.get() + 1);
                        Ok(Device {
                            drops: &drops,
                            baseline: false,
                        })
                    },
                )
                .unwrap();
            for expected in [false, true] {
                assert_eq!(
                    session.sample(|device| Ok(std::mem::replace(&mut device.baseline, true))),
                    Ok(expected)
                );
            }
            session.pause();
            session.pause();
            assert_eq!(drops.get(), opens.get());
            assert!(session.sample(|_| Ok(())).is_err());
        }
        assert_eq!(inventories.get(), 1);
        assert_eq!(opens.get(), 5);
    }

    #[test]
    fn inventory_failure_is_a_preparation_error_never_a_sample() {
        let mut session: ReaderSession<(), ()> = ReaderSession::new();
        assert_eq!(
            session.prepare(
                || Err("inventory deadline".into()),
                |_| panic!("no identity")
            ),
            Err("inventory deadline".into())
        );
        assert!(session.sample::<()>(|_| panic!("no reader")).is_err());
        session.prepare(|| Ok(()), |_| Ok(())).unwrap();
        session
            .prepare(|| panic!("already prepared"), |_| panic!("already open"))
            .unwrap();
    }

    #[test]
    fn reopen_failure_cannot_reuse_an_old_sample_or_device() {
        let drops = Cell::new(0);
        let mut session = ReaderSession::new();
        session
            .prepare(
                || Ok(()),
                |_| {
                    Ok(Device {
                        drops: &drops,
                        baseline: false,
                    })
                },
            )
            .unwrap();
        assert_eq!(
            session.sample::<()>(|_| Err("read failed".into())),
            Err("read failed".into())
        );
        assert_eq!(drops.get(), 1);
        assert_eq!(
            session.prepare(
                || panic!("identity already known"),
                |_| Err("driver changed".into())
            ),
            Err("driver changed".into())
        );
        assert!(
            session
                .sample::<()>(|_| panic!("failed device must not survive"))
                .is_err()
        );
    }
}
