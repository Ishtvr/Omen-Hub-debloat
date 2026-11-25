//! Application preferences and the "Start with Windows" logon task. No hardware/startup
//! approval API is reachable through this module.
use omen_core::appearance::Appearance;
use omen_core::{app_settings::AppSettings, message::msg};
use omen_platform::profile_store::ProfileStore;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub options: AppSettings,
    pub appearance: Appearance,
    pub gpu_tgp_max_w: u32,
    pub run_entry: Option<String>,
}
trait Backend {
    fn read(&self) -> Result<Snapshot, String>;
    fn options(
        &mut self,
        options: &AppSettings,
        appearance: &Appearance,
        gpu_tgp_max_w: u32,
    ) -> Result<(), String>;
    fn run_entry(&mut self, entry: Option<&str>) -> Result<(), String>;
}
struct Store(ProfileStore);
impl Backend for Store {
    fn read(&self) -> Result<Snapshot, String> {
        let run_entry = crate::autostart::entry()?;
        let legacy_tray = run_entry
            .as_ref()
            .is_some_and(|entry| entry.trim_end().ends_with(" --tray"));
        Ok(Snapshot {
            options: self.0.app_settings_with_start_default(legacy_tray)?,
            appearance: self.0.ui_appearance()?,
            gpu_tgp_max_w: self.0.gpu_tgp_max_w()?,
            run_entry,
        })
    }
    fn options(
        &mut self,
        options: &AppSettings,
        appearance: &Appearance,
        gpu_tgp_max_w: u32,
    ) -> Result<(), String> {
        persist_options(&self.0, options, appearance, gpu_tgp_max_w)
    }
    fn run_entry(&mut self, entry: Option<&str>) -> Result<(), String> {
        crate::autostart::set_entry(entry)
    }
}
pub fn load() -> Result<Snapshot, String> {
    Store(ProfileStore::user_data()?).read()
}
pub fn save(
    baseline: &Snapshot,
    options: &AppSettings,
    appearance: &Appearance,
    gpu_tgp_max_w: u32,
    autostart: bool,
) -> Result<Snapshot, String> {
    commit(
        &mut Store(ProfileStore::user_data()?),
        baseline,
        options,
        appearance,
        gpu_tgp_max_w,
        autostart,
        &std::env::current_exe().map_err(|e| e.to_string())?,
    )
}
pub fn validate_gpu_tgp_max_w(max_w: u32) -> Result<(), String> {
    use omen_core::profiles::{GPU_DYNAMIC_BOOST_LIMIT_HARD_MAX_W, GPU_DYNAMIC_BOOST_LIMIT_MIN_W};
    if !(GPU_DYNAMIC_BOOST_LIMIT_MIN_W..=GPU_DYNAMIC_BOOST_LIMIT_HARD_MAX_W).contains(&max_w) {
        return Err(msg(
            "settings.invalid_gpu_tgp_max",
            &[
                &GPU_DYNAMIC_BOOST_LIMIT_MIN_W,
                &GPU_DYNAMIC_BOOST_LIMIT_HARD_MAX_W,
            ],
        ));
    }
    Ok(())
}
fn commit(
    backend: &mut impl Backend,
    baseline: &Snapshot,
    options: &AppSettings,
    appearance: &Appearance,
    gpu_tgp_max_w: u32,
    autostart: bool,
    executable: &Path,
) -> Result<Snapshot, String> {
    options.validate()?;
    appearance.validate()?;
    validate_gpu_tgp_max_w(gpu_tgp_max_w)?;
    if backend.read()? != *baseline {
        return Err(msg("settings.changed_elsewhere", &[]));
    }
    let run_entry = if !autostart {
        None
    } else if baseline.run_entry.is_some()
        && baseline.options.start_in_tray == options.start_in_tray
    {
        // A language/close edit must not redirect an existing startup path.
        baseline.run_entry.clone()
    } else {
        Some(crate::autostart::command(executable, options.start_in_tray))
    };
    let target = Snapshot {
        options: options.clone(),
        appearance: appearance.clone(),
        gpu_tgp_max_w,
        run_entry,
    };
    let changed_run = target.run_entry != baseline.run_entry;
    if changed_run {
        backend.run_entry(target.run_entry.as_deref())?;
    }
    if let Err(error) = backend.options(options, appearance, gpu_tgp_max_w) {
        if changed_run {
            let current = backend.read()?;
            if current.run_entry != target.run_entry {
                return Err(msg(
                    "settings.rollback_failed",
                    &[&error, &msg("settings.changed_elsewhere", &[])],
                ));
            }
            if let Err(rollback) = backend.run_entry(baseline.run_entry.as_deref()) {
                return Err(msg("settings.rollback_failed", &[&error, &rollback]));
            }
        }
        return Err(error);
    }
    Ok(target)
}

/// Each file is replaced atomically. Roll the cosmetic/TGP files back on a failed
/// options write; do not overwrite a concurrent editor. Older protected helpers
/// continue to read their unchanged app-settings schema and ignore sidecar files.
pub(crate) fn persist_options(
    store: &ProfileStore,
    options: &AppSettings,
    appearance: &Appearance,
    gpu_tgp_max_w: u32,
) -> Result<(), String> {
    options.validate()?;
    appearance.validate()?;
    validate_gpu_tgp_max_w(gpu_tgp_max_w)?;
    let before = store.ui_appearance()?;
    let before_tgp = store.gpu_tgp_max_w()?;
    let changed = before != *appearance;
    let tgp_changed = before_tgp != gpu_tgp_max_w;
    if changed {
        store.save_ui_appearance(appearance)?;
    }
    if tgp_changed {
        if let Err(error) = store.save_gpu_tgp_max_w(gpu_tgp_max_w) {
            if changed {
                let _ = store.save_ui_appearance(&before);
            }
            return Err(error);
        }
    }
    if let Err(error) = store.save_app_settings(options) {
        if tgp_changed {
            if store.gpu_tgp_max_w()? != gpu_tgp_max_w {
                return Err(msg(
                    "settings.rollback_failed",
                    &[&error, &msg("settings.changed_elsewhere", &[])],
                ));
            }
            if let Err(restore) = store.save_gpu_tgp_max_w(before_tgp) {
                return Err(msg("settings.rollback_failed", &[&error, &restore]));
            }
        }
        if changed {
            if store.ui_appearance()? != *appearance {
                return Err(msg(
                    "settings.rollback_failed",
                    &[&error, &msg("settings.changed_elsewhere", &[])],
                ));
            }
            if let Err(restore) = store.save_ui_appearance(&before) {
                return Err(msg("settings.rollback_failed", &[&error, &restore]));
            }
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_global_write_restores_cosmetic_file_without_touching_startup() {
        let root = std::env::temp_dir().join(format!(
            "omen-settings-rollback-{}",
            crate::native_ui::token()
        ));
        let store = ProfileStore::at(root.clone()).unwrap();
        let before = Appearance::default();
        store.save_ui_appearance(&before).unwrap();
        let appearance = Appearance {
            palette: omen_core::appearance::PalettePreset::Vesper,
            ..Default::default()
        };
        // This test owns the unique temp directory; a directory at the file
        // destination deterministically fails atomic replacement.
        std::fs::create_dir(root.join("app-settings.json")).unwrap();
        assert!(persist_options(&store, &AppSettings::default(), &appearance, 150).is_err());
        assert_eq!(store.ui_appearance().unwrap(), before);
        assert_eq!(store.gpu_tgp_max_w().unwrap(), 135);
        assert!(!root.join("startup.json").exists());
        std::fs::remove_dir(root.join("app-settings.json")).unwrap();
        std::fs::remove_file(root.join("ui-settings.json")).unwrap();
        let _ = std::fs::remove_file(root.join("gpu-tgp-settings.json"));
        std::fs::remove_dir(root.join("profiles")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn saved_preferences_survive_reopen_without_granting_preset_startup() {
        struct Files {
            store: ProfileStore,
            entry: Option<String>,
        }
        impl Backend for Files {
            fn read(&self) -> Result<Snapshot, String> {
                Ok(Snapshot {
                    options: self.store.app_settings()?,
                    appearance: self.store.ui_appearance()?,
                    gpu_tgp_max_w: self.store.gpu_tgp_max_w()?,
                    run_entry: self.entry.clone(),
                })
            }
            fn options(
                &mut self,
                options: &AppSettings,
                appearance: &Appearance,
                gpu_tgp_max_w: u32,
            ) -> Result<(), String> {
                persist_options(&self.store, options, appearance, gpu_tgp_max_w)
            }
            fn run_entry(&mut self, entry: Option<&str>) -> Result<(), String> {
                self.entry = entry.map(str::to_owned);
                Ok(())
            }
        }
        let root = std::env::temp_dir().join(format!(
            "omen-settings-reopen-{}",
            crate::native_ui::token()
        ));
        let store = ProfileStore::at(root.clone()).unwrap();
        let approval = store.startup().unwrap();
        let mut files = Files { store, entry: None };
        let baseline = files.read().unwrap();
        let options = AppSettings {
            language: omen_core::app_settings::LanguagePreference::English,
            start_in_tray: true,
            close_to_tray: false,
            battery_preset: Some("mode:quiet".into()),
            hold_preset: true,
            ..Default::default()
        };
        let appearance = omen_core::appearance::Appearance {
            palette: omen_core::appearance::PalettePreset::Custom,
            custom: omen_core::appearance::Colors {
                background: "#16191E".into(),
                foreground: "#ECEEF2".into(),
                accent: "#9CA8FF".into(),
            },
        };
        commit(
            &mut files,
            &baseline,
            &options,
            &appearance,
            150,
            true,
            Path::new("C:\\Omen\\app.exe"),
        )
        .unwrap();
        drop(files);
        let reopened = ProfileStore::at(root.clone()).unwrap();
        assert_eq!(reopened.app_settings().unwrap(), options);
        assert_eq!(reopened.ui_appearance().unwrap(), appearance);
        assert_eq!(reopened.gpu_tgp_max_w().unwrap(), 150);
        let legacy: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("app-settings.json")).unwrap())
                .unwrap();
        assert!(
            legacy.get("appearance").is_none() && legacy.get("gpu_tgp_max_w").is_none(),
            "Older protected helpers must still parse app-settings"
        );
        assert_eq!(
            serde_json::to_value(reopened.startup().unwrap()).unwrap(),
            serde_json::to_value(approval).unwrap()
        );
        // Only this test's uniquely created temporary directory is removed.
        std::fs::remove_file(root.join("app-settings.json")).unwrap();
        std::fs::remove_file(root.join("ui-settings.json")).unwrap();
        std::fs::remove_file(root.join("gpu-tgp-settings.json")).unwrap();
        std::fs::remove_dir(root.join("profiles")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    struct Fake {
        state: Snapshot,
        fail_save: bool,
        run_writes: usize,
    }
    impl Backend for Fake {
        fn read(&self) -> Result<Snapshot, String> {
            Ok(self.state.clone())
        }
        fn options(
            &mut self,
            value: &AppSettings,
            appearance: &Appearance,
            gpu_tgp_max_w: u32,
        ) -> Result<(), String> {
            if self.fail_save {
                return Err("disk failure".into());
            }
            self.state.options = value.clone();
            self.state.appearance = appearance.clone();
            self.state.gpu_tgp_max_w = gpu_tgp_max_w;
            Ok(())
        }
        fn run_entry(&mut self, value: Option<&str>) -> Result<(), String> {
            self.run_writes += 1;
            self.state.run_entry = value.map(str::to_owned);
            Ok(())
        }
    }
    #[test]
    fn language_change_keeps_the_existing_startup_path() {
        let baseline = Snapshot {
            options: Default::default(),
            appearance: Default::default(),
            gpu_tgp_max_w: 135,
            run_entry: Some(r#""D:\dist\Omen Hub++.exe" --tray"#.into()),
        };
        let mut backend = Fake {
            state: baseline.clone(),
            fail_save: false,
            run_writes: 0,
        };
        let mut options = baseline.options.clone();
        options.language = omen_core::app_settings::LanguagePreference::English;
        let saved = commit(
            &mut backend,
            &baseline,
            &options,
            &baseline.appearance,
            baseline.gpu_tgp_max_w,
            true,
            Path::new("test.exe"),
        )
        .unwrap();
        assert_eq!(saved.run_entry, baseline.run_entry);
        assert_eq!(backend.run_writes, 0);
        assert_eq!(backend.state.options, options);
    }
    #[test]
    fn failed_settings_save_restores_the_previous_run_entry() {
        let baseline = Snapshot {
            options: Default::default(),
            appearance: Default::default(),
            gpu_tgp_max_w: 135,
            run_entry: None,
        };
        let mut backend = Fake {
            state: baseline.clone(),
            fail_save: true,
            run_writes: 0,
        };
        assert!(
            commit(
                &mut backend,
                &baseline,
                &baseline.options,
                &baseline.appearance,
                baseline.gpu_tgp_max_w,
                true,
                Path::new("test.exe")
            )
            .is_err()
        );
        assert_eq!(backend.state, baseline);
    }
    #[test]
    fn stale_settings_window_cannot_overwrite_another_change() {
        let baseline = Snapshot {
            options: Default::default(),
            appearance: Default::default(),
            gpu_tgp_max_w: 135,
            run_entry: None,
        };
        let mut backend = Fake {
            state: baseline.clone(),
            fail_save: false,
            run_writes: 0,
        };
        backend.state.options.close_to_tray = false;
        assert!(
            commit(
                &mut backend,
                &baseline,
                &baseline.options,
                &baseline.appearance,
                baseline.gpu_tgp_max_w,
                true,
                Path::new("test.exe")
            )
            .is_err()
        );
        assert!(!backend.state.options.close_to_tray);
        assert_eq!(backend.run_writes, 0);
    }
}
