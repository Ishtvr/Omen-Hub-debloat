//! V/F policies in physical units. Voltage anchors are identities, not write fields.
use crate::features::Feature;
use crate::profiles::{Setting, Value, VfPoint};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VfNode {
    pub index: u16,
    pub voltage_uv: u32,
    pub frequency_khz: u32,
    pub delta_khz: i32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VfState {
    pub nodes: Vec<VfNode>,
    pub minimum_delta_khz: i32,
    pub maximum_delta_khz: i32,
    pub first_index: u16,
    pub last_index: u16,
}
impl VfState {
    /// Driver-reported frequencies can move while all programmed deltas remain
    /// unchanged. Restore verifies the complete writable image and voltage grid,
    /// not an old instantaneous boost-frequency observation.
    pub fn controls_match(&self, actual: &Self) -> Result<bool, String> {
        Ok(self.domain_matches(actual)?
            && self
                .nodes
                .iter()
                .zip(&actual.nodes)
                .all(|(a, b)| a.delta_khz == b.delta_khz))
    }
    pub fn domain_matches(&self, actual: &Self) -> Result<bool, String> {
        self.validate()?;
        actual.validate()?;
        Ok(self.first_index == actual.first_index
            && self.last_index == actual.last_index
            && self.minimum_delta_khz == actual.minimum_delta_khz
            && self.maximum_delta_khz == actual.maximum_delta_khz
            && self.nodes.len() == actual.nodes.len()
            && self.nodes.iter().zip(&actual.nodes).all(|(before, now)| {
                before.index == now.index && before.voltage_uv == now.voltage_uv
            }))
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.last_index < self.first_index
            || self.last_index >= 255
            || self.nodes.len() != usize::from(self.last_index - self.first_index) + 1
            || self.minimum_delta_khz > self.maximum_delta_khz
        {
            return Err(crate::message::msg("vf.err.domain", &[]));
        }
        for (position, node) in self.nodes.iter().enumerate() {
            if usize::from(node.index) != usize::from(self.first_index) + position
                || node.voltage_uv == 0
                || node.frequency_khz == 0
                || !(self.minimum_delta_khz..=self.maximum_delta_khz).contains(&node.delta_khz)
            {
                return Err(crate::message::msg("vf.err.node", &[]));
            }
        }
        if self
            .nodes
            .windows(2)
            .any(|p| p[0].voltage_uv >= p[1].voltage_uv)
        {
            return Err(crate::message::msg("vf.err.grid_order", &[]));
        }
        Ok(())
    }
    pub fn value(&self) -> Result<Value, String> {
        self.validate()?;
        if self
            .nodes
            .iter()
            .any(|p| p.voltage_uv % 1000 != 0 || p.frequency_khz % 1000 != 0)
        {
            return Err(crate::message::msg("vf.err.units", &[]));
        }
        let value = Value::VfCurve(
            self.nodes
                .iter()
                .map(|p| VfPoint {
                    voltage_mv: p.voltage_uv / 1000,
                    frequency_mhz: p.frequency_khz / 1000,
                })
                .collect(),
        );
        Setting {
            feature: Feature::GpuVfCurve,
            value: value.clone(),
        }
        .validate()?;
        Ok(value)
    }
    pub fn target_deltas(&self, target: &[VfPoint]) -> Result<Vec<(u16, i32)>, String> {
        self.validate()?;
        Setting {
            feature: Feature::GpuVfCurve,
            value: Value::VfCurve(target.to_vec()),
        }
        .validate()?;
        if target.len() != self.nodes.len() {
            return Err(crate::message::msg("vf.err.complete_grid", &[]));
        }
        self.nodes
            .iter()
            .zip(target)
            .map(|(original, wanted)| {
                if u64::from(wanted.voltage_mv) * 1000 != u64::from(original.voltage_uv) {
                    return Err(crate::message::msg("vf.err.anchors", &[]));
                }
                let delta = i64::from(wanted.frequency_mhz) * 1000
                    - i64::from(original.frequency_khz)
                    + i64::from(original.delta_khz);
                let delta = i32::try_from(delta)
                    .map_err(|_| crate::message::msg("vf.err.delta_representation", &[]))?;
                if !(self.minimum_delta_khz..=self.maximum_delta_khz).contains(&delta) {
                    return Err(crate::message::msg("vf.err.bounds", &[]));
                }
                Ok((original.index, delta))
            })
            .collect()
    }
    /// Derive once from the durable capture, so recovery can reconstruct the
    /// exact controls even if boost observations change before or after SET.
    pub fn target_state(&self, target: &[VfPoint]) -> Result<Self, String> {
        let deltas = self.target_deltas(target)?;
        let mut state = self.clone();
        for ((node, (_, delta)), point) in state.nodes.iter_mut().zip(deltas).zip(target) {
            node.delta_khz = delta;
            node.frequency_khz = point
                .frequency_mhz
                .checked_mul(1000)
                .ok_or(crate::message::msg("vf.err.frequency", &[]))?;
        }
        Ok(state)
    }
}

/// Keep the lower curve and flatten from an existing voltage anchor. This is a
/// frequency-at-voltage target, not a guaranteed physical voltage ceiling.
pub fn plateau(state: &VfState, voltage_mv: u32, frequency_mhz: u32) -> Result<Value, String> {
    let Value::VfCurve(mut points) = state.value()? else {
        unreachable!()
    };
    let first = points
        .iter()
        .position(|p| p.voltage_mv == voltage_mv)
        .ok_or(crate::message::msg("vf.err.anchor_required", &[]))?;
    for point in &mut points[first..] {
        point.frequency_mhz = frequency_mhz;
    }
    state.target_deltas(&points)?;
    Ok(Value::VfCurve(points))
}

/// Fixed qualification operations preserve the existing curve, with only the
/// selected frequency deltas shifted. No assumed factory image is substituted.
#[derive(Debug, Clone, Copy)]
pub enum CurveTrial {
    AllPointsMinus15,
    FirstPointMinus15,
}

impl CurveTrial {
    pub fn target(self, state: &VfState) -> Result<Value, String> {
        let Value::VfCurve(mut points) = state.value()? else {
            unreachable!()
        };
        for (index, point) in points.iter_mut().enumerate() {
            if matches!(self, Self::AllPointsMinus15) || index == 0 {
                point.frequency_mhz = point
                    .frequency_mhz
                    .checked_sub(15)
                    .filter(|value| *value > 0)
                    .ok_or("Frequency is too low for the fixed trial")?;
            }
        }
        self.validate(state, &points)?;
        Ok(Value::VfCurve(points))
    }

    pub fn validate(self, state: &VfState, points: &[VfPoint]) -> Result<(), String> {
        for (position, ((_, delta), original)) in state
            .target_deltas(points)?
            .iter()
            .zip(&state.nodes)
            .enumerate()
        {
            let shift = if matches!(self, Self::AllPointsMinus15) || position == 0 {
                15_000
            } else {
                0
            };
            if original.delta_khz.checked_sub(shift) != Some(*delta) {
                return Err(
                    "Fixed curve trial permits only its selected minus-15-MHz delta".into(),
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> VfState {
        VfState {
            nodes: vec![
                VfNode {
                    index: 3,
                    voltage_uv: 800000,
                    frequency_khz: 1000000,
                    delta_khz: 15000,
                },
                VfNode {
                    index: 4,
                    voltage_uv: 900000,
                    frequency_khz: 1200000,
                    delta_khz: 30000,
                },
                VfNode {
                    index: 5,
                    voltage_uv: 1000000,
                    frequency_khz: 1400000,
                    delta_khz: 45000,
                },
            ],
            minimum_delta_khz: -500000,
            maximum_delta_khz: 500000,
            first_index: 3,
            last_index: 5,
        }
    }
    #[test]
    fn deltas_preserve_existing_tuning_and_target_the_exact_grid() {
        let state = state();
        let Value::VfCurve(points) = plateau(&state, 900, 1200).unwrap() else {
            panic!()
        };
        assert_eq!(
            state.target_deltas(&points).unwrap(),
            [(3, 15000), (4, 30000), (5, -155000)]
        );
        let mut wrong = points.clone();
        wrong[1].voltage_mv = 905;
        assert!(state.target_deltas(&wrong).is_err());
        assert!(state.target_deltas(&points[..2]).is_err());
        assert!(plateau(&state, 900, 999).is_err());
        assert!(plateau(&state, 900, u32::MAX).is_err());
    }
    #[test]
    fn incomplete_and_nonrepresentable_states_are_not_rounded() {
        let mut state = state();
        state.nodes[1].frequency_khz += 500;
        assert!(state.value().is_err());
        state.nodes.remove(1);
        assert!(state.validate().is_err());
    }
    #[test]
    fn restore_compares_all_controls_without_claiming_a_fixed_boost_observation() {
        let original = state();
        let mut live = original.clone();
        for point in &mut live.nodes {
            point.frequency_khz += 30000;
        }
        assert!(original.controls_match(&live).unwrap());
        live.nodes[1].delta_khz += 15000;
        assert!(!original.controls_match(&live).unwrap());
        live.nodes[1].delta_khz = original.nodes[1].delta_khz;
        live.nodes[1].voltage_uv += 1000;
        assert!(!original.controls_match(&live).unwrap());
    }

    #[test]
    fn fixed_trials_preserve_an_existing_nonuniform_curve_and_select_exact_points() {
        let original = state();
        let Value::VfCurve(all) = CurveTrial::AllPointsMinus15.target(&original).unwrap() else {
            panic!()
        };
        assert_eq!(
            original.target_deltas(&all).unwrap(),
            [(3, 0), (4, 15000), (5, 30000)]
        );
        let Value::VfCurve(first) = CurveTrial::FirstPointMinus15.target(&original).unwrap() else {
            panic!()
        };
        assert_eq!(
            original.target_deltas(&first).unwrap(),
            [(3, 0), (4, 30000), (5, 45000)]
        );
        assert!(
            CurveTrial::AllPointsMinus15
                .validate(&original, &first)
                .is_err()
        );
        assert!(
            CurveTrial::FirstPointMinus15
                .validate(&original, &all)
                .is_err()
        );
        assert_eq!(
            original
                .nodes
                .iter()
                .map(|n| n.delta_khz)
                .collect::<Vec<_>>(),
            [15000, 30000, 45000]
        );
    }

    #[test]
    fn trial_rejects_changed_observation_grid_and_insufficient_headroom() {
        let original = state();
        let Value::VfCurve(target) = CurveTrial::AllPointsMinus15.target(&original).unwrap() else {
            panic!()
        };
        let mut changed = original.clone();
        changed.nodes[1].frequency_khz += 15000;
        assert!(
            CurveTrial::AllPointsMinus15
                .validate(&changed, &target)
                .is_err()
        );
        changed = original.clone();
        changed.nodes[0].voltage_uv += 1000;
        assert!(
            CurveTrial::AllPointsMinus15
                .validate(&changed, &target)
                .is_err()
        );
        changed = original;
        changed.minimum_delta_khz = 15000;
        assert!(CurveTrial::AllPointsMinus15.target(&changed).is_err());
        changed.minimum_delta_khz = -500000;
        changed.nodes[0].frequency_khz = 10000;
        assert!(CurveTrial::FirstPointMinus15.target(&changed).is_err());
    }
}
