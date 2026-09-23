//! Le raccourci global, et la détection des conflits.
//!
//! macOS n'indique pas quand un raccourci est déjà pris par une autre app
//! (`RegisterEventHotKey` accepte les doublons) : on écarte donc nous-mêmes
//! les combinaisons qui tapent un caractère, qui volent un raccourci d'édition
//! universel, ou que le système utilise (lu dans les réglages Clavier).
//!
//! Attention : le plugin tient un verrou pendant qu'il appelle un handler.
//! Un handler ne doit donc jamais (dés)inscrire de raccourci directement ;
//! tout passe par une tâche différée.

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutProblem {
    /// Illisible.
    Invalid,
    /// Une touche seule, ou avec ⇧ : elle servirait à écrire.
    NeedsModifier,
    /// ⌥ ou ⌥⇧ + touche : tape un caractère spécial (ø, ®…).
    TypesCharacter,
    /// ⌘ ou ⌘⇧ + touche : raccourci standard des apps (copier, enregistrer…).
    AppShortcut,
    /// Déjà utilisé par macOS.
    System,
    /// Refusé à l'enregistrement.
    Unavailable,
}

/// Raccourci actif, pour pouvoir le retirer ou le remettre, et état voulu
/// pour Échap.
#[derive(Default)]
pub struct Registered {
    current: Mutex<Option<Shortcut>>,
    cancel_wanted: AtomicBool,
}

fn escape() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

pub fn parse(spec: &str) -> Result<Shortcut, ShortcutProblem> {
    Shortcut::from_str(spec).map_err(|_| ShortcutProblem::Invalid)
}

/// Code de touche macOS (`kVK_*`), pour comparer aux raccourcis du système.
fn mac_keycode(code: Code) -> Option<u16> {
    use Code::*;
    Some(match code {
        KeyA => 0, KeyS => 1, KeyD => 2, KeyF => 3, KeyH => 4, KeyG => 5, KeyZ => 6,
        KeyX => 7, KeyC => 8, KeyV => 9, KeyB => 11, KeyQ => 12, KeyW => 13, KeyE => 14,
        KeyR => 15, KeyY => 16, KeyT => 17, Digit1 => 18, Digit2 => 19, Digit3 => 20,
        Digit4 => 21, Digit6 => 22, Digit5 => 23, Equal => 24, Digit9 => 25, Digit7 => 26,
        Minus => 27, Digit8 => 28, Digit0 => 29, BracketRight => 30, KeyO => 31, KeyU => 32,
        BracketLeft => 33, KeyI => 34, KeyP => 35, Enter => 36, KeyL => 37, KeyJ => 38,
        Quote => 39, KeyK => 40, Semicolon => 41, Backslash => 42, Comma => 43, Slash => 44,
        KeyN => 45, KeyM => 46, Period => 47, Tab => 48, Space => 49, Backquote => 50,
        Backspace => 51, Escape => 53, F1 => 122, F2 => 120, F3 => 99, F4 => 118, F5 => 96,
        F6 => 97, F7 => 98, F8 => 100, F9 => 101, F10 => 109, F11 => 103, F12 => 111,
        ArrowLeft => 123, ArrowRight => 124, ArrowDown => 125, ArrowUp => 126,
        _ => return None,
    })
}

const SHIFT: u64 = 0x2_0000;
const CONTROL: u64 = 0x4_0000;
const OPTION: u64 = 0x8_0000;
const COMMAND: u64 = 0x10_0000;

fn mac_modifiers(mods: Modifiers) -> u64 {
    let mut flags = 0;
    if mods.contains(Modifiers::SHIFT) {
        flags |= SHIFT;
    }
    if mods.contains(Modifiers::CONTROL) {
        flags |= CONTROL;
    }
    if mods.contains(Modifiers::ALT) {
        flags |= OPTION;
    }
    if mods.contains(Modifiers::SUPER) {
        flags |= COMMAND;
    }
    flags
}

/// Raccourcis système actifs par défaut : (identifiant, touche, modificateurs).
/// Les réglages de l'utilisateur, lus ensuite, les désactivent ou les
/// remplacent.
const SYSTEM_DEFAULTS: &[(&str, u16, u64)] = &[
    ("64", 49, COMMAND),                    // Spotlight
    ("65", 49, COMMAND | OPTION),           // Recherche du Finder
    ("60", 49, CONTROL),                    // Source de saisie précédente
    ("61", 49, CONTROL | OPTION),           // Source de saisie suivante
    ("28", 20, COMMAND | SHIFT),            // Capture d'écran
    ("29", 20, COMMAND | SHIFT | CONTROL),
    ("30", 21, COMMAND | SHIFT),
    ("31", 21, COMMAND | SHIFT | CONTROL),
    ("184", 23, COMMAND | SHIFT),
    ("32", 126, CONTROL),                   // Mission Control
    ("33", 125, CONTROL),                   // Fenêtres de l'app
    ("79", 123, CONTROL),                   // Bureau précédent
    ("81", 124, CONTROL),                   // Bureau suivant
    ("36", 103, 0),                         // Afficher le bureau
    ("52", 2, COMMAND | OPTION),            // Masquer le Dock
    ("lock", 12, COMMAND | CONTROL),        // Verrouiller l'écran
    ("emoji", 49, COMMAND | CONTROL),       // Emoji et symboles
    ("force-quit", 53, COMMAND | OPTION),   // Forcer à quitter
    ("app-switcher", 48, COMMAND),
    ("window-cycle", 50, COMMAND),
];

/// Raccourcis système, d'après `com.apple.symbolichotkeys`.
fn system_hotkeys() -> Vec<(u16, u64)> {
    let mut entries: std::collections::HashMap<String, Option<(u16, u64)>> = SYSTEM_DEFAULTS
        .iter()
        .map(|(id, key, mods)| ((*id).to_string(), Some((*key, *mods))))
        .collect();

    let path = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|home| home.join("Library/Preferences/com.apple.symbolichotkeys.plist"));
    let hotkeys = path
        .and_then(|path| plist::Value::from_file(path).ok())
        .and_then(|value| value.into_dictionary())
        .and_then(|mut root| root.remove("AppleSymbolicHotKeys"))
        .and_then(|value| value.into_dictionary());
    if let Some(hotkeys) = hotkeys {
        for (id, entry) in hotkeys {
            let Some(entry) = entry.as_dictionary() else { continue };
            let enabled = entry
                .get("enabled")
                .and_then(|value| value.as_boolean().or_else(|| value.as_signed_integer().map(|n| n != 0)))
                .unwrap_or(false);
            let parameters = entry
                .get("value")
                .and_then(|value| value.as_dictionary())
                .and_then(|value| value.get("parameters"))
                .and_then(|value| value.as_array());
            let binding = parameters.and_then(|parameters| {
                let key = parameters.get(1)?.as_signed_integer()?;
                let mods = parameters.get(2)?.as_signed_integer()?;
                (key != 65535).then_some((key as u16, mods as u64))
            });
            entries.insert(id, if enabled { binding } else { None });
        }
    }
    entries.into_values().flatten().collect()
}

const CHARACTER_KEYS: &[Code] = &[
    Code::Backquote, Code::Backslash, Code::BracketLeft, Code::BracketRight, Code::Comma,
    Code::Equal, Code::Minus, Code::Period, Code::Quote, Code::Semicolon, Code::Slash,
    Code::Space,
];

fn is_character_key(code: Code) -> bool {
    let name = format!("{code:?}");
    name.starts_with("Key") || name.starts_with("Digit") || CHARACTER_KEYS.contains(&code)
}

fn is_function_key(code: Code) -> bool {
    let name = format!("{code:?}");
    name.len() > 1 && name.starts_with('F') && name[1..].chars().all(|c| c.is_ascii_digit())
}

/// Vérifie une combinaison, sans l'enregistrer.
pub fn check(shortcut: &Shortcut) -> Option<ShortcutProblem> {
    check_against(shortcut, &system_hotkeys())
}

fn check_against(shortcut: &Shortcut, system: &[(u16, u64)]) -> Option<ShortcutProblem> {
    let mods = shortcut.mods;
    let strong = Modifiers::SUPER | Modifiers::CONTROL | Modifiers::ALT;
    if shortcut.key == Code::Escape {
        return Some(ShortcutProblem::Unavailable);
    }
    if !mods.intersects(strong) {
        // F13 à F20 n'ont pas d'autre usage : seules, elles conviennent.
        let late_function_key = is_function_key(shortcut.key) && mac_keycode(shortcut.key).is_none();
        if !late_function_key {
            return Some(ShortcutProblem::NeedsModifier);
        }
    }
    if mods.contains(Modifiers::ALT)
        && !mods.intersects(Modifiers::SUPER | Modifiers::CONTROL)
        && is_character_key(shortcut.key)
    {
        return Some(ShortcutProblem::TypesCharacter);
    }
    if let Some(key) = mac_keycode(shortcut.key) {
        let flags = mac_modifiers(mods);
        let relevant = SHIFT | CONTROL | OPTION | COMMAND;
        if system.iter().any(|(k, m)| *k == key && (m & relevant) == flags) {
            return Some(ShortcutProblem::System);
        }
    }
    if mods.contains(Modifiers::SUPER)
        && !mods.intersects(Modifiers::CONTROL | Modifiers::ALT)
        && is_character_key(shortcut.key)
        && shortcut.key != Code::Space
    {
        return Some(ShortcutProblem::AppShortcut);
    }
    None
}

/// Inscrit le raccourci principal (et retire l'ancien). En cas d'échec,
/// l'ancien reste actif.
pub fn register(app: &AppHandle, spec: &str) -> Result<(), ShortcutProblem> {
    let shortcut = parse(spec)?;
    if let Some(problem) = check(&shortcut) {
        return Err(problem);
    }
    install(app, Some(shortcut))
}

/// Inscrit le raccourci enregistré au démarrage, sans le refuser pour un
/// conflit apparu depuis : l'utilisateur l'a choisi en connaissance de cause.
pub fn register_saved(app: &AppHandle, spec: &str) -> Result<(), ShortcutProblem> {
    install(app, Some(parse(spec)?))
}

fn install(app: &AppHandle, shortcut: Option<Shortcut>) -> Result<(), ShortcutProblem> {
    let state = app.state::<Registered>();
    let mut current = state.current.lock().map_err(|_| ShortcutProblem::Unavailable)?;
    let manager = app.global_shortcut();

    if let Some(previous) = *current {
        let _ = manager.unregister(previous);
    }
    let Some(shortcut) = shortcut else {
        *current = None;
        return Ok(());
    };
    let registered = manager.on_shortcut(shortcut, |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            crate::trigger_rewrite(app);
        }
    });
    match registered {
        Ok(()) => {
            *current = Some(shortcut);
            Ok(())
        }
        Err(error) => {
            log::warn!("raccourci refusé : {error}");
            if let Some(previous) = *current {
                let _ = manager.on_shortcut(previous, |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        crate::trigger_rewrite(app);
                    }
                });
            }
            Err(ShortcutProblem::Unavailable)
        }
    }
}

/// Suspend le raccourci pendant qu'on en enregistre un nouveau dans le
/// panneau : sinon l'appuyer lancerait une reformulation.
pub fn pause(app: &AppHandle, paused: bool, spec: &str) {
    if paused {
        let _ = install(app, None);
    } else {
        let _ = register_saved(app, spec);
    }
}

/// Échap annule une opération en cours. Il n'est capturé que le temps du
/// traitement, jamais en permanence.
pub fn set_cancel(app: &AppHandle, active: bool) {
    app.state::<Registered>().cancel_wanted.store(active, Ordering::SeqCst);
    let app = app.clone();
    // Tâche différée (voir l'en-tête). Elle applique l'état voulu le plus
    // récent : deux tâches exécutées dans le désordre ne laissent pas Échap
    // capturé par erreur.
    tauri::async_runtime::spawn(async move {
        let active = app.state::<Registered>().cancel_wanted.load(Ordering::SeqCst);
        let manager = app.global_shortcut();
        let registered = manager.is_registered(escape());
        if active && !registered {
            let _ = manager.on_shortcut(escape(), |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    crate::cancel_rewrite(app);
                }
            });
        } else if !active && registered {
            let _ = manager.unregister(escape());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem(spec: &str) -> Option<ShortcutProblem> {
        check_against(&parse(spec).unwrap(), &[(49, COMMAND), (21, COMMAND | SHIFT)])
    }

    #[test]
    fn default_shortcut_is_accepted() {
        assert_eq!(problem(crate::settings::DEFAULT_SHORTCUT), None);
        assert_eq!(problem("Control+Alt+Super+KeyR"), None);
        assert_eq!(problem("Alt+Super+KeyE"), None);
    }

    #[test]
    fn rejects_combinations_that_type_or_steal() {
        assert_eq!(problem("KeyR"), Some(ShortcutProblem::NeedsModifier));
        assert_eq!(problem("Shift+KeyR"), Some(ShortcutProblem::NeedsModifier));
        assert_eq!(problem("Alt+KeyR"), Some(ShortcutProblem::TypesCharacter));
        assert_eq!(problem("Alt+Shift+KeyR"), Some(ShortcutProblem::TypesCharacter));
        assert_eq!(problem("Super+KeyC"), Some(ShortcutProblem::AppShortcut));
        assert_eq!(problem("Super+Shift+KeyS"), Some(ShortcutProblem::AppShortcut));
        assert_eq!(problem("Escape"), Some(ShortcutProblem::Unavailable));
    }

    #[test]
    fn rejects_system_shortcuts() {
        assert_eq!(problem("Super+Space"), Some(ShortcutProblem::System));
        assert_eq!(problem("Super+Shift+Digit4"), Some(ShortcutProblem::System));
    }

    #[test]
    fn invalid_spec_is_reported() {
        assert_eq!(parse("Control+Alt+"), Err(ShortcutProblem::Invalid));
    }
}
