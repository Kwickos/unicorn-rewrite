//! ⌘C et ⌘V synthétiques, pour le repli copier/coller.

use std::thread;
use std::time::{Duration, Instant};

use super::ffi::*;

const MODIFIERS: CGEventFlags = kCGEventFlagMaskShift
    | kCGEventFlagMaskControl
    | kCGEventFlagMaskAlternate
    | kCGEventFlagMaskCommand;

/// Modificateurs physiquement enfoncés en ce moment.
fn held_modifiers() -> CGEventFlags {
    // SAFETY : fonction sans précondition.
    unsafe { CGEventSourceFlagsState(kCGEventSourceStateCombinedSessionState) & MODIFIERS }
}

/// Attend que l'utilisateur relâche les touches du raccourci : un ⌥ encore
/// enfoncé transformerait notre ⌘C en ⌥⌘C dans certaines apps.
/// Renvoie faux si le délai expire.
pub fn wait_for_modifiers_release(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while held_modifiers() != 0 {
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
    true
}

fn post_command(key: CGKeyCode) {
    // SAFETY : événements créés puis libérés ici ; les drapeaux posés
    // explicitement remplacent ceux du clavier physique.
    unsafe {
        let source = CGEventSourceCreate(kCGEventSourceStateHIDSystemState);
        for down in [true, false] {
            let event = CGEventCreateKeyboardEvent(source, key, down);
            if event.is_null() {
                continue;
            }
            CGEventSetFlags(event, kCGEventFlagMaskCommand);
            CGEventPost(kCGHIDEventTap, event);
            CFRelease(event as _);
        }
        if !source.is_null() {
            CFRelease(source as _);
        }
    }
}

pub fn copy() {
    post_command(kVK_ANSI_C);
}

pub fn paste() {
    post_command(kVK_ANSI_V);
}
