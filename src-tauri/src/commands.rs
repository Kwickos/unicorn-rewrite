//! Commandes appelées par le panneau. Aucune ne renvoie la clé API, et aucune
//! ne journalise de texte.

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::ai::{self, openrouter::{self, CatalogModel}, AiError};
use crate::engine::{ErrorCode, Status};
use crate::macos::ax;
use crate::settings::{Settings, SettingsPatch};
use crate::shortcut::{self, ShortcutProblem};
use crate::{secrets, tray, AppState};

/// Tout ce que le panneau affiche, en un appel.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    settings: Settings,
    has_key: bool,
    trusted: bool,
    model: String,
    /// La clé permet de choisir le modèle (OpenRouter).
    model_choice: bool,
    /// Fournisseur simulé actif (développement).
    mock: bool,
    /// Nouvelle version installée, active au redémarrage.
    update_ready: Option<String>,
    version: String,
}

#[tauri::command]
pub fn get_state(app: AppHandle, state: State<'_, AppState>, pending: State<'_, crate::updater::PendingUpdate>) -> Snapshot {
    Snapshot {
        settings: state.settings.get(),
        has_key: secrets::has_api_key(),
        trusted: ax::is_trusted(false),
        model: state.router.model(secrets::api_key().as_deref()),
        model_choice: secrets::api_key().as_deref().and_then(ai::detect) == Some(ai::ProviderKind::OpenRouter),
        mock: state.mock.is_some(),
        update_ready: pending.0.lock().ok().and_then(|pending| pending.clone()),
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> Status {
    state.engine.status()
}

#[tauri::command]
pub fn update_settings(state: State<'_, AppState>, patch: SettingsPatch) -> Settings {
    state.settings.update(|settings| settings.apply(patch))
}

/// Libellé de l'infobulle de l'icône, avec le raccourci actif.
pub fn refresh_tooltip(app: &AppHandle) {
    use tauri::Manager;
    let spec = app.state::<AppState>().settings.get().shortcut;
    tray::set_tooltip(app, &format!("Unicorn Rewrite — {}", shortcut_label(&spec)));
}

fn shortcut_label(spec: &str) -> String {
    spec.split('+')
        .map(|token| match token {
            "Control" => "⌃".to_string(),
            "Alt" => "⌥".to_string(),
            "Shift" => "⇧".to_string(),
            "Super" => "⌘".to_string(),
            key => key.trim_start_matches("Key").trim_start_matches("Digit").to_string(),
        })
        .collect()
}

#[tauri::command]
pub fn set_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    spec: String,
) -> Result<Settings, ShortcutProblem> {
    shortcut::register(&app, &spec)?;
    let settings = state.settings.update(|settings| settings.shortcut = spec);
    refresh_tooltip(&app);
    Ok(settings)
}

#[tauri::command]
pub fn pause_shortcut(app: AppHandle, state: State<'_, AppState>, paused: bool) {
    shortcut::pause(&app, paused, &state.settings.get().shortcut);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyCheck {
    /// Clé enregistrée et acceptée par le fournisseur.
    Valid,
    /// Clé enregistrée, mais le fournisseur n'a pas pu être joint.
    Unverified,
}

/// Enregistre une clé après l'avoir fait vérifier. Une clé refusée n'est pas
/// enregistrée ; une clé qu'on n'a pas pu vérifier (réseau) l'est, et c'est dit.
#[tauri::command]
pub async fn save_api_key(state: State<'_, AppState>, key: String) -> Result<KeyCheck, ErrorCode> {
    let key = key.trim().to_owned();
    if key.is_empty() || key.chars().any(char::is_whitespace) || key.len() > 200 {
        return Err(ErrorCode::InvalidKey);
    }
    let outcome = state.router.verify_key(&key).await;
    match outcome {
        Ok(()) | Err(AiError::Network | AiError::Timeout | AiError::Unavailable) => {
            secrets::set_api_key(&key).map_err(|_| ErrorCode::Provider)?;
            Ok(if outcome.is_ok() { KeyCheck::Valid } else { KeyCheck::Unverified })
        }
        Err(error) => Err(error.into()),
    }
}

#[tauri::command]
pub async fn check_api_key(state: State<'_, AppState>) -> Result<KeyCheck, ErrorCode> {
    let key = secrets::api_key().ok_or(ErrorCode::MissingKey)?;
    state.router.verify_key(&key).await.map(|()| KeyCheck::Valid).map_err(ErrorCode::from)
}

#[tauri::command]
pub fn delete_api_key() -> Result<(), ErrorCode> {
    secrets::delete_api_key().map_err(|_| ErrorCode::Provider)
}

const ACCESSIBILITY_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/// Déclenche l'invitation de macOS (qui ajoute l'app à la liste) et ouvre le
/// bon volet des Réglages Système. Renvoie l'état réel.
#[tauri::command]
pub fn request_accessibility() -> bool {
    let trusted = ax::is_trusted(true);
    if !trusted {
        open_accessibility_settings();
    }
    trusted
}

#[tauri::command]
pub fn open_accessibility_settings() {
    let _ = std::process::Command::new("open").arg(ACCESSIBILITY_PANE).spawn();
}

/// Page des clés Cerebras. Adresse fixe : la page ne
/// peut pas faire ouvrir une URL de son choix.
#[tauri::command]
pub fn open_key_page() {
    let _ = std::process::Command::new("open").arg("https://cloud.cerebras.ai/platform").spawn();
}

#[tauri::command]
pub fn cancel_operation(state: State<'_, AppState>) -> bool {
    state.engine.cancel()
}

#[tauri::command]
pub async fn restore_last(state: State<'_, AppState>) -> Result<bool, ()> {
    Ok(state.engine.clone().restore().await)
}

#[tauri::command]
pub fn copy_text(state: State<'_, AppState>, original: bool) -> bool {
    state.engine.copy(original)
}

#[tauri::command]
pub fn dismiss_result(state: State<'_, AppState>) {
    state.engine.dismiss();
}

/// Quitter depuis le panneau : sans menu applicatif (l'app est en mode
/// accessoire), c'est la seule sortie possible.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_label_uses_mac_symbols() {
        assert_eq!(shortcut_label("Control+Alt+KeyR"), "⌃⌥R");
        assert_eq!(shortcut_label("Alt+Super+Digit5"), "⌥⌘5");
    }
}

/// Catalogue OpenRouter (modèles récents, prix, vitesse mesurée).
#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<CatalogModel>, ErrorCode> {
    let key = secrets::api_key().ok_or(ErrorCode::MissingKey)?;
    state.router.openrouter.catalog(&key).await.map_err(ErrorCode::from)
}

#[tauri::command]
pub fn set_model(state: State<'_, AppState>, model: String) -> Settings {
    state.router.set_openrouter_model(&model);
    state.settings.update(|settings| {
        settings.model = Some(model);
        settings.upgraded_from = None;
    })
}

/// Montée de version dans la même famille, ou remplacement d'un modèle
/// retiré. Jamais vers un autre type de modèle.
pub async fn check_model_upgrade(app: &AppHandle) {
    use tauri::{Emitter, Manager};
    let state = app.state::<AppState>();
    let Some(key) = secrets::api_key().filter(|key| ai::detect(key) == Some(ai::ProviderKind::OpenRouter)) else {
        return;
    };
    let Ok(catalog) = state.router.openrouter.catalog(&key).await else { return };
    let current = state.router.openrouter_model();
    if let Some(next) = openrouter::successor(&current, &catalog) {
        log::info!("modèle mis à jour : {current} → {}", next.id);
        state.router.set_openrouter_model(&next.id);
        let next_id = next.id.clone();
        state.settings.update(|settings| {
            settings.model = Some(next_id);
            settings.upgraded_from = Some(current);
        });
        let _ = app.emit("panel-shown", ());
    }
}

/// Redémarre sur la version installée. Refusé pendant une reformulation.
#[tauri::command]
pub fn restart_app(app: AppHandle, state: State<'_, AppState>) {
    if !state.engine.is_busy() {
        app.restart();
    }
}
