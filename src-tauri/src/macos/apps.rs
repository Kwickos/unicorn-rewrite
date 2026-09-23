//! Les autres apps : nom, identifiant, réactivation.

use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSApplicationActivationOptions, NSBeep, NSRunningApplication, NSWorkspace};

pub struct AppInfo {
    pub name: String,
}

pub fn info(pid: i32) -> AppInfo {
    autoreleasepool(|_| {
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid);
        AppInfo {
            name: app
                .as_ref()
                .and_then(|app| app.localizedName())
                .map(|name| name.to_string())
                .unwrap_or_default(),
        }
    })
}

/// App au premier plan selon AppKit, quand Accessibility ne répond pas.
pub fn frontmost_pid() -> Option<i32> {
    autoreleasepool(|_| {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|app| app.processIdentifier())
    })
}

/// Ramène une app au premier plan (restauration depuis le panneau).
pub fn activate(pid: i32) -> bool {
    autoreleasepool(|_| {
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .is_some_and(|app| app.activateWithOptions(NSApplicationActivationOptions::empty()))
    })
}

/// Le son d'alerte système : le seul retour d'une erreur quand le panneau
/// est fermé, en plus de l'icône.
pub fn beep() {
    NSBeep();
}
