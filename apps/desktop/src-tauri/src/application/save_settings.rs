use crate::domain::settings::{prepare_settings_snapshot, AppSettingsSnapshot, CredentialUpdate};

pub trait SettingsPort {
    fn load(&self) -> Result<AppSettingsSnapshot, String>;
    fn save(&self, snapshot: &AppSettingsSnapshot) -> Result<AppSettingsSnapshot, String>;
}

pub fn save_settings(
    port: &impl SettingsPort,
    incoming: AppSettingsSnapshot,
    credential: CredentialUpdate,
) -> Result<AppSettingsSnapshot, String> {
    let stored = port.load()?;
    let prepared = prepare_settings_snapshot(incoming, credential, &stored)?;
    port.save(&prepared)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use crate::domain::settings::AppSettings;

    use super::*;

    struct FakeSettingsPort {
        current: RefCell<AppSettingsSnapshot>,
    }

    impl SettingsPort for FakeSettingsPort {
        fn load(&self) -> Result<AppSettingsSnapshot, String> {
            Ok(self.current.borrow().clone())
        }

        fn save(&self, snapshot: &AppSettingsSnapshot) -> Result<AppSettingsSnapshot, String> {
            let current_revision = self.current.borrow().settings_revision;
            if snapshot.settings_revision != current_revision {
                return Err("settings_conflict".into());
            }
            let mut saved = snapshot.clone();
            saved.settings_revision = current_revision.map(|revision| revision + 1);
            self.current.replace(saved.clone());
            Ok(saved)
        }
    }

    #[test]
    fn saves_without_webview_and_preserves_secret() {
        let stored = AppSettings {
            api_token: "stored-secret".into(),
            ..AppSettings::default()
        };
        let port = FakeSettingsPort {
            current: RefCell::new(AppSettingsSnapshot {
                settings: stored,
                settings_revision: Some(2),
            }),
        };
        let incoming = AppSettingsSnapshot {
            settings: AppSettings::default(),
            settings_revision: Some(2),
        };

        let saved = save_settings(&port, incoming, CredentialUpdate::Keep).expect("save");

        assert_eq!(saved.settings_revision, Some(3));
        assert_eq!(saved.settings.api_token, "stored-secret");
    }

    #[test]
    fn exposes_revision_conflict_from_port() {
        let port = FakeSettingsPort {
            current: RefCell::new(AppSettingsSnapshot {
                settings: AppSettings::default(),
                settings_revision: Some(3),
            }),
        };
        let incoming = AppSettingsSnapshot {
            settings: AppSettings::default(),
            settings_revision: Some(2),
        };

        assert!(matches!(
            save_settings(&port, incoming, CredentialUpdate::Keep),
            Err(message) if message == "settings_conflict"
        ));
    }
}
