//! CPU busy-time calculation, independent of Windows and hardware adapters.

#[derive(Debug, Clone, Copy)]
pub struct CpuTimes {
    pub idle: u64,
    /// Kernel time includes idle time, matching the GetSystemTimes contract.
    pub kernel: u64,
    pub user: u64,
}

pub fn busy_percent(previous: CpuTimes, current: CpuTimes) -> Option<f64> {
    let idle = current.idle.checked_sub(previous.idle)?;
    let kernel = current.kernel.checked_sub(previous.kernel)?;
    let user = current.user.checked_sub(previous.user)?;
    let total = kernel.checked_add(user)?;
    if total == 0 || idle > kernel {
        return None;
    }
    Some((total - idle) as f64 / total as f64 * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    const BASE: CpuTimes = CpuTimes {
        idle: 100,
        kernel: 200,
        user: 100,
    };
    #[test]
    fn excludes_idle_from_kernel_time() {
        let value = busy_percent(
            BASE,
            CpuTimes {
                idle: 160,
                kernel: 280,
                user: 120,
            },
        )
        .unwrap();
        assert!((value - 40.0).abs() < 0.001);
    }
    #[test]
    fn distinguishes_zero_activity_from_full_activity() {
        assert_eq!(
            busy_percent(
                BASE,
                CpuTimes {
                    idle: 200,
                    kernel: 300,
                    user: 100
                }
            ),
            Some(0.0)
        );
        assert_eq!(
            busy_percent(
                BASE,
                CpuTimes {
                    idle: 100,
                    kernel: 300,
                    user: 100
                }
            ),
            Some(100.0)
        );
    }
    #[test]
    fn rejects_reset_inconsistent_and_overflowed_counters() {
        assert_eq!(busy_percent(BASE, BASE), None);
        assert_eq!(
            busy_percent(
                BASE,
                CpuTimes {
                    idle: 0,
                    kernel: 0,
                    user: 0
                }
            ),
            None
        );
        assert_eq!(
            busy_percent(
                BASE,
                CpuTimes {
                    idle: 180,
                    kernel: 210,
                    user: 120
                }
            ),
            None
        );
        assert_eq!(
            busy_percent(
                CpuTimes {
                    idle: 0,
                    kernel: 0,
                    user: 0
                },
                CpuTimes {
                    idle: 0,
                    kernel: u64::MAX,
                    user: 1
                }
            ),
            None
        );
    }
}
