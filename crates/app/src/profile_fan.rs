//! Shared-helper policy coordination around the verified direct-control subset.
//! Caller owns the foreground lease. No helper replacement or firmware opcode.
use omen_core::{
    Inventory,
    message::msg,
    profiles::PerformanceProfile,
    transaction::{self, Applied, ApplyFailure},
};
use omen_platform::{
    Controller, RestoreState,
    profile_store::{
        FanPolicyOwner, FanPolicyPhase, FanPolicyRequest, ProfileStore, profile_digest,
    },
};

fn supported(board: &str, bios: &str) -> Result<(), String> {
    if board != "8E35" || bios != "F.13" {
        return Err(msg("fan_profile.err.device", &[]));
    }
    Ok(())
}
pub fn prepare(
    profile: &PerformanceProfile,
    store: &ProfileStore,
    inventory: &Inventory,
) -> Result<Option<FanPolicyRequest>, String> {
    let parts = omen_core::fan_profile::split(profile)?;
    let Some(points) = parts.shared else {
        return Ok(None);
    };
    supported(&inventory.board, &inventory.bios)?;
    omen_platform::hp_control::fan_policy_change_allowed()?;
    // Inspect/resume only the installed receipt-bound helper. Never replace an
    // active older image merely because it differs from this UI executable.
    let request = store.prepare_fan_policy(
        FanPolicyOwner::Profile {
            id: profile.id.clone(),
            digest: profile_digest(profile)?,
        },
        &inventory.board,
        &inventory.bios,
        &points,
    )?;
    if let Err(error) = omen_platform::ensure_fan_service_running() {
        let rollback = store.restore_fan_policy(&request).err().unwrap_or_default();
        return Err(msg(
            "fan_profile.err.request",
            &[&msg("fan_profile.err.helper", &[&error]), &rollback],
        ));
    }
    Ok(Some(request))
}
pub fn recover_pending(store: &ProfileStore) -> Result<(), String> {
    if let Some(request) = store.fan_policy_request()?.filter(|r| {
        matches!(
            r.phase,
            FanPolicyPhase::Prepared | FanPolicyPhase::Recovering
        )
    }) {
        omen_platform::hp_control::fan_policy_change_allowed()?;
        supported(&request.board, &request.bios)?;
        store.restore_fan_policy(&request)?;
    }
    Ok(())
}
pub fn restore_for(store: &ProfileStore, id: &str, digest: &str) -> Result<bool, String> {
    let Some(request) = store
        .fan_policy_request()?
        .filter(|r| r.belongs_to(id, digest) && r.phase != FanPolicyPhase::Restored)
    else {
        return Ok(false);
    };
    omen_platform::hp_control::fan_policy_change_allowed()?;
    supported(&request.board, &request.bios)?;
    store.restore_fan_policy(&request)?;
    Ok(true)
}
/// Direct result remains exactly the verified numeric controls. Policy request
/// state is separate, because the legacy helper has no curve-digest ACK.
pub fn finish(
    store: &mut ProfileStore,
    controller: &mut Controller,
    profile: &PerformanceProfile,
    request: Option<&FanPolicyRequest>,
    result: Result<Applied, ApplyFailure>,
) -> (Result<Applied, ApplyFailure>, Option<serde_json::Value>) {
    let Some(request) = request else {
        return (result, None);
    };
    let result = match result {
        Err(mut failure) => {
            if let Err(e) = store.restore_fan_policy(request) {
                failure.rollback_errors.push(e);
            }
            return (Err(failure), None);
        }
        Ok(applied) => applied,
    };
    let expected = omen_core::fan_profile::split(profile).map(|p| p.direct.effective().settings);
    let checked = expected.and_then(|targets| {
        // A skipped value no longer withdraws the whole preset or its fan
        // request (user rule); every target must still be accounted for.
        let accounted = |target: &omen_core::profiles::Setting| {
            result.verified.contains(target)
                || result.skipped.iter().any(|s| s.feature == target.feature)
        };
        if !targets.iter().all(accounted) {
            return Err(msg("fan_profile.err.direct_partial", &[]));
        }
        store.commit_fan_policy(request)
    });
    match checked {
        Ok(()) => {
            let parts =
                omen_core::fan_profile::split(profile).expect("Previously checked fan pair");
            (
                Ok(result),
                Some(
                    serde_json::json!({"state":"requested","point_count":parts.shared.unwrap().len(),"physical_digest_ack":false}),
                ),
            )
        }
        Err(error) => {
            let mut errors = vec![];
            let direct = store.last_transaction::<RestoreState>().and_then(|saved| {
                if saved.profile_id != profile.id
                    || saved.profile_digest != profile_digest(profile)?
                {
                    return Err(msg("fan_profile.err.journal_changed", &[]));
                }
                transaction::recover(&saved, controller, store)
                    .map_err(|e| serde_json::to_string(&e).unwrap_or(e.error))
            });
            let direct_status = match direct {
                Ok(()) => msg("fan_profile.direct_restored", &[]),
                Err(e) => {
                    errors.push(e.clone());
                    msg("fan_profile.direct_pending", &[&e])
                }
            };
            let cooling_status = match store.restore_fan_policy(request) {
                Ok(()) => msg("fan_profile.policy_restored", &[]),
                Err(e) => {
                    errors.push(e.clone());
                    msg("fan_profile.policy_pending", &[&e])
                }
            };
            (
                Err(ApplyFailure {
                    error: msg(
                        "fan_profile.err.after_direct",
                        &[&error, &direct_status, &cooling_status],
                    ),
                    rollback_errors: errors,
                }),
                None,
            )
        }
    }
}
/// Standalone library Apply uses the same request journal and installed helper,
/// without modifying a performance profile or its hardware journal.
pub fn request_library(
    store: &ProfileStore,
    preset: &omen_core::fan_profile::FanPreset,
) -> Result<(), String> {
    let _lease = omen_platform::ControlLease::acquire_foreground()?;
    preset.validate()?;
    let inventory = omen_platform::inventory();
    supported(&inventory.board, &inventory.bios)?;
    if preset.board != inventory.board || preset.bios != inventory.bios {
        return Err(msg("fan_profile.err.device", &[]));
    }
    omen_platform::hp_control::fan_policy_change_allowed()?;
    if store.pending()? || store.fan_pending_direct_applied()? {
        return Err(msg("store.err.pending_checkpoint", &[]));
    }
    recover_pending(store)?;
    let request = store.prepare_library_fan_policy(preset)?;
    if let Err(error) = omen_platform::ensure_fan_service_running() {
        let rollback = store.restore_fan_policy(&request).err().unwrap_or_default();
        return Err(msg(
            "fan_profile.err.request",
            &[&msg("fan_profile.err.helper", &[&error]), &rollback],
        ));
    }
    if let Err(error) = store.commit_fan_policy(&request) {
        let rollback = store.restore_fan_policy(&request).err().unwrap_or_default();
        return Err(msg("fan_profile.err.request", &[&error, &rollback]));
    }
    Ok(())
}
pub fn verify_request(store: &ProfileStore, profile: &PerformanceProfile) -> Result<(), String> {
    if omen_core::fan_profile::split(profile)?.shared.is_some()
        && !store.fan_policy_matches(&profile.id, &profile_digest(profile)?)?
    {
        return Err(msg("fan_profile.err.request_changed", &[]));
    }
    Ok(())
}
pub fn prelude_failed(
    store: &ProfileStore,
    request: Option<&FanPolicyRequest>,
    error: String,
) -> String {
    if let Some(request) = request {
        let restore = store.restore_fan_policy(request).err().unwrap_or_default();
        msg("fan_profile.err.prelude", &[&error, &restore])
    } else {
        error
    }
}
