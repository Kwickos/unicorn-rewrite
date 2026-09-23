use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{
    tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};
use tauri_plugin_positioner::{Position, WindowExt};

mod ai;
mod commands;
mod engine;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod panel;
mod platform;
mod profiles;
mod secrets;
mod settings;
mod shortcut;
mod tray;
mod updater;

use ai::{mock::Mock, Rewriter, Router};
use engine::{Engine, Hooks, Request, Status};
use settings::SettingsStore;

const WINDOW_ID: &str = "main";

#[cfg(target_os = "macos")]
pub type AppEngine = Engine<macos::MacPlatform>;

pub struct AppState {
    pub engine: Arc<AppEngine>,
    pub settings: SettingsStore,
    pub router: Arc<Router>,
    /// Fournisseur simulé, en développement seulement.
    pub mock: Option<Arc<Mock>>,
}

/// Le clic sur l'icône fait d'abord perdre le focus à la fenêtre, ce qui la
/// masque, avant d'arriver ici : sans ce garde-fou le panneau se rouvrirait
/// aussitôt et on ne pourrait jamais le refermer depuis la barre de menu.
#[derive(Default)]
struct LastAutoHide(Mutex<Option<Instant>>);

const REOPEN_GUARD: Duration = Duration::from_millis(250);

/// En développement, `UNICORN_REWRITE_KEEP_OPEN=1` empêche le panneau de se
/// refermer dès qu'il perd le focus — indispensable pour l'inspecter.
fn keep_open() -> bool {
    cfg!(debug_assertions) && std::env::var("UNICORN_REWRITE_KEEP_OPEN").is_ok()
}

pub(crate) fn show_panel(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WINDOW_ID) else {
        return;
    };
    let _ = window.move_window(Position::TrayCenter);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit("panel-shown", ());
}

pub(crate) fn hide_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW_ID) {
        let _ = window.hide();
    }
}

fn toggle_panel(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WINDOW_ID) else {
        return;
    };
    let hidden_just_now = app
        .state::<LastAutoHide>()
        .0
        .lock()
        .ok()
        .and_then(|guard| *guard)
        .is_some_and(|at| at.elapsed() < REOPEN_GUARD);

    if window.is_visible().unwrap_or(false) || hidden_just_now {
        let _ = window.hide();
        return;
    }
    show_panel(app);
}

/// Le raccourci : lit le profil actif et lance l'opération, sans jamais
/// ouvrir le panneau ni prendre le focus.
pub(crate) fn trigger_rewrite(app: &AppHandle) {
    log::info!("raccourci reçu");
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let request = Request { profile: settings.profile, custom: settings.custom_instruction };
    let engine = state.engine.clone();
    tauri::async_runtime::spawn(async move {
        engine.run(request).await;
    });
}

pub(crate) fn cancel_rewrite(app: &AppHandle) {
    let engine = app.state::<AppState>().engine.clone();
    tauri::async_runtime::spawn(async move {
        engine.cancel();
    });
}

/// Le moteur parle au reste de l'app par ici.
struct AppHooks(AppHandle);

impl Hooks for AppHooks {
    fn status_changed(&self, status: &Status) {
        let _ = self.0.emit("status", status);
        tray::reflect(&self.0, status);
    }

    fn show_panel(&self) {
        let app = self.0.clone();
        let _ = self.0.run_on_main_thread(move || show_panel(&app));
    }

    fn hide_panel(&self) {
        let app = self.0.clone();
        let _ = self.0.run_on_main_thread(move || hide_panel(&app));
    }

    fn cancel_shortcut(&self, active: bool) {
        shortcut::set_cancel(&self.0, active);
    }

    fn alert(&self) {
        #[cfg(target_os = "macos")]
        {
            let _ = self.0.run_on_main_thread(macos::beep);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Une seule instance : deux copies se disputeraient le raccourci.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_panel(app);
        }))
        .manage(LastAutoHide::default())
        .manage(shortcut::Registered::default())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::PendingUpdate::default())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::get_status,
            commands::update_settings,
            commands::set_shortcut,
            commands::pause_shortcut,
            commands::save_api_key,
            commands::check_api_key,
            commands::delete_api_key,
            commands::request_accessibility,
            commands::open_accessibility_settings,
            commands::open_key_page,
            commands::cancel_operation,
            commands::restore_last,
            commands::copy_text,
            commands::dismiss_result,
            commands::quit_app,
            commands::list_models,
            commands::set_model,
            commands::restart_app,
            panel::resize_panel,
        ])
        .setup(|app| {
            // Journal de diagnostic, aussi en release : étapes, durées et codes
            // d'erreur seulement — jamais de texte ni de clé.
            // ~/Library/Logs/com.digitalunicorn.unicorn-rewrite/diagnostic.log
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .level_for("reqwest", log::LevelFilter::Warn)
                    .level_for("hyper_util", log::LevelFilter::Warn)
                    .max_file_size(512 * 1024)
                    .targets([
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                            file_name: Some("diagnostic".into()),
                        }),
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    ])
                    .build(),
            )?;

            // Lancement à l'ouverture de session, activable dans les réglages.
            // `LaunchAgent` : le mécanisme que macOS accepte sans signature.
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))?;

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window(WINDOW_ID) {
                panel::install_backdrop(&window)?;
            }

            let settings = SettingsStore::load(app.path().app_config_dir()?.join("settings.json"));
            let router = Arc::new(Router::new());
            let mock = Mock::enabled_by_env().then(|| Arc::new(Mock::new(Duration::from_millis(1_200))));
            let rewriter: Arc<dyn Rewriter> = match &mock {
                Some(mock) => mock.clone(),
                None => router.clone(),
            };
            let engine = Engine::new(
                macos::MacPlatform,
                rewriter,
                Arc::new(AppHooks(app.handle().clone())),
            );
            let shortcut_spec = settings.get().shortcut;
            if let Some(model) = settings.get().model {
                router.set_openrouter_model(&model);
            }
            app.manage(AppState { engine, settings, router: router.clone(), mock });
            // Première connexion ouverte dès le lancement.
            tauri::async_runtime::spawn(async move { router.prepare().await });
            // Nouvelle version du modèle choisi : au lancement, puis deux fois
            // par jour.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    commands::check_model_upgrade(&handle).await;
                    tokio::time::sleep(Duration::from_secs(12 * 3600)).await;
                }
            });

            TrayIconBuilder::with_id(tray::TRAY_ID)
                .icon(tray::idle_icon().expect("icône de barre de menu"))
                .icon_as_template(true)
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    if let TrayIconEvent::Click { button_state: MouseButtonState::Up, .. } = event {
                        toggle_panel(tray.app_handle());
                    }
                })
                .build(app)?;

            if let Err(problem) = shortcut::register_saved(app.handle(), &shortcut_spec) {
                log::warn!("raccourci enregistré inutilisable : {problem:?}");
            }
            commands::refresh_tooltip(app.handle());

            // Premier lancement, ou permission retirée depuis : on explique
            // plutôt que d'attendre un raccourci qui échouerait.
            let onboarded = app.state::<AppState>().settings.get().onboarded;
            if !onboarded || !macos::ax::is_trusted(false) || keep_open() {
                show_panel(app.handle());
            }

            updater::start(app.handle());

            #[cfg(feature = "selftest")]
            macos::selftest::start_if_requested(app.handle());

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(false) = event {
                if keep_open() {
                    return;
                }
                let app = window.app_handle();
                if let Ok(mut guard) = app.state::<LastAutoHide>().0.lock() {
                    *guard = Some(Instant::now());
                }
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
