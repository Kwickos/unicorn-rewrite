//! Réglages, tenus côté natif : le raccourci doit connaître le profil actif
//! même quand le panneau est fermé et que macOS a suspendu la page.
//! Aucun texte reformulé n'y est jamais écrit.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::profiles::{ProfileId, CUSTOM_INSTRUCTION_MAX_CHARS};

pub const DEFAULT_SHORTCUT: &str = "Control+Alt+KeyR";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    Fr,
    En,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub profile: ProfileId,
    pub custom_instruction: String,
    /// Format `Modificateur+…+Code`, par exemple `Control+Alt+KeyR`.
    pub shortcut: String,
    pub theme: Theme,
    /// `None` jusqu'au premier lancement : la page la déduit du système, puis
    /// c'est un choix figé.
    pub locale: Option<Locale>,
    /// Écran d'accueil (permission, confidentialité) déjà vu.
    pub onboarded: bool,
    /// Modèle OpenRouter choisi (`None` : celui par défaut).
    pub model: Option<String>,
    /// Modèle remplacé par la dernière montée de version automatique.
    pub upgraded_from: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            profile: ProfileId::Correct,
            custom_instruction: String::new(),
            shortcut: DEFAULT_SHORTCUT.into(),
            theme: Theme::System,
            locale: None,
            onboarded: false,
            model: None,
            upgraded_from: None,
        }
    }
}

/// Modification partielle envoyée par le panneau. Le raccourci n'y figure
/// pas : il passe par `set_shortcut`, qui vérifie les conflits.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsPatch {
    pub profile: Option<ProfileId>,
    pub custom_instruction: Option<String>,
    pub theme: Option<Theme>,
    pub locale: Option<Locale>,
    pub onboarded: Option<bool>,
}

impl Settings {
    pub fn apply(&mut self, patch: SettingsPatch) {
        if let Some(profile) = patch.profile {
            self.profile = profile;
        }
        if let Some(custom) = patch.custom_instruction {
            self.custom_instruction = custom.chars().take(CUSTOM_INSTRUCTION_MAX_CHARS).collect();
        }
        if let Some(theme) = patch.theme {
            self.theme = theme;
        }
        if let Some(locale) = patch.locale {
            self.locale = Some(locale);
        }
        if let Some(onboarded) = patch.onboarded {
            self.onboarded = onboarded;
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Lecture tolérante : un fichier corrompu ne doit jamais empêcher l'app
    /// de démarrer.
    pub fn load(path: PathBuf) -> Self {
        let current = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        Self { path, current: Mutex::new(current) }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().map(|settings| settings.clone()).unwrap_or_default()
    }

    pub fn update(&self, change: impl FnOnce(&mut Settings)) -> Settings {
        let Ok(mut settings) = self.current.lock() else {
            return Settings::default();
        };
        change(&mut settings);
        self.save(&settings);
        settings.clone()
    }

    fn save(&self, settings: &Settings) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        // Écriture atomique : fichier temporaire puis renommage.
        let temporary = self.path.with_extension("json.tmp");
        if let Ok(json) = serde_json::to_string_pretty(settings) {
            if fs::write(&temporary, json).is_ok() {
                let _ = fs::rename(&temporary, &self.path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_or_missing_fields_fall_back_to_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"profile":"warm","extra":1}"#).unwrap();
        assert_eq!(settings.profile, ProfileId::Warm);
        assert_eq!(settings.shortcut, DEFAULT_SHORTCUT);
    }

    #[test]
    fn patch_bounds_the_custom_instruction() {
        let mut settings = Settings::default();
        settings.apply(SettingsPatch {
            custom_instruction: Some("é".repeat(500)),
            ..Default::default()
        });
        assert_eq!(settings.custom_instruction.chars().count(), CUSTOM_INSTRUCTION_MAX_CHARS);
    }

    #[test]
    fn corrupt_file_gives_defaults() {
        let dir = std::env::temp_dir().join(format!("unicorn-rewrite-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, "{ pas du json").unwrap();
        let store = SettingsStore::load(path.clone());
        assert_eq!(store.get(), Settings::default());
        store.update(|settings| settings.profile = ProfileId::Concise);
        assert_eq!(SettingsStore::load(path).get().profile, ProfileId::Concise);
        let _ = fs::remove_dir_all(dir);
    }
}
