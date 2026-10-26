//! Public GPU wattage adapter. Original values and driver limits are durable;
//! no private NVAPI layout or guessed notebook TGP command is used.
use super::*;
use omen_core::gpu_power::PowerLimitState;

impl Controller {
    pub(super) fn public_power_capability(&self) -> Capability {
        let feature = Feature::GpuPowerLimitMw;
        let supported: Result<PowerLimitState, String> = (|| {
            let gpu = self.gpu()?;
            let state = gpu.power_limit()?;
            if !gpu.power_limit_setter() {
                return Err("Public NVML power-limit setter is unavailable".into());
            }
            Ok(state)
        })();
        match supported {
            Ok(state) => Capability {
                feature,
                access: Access::ReadWrite,
                reason: "Public NVML GPU power limit in live driver-reported milliwatt bounds"
                    .into(),
                integer_range: Some((i64::from(state.minimum_mw), i64::from(state.maximum_mw))),
                requires_ac: false,
                startup_safe: true,
            },
            Err(reason) => Capability::unsupported(feature, reason),
        }
    }
    pub(super) fn capture_public_power(&mut self) -> Result<Captured<RestoreState>, String> {
        let state = self.gpu()?.power_limit()?;
        let saved = Captured {
            feature: Feature::GpuPowerLimitMw,
            value: Value::Integer(i64::from(state.current_mw)),
            restore: RestoreState::NvidiaPowerLimit {
                identity: self.gpu()?.identity()?,
                state,
            },
        };
        self.captured.insert(saved.feature, saved.clone());
        Ok(saved)
    }
    fn public_power_original<'a>(
        &self,
        saved: &'a Captured<RestoreState>,
    ) -> Result<&'a PowerLimitState, String> {
        let RestoreState::NvidiaPowerLimit { identity, state } = &saved.restore else {
            return Err("Wrong GPU power adapter".into());
        };
        state.validate()?;
        if saved.feature != Feature::GpuPowerLimitMw
            || saved.value != Value::Integer(i64::from(state.current_mw))
            || &self.gpu()?.identity()? != identity
        {
            return Err("GPU power-limit original identity/value differs".into());
        }
        Ok(state)
    }
    pub(super) fn verify_public_power(&self, saved: &Captured<RestoreState>) -> Result<(), String> {
        let original = self.public_power_original(saved)?;
        let attempted = match self.windows_targets.get(&saved.feature) {
            Some(Value::Integer(value)) => Some(*value),
            None => None,
            _ => return Err("GPU power target has a different value kind".into()),
        };
        if !original.owns(&self.gpu()?.power_limit()?, attempted)? {
            return Err(
                "GPU power limit/constraints differ from the original or this transaction's target"
                    .into(),
            );
        }
        Ok(())
    }
    pub(super) fn write_public_power(&mut self, setting: &Setting) -> Result<(), String> {
        let Value::Integer(value) = setting.value else {
            return Err("GPU power needs milliwatts".into());
        };
        let saved = self
            .captured
            .get(&setting.feature)
            .ok_or("No GPU power original captured")?;
        self.verify_public_power(saved)?;
        let current = self.gpu()?.power_limit()?;
        current.target(value)?;
        self.gpu()?.thermal_guard(u32::MAX)?;
        // Record intent before dispatch, including an API error after effect.
        self.windows_targets
            .insert(setting.feature, setting.value.clone());
        self.gpu()?.set_power_limit(&current, value)
    }
    pub(super) fn verify_public_power_target(&self, setting: &Setting) -> Result<Value, String> {
        let saved = self
            .captured
            .get(&setting.feature)
            .ok_or("GPU power original is absent")?;
        let original = self.public_power_original(saved)?;
        let Value::Integer(value) = setting.value else {
            return Err("GPU power needs milliwatts".into());
        };
        let expected = original.target(value)?;
        let live = self.gpu()?.power_limit()?;
        if !original.same_limits(&live) || live.current_mw != expected {
            return Err("Complete GPU power-limit target differs".into());
        }
        Ok(Value::Integer(i64::from(live.current_mw)))
    }
    pub(super) fn restore_public_power(
        &self,
        saved: &Captured<RestoreState>,
    ) -> Result<(), String> {
        self.verify_public_power(saved)?;
        let original = self.public_power_original(saved)?;
        let current = self.gpu()?.power_limit()?;
        self.gpu()?
            .set_power_limit(&current, i64::from(original.current_mw))?;
        if self.gpu()?.power_limit()? != *original {
            return Err("Full GPU power-limit original did not return".into());
        }
        Ok(())
    }
    pub(super) fn prepare_public_power_recovery(
        &mut self,
        saved: &Captured<RestoreState>,
        requested: &Setting,
    ) -> Result<(), String> {
        if requested.feature != saved.feature {
            return Err("GPU power recovery feature differs".into());
        }
        let original = self.public_power_original(saved)?;
        let Value::Integer(value) = requested.value else {
            return Err("GPU power recovery needs milliwatts".into());
        };
        if !original.owns(&self.gpu()?.power_limit()?, Some(value))? {
            return Err("GPU power recovery cannot own another controller's settings".into());
        }
        self.windows_targets
            .insert(saved.feature, requested.value.clone());
        Ok(())
    }
}
