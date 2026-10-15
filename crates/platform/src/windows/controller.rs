//! One controller owns typed adapters. Capabilities gate writes again at dispatch.
use super::{
    GpuControlsReport, NvidiaIdentity, OffsetState, gpu::NvidiaControl, inventory, power_status,
};
use omen_core::{
    Inventory,
    features::Feature,
    keyboard::KeyboardState,
    profiles::{Choice, DeviceBinding, PROFILE_SCHEMA, PerformanceProfile, Setting, Value},
    transaction::{
        self, Access, Applied, Backend, Capability, Captured, Context, Journal, PendingTransaction,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Changed only after a measured clock GET/SET/readback/restore record exists.
// 2026-10-13 15:17 CEST: native -15/-24 MHz trial, exact readback and full restore.
// 16:07 CEST: both -15/-24 and +15/+24 MHz verified/restored through v3 trial.
// This verifies the control path, not stability at arbitrary overclock values.
const CLOCK_WRITE_VERIFIED: bool = true;
// 2026-10-13 18:02 CEST: all 127 Graphics deltas -15000 kHz, exact
// full-curve readback and restore; voltage anchors/global offsets unchanged.
const CURVE_WRITE_VERIFIED: bool = true;
// 2026-10-14 02:35 CEST, build98F3B089...: zone1 red +/-1 byte,
// complete 128-byte color/brightness/capability readback and original restore;
// existing programmed GPU controls and prior journal/startup retained.
// This qualifies classic RGB only, not animation or logon/lifecycle ownership.
const KEYBOARD_COLOR_WRITE_VERIFIED: bool = true;
// 2026-10-14 02:48 CEST, build5E5926A8...: coupled RGB plus64->E4,
// exact complete readback,64/original RGB restore, GPU controls unchanged.
const KEYBOARD_MODE_WRITE_VERIFIED: bool = true;
// 2026-10-15 fixed ON/held-readback/OFF restore, build063CF16D...;
// full flag restored, GPU controls/power and production metadata unchanged.
// Startup and software curves remain unqualified.
const MAX_FAN_WRITE_VERIFIED: bool = true;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "adapter", rename_all = "snake_case", deny_unknown_fields)]
pub enum RestoreState {
    HpMaxFan {
        state: super::cooling::MaxFanImage,
    },
    HpKeyboardColors {
        state: KeyboardState,
    },
    NvidiaOffset {
        identity: NvidiaIdentity,
        state: OffsetState,
        #[serde(default)]
        gfx_curve: Option<omen_core::vf::VfState>,
        #[serde(default)]
        related_core: Option<OffsetState>,
    },
    NvidiaVfCurve {
        identity: NvidiaIdentity,
        state: omen_core::vf::VfState,
        core_offset: OffsetState,
    },
}

pub struct Controller {
    inventory: Inventory,
    nvidia: Option<NvidiaControl>,
    calibration: bool,
    curve_calibration: bool,
    vf: Option<super::vf::VfControl>,
    captured: BTreeMap<Feature, Captured<RestoreState>>,
    curve_target: Option<(Value, omen_core::vf::VfState)>,
    memory_last_trial: bool,
    keyboard: Option<super::hp::KeyboardControl>,
    keyboard_calibration: Option<Vec<[u8; 3]>>,
    keyboard_target: Option<KeyboardState>,
    keyboard_mode_calibration: Option<Choice>,
    fan: Option<super::cooling::FanControl>,
    fan_target: Option<omen_core::cooling::MaxFanState>,
    fan_calibration: bool,
    fan_observations: Vec<super::cooling::FanReport>,
}

fn uses_nvidia(feature: Feature) -> bool {
    matches!(
        feature.descriptor().category,
        omen_core::features::Category::Gpu
    )
}
fn uses_keyboard(feature: Feature) -> bool {
    matches!(feature, Feature::KeyboardColors | Feature::KeyboardEffect)
}
fn keyboard_value(feature: Feature, state: &KeyboardState) -> Result<Value, String> {
    match feature {
        Feature::KeyboardColors => state.colors().map(Value::Colors),
        Feature::KeyboardEffect => Ok(Value::Choice(if state.custom_colors_enabled()? {
            Choice::Static
        } else {
            Choice::FirmwareDefault
        })),
        _ => Err("Not a classic keyboard feature".into()),
    }
}
fn lighting_target(state: &KeyboardState, settings: &[Setting]) -> Result<KeyboardState, String> {
    let mut target = state.clone();
    for s in settings {
        match (&s.feature, &s.value) {
            (Feature::KeyboardColors, Value::Colors(colors)) => {
                target = target.with_colors(colors)?
            }
            (Feature::KeyboardEffect, Value::Choice(Choice::Static)) => {
                target = target.with_classic_mode(true)?
            }
            (Feature::KeyboardEffect, Value::Choice(Choice::FirmwareDefault)) => {
                target = target.with_classic_mode(false)?
            }
            (Feature::KeyboardEffect, _) => {
                return Err("Only own static colors and firmware preset are qualified".into());
            }
            _ => {}
        }
    }
    Ok(target)
}

impl Controller {
    pub fn new(needs_nvidia: bool, uuid: Option<&str>) -> Result<Self, String> {
        Ok(Self {
            inventory: inventory(),
            nvidia: if needs_nvidia {
                Some(NvidiaControl::new(uuid)?)
            } else {
                None
            },
            calibration: false,
            curve_calibration: false,
            vf: None,
            captured: BTreeMap::new(),
            curve_target: None,
            memory_last_trial: false,
            keyboard: None,
            keyboard_calibration: None,
            keyboard_target: None,
            keyboard_mode_calibration: None,
            fan: None,
            fan_target: None,
            fan_calibration: false,
            fan_observations: vec![],
        })
    }
    pub fn for_profile(profile: &PerformanceProfile) -> Result<Self, String> {
        Self::new(
            profile.settings.iter().any(|s| uses_nvidia(s.feature)),
            profile.binding.nvidia_uuid.as_deref(),
        )
    }
    pub fn for_recovery(saved: &PendingTransaction<RestoreState>) -> Result<Self, String> {
        Self::new(
            saved.requested.iter().any(|s| uses_nvidia(s.feature)),
            saved.binding.nvidia_uuid.as_deref(),
        )
    }
    fn keyboard(&mut self) -> Result<&super::hp::KeyboardControl, String> {
        if self.keyboard.is_none() {
            self.keyboard = Some(super::hp::KeyboardControl::new(&self.inventory)?);
        }
        self.keyboard
            .as_ref()
            .ok_or("HP keyboard connection is missing".into())
    }
    pub fn keyboard_state(&mut self) -> Result<KeyboardState, String> {
        self.keyboard()?.read()
    }
    fn fan(&mut self) -> Result<&super::cooling::FanControl, String> {
        if self.fan.is_none() {
            self.fan = Some(super::cooling::FanControl::new(&self.inventory)?);
        }
        self.fan.as_ref().ok_or("Missing fan connection".into())
    }
    pub fn fan_report(&mut self) -> Result<super::cooling::FanReport, String> {
        self.fan()?.report()
    }
    pub fn fan_validation_observations(&self) -> &[super::cooling::FanReport] {
        &self.fan_observations
    }
    pub fn validate_max_fan(
        &mut self,
        journal: &mut impl Journal<RestoreState>,
    ) -> Result<Applied, String> {
        let before = self.fan()?.capture()?;
        if before.state.enabled {
            return Err("Fixed qualification requires Max-Fan OFF; it never lowers existing maximum cooling".into());
        }
        let profile = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "max-fan-validation".into(),
            name: "Fixed max-fan ON/restore qualification".into(),
            binding: self.context()?.binding.for_settings(&[]),
            settings: vec![Setting {
                feature: Feature::MaxFan,
                value: Value::Boolean(true),
            }],
        };
        let digest = crate::profile_store::profile_digest(&profile)?;
        self.fan_calibration = true;
        self.fan_observations.clear();
        let result = transaction::trial(&profile, &digest, self, journal);
        self.fan_calibration = false;
        let result = result.map_err(|e| serde_json::to_string_pretty(&e).unwrap_or(e.error))?;
        if self.fan()?.capture()? != before {
            return Err("Complete original Max-Fan flag did not restore".into());
        }
        Ok(result)
    }
    pub fn validate_keyboard_lighting(
        &mut self,
        journal: &mut impl Journal<RestoreState>,
    ) -> Result<Applied, String> {
        let before = self.keyboard_state()?;
        if ![0x64, 0xe4].contains(
            &before
                .legacy
                .as_ref()
                .ok_or("Missing classic state")?
                .brightness_raw_byte,
        ) {
            return Err("Fixed mode trial requires64/E4 raw brightness".into());
        }
        let mode = if before.custom_colors_enabled()? {
            Choice::FirmwareDefault
        } else {
            Choice::Static
        };
        let mut colors = before.colors()?;
        colors[0][0] = if colors[0][0] == 255 {
            254
        } else {
            colors[0][0] + 1
        };
        let profile = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "keyboard-lighting-validation".into(),
            name: "Fixed coupled RGB/mode qualification".into(),
            binding: self.context()?.binding.for_settings(&[]),
            settings: vec![
                Setting {
                    feature: Feature::KeyboardColors,
                    value: Value::Colors(colors.clone()),
                },
                Setting {
                    feature: Feature::KeyboardEffect,
                    value: Value::Choice(mode),
                },
            ],
        };
        let digest = crate::profile_store::profile_digest(&profile)?;
        self.keyboard_calibration = Some(colors);
        self.keyboard_mode_calibration = Some(mode);
        let result = transaction::trial(&profile, &digest, self, journal);
        self.keyboard_calibration = None;
        self.keyboard_mode_calibration = None;
        let result = result.map_err(|e| serde_json::to_string_pretty(&e).unwrap_or(e.error))?;
        if self.keyboard_state()? != before {
            return Err("The coupled lighting trial did not fully restore".into());
        }
        Ok(result)
    }
    /// Fixed color-only qualification: change one component in zone 1 by one
    /// byte, verify the entire state and restore through the durable journal.
    pub fn validate_keyboard_colors(
        &mut self,
        journal: &mut impl Journal<RestoreState>,
    ) -> Result<Applied, String> {
        let before = self.keyboard_state()?;
        let mut colors = before.colors()?;
        colors[0][0] = if colors[0][0] == 255 {
            254
        } else {
            colors[0][0] + 1
        };
        let profile = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "keyboard-color-validation".into(),
            name: "Fixed keyboard color qualification".into(),
            binding: self.context()?.binding.for_settings(&[]),
            settings: vec![Setting {
                feature: Feature::KeyboardColors,
                value: Value::Colors(colors.clone()),
            }],
        };
        let digest = crate::profile_store::profile_digest(&profile)?;
        self.keyboard_calibration = Some(colors);
        let result = transaction::trial(&profile, &digest, self, journal);
        self.keyboard_calibration = None;
        let result = result.map_err(|e| serde_json::to_string_pretty(&e).unwrap_or(e.error))?;
        if self.keyboard_state()? != before {
            return Err("Keyboard trial did not restore the complete original".into());
        }
        Ok(result)
    }
    fn gpu(&self) -> Result<&NvidiaControl, String> {
        self.nvidia
            .as_ref()
            .ok_or("Profile has no NVIDIA driver session".into())
    }

    fn qualifies(&self) -> Result<(), String> {
        let identity = self.gpu()?.identity()?;
        if self.inventory.manufacturer != "HP"
            || self.inventory.board != "8E35"
            || self.inventory.bios != "F.13"
            || identity.name != "NVIDIA GeForce RTX 5070 Laptop GPU"
            || identity.driver != "617.14"
            || identity.vbios != "98.06.2a.40.da"
        {
            return Err(
                "GPU write evidence is limited to HP 8E35/F.13, RTX 5070 Laptop, driver 617.14 and VBIOS 98.06.2a.40.da"
                    .into(),
            );
        }
        Ok(())
    }

    pub fn gpu_report(&self) -> Result<GpuControlsReport, String> {
        self.gpu()?.report()
    }
    fn curve(&mut self) -> Result<&super::vf::VfControl, String> {
        self.qualifies()?;
        if self.vf.is_none() {
            self.vf = Some(super::vf::VfControl::new(&self.gpu()?.identity()?)?);
        }
        self.vf.as_ref().ok_or("V/F session unavailable".into())
    }
    pub fn gpu_curve(&mut self) -> Result<omen_core::vf::VfState, String> {
        self.curve()?.snapshot()
    }
    fn restore_graphics(
        &mut self,
        original: &omen_core::vf::VfState,
        core: Option<&OffsetState>,
    ) -> Result<(), String> {
        if let Some(core) = core {
            if core.feature != Feature::GpuCoreOffsetMhz {
                return Err("Related offset has another clock domain".into());
            }
            if self.gpu()?.offset(Feature::GpuCoreOffsetMhz)?.current_mhz != core.current_mhz {
                self.gpu()?.set_offset(&Setting {
                    feature: Feature::GpuCoreOffsetMhz,
                    value: Value::Integer(i64::from(core.current_mhz)),
                })?;
            }
        }
        let current = self.gpu_curve()?;
        if !original.domain_matches(&current)? {
            return Err("Graphics restore domain/grid changed".into());
        }
        if !original.controls_match(&current)? {
            let deltas: Vec<_> = original
                .nodes
                .iter()
                .map(|p| (p.index, p.delta_khz))
                .collect();
            self.curve()?.set_deltas(&current, &deltas)?;
        }
        if !original.controls_match(&self.gpu_curve()?)? {
            return Err("Full Graphics control restore differs".into());
        }
        if let Some(core) = core {
            if self.gpu()?.offset(Feature::GpuCoreOffsetMhz)? != *core {
                return Err("Related global core restore differs".into());
            }
        }
        Ok(())
    }
    /// Read-only verification of a completed revision before protected startup
    /// approval. The caller first proves the exact journal/profile revision.
    pub fn verify_applied(
        &mut self,
        saved: &PendingTransaction<RestoreState>,
    ) -> Result<Vec<Setting>, String> {
        self.prepare_recovery(saved)?;
        self.curve_target = None;
        for (original, requested) in saved.original.iter().zip(&saved.requested) {
            if let RestoreState::NvidiaVfCurve { state, .. } = &original.restore {
                let Value::VfCurve(points) = &requested.value else {
                    return Err("Missing applied curve target".into());
                };
                self.curve_target = Some((requested.value.clone(), state.target_state(points)?));
            } else if requested.feature == Feature::GpuCoreOffsetMhz {
                let Value::Integer(wanted) = requested.value else {
                    return Err("Missing global core target".into());
                };
                let live = self.gpu_curve()?;
                if live
                    .nodes
                    .iter()
                    .any(|p| p.delta_khz != live.nodes[0].delta_khz)
                    || (live.nodes[0].delta_khz != 0
                        && i64::from(live.nodes[0].delta_khz) != wanted * 1000)
                {
                    return Err("Applied global offset has a foreign private curve".into());
                }
                self.curve_target = Some((requested.value.clone(), live));
            }
        }
        saved
            .requested
            .iter()
            .map(|requested| {
                self.verify_target(requested).map(|value| Setting {
                    feature: requested.feature,
                    value,
                })
            })
            .collect()
    }
    pub fn validate_gpu_curve(
        &mut self,
        journal: &mut impl Journal<RestoreState>,
    ) -> Result<Applied, String> {
        self.qualifies()?;
        self.gpu()?.thermal_guard(87)?;
        if self.gpu()?.offset(Feature::GpuCoreOffsetMhz)?.current_mhz != 0
            || self.gpu()?.offset(Feature::GpuMemoryOffsetMhz)?.current_mhz != 0
        {
            return Err("Curve validation requires existing zero global offsets".into());
        }
        let state = self.gpu_curve()?;
        if state
            .nodes
            .iter()
            .any(|p| p.delta_khz != 0 || p.frequency_khz < 16000)
        {
            return Err("Curve validation did not overwrite pre-existing user tuning".into());
        }
        let Value::VfCurve(mut points) = state.value()? else {
            unreachable!()
        };
        for point in &mut points {
            point.frequency_mhz = point
                .frequency_mhz
                .checked_sub(15)
                .ok_or("Curve frequency too low for fixed trial")?;
        }
        let profile = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "vf-driver-validation".into(),
            name: "Fixed Graphics curve frequency trial".into(),
            binding: self.context()?.binding,
            settings: vec![Setting {
                feature: Feature::GpuVfCurve,
                value: Value::VfCurve(points),
            }],
        };
        self.curve_calibration = true;
        let result = transaction::trial(&profile, "fixed-vf-frequency-trial-v1", self, journal);
        self.curve_calibration = false;
        let result =
            result.map_err(|e| format!("{}; rollback errors: {:?}", e.error, e.rollback_errors))?;
        if !state.controls_match(&self.gpu_curve()?)? {
            return Err("Complete Graphics V/F restore image differs".into());
        }
        if self.gpu()?.offset(Feature::GpuCoreOffsetMhz)?.current_mhz != 0
            || self.gpu()?.offset(Feature::GpuMemoryOffsetMhz)?.current_mhz != 0
        {
            return Err("Global offsets changed during private curve trial".into());
        }
        Ok(result)
    }

    /// A fixed durable trial exercises Memory SET after an already programmed
    /// nonuniform Graphics curve. It leaves every captured control restored.
    pub fn validate_memory_curve_preservation(
        &mut self,
        journal: &mut impl Journal<RestoreState>,
    ) -> Result<Applied, String> {
        self.qualifies()?;
        self.gpu()?.thermal_guard(87)?;
        let state = self.gpu_curve()?;
        if self.gpu()?.offset(Feature::GpuCoreOffsetMhz)?.current_mhz != 0
            || self.gpu()?.offset(Feature::GpuMemoryOffsetMhz)?.current_mhz != 0
            || state.nodes.iter().any(|p| p.delta_khz != 0)
        {
            return Err(
                "Preservation trial requires original zero offsets; existing tuning is not reset"
                    .into(),
            );
        }
        let Value::VfCurve(mut points) = state.value()? else {
            unreachable!()
        };
        points[0].frequency_mhz = points[0]
            .frequency_mhz
            .checked_sub(15)
            .ok_or("Frequency below fixed trial bounds")?;
        let profile = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "memory-curve-validation".into(),
            name: "Fixed Graphics preservation trial".into(),
            binding: self.context()?.binding,
            settings: vec![
                Setting {
                    feature: Feature::GpuVfCurve,
                    value: Value::VfCurve(points),
                },
                Setting {
                    feature: Feature::GpuMemoryOffsetMhz,
                    value: Value::Integer(-24),
                },
            ],
        };
        self.memory_last_trial = true;
        let result = transaction::trial(
            &profile,
            "fixed-memory-curve-preservation-v1",
            self,
            journal,
        );
        self.memory_last_trial = false;
        result.map_err(|e| format!("{}; rollback errors: {:?}", e.error, e.rollback_errors))
    }

    /// Fixed, minimal underclock probe only; no user values or voltage/power writes.
    /// Durable journaling is required and the core always restores before return.
    pub fn validate_gpu_offsets(
        &mut self,
        journal: &mut impl Journal<RestoreState>,
    ) -> Result<Vec<Applied>, String> {
        self.qualifies()?;
        let gpu = self.gpu()?;
        gpu.thermal_guard(87)?;
        let core = gpu.offset(Feature::GpuCoreOffsetMhz)?;
        let memory = gpu.offset(Feature::GpuMemoryOffsetMhz)?;
        if core.current_mhz != 0 || memory.current_mhz != 0 {
            return Err(
                "Validation requires unchanged zero offsets; existing user tuning was not reset"
                    .into(),
            );
        }
        let mut profile = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "driver-validation".into(),
            name: "Fixed NVIDIA driver validation".into(),
            binding: self.context()?.binding,
            settings: vec![
                Setting {
                    feature: Feature::GpuCoreOffsetMhz,
                    value: Value::Integer(-15),
                },
                Setting {
                    feature: Feature::GpuMemoryOffsetMhz,
                    value: Value::Integer(-24),
                },
            ],
        };
        self.calibration = true;
        let result = (|| {
            let negative = transaction::trial(
                &profile,
                "fixed-driver-validation-v3-negative",
                self,
                journal,
            )?;
            profile.settings[0].value = Value::Integer(15);
            profile.settings[1].value = Value::Integer(24);
            let positive = transaction::trial(
                &profile,
                "fixed-driver-validation-v3-positive",
                self,
                journal,
            )?;
            Ok(vec![negative, positive])
        })();
        self.calibration = false;
        result.map_err(|error: transaction::ApplyFailure| {
            format!(
                "{}; rollback errors: {:?}",
                error.error, error.rollback_errors
            )
        })
    }
}

impl Backend for Controller {
    type RestoreState = RestoreState;
    fn write_order(&self, settings: &[Setting]) -> Vec<usize> {
        let mut order: Vec<_> = (0..settings.len()).collect();
        if self.memory_last_trial {
            return order;
        }
        // Measured 617.14 behavior: the NVML memory setter resets a custom
        // Graphics curve. Preserve it, then apply the final Graphics target.
        order.sort_by_key(|i| match settings[*i].feature {
            Feature::GpuMemoryOffsetMhz => 0,
            Feature::KeyboardEffect => 2,
            _ => 1,
        });
        order
    }
    fn context(&mut self) -> Result<Context, String> {
        let identity = self.nvidia.as_ref().map(|gpu| gpu.identity()).transpose()?;
        Ok(Context {
            binding: DeviceBinding {
                manufacturer: self.inventory.manufacturer.clone(),
                board: self.inventory.board.clone(),
                bios: self.inventory.bios.clone(),
                cpu_names: self.inventory.cpus.clone(),
                nvidia_uuid: identity.as_ref().map(|id| id.uuid.clone()),
                nvidia_driver: identity.as_ref().map(|id| id.driver.clone()),
                nvidia_vbios: identity.map(|id| id.vbios),
            },
            ac_connected: power_status().ac_connected,
        })
    }

    fn capability(&mut self, feature: Feature) -> Capability {
        if feature == Feature::MaxFan {
            // Capabilities describe qualified board support. Permission, live
            // table/opaque state and durable originals are checked at capture,
            // for every setting before the transaction dispatches its first SET.
            if self.inventory.manufacturer != "HP"
                || self.inventory.board != "8E35"
                || self.inventory.bios != "F.13"
            {
                return Capability::unsupported(
                    feature,
                    "Max-Fan is qualified only on HP8E35/F.13",
                );
            }
            return Capability{feature,access:if MAX_FAN_WRITE_VERIFIED||self.fan_calibration{Access::ReadWrite}else{Access::NeedsValidation},reason:"Validated maximum-cooling override; live elevated capture checks exact fan table/state before any write. OFF releases only the override. Software curves/startup/lifecycle remain unqualified".into(),integer_range:None,requires_ac:true,startup_safe:false};
        }
        if feature == Feature::KeyboardEffect {
            return match self.keyboard_state() {
                Ok(state) if state.legacy.as_ref().is_some_and(|s|[0x64,0xe4].contains(&s.brightness_raw_byte))=>Capability {feature,access:if KEYBOARD_MODE_WRITE_VERIFIED||self.keyboard_mode_calibration.is_some(){Access::ReadWrite}else{Access::NeedsValidation},reason:"Classic own static colors versus firmware preset; exact64/E4 mode state only; no animation or percentage-brightness control".into(),integer_range:None,requires_ac:false,startup_safe:false},
                Ok(_)=>Capability::unsupported(feature,"Mode restore is not qualified at this raw brightness level"),
                Err(reason)=>Capability::unsupported(feature,reason),
            };
        }
        if feature == Feature::KeyboardColors {
            return match self.keyboard_state() {
                Ok(_)=>Capability {feature,access:if KEYBOARD_COLOR_WRITE_VERIFIED||self.keyboard_calibration.is_some(){Access::ReadWrite}else{Access::NeedsValidation},
                    reason:"Direct board8E35/F.13 classic four-zone RGB; complete opaque/brightness state must remain unchanged. Animations and independent HP-controller replacement are not qualified".into(),
                    integer_range:None,requires_ac:false,startup_safe:false},
                Err(reason)=>Capability::unsupported(feature,reason),
            };
        }
        if feature == Feature::GpuVfCurve {
            let state: Result<omen_core::vf::VfState, String> = (|| {
                let state = self.gpu_curve()?;
                state.value()?;
                Ok(state)
            })();
            return match state {Ok(_)=>Capability{feature,access:if CURVE_WRITE_VERIFIED||self.curve_calibration{Access::ReadWrite}else{Access::NeedsValidation},reason:"Exact private Graphics frequency-at-voltage contract; voltage anchors are immutable, physical voltage ceiling and load stability are not certified".into(),integer_range:None,requires_ac:true,startup_safe:CURVE_WRITE_VERIFIED&&!self.curve_calibration},Err(reason)=>Capability::unsupported(feature,reason)};
        }
        if matches!(
            feature,
            Feature::GpuCoreOffsetMhz | Feature::GpuMemoryOffsetMhz
        ) {
            if feature == Feature::GpuMemoryOffsetMhz {
                if let Err(reason) = self.gpu_curve() {
                    return Capability::unsupported(feature, reason);
                }
            }
            if feature == Feature::GpuCoreOffsetMhz {
                match self.gpu_curve() {
                    Ok(state) => {
                        if state
                            .nodes
                            .iter()
                            .any(|p| p.delta_khz != state.nodes[0].delta_khz)
                        {
                            return Capability::unsupported(
                                feature,
                                "A nonuniform V/F curve is active; use its full curve profile instead of a global core offset",
                            );
                        }
                        let delta = state.nodes[0].delta_khz;
                        let core = match self.gpu().and_then(|g| g.offset(feature)) {
                            Ok(core) => core,
                            Err(reason) => return Capability::unsupported(feature, reason),
                        };
                        if delta != 0 && i64::from(delta) != i64::from(core.current_mhz) * 1000 {
                            return Capability::unsupported(
                                feature,
                                "An independent uniform V/F curve is active; restore it before choosing a global core offset",
                            );
                        }
                    }
                    Err(reason) => return Capability::unsupported(feature, reason),
                }
            }
            let state = self.qualifies().and_then(|()| self.gpu()?.offset(feature));
            return match state {
                Ok(state) => {
                    let setter = self
                        .gpu()
                        .and_then(|g| g.report())
                        .is_ok_and(|r| r.clock_setter_export);
                    let ready = setter && (CLOCK_WRITE_VERIFIED || self.calibration);
                    let range = if self.calibration {
                        (
                            if feature == Feature::GpuCoreOffsetMhz {
                                -15
                            } else {
                                -24
                            },
                            if feature == Feature::GpuCoreOffsetMhz {
                                15
                            } else {
                                24
                            },
                        )
                    } else {
                        (i64::from(state.minimum_mhz), i64::from(state.maximum_mhz))
                    };
                    Capability {
                        feature,
                        access: if ready {
                            Access::ReadWrite
                        } else {
                            Access::NeedsValidation
                        },
                        reason: if ready {
                            "Public NVML MHz offsets; driver ranges are not a stability guarantee"
                                .into()
                        } else {
                            "Clock GET is verified; SET/readback/restore validation is pending"
                                .into()
                        },
                        integer_range: Some(range),
                        requires_ac: true,
                        startup_safe: CLOCK_WRITE_VERIFIED && !self.calibration,
                    }
                }
                Err(reason) => Capability::unsupported(feature, reason),
            };
        }
        let reason = match feature {
            Feature::CpuFanCurve | Feature::GpuFanCurve => {
                "Fan curves require verified performance-mode/fan-ownership readback and restore, coupled CPU/GPU/IR safety and lifecycle validation; saved Auto and Max-Fan OFF are not proof of BIOS ownership"
            }
            Feature::GpuUnleashed
            | Feature::GpuCustomTgp
            | Feature::GpuDynamicBoost
            | Feature::OmenPerformanceMode => {
                "HP model configuration is identified; coupled power, thermal, heartbeat and full restore behavior are not validated. D-state 0 remains unknown"
            }
            Feature::GpuVoltageOffsetMv | Feature::GpuVoltageLimitMv => {
                "Frequency-at-voltage curves are supported; no verified direct millivolt setter or physical voltage-ceiling contract"
            }
            Feature::GpuMemoryVoltageOffsetMv => {
                "No verified VRAM-voltage control contract on this hardware; VRAM clock is a separate setting"
            }
            Feature::GpuCoreClockLimitMhz | Feature::GpuMemoryClockLimitMhz => {
                "A full current locked-clock state and restore contract are required before exposing the public clock-lock setter"
            }
            Feature::GpuPowerLimitMw => {
                "Current power-limit readback/restore and model write validation are missing; a maximum constraint is not the current limit"
            }
            _ => {
                "Profile data is supported; no validated hardware/Windows adapter for this feature yet"
            }
        };
        Capability::unsupported(feature, reason)
    }

    fn read(&mut self, feature: Feature) -> Result<Value, String> {
        if feature == Feature::MaxFan {
            return Ok(Value::Boolean(self.fan()?.capture()?.state.enabled));
        }
        if uses_keyboard(feature) {
            return keyboard_value(feature, &self.keyboard_state()?);
        }
        if feature == Feature::GpuVfCurve {
            return self.gpu_curve()?.value();
        }
        self.gpu()?
            .offset(feature)
            .map(|s| Value::Integer(i64::from(s.current_mhz)))
    }

    fn capture(&mut self, feature: Feature) -> Result<Captured<RestoreState>, String> {
        if feature == Feature::MaxFan {
            let state = self.fan()?.capture()?;
            let saved = Captured {
                feature,
                value: Value::Boolean(state.state.enabled),
                restore: RestoreState::HpMaxFan { state },
            };
            self.captured.insert(feature, saved.clone());
            return Ok(saved);
        }
        if uses_keyboard(feature) {
            let state = self.keyboard_state()?;
            let saved = Captured {
                feature,
                value: keyboard_value(feature, &state)?,
                restore: RestoreState::HpKeyboardColors { state },
            };
            self.captured.insert(feature, saved.clone());
            return Ok(saved);
        }
        if feature == Feature::GpuVfCurve {
            let state = self.gpu_curve()?;
            self.curve_target = Some((state.value()?, state.clone()));
            let saved = Captured {
                feature,
                value: state.value()?,
                restore: RestoreState::NvidiaVfCurve {
                    identity: self.gpu()?.identity()?,
                    state,
                    core_offset: self.gpu()?.offset(Feature::GpuCoreOffsetMhz)?,
                },
            };
            self.captured.insert(feature, saved.clone());
            return Ok(saved);
        }
        let gfx_curve = if matches!(
            feature,
            Feature::GpuCoreOffsetMhz | Feature::GpuMemoryOffsetMhz
        ) {
            Some(self.gpu_curve()?)
        } else {
            None
        };
        let gpu = self.gpu()?;
        let state = gpu.offset(feature)?;
        let identity = gpu.identity()?;
        let related_core = if feature == Feature::GpuMemoryOffsetMhz {
            Some(gpu.offset(Feature::GpuCoreOffsetMhz)?)
        } else {
            None
        };
        if feature == Feature::GpuCoreOffsetMhz {
            self.curve_target = Some((
                Value::Integer(i64::from(state.current_mhz)),
                gfx_curve.clone().ok_or("No complete Graphics capture")?,
            ));
        }
        let captured = Captured {
            feature,
            value: Value::Integer(i64::from(state.current_mhz)),
            restore: RestoreState::NvidiaOffset {
                identity,
                state,
                gfx_curve,
                related_core,
            },
        };
        self.captured.insert(feature, captured.clone());
        Ok(captured)
    }

    fn restore(&mut self, saved: &Captured<RestoreState>) -> Result<(), String> {
        if self.captured.get(&saved.feature) != Some(saved) {
            return Err("Restore state was not captured by this controller session".into());
        }
        if let RestoreState::HpMaxFan { state } = &saved.restore {
            if saved.feature != Feature::MaxFan
                || saved.value != Value::Boolean(state.state.enabled)
            {
                return Err("Max-fan restore feature/value differs".into());
            }
            state.state.validate_restore()?;
            let current = self.fan()?.capture()?;
            let mut owned = state.clone();
            owned.state = self
                .fan_target
                .clone()
                .unwrap_or_else(|| state.state.clone());
            if current != *state && current != owned {
                return Err("Fan state changed outside the captured transaction".into());
            }
            self.fan()?.program(&current, state.state.enabled)?;
            if self.fan()?.capture()? != *state {
                return Err("Complete max-fan original differs after restore".into());
            }
            self.fan_target = Some(state.state.clone());
            return Ok(());
        }
        if let RestoreState::HpKeyboardColors { state } = &saved.restore {
            if !uses_keyboard(saved.feature) || saved.value != keyboard_value(saved.feature, state)?
            {
                return Err("Keyboard restore image/value differs".into());
            }
            let current = self.keyboard_state()?;
            if state.board != current.board
                || state.bios != current.bios
                || state.capability != current.capability
            {
                return Err(
                    "Keyboard identity changed; restore will not address another device".into(),
                );
            }
            // Best-effort rollback of the bounded RGB fields uses a fresh wire
            // image. Never send opaque bytes from the saved image. A concurrent
            // brightness/mode change remains untouched and fails full verification.
            self.keyboard()?.set(&current, &state.colors()?)?;
            if self.captured.contains_key(&Feature::KeyboardEffect) {
                let live = self.keyboard_state()?;
                self.keyboard()?
                    .set_mode(&live, state.custom_colors_enabled()?)?;
            }
            if self.keyboard_state()? != *state {
                return Err("Complete keyboard original did not restore".into());
            }
            return Ok(());
        }
        self.qualifies()?;
        if let RestoreState::NvidiaVfCurve {
            identity,
            state,
            core_offset,
        } = &saved.restore
        {
            if saved.feature != Feature::GpuVfCurve
                || identity != &self.gpu()?.identity()?
                || saved.value != state.value()?
            {
                return Err("V/F restore image identity/value differs".into());
            }
            return self.restore_graphics(state, Some(core_offset));
        }
        let RestoreState::NvidiaOffset {
            identity,
            state,
            gfx_curve,
            related_core,
        } = &saved.restore
        else {
            unreachable!()
        };
        let gpu = self.gpu()?;
        if identity != &gpu.identity()?
            || state.feature != saved.feature
            || saved.value != Value::Integer(i64::from(state.current_mhz))
        {
            return Err("Restore image does not match this GPU/driver/domain".into());
        }
        let current = gpu.offset(saved.feature)?;
        if current.current_mhz != state.current_mhz {
            // Returning to a captured original must remain possible near a thermal guard.
            gpu.set_offset(&Setting {
                feature: saved.feature,
                value: saved.value.clone(),
            })?;
        }
        if let Some(original) = gfx_curve {
            self.restore_graphics(original, related_core.as_ref())?;
        }
        if self.gpu()?.offset(saved.feature)? != *state {
            return Err("Full clock-offset restore readback differs".into());
        }
        Ok(())
    }

    fn verify_captured(&mut self, saved: &Captured<RestoreState>) -> Result<(), String> {
        if self.captured.get(&saved.feature) != Some(saved) {
            return Err("Control verification image is not owned by this session".into());
        }
        if let RestoreState::HpMaxFan { state } = &saved.restore {
            if saved.feature != Feature::MaxFan
                || saved.value != Value::Boolean(state.state.enabled)
            {
                return Err("Max-fan capture differs".into());
            }
            let current = self.fan()?.capture()?;
            let mut owned = state.clone();
            owned.state = self
                .fan_target
                .clone()
                .unwrap_or_else(|| state.state.clone());
            if current != *state && current != owned {
                return Err("Max-fan controls differ from original/owned target".into());
            }
            return Ok(());
        }
        if let RestoreState::HpKeyboardColors { state } = &saved.restore {
            let actual = self.keyboard_state()?;
            if actual != *state && self.keyboard_target.as_ref() != Some(&actual) {
                return Err("Keyboard controls differ from the captured complete state".into());
            }
            return Ok(());
        }
        let (offset, curve, related) = match &saved.restore {
            RestoreState::NvidiaOffset {
                state,
                gfx_curve,
                related_core,
                ..
            } => (state, gfx_curve.as_ref(), related_core.as_ref()),
            RestoreState::NvidiaVfCurve {
                state, core_offset, ..
            } => (core_offset, Some(state), None),
            RestoreState::HpKeyboardColors { .. } | RestoreState::HpMaxFan { .. } => unreachable!(),
        };
        if self.gpu()?.offset(offset.feature)? != *offset {
            return Err("Captured global offset controls differ".into());
        }
        let curve_matches = curve
            .map(|curve| curve.controls_match(&self.gpu_curve()?))
            .transpose()?;
        if curve_matches == Some(false) {
            // A preceding setting in this same transaction may intentionally
            // change a Memory adapter's captured Graphics alias. Accept only
            // the exact independently read-back control image we programmed.
            let owned = if saved.feature == Feature::GpuMemoryOffsetMhz {
                self.curve_target
                    .clone()
                    .map(|(_, target)| target.controls_match(&self.gpu_curve()?))
                    .transpose()?
                    .unwrap_or(false)
            } else {
                false
            };
            if !owned {
                return Err("Captured full Graphics curve controls differ".into());
            }
        }
        if let Some(related) = related {
            let live = self.gpu()?.offset(related.feature)?;
            let owned_core =
                self.curve_target.as_ref().is_some_and(|(value, _)| {
                    *value == Value::Integer(i64::from(live.current_mhz))
                }) && live.feature == related.feature
                    && live.minimum_mhz == related.minimum_mhz
                    && live.maximum_mhz == related.maximum_mhz;
            let owned_curve = if let Some((Value::VfCurve(_), target)) = self.curve_target.clone() {
                target.controls_match(&self.gpu_curve()?)?
                    && live.feature == related.feature
                    && live.minimum_mhz == related.minimum_mhz
                    && live.maximum_mhz == related.maximum_mhz
                    && (live.minimum_mhz..=live.maximum_mhz).contains(&live.current_mhz)
            } else {
                false
            };
            if live != *related && !owned_core && !owned_curve {
                return Err("Captured related core controls differ".into());
            }
        }
        Ok(())
    }

    fn verify_target(&mut self, setting: &Setting) -> Result<Value, String> {
        if setting.feature == Feature::MaxFan {
            let Value::Boolean(enabled) = setting.value else {
                return Err("Max-fan target is not boolean".into());
            };
            let saved = self
                .captured
                .get(&Feature::MaxFan)
                .ok_or("No max-fan capture")?;
            let RestoreState::HpMaxFan { state } = &saved.restore else {
                return Err("Wrong max-fan adapter".into());
            };
            let mut target = state.clone();
            target.state = state.state.target(enabled)?;
            if self.fan()?.capture()? != target {
                return Err("Complete max-fan target readback differs".into());
            }
            if self.fan_calibration {
                std::thread::sleep(std::time::Duration::from_millis(1000));
                let report = self.fan_report()?;
                if report.max_fan != target.state {
                    return Err("Max-fan flag did not remain set during the fixed trial".into());
                }
                self.fan_observations.push(report);
            }
            return Ok(setting.value.clone());
        }
        if uses_keyboard(setting.feature) {
            let expected = self
                .keyboard_target
                .as_ref()
                .ok_or("No complete keyboard target proof")?
                .clone();
            let actual = self.keyboard_state()?;
            if actual != expected || setting.value != keyboard_value(setting.feature, &actual)? {
                return Err("Keyboard target, opaque fields or brightness readback differs".into());
            }
            return Ok(setting.value.clone());
        }
        if matches!(
            setting.feature,
            Feature::GpuVfCurve | Feature::GpuCoreOffsetMhz
        ) {
            let (wanted_value, wanted_state) = self
                .curve_target
                .clone()
                .ok_or("No complete programmed curve proof")?;
            let actual = self.gpu_curve()?;
            if setting.value != wanted_value || !wanted_state.controls_match(&actual)? {
                return Err(format!(
                    "Programmed V/F control image differs: expected deltas {:?}, actual {:?}",
                    wanted_state
                        .nodes
                        .iter()
                        .map(|p| p.delta_khz)
                        .collect::<Vec<_>>(),
                    actual.nodes.iter().map(|p| p.delta_khz).collect::<Vec<_>>()
                ));
            }
            if setting.feature == Feature::GpuVfCurve {
                return actual.value();
            }
        }
        let actual = self.read(setting.feature)?;
        if setting.feature == Feature::GpuMemoryOffsetMhz
            && !self.captured.contains_key(&Feature::GpuVfCurve)
            && !self.captured.contains_key(&Feature::GpuCoreOffsetMhz)
        {
            let saved = self
                .captured
                .get(&setting.feature)
                .cloned()
                .ok_or("No complete memory capture")?;
            if let RestoreState::NvidiaOffset {
                gfx_curve: Some(original),
                related_core: Some(core),
                ..
            } = &saved.restore
            {
                if !original.controls_match(&self.gpu_curve()?)?
                    || self.gpu()?.offset(Feature::GpuCoreOffsetMhz)? != *core
                {
                    return Err("Memory-only tuning changed the captured Graphics controls".into());
                }
            } else {
                return Err("No coupled Graphics restore image for memory tuning".into());
            }
        }
        if actual == setting.value {
            Ok(actual)
        } else {
            Err(format!(
                "Readback mismatch for {:?}: requested {:?}, obtained {:?}",
                setting.feature, setting.value, actual
            ))
        }
    }

    fn prepare_recovery(
        &mut self,
        transaction: &PendingTransaction<RestoreState>,
    ) -> Result<(), String> {
        transaction.validate()?;
        if transaction.requested.iter().any(|s| uses_nvidia(s.feature)) {
            self.qualifies()?;
        }
        if !transaction.binding.matches(&self.context()?.binding) {
            return Err("Recovery device binding differs".into());
        }
        // Validate every image/current value before arming any restore. A user or
        // other controller's unrelated tuning must never be overwritten silently.
        for (saved, requested) in transaction.original.iter().zip(&transaction.requested) {
            if let RestoreState::HpMaxFan { state } = &saved.restore {
                if saved.feature != Feature::MaxFan
                    || requested.feature != Feature::MaxFan
                    || saved.value != Value::Boolean(state.state.enabled)
                {
                    return Err("Persisted max-fan feature/value differs".into());
                }
                state.state.validate_restore()?;
                let Value::Boolean(enabled) = requested.value else {
                    return Err("Persisted max-fan target is not boolean".into());
                };
                let mut target = state.clone();
                target.state = state.state.target(enabled)?;
                let current = self.fan()?.capture()?;
                if current != *state && current != target {
                    return Err("Persisted max-fan identity/opaque state differs".into());
                }
                self.fan_target = Some(target.state);
                continue;
            }
            if let RestoreState::HpKeyboardColors { state } = &saved.restore {
                if !uses_keyboard(saved.feature)
                    || requested.feature != saved.feature
                    || saved.value != keyboard_value(saved.feature, state)?
                {
                    return Err("Persisted keyboard restore image/value differs".into());
                }
                let target = lighting_target(state, &transaction.requested)?;
                let allow_mode = transaction
                    .requested
                    .iter()
                    .any(|s| s.feature == Feature::KeyboardEffect);
                let current = self.keyboard_state()?;
                if !state.same_invariant_controls(&current, allow_mode)? {
                    return Err("Keyboard opaque fields differ; no recovery SET is allowed".into());
                }
                let original = state.colors()?;
                let live = current.colors()?;
                let colors = target.colors()?;
                if original
                    .iter()
                    .zip(&live)
                    .zip(&colors)
                    .any(|((old, now), wanted)| {
                        (0..3).any(|i| now[i] != old[i] && now[i] != wanted[i])
                    })
                {
                    return Err(
                        "Keyboard colors differ from both original and requested controls".into(),
                    );
                }
                let original_raw = state
                    .legacy
                    .as_ref()
                    .ok_or("Missing original mode")?
                    .brightness_raw_byte;
                let wanted_raw = target
                    .legacy
                    .as_ref()
                    .ok_or("Missing target mode")?
                    .brightness_raw_byte;
                let live_raw = current
                    .legacy
                    .as_ref()
                    .ok_or("Missing live mode")?
                    .brightness_raw_byte;
                if live_raw != original_raw && live_raw != wanted_raw {
                    return Err("Keyboard mode differs from original and requested controls".into());
                }
                self.keyboard_target = Some(target);
                continue;
            }
            if let RestoreState::NvidiaVfCurve {
                identity,
                state,
                core_offset,
            } = &saved.restore
            {
                if identity != &self.gpu()?.identity()?
                    || saved.feature != Feature::GpuVfCurve
                    || saved.value != state.value()?
                {
                    return Err("Persisted V/F restore image differs".into());
                }
                let live_offset = self.gpu()?.offset(Feature::GpuCoreOffsetMhz)?;
                if core_offset.feature != Feature::GpuCoreOffsetMhz
                    || core_offset.minimum_mhz != live_offset.minimum_mhz
                    || core_offset.maximum_mhz != live_offset.maximum_mhz
                    || !(core_offset.minimum_mhz..=core_offset.maximum_mhz)
                        .contains(&core_offset.current_mhz)
                {
                    return Err("Persisted V/F offset domain or driver bounds differ".into());
                }
                let Value::VfCurve(target) = &requested.value else {
                    return Err("Persisted V/F target is invalid".into());
                };
                let deltas = state.target_deltas(target)?;
                let current = self.gpu_curve()?;
                if state.first_index != current.first_index
                    || state.last_index != current.last_index
                    || state.nodes.len() != current.nodes.len()
                    || state.minimum_delta_khz != current.minimum_delta_khz
                    || state.maximum_delta_khz != current.maximum_delta_khz
                    || state.nodes.iter().zip(&current.nodes).zip(&deltas).any(
                        |((before, actual), (_, wanted))| {
                            before.index != actual.index
                                || before.voltage_uv != actual.voltage_uv
                                || (actual.delta_khz != before.delta_khz
                                    && actual.delta_khz != *wanted)
                        },
                    )
                {
                    return Err("V/F state differs from recorded original/target nodes; inspect before recovery".into());
                }
                continue;
            }
            let RestoreState::NvidiaOffset {
                identity,
                state,
                gfx_curve,
                related_core,
            } = &saved.restore
            else {
                unreachable!()
            };
            let gpu = self.gpu()?;
            let current = gpu.offset(saved.feature)?;
            if identity != &gpu.identity()?
                || state.feature != saved.feature
                || saved.value != Value::Integer(i64::from(state.current_mhz))
                || current.minimum_mhz != state.minimum_mhz
                || current.maximum_mhz != state.maximum_mhz
                || !(state.minimum_mhz..=state.maximum_mhz).contains(&state.current_mhz)
            {
                return Err("Recovery image identity/domain/limits differ from the driver".into());
            }
            let actual = Value::Integer(i64::from(current.current_mhz));
            if actual != saved.value && actual != requested.value {
                return Err("GPU tuning differs from both recorded original and target; inspect it before recovery".into());
            }
            if let Some(before) = gfx_curve {
                if saved.feature == Feature::GpuMemoryOffsetMhz
                    && related_core.is_none()
                    && current.current_mhz != state.current_mhz
                {
                    return Err("Memory recovery is missing its related global core image".into());
                }
                before.validate()?;
                let current = self.gpu_curve()?;
                let curve_target = transaction
                    .original
                    .iter()
                    .zip(&transaction.requested)
                    .find_map(|(original, wanted)| {
                        if let (RestoreState::NvidiaVfCurve { state, .. }, Value::VfCurve(points)) =
                            (&original.restore, &wanted.value)
                        {
                            Some(state.target_state(points))
                        } else {
                            None
                        }
                    })
                    .transpose()?;
                let core_target = transaction.requested.iter().find_map(|s| {
                    if s.feature == Feature::GpuCoreOffsetMhz {
                        if let Value::Integer(v) = s.value {
                            Some(v * 1000)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });
                if !before.domain_matches(&current)?
                    || before
                        .nodes
                        .iter()
                        .zip(&current.nodes)
                        .enumerate()
                        .any(|(i, (old, now))| {
                            now.delta_khz != old.delta_khz
                                && !curve_target.as_ref().is_some_and(|target| {
                                    target.nodes[i].delta_khz == now.delta_khz
                                })
                                && !core_target.is_some_and(|target| {
                                    i64::from(now.delta_khz) == target || now.delta_khz == 0
                                })
                        })
                {
                    return Err("Recorded global-offset V/F image or current curve is not compatible; inspect before recovery".into());
                }
            } else if matches!(
                saved.feature,
                Feature::GpuCoreOffsetMhz | Feature::GpuMemoryOffsetMhz
            ) {
                let curve = self.gpu_curve()?;
                if current.current_mhz != state.current_mhz
                    || curve
                        .nodes
                        .iter()
                        .any(|p| p.delta_khz != curve.nodes[0].delta_khz)
                {
                    return Err("Legacy offset recovery lacks a complete V/F original; no clock write is permitted".into());
                }
            }
            if let Some(before) = related_core {
                let actual = self.gpu()?.offset(Feature::GpuCoreOffsetMhz)?;
                let requested_core = transaction.requested.iter().find_map(|s| {
                    if s.feature == Feature::GpuCoreOffsetMhz {
                        if let Value::Integer(v) = s.value {
                            Some(v)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });
                // A complete V/F SET can change the scalar NVML alias. Every
                // original/target Graphics control was checked above; its
                // summary must keep the same domain and hardware bounds.
                let curve_alias = transaction
                    .requested
                    .iter()
                    .any(|s| s.feature == Feature::GpuVfCurve)
                    && (actual.minimum_mhz..=actual.maximum_mhz).contains(&actual.current_mhz);
                if before.feature != Feature::GpuCoreOffsetMhz
                    || before.minimum_mhz != actual.minimum_mhz
                    || before.maximum_mhz != actual.maximum_mhz
                    || !(before.minimum_mhz..=before.maximum_mhz).contains(&before.current_mhz)
                    || (actual.current_mhz != before.current_mhz
                        && Some(i64::from(actual.current_mhz)) != requested_core
                        && !curve_alias)
                {
                    return Err(
                        "Related core offset differs from the recorded original/target".into(),
                    );
                }
            }
        }
        self.captured = transaction
            .original
            .iter()
            .map(|s| (s.feature, s.clone()))
            .collect();
        Ok(())
    }

    fn write(&mut self, setting: &Setting) -> Result<(), String> {
        let context = self.context()?;
        let capability = self.capability(setting.feature);
        if capability.access != Access::ReadWrite
            || (capability.requires_ac && context.ac_connected != Some(true))
        {
            return Err(capability.reason);
        }
        if setting.feature == Feature::MaxFan {
            let Value::Boolean(enabled) = setting.value else {
                return Err("Max-fan target requires a boolean".into());
            };
            if self.fan_calibration && !enabled {
                return Err("Qualification only enables maximum cooling".into());
            }
            let saved = self
                .captured
                .get(&Feature::MaxFan)
                .ok_or("Missing durable max-fan original")?
                .clone();
            let RestoreState::HpMaxFan { state } = &saved.restore else {
                return Err("Wrong max-fan adapter".into());
            };
            let current = self.fan()?.capture()?;
            let mut owned = state.clone();
            owned.state = self
                .fan_target
                .clone()
                .unwrap_or_else(|| state.state.clone());
            if current != *state && current != owned {
                return Err("Max-fan controls changed outside this transaction".into());
            }
            self.fan_target = Some(state.state.target(enabled)?);
            self.fan()?.program(&current, enabled)?;
            return Ok(());
        }
        if setting.feature == Feature::KeyboardEffect {
            let Value::Choice(mode) = setting.value else {
                return Err("Missing classic keyboard mode".into());
            };
            if !matches!(mode, Choice::Static | Choice::FirmwareDefault)
                || self
                    .keyboard_mode_calibration
                    .is_some_and(|allowed| allowed != mode)
            {
                return Err("Keyboard mode is outside the qualified operation".into());
            }
            let saved = self
                .captured
                .get(&setting.feature)
                .ok_or("No durable mode capture")?
                .clone();
            let RestoreState::HpKeyboardColors { state } = saved.restore else {
                return Err("Wrong keyboard mode adapter".into());
            };
            let live = self.keyboard_state()?;
            if live != state && self.keyboard_target.as_ref() != Some(&live) {
                return Err("Mode controls changed outside this transaction".into());
            }
            self.keyboard_target = Some(self.keyboard()?.set_mode(&live, mode == Choice::Static)?);
            return Ok(());
        }
        if setting.feature == Feature::KeyboardColors {
            let Value::Colors(colors) = &setting.value else {
                return Err("Keyboard color target is not typed RGB".into());
            };
            if self
                .keyboard_calibration
                .as_ref()
                .is_some_and(|allowed| allowed != colors)
            {
                return Err(
                    "Keyboard calibration permits only its fixed one-component delta".into(),
                );
            }
            let saved = self
                .captured
                .get(&setting.feature)
                .ok_or("No durable keyboard capture")?
                .clone();
            let RestoreState::HpKeyboardColors { state } = saved.restore else {
                return Err("Keyboard capture has another adapter".into());
            };
            let live = self.keyboard_state()?;
            if live != state && self.keyboard_target.as_ref() != Some(&live) {
                return Err("RGB controls changed outside this transaction".into());
            }
            self.keyboard_target = Some(self.keyboard()?.set(&live, colors)?);
            return Ok(());
        }
        if setting.feature == Feature::GpuVfCurve {
            self.validate_setting(setting)?;
            self.gpu()?.thermal_guard(87)?;
            let Value::VfCurve(target) = &setting.value else {
                return Err("V/F target must be a complete typed curve".into());
            };
            let state = match &self
                .captured
                .get(&setting.feature)
                .ok_or("No durable curve capture")?
                .restore
            {
                RestoreState::NvidiaVfCurve { state, .. } => state.clone(),
                _ => return Err("Curve capture has another adapter".into()),
            };
            let proof = state.target_state(target)?;
            let deltas: Vec<_> = proof.nodes.iter().map(|p| (p.index, p.delta_khz)).collect();
            self.curve()?.set_deltas(&state, &deltas)?;
            self.curve_target = Some((setting.value.clone(), proof));
            return Ok(());
        }
        let Value::Integer(value) = setting.value else {
            return Err("Clock offset needs an integer".into());
        };
        let (minimum, maximum) = capability.integer_range.ok_or("No driver bounds")?;
        if !(minimum..=maximum).contains(&value) {
            return Err("Value outside validated operation bounds".into());
        }
        let gpu = self.gpu()?;
        // The measured HP threshold is 87 C; keep three degrees of headroom
        // below the stricter of that threshold and the driver's slowdown limit.
        gpu.thermal_guard(87)?;
        let memory_graphics = if setting.feature == Feature::GpuMemoryOffsetMhz {
            Some((
                self.gpu_curve()?,
                self.gpu()?.offset(Feature::GpuCoreOffsetMhz)?,
            ))
        } else {
            None
        };
        self.gpu()?.set_offset(setting)?;
        if setting.feature == Feature::GpuMemoryOffsetMhz {
            if let Some((curve, core)) = memory_graphics {
                self.restore_graphics(&curve, Some(&core))?;
            } else {
                return Err("Memory write requires a complete Graphics restore image".into());
            }
        } else if setting.feature == Feature::GpuCoreOffsetMhz {
            self.curve_target = Some((setting.value.clone(), self.gpu_curve()?));
        }
        Ok(())
    }
    fn validate_setting(&mut self, setting: &Setting) -> Result<(), String> {
        setting.validate()?;
        if setting.feature == Feature::KeyboardEffect {
            let Value::Choice(mode) = setting.value else {
                return Err("Typed keyboard mode missing".into());
            };
            if !matches!(mode, Choice::Static | Choice::FirmwareDefault)
                || self
                    .keyboard_mode_calibration
                    .is_some_and(|allowed| allowed != mode)
            {
                return Err("Only the measured classic mode choices are supported".into());
            }
        }
        if setting.feature == Feature::KeyboardColors {
            let Value::Colors(colors) = &setting.value else {
                return Err("Typed RGB colors are required".into());
            };
            self.keyboard_state()?.with_colors(colors)?;
            if self
                .keyboard_calibration
                .as_ref()
                .is_some_and(|allowed| allowed != colors)
            {
                return Err("The fixed RGB trial does not allow other targets".into());
            }
        }
        if setting.feature == Feature::GpuVfCurve {
            let Value::VfCurve(points) = &setting.value else {
                return Err("V/F curve value missing".into());
            };
            let state = self.gpu_curve()?;
            let deltas = state.target_deltas(points)?;
            if self.curve_calibration
                && deltas
                    .iter()
                    .zip(&state.nodes)
                    .any(|((_, delta), node)| *delta != node.delta_khz - 15000)
            {
                return Err("Fixed curve trial only permits minus 15 MHz per Graphics node".into());
            }
        }
        Ok(())
    }
}
