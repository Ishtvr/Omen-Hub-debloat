//! Application preferences and the "Start with Windows" logon task. No hardware/startup
//! approval API is reachable through this module.
use omen_core::{app_settings::AppSettings, message::msg};
use omen_platform::profile_store::ProfileStore;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub options: AppSettings,
    pub run_entry: Option<String>,
}
trait Backend {
    fn read(&self) -> Result<Snapshot, String>;
    fn options(&mut self, options: &AppSettings) -> Result<(), String>;
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
            run_entry,
        })
    }
    fn options(&mut self, options: &AppSettings) -> Result<(), String> {
        self.0.save_app_settings(options)
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
    autostart: bool,
) -> Result<Snapshot, String> {
    commit(
        &mut Store(ProfileStore::user_data()?),
        baseline,
        options,
        autostart,
        &std::env::current_exe().map_err(|e| e.to_string())?,
    )
}
fn commit(
    backend: &mut impl Backend,
    baseline: &Snapshot,
    options: &AppSettings,
    autostart: bool,
    executable: &Path,
) -> Result<Snapshot, String> {
    options.validate()?;
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
        run_entry,
    };
    let changed_run = target.run_entry != baseline.run_entry;
    if changed_run {
        backend.run_entry(target.run_entry.as_deref())?;
    }
    if let Err(error) = backend.options(options) {
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

#[cfg(test)]
mod tests {
    use super::*;
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
                    run_entry: self.entry.clone(),
                })
            }
            fn options(&mut self, options: &AppSettings) -> Result<(), String> {
                self.store.save_app_settings(options)
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
        commit(
            &mut files,
            &baseline,
            &options,
            true,
            Path::new("C:\\Omen\\app.exe"),
        )
        .unwrap();
        drop(files);
        let reopened = ProfileStore::at(root.clone()).unwrap();
        assert_eq!(reopened.app_settings().unwrap(), options);
        assert_eq!(
            serde_json::to_value(reopened.startup().unwrap()).unwrap(),
            serde_json::to_value(approval).unwrap()
        );
        // Only this test's uniquely created temporary directory is removed.
        std::fs::remove_file(root.join("app-settings.json")).unwrap();
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
        fn options(&mut self, value: &AppSettings) -> Result<(), String> {
            if self.fail_save {
                return Err("disk failure".into());
            }
            self.state.options = value.clone();
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
                true,
                Path::new("test.exe")
            )
            .is_err()
        );
        assert!(!backend.state.options.close_to_tray);
        assert_eq!(backend.run_writes, 0);
    }
}
