//! Fixed public NVML power-limit ABI. No private voltage/TGP commands.
use super::{Handle, Library, status, symbol};
use omen_core::gpu_power::PowerLimitState;
use std::time::{Duration, Instant};

type Get = unsafe extern "C" fn(Handle, *mut u32) -> i32;
type Constraints = unsafe extern "C" fn(Handle, *mut u32, *mut u32) -> i32;
type Set = unsafe extern "C" fn(Handle, u32) -> i32;

pub(super) struct PowerControl {
    get: Option<Get>,
    constraints: Option<Constraints>,
    set: Option<Set>,
}

impl PowerControl {
    pub(super) fn load(library: &Library) -> Self {
        Self {
            get: symbol(library, b"nvmlDeviceGetPowerManagementLimit\0").ok(),
            constraints: symbol(library, b"nvmlDeviceGetPowerManagementLimitConstraints\0").ok(),
            set: symbol(library, b"nvmlDeviceSetPowerManagementLimit\0").ok(),
        }
    }
    pub(super) fn can_set(&self) -> bool {
        self.set.is_some()
    }
    pub(super) fn read(&self, handle: Handle) -> Result<PowerLimitState, String> {
        let get = self
            .get
            .ok_or("NVML current power-limit query is unavailable")?;
        // Query the actual contracts. The deprecated mode API can return
        // NOT_SUPPORTED even when modern drivers expose power constraints.
        let (minimum, maximum) = self.bounds(handle)?;
        let mut current = 0;
        status("power limit GET", unsafe { get(handle, &mut current) })?;
        let state = PowerLimitState {
            current_mw: current,
            minimum_mw: minimum,
            maximum_mw: maximum,
        };
        state.validate()?;
        Ok(state)
    }
    pub(super) fn bounds(&self, handle: Handle) -> Result<(u32, u32), String> {
        let constraints = self
            .constraints
            .ok_or("NVML power-limit constraint query is unavailable")?;
        let mut minimum = 0;
        let mut maximum = 0;
        // SAFETY: Fixed public ABI, live device and exact u32 output cells.
        status("power limit constraints GET", unsafe {
            constraints(handle, &mut minimum, &mut maximum)
        })?;
        if minimum == 0 || minimum > maximum {
            return Err("Driver power-limit bounds are inconsistent".into());
        }
        Ok((minimum, maximum))
    }
    pub(super) fn write(
        &self,
        handle: Handle,
        expected: &PowerLimitState,
        requested_mw: i64,
    ) -> Result<(), String> {
        let target = expected.target(requested_mw)?;
        if self.read(handle)? != *expected {
            return Err("GPU power limit changed before dispatch".into());
        }
        if expected.current_mw == target {
            return Ok(());
        }
        let set = self.set.ok_or("NVML power-limit SET is unavailable")?;
        // SAFETY: Same device and checked live driver bounds, in milliwatts.
        // The driver retains its own hardware/firmware enforcement.
        status("power limit SET", unsafe { set(handle, target) })?;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let actual = self.read(handle)?;
            if !expected.same_limits(&actual) {
                return Err("Driver power-limit constraints changed during readback".into());
            }
            if actual.current_mw == target {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(
                    "GPU power-limit readback differs from the exact requested value".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    struct Driver {
        current: Cell<u32>,
        writes: Cell<usize>,
        fail_after_effect: Cell<bool>,
    }
    impl Driver {
        fn handle(&self) -> Handle {
            (self as *const Self).cast_mut().cast()
        }
    }
    unsafe extern "C" fn constraints(_h: Handle, min: *mut u32, max: *mut u32) -> i32 {
        unsafe {
            *min = 60_000;
            *max = 115_000;
        }
        0
    }
    unsafe extern "C" fn get(h: Handle, value: *mut u32) -> i32 {
        let driver = unsafe { &*h.cast::<Driver>() };
        unsafe {
            *value = driver.current.get();
        }
        0
    }
    unsafe extern "C" fn set(h: Handle, value: u32) -> i32 {
        let driver = unsafe { &*h.cast::<Driver>() };
        driver.writes.set(driver.writes.get() + 1);
        driver.current.set(value);
        if driver.fail_after_effect.replace(false) {
            4
        } else {
            0
        }
    }
    fn fixture() -> (PowerControl, Driver) {
        (
            PowerControl {
                get: Some(get),
                constraints: Some(constraints),
                set: Some(set),
            },
            Driver {
                current: Cell::new(90_000),
                writes: Cell::new(0),
                fail_after_effect: Cell::new(false),
            },
        )
    }
    #[test]
    fn public_abi_writes_exact_milliwatts_and_restores_the_actual_original() {
        let (control, driver) = fixture();
        let original = control.read(driver.handle()).unwrap();
        control.write(driver.handle(), &original, 90_125).unwrap();
        let live = control.read(driver.handle()).unwrap();
        assert_eq!(live.current_mw, 90_125);
        control
            .write(driver.handle(), &live, i64::from(original.current_mw))
            .unwrap();
        assert_eq!(control.read(driver.handle()).unwrap(), original);
        assert_eq!(driver.writes.get(), 2);
    }
    #[test]
    fn changed_original_out_of_range_and_missing_setter_send_no_command() {
        let (mut control, driver) = fixture();
        let original = control.read(driver.handle()).unwrap();
        assert!(control.write(driver.handle(), &original, 115_001).is_err());
        driver.current.set(80_000);
        assert!(control.write(driver.handle(), &original, 70_000).is_err());
        control.set = None;
        assert!(!control.can_set());
        assert!(
            control
                .write(
                    driver.handle(),
                    &control.read(driver.handle()).unwrap(),
                    70_000
                )
                .is_err()
        );
        assert_eq!(driver.writes.get(), 0);
    }
    #[test]
    fn readable_bounds_do_not_substitute_for_a_missing_current_limit() {
        unsafe extern "C" fn unsupported(_h: Handle, _value: *mut u32) -> i32 {
            3
        }
        let (mut control, driver) = fixture();
        let original = control.read(driver.handle()).unwrap();
        control.get = Some(unsupported);
        assert_eq!(control.bounds(driver.handle()).unwrap(), (60_000, 115_000));
        assert!(
            control
                .read(driver.handle())
                .unwrap_err()
                .contains("status 3")
        );
        assert!(control.write(driver.handle(), &original, 80_000).is_err());
        assert_eq!(driver.writes.get(), 0);
        assert_eq!(driver.current.get(), 90_000);
    }
    #[test]
    fn error_after_effect_can_be_read_and_returned_without_repeating_the_target() {
        let (control, driver) = fixture();
        let original = control.read(driver.handle()).unwrap();
        driver.fail_after_effect.set(true);
        assert!(control.write(driver.handle(), &original, 80_000).is_err());
        let live = control.read(driver.handle()).unwrap();
        assert!(original.owns(&live, Some(80_000)).unwrap());
        control
            .write(driver.handle(), &live, i64::from(original.current_mw))
            .unwrap();
        assert_eq!(control.read(driver.handle()).unwrap(), original);
        assert_eq!(driver.writes.get(), 2);
    }
}
