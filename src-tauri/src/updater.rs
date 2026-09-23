//! Mises à jour automatiques depuis les releases GitHub.
//!
//! Vérification au lancement puis toutes les six heures ; la nouvelle version
//! est téléchargée, vérifiée (signature minisign) et installée en arrière-plan.
//! L'app redémarre ensuite d'elle-même dès qu'elle est inactive (aucune
//! reformulation en cours, panneau fermé) : un redémarrage d'une seconde,
//! invisible pour une app de barre de menu. Panneau ouvert, un bouton le
//! propose.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use tauri_plugin_updater::UpdaterExt;

const FIRST_CHECK: Duration = Duration::from_secs(15);
const INTERVAL: Duration = Duration::from_secs(6 * 3600);

/// Version installée, prête au prochain lancement.
#[derive(Default)]
pub struct PendingUpdate(pub Mutex<Option<String>>);

pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if app.state::<PendingUpdate>().0.lock().map(|pending| pending.is_none()).unwrap_or(false) {
                check(&app).await;
            }
            tokio::time::sleep(INTERVAL).await;
        }
    });
}

async fn check(app: &AppHandle) {
    let update = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(error) => {
            log::warn!("mise à jour : configuration invalide ({error})");
            return;
        }
    };
    let update = match update {
        Ok(Some(update)) => update,
        Ok(None) => return,
        Err(error) => {
            log::info!("mise à jour : vérification impossible ({error})");
            return;
        }
    };
    log::info!("mise à jour : {} disponible, téléchargement", update.version);
    match update.download_and_install(|_, _| {}, || {}).await {
        Ok(()) => {
            log::info!("mise à jour : {} installée, active au redémarrage", update.version);
            if let Ok(mut pending) = app.state::<PendingUpdate>().0.lock() {
                *pending = Some(update.version.clone());
            }
            let _ = app.emit("panel-shown", ());
            restart_when_idle(app).await;
        }
        Err(error) => log::warn!("mise à jour : installation impossible ({error})"),
    }
}

/// Redémarre dès que personne n'utilise l'app : ni reformulation en cours,
/// ni panneau ouvert.
async fn restart_when_idle(app: &AppHandle) {
    loop {
        let busy = app.state::<AppState>().engine.is_busy();
        let panel_open = app
            .get_webview_window("main")
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false);
        if !busy && !panel_open {
            log::info!("mise à jour : redémarrage");
            app.restart();
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
