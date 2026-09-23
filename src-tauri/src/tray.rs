//! L'icône de la barre de menu, seul retour visuel quand le panneau est fermé :
//! un stylo au repos, des points pendant le traitement, une coche ou une
//! alerte un court instant.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{image::Image, AppHandle};

use crate::engine::{Phase, Status};

pub const TRAY_ID: &str = "unicorn-rewrite";

const SUCCESS_VISIBLE: Duration = Duration::from_millis(1_600);
const ERROR_VISIBLE: Duration = Duration::from_millis(4_000);

/// Change à chaque mise à jour : un retour au repos programmé ne doit pas
/// effacer un état plus récent.
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Idle,
    Busy,
    Done,
    Error,
}

fn icon(look: Look) -> Option<Image<'static>> {
    let bytes: &'static [u8] = match look {
        Look::Idle => include_bytes!("../icons/tray@2x.png"),
        Look::Busy => include_bytes!("../icons/tray-busy@2x.png"),
        Look::Done => include_bytes!("../icons/tray-done@2x.png"),
        Look::Error => include_bytes!("../icons/tray-error@2x.png"),
    };
    Image::from_bytes(bytes).ok()
}

pub fn idle_icon() -> Option<Image<'static>> {
    icon(Look::Idle)
}

fn apply(app: &AppHandle, look: Look) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let _ = tray.set_icon(icon(look));
    let _ = tray.set_icon_as_template(true);
}

pub fn reflect(app: &AppHandle, status: &Status) {
    let look = match status.phase {
        Phase::Reading | Phase::Rewriting | Phase::Replacing => Look::Busy,
        Phase::Done | Phase::Unchanged | Phase::Restored => Look::Done,
        Phase::Error | Phase::Review => Look::Error,
        Phase::Idle | Phase::Cancelled => Look::Idle,
    };
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app_ = app.clone();
    let _ = app.run_on_main_thread(move || apply(&app_, look));

    let linger = match look {
        Look::Done => SUCCESS_VISIBLE,
        Look::Error => ERROR_VISIBLE,
        Look::Idle | Look::Busy => return,
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(linger).await;
        if GENERATION.load(Ordering::SeqCst) == generation {
            let app_ = app.clone();
            let _ = app.run_on_main_thread(move || apply(&app_, Look::Idle));
        }
    });
}

pub fn set_tooltip(app: &AppHandle, text: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}
