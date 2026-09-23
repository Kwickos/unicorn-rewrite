//! Intégration macOS : lire et remplacer la sélection d'une autre app.
//!
//! Deux voies, dans cet ordre :
//! 1. **Accessibility** : l'app expose le champ qui a le focus, sa sélection
//!    et son texte (TextEdit, Notes, Mail, la plupart des champs natifs,
//!    souvent les champs web). On lit et on remplace sans toucher au
//!    presse-papiers.
//! 2. **Copier/coller contrôlé** : sinon (Electron, éditeurs riches), ⌘C
//!    synthétique, lecture, puis restauration du presse-papiers ; ⌘V
//!    synthétique pour remplacer, puis restauration. Le `changeCount` prouve
//!    que la copie a bien produit la sélection, et qu'aucune copie de
//!    l'utilisateur n'est écrasée.

mod apps;
pub mod ax;
mod ffi;
mod keys;
mod pasteboard;
#[cfg(feature = "selftest")]
pub mod selftest;

use std::thread;
use std::time::{Duration, Instant};

use crate::platform::{Capture, CaptureError, Platform};
use ax::AxElement;
use ffi::CFRange;

pub use apps::beep;

/// Auto-test : force la voie copier/coller même quand Accessibility suffirait.
#[cfg(feature = "selftest")]
pub static FORCE_CLIPBOARD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn force_clipboard() -> bool {
    #[cfg(feature = "selftest")]
    {
        FORCE_CLIPBOARD.load(std::sync::atomic::Ordering::SeqCst)
    }
    #[cfg(not(feature = "selftest"))]
    {
        false
    }
}

/// Attente du relâchement des touches du raccourci (nos événements portent
/// de toute façon leurs propres modificateurs).
const MODIFIERS_TIMEOUT: Duration = Duration::from_millis(250);
/// Attente de l'effet d'un ⌘C synthétique sur le presse-papiers.
const COPY_TIMEOUT: Duration = Duration::from_millis(600);
/// Attente maximale de l'effet d'un ⌘V quand Accessibility permet de
/// l'observer ; délai fixe sinon. Restaurer le presse-papiers trop tôt ferait
/// coller l'ancien contenu dans les apps lentes à lire.
const PASTE_OBSERVE_TIMEOUT: Duration = Duration::from_millis(400);
const PASTE_SETTLE: Duration = Duration::from_millis(600);
/// Délai de retour au premier plan de l'app cible, pour une restauration.
const ACTIVATE_TIMEOUT: Duration = Duration::from_millis(1_000);

pub struct MacTarget {
    pid: i32,
    element: Option<AxElement>,
    /// Sélection au moment de la capture, en unités UTF-16 (convention AX).
    range: Option<CFRange>,
    /// Source lue par Accessibility (sinon, par copie).
    via_ax: bool,
}

pub struct MacApplied {
    pid: i32,
    element: Option<AxElement>,
    /// Début du texte inséré, en UTF-16, quand Accessibility le donne.
    start: Option<isize>,
}

#[derive(Default)]
pub struct MacPlatform;

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn utf16_len(text: &str) -> isize {
    text.encode_utf16().count() as isize
}

/// Extrait `[start, start + len)` d'un texte en unités UTF-16.
fn utf16_slice(text: &str, start: isize, len: isize) -> Option<String> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let (start, end) = (usize::try_from(start).ok()?, usize::try_from(start + len).ok()?);
    String::from_utf16(units.get(start..end)?).ok()
}

/// App au premier plan : par Accessibility, sinon par AppKit.
fn frontmost() -> (Option<AxElement>, Option<i32>) {
    let app = ax::focused_application();
    let pid = app.as_ref().and_then(AxElement::pid).or_else(apps::frontmost_pid);
    (app, pid)
}

/// Remise en place du presse-papiers après un collage, faite en arrière-plan
/// pour ne pas faire attendre l'utilisateur. Toute nouvelle opération attend
/// d'abord qu'elle soit terminée.
static PENDING_RESTORE: std::sync::Mutex<Option<thread::JoinHandle<()>>> = std::sync::Mutex::new(None);

fn finish_pending_restore() {
    let handle = PENDING_RESTORE.lock().ok().and_then(|mut pending| pending.take());
    if let Some(handle) = handle {
        let _ = handle.join();
    }
}

/// ⌘C contrôlé : renvoie le texte copié, ou `None` si la copie n'a rien
/// produit. Le presse-papiers de l'utilisateur est remis tel qu'il était.
fn copy_selection() -> Result<Option<String>, CaptureError> {
    finish_pending_restore();
    keys::wait_for_modifiers_release(MODIFIERS_TIMEOUT);
    let snapshot = pasteboard::snapshot();
    if !snapshot.complete {
        // Contenu trop lourd pour être remis ensuite : on n'y touche pas.
        return Err(CaptureError::ReadFailed);
    }
    let before = pasteboard::change_count();
    keys::copy();

    let deadline = Instant::now() + COPY_TIMEOUT;
    let mut copied = before;
    while copied == before && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(15));
        copied = pasteboard::change_count();
    }
    if copied == before {
        // Aucune copie : pas de sélection, et surtout pas de lecture de
        // l'ancien contenu du presse-papiers.
        return Ok(None);
    }
    // Certaines apps publient en deux temps (effacement puis écriture).
    thread::sleep(Duration::from_millis(30));
    let text = pasteboard::read_text();
    let empty_selection = pasteboard::is_empty_selection_copy();

    // Remise en état, sauf si quelqu'un a encore écrit entre-temps.
    if pasteboard::change_count() == copied {
        pasteboard::restore(&snapshot);
    }
    if empty_selection {
        return Ok(None);
    }
    Ok(text.filter(|text| !text.is_empty()))
}

/// ⌘V contrôlé. Renvoie vrai si le collage a été observé (ou n'est pas
/// observable), faux s'il n'a visiblement pas eu lieu.
fn paste_text(text: &str, observe: Option<(&AxElement, isize, &str)>) -> bool {
    finish_pending_restore();
    keys::wait_for_modifiers_release(MODIFIERS_TIMEOUT);
    let snapshot = pasteboard::snapshot();
    if !snapshot.complete {
        return false;
    }
    let ours = pasteboard::write_transient_text(text);
    keys::paste();

    let pasted = match observe {
        Some((element, start, source)) => {
            let expected = normalize(text);
            let length = utf16_len(text);
            let deadline = Instant::now() + PASTE_OBSERVE_TIMEOUT;
            let mut outcome = None;
            while outcome.is_none() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(50));
                if let Ok(Some(value)) = element.string_attribute("AXValue") {
                    if utf16_slice(&value, start, length).is_some_and(|slice| normalize(&slice) == expected) {
                        outcome = Some(true);
                    }
                } else {
                    // Plus lisible : on suppose le collage fait.
                    outcome = Some(true);
                }
            }
            // Pas d'effet visible : le texte d'origine est-il toujours là ?
            outcome.unwrap_or_else(|| {
                let still_source = element
                    .string_attribute("AXValue")
                    .ok()
                    .flatten()
                    .and_then(|value| utf16_slice(&value, start, utf16_len(source)))
                    .is_some_and(|slice| normalize(&slice) == normalize(source));
                !still_source
            })
        }
        None => true,
    };

    // L'app cible lit le presse-papiers à son rythme : on attend un peu avant
    // de remettre l'ancien contenu, en arrière-plan. Et seulement s'il est
    // encore le nôtre : une copie faite par l'utilisateur entre-temps est gardée.
    let handle = thread::spawn(move || {
        thread::sleep(PASTE_SETTLE);
        if pasteboard::change_count() == ours {
            pasteboard::restore(&snapshot);
        }
    });
    if let Ok(mut pending) = PENDING_RESTORE.lock() {
        *pending = Some(handle);
    }
    pasted
}

impl MacPlatform {
    fn selected_text(element: &AxElement) -> Option<String> {
        element.string_attribute("AXSelectedText").ok().flatten()
    }

    fn selected_range(element: &AxElement) -> Option<CFRange> {
        element.range_attribute("AXSelectedTextRange").ok().flatten()
    }
}

impl Platform for MacPlatform {
    type Target = MacTarget;
    type Applied = MacApplied;

    fn is_trusted(&self) -> bool {
        ax::is_trusted(false)
    }

    fn capture(&self) -> Result<Capture<MacTarget>, CaptureError> {
        let (app, pid) = frontmost();
        let pid = pid.ok_or(CaptureError::ReadFailed)?;
        if pid == std::process::id() as i32 {
            return Err(CaptureError::SelfTarget);
        }
        let app_name = apps::info(pid).name;
        let element = app.as_ref().and_then(ax::focused_element);

        match &element {
            Some(element) if element.is_secure() => return Err(CaptureError::SecureField),
            // Champ inconnu et saisie sécurisée active : on s'abstient.
            None if ax::secure_input_enabled() => return Err(CaptureError::SecureField),
            _ => {}
        }

        log::info!(
            "cible : pid {pid}, rôle {:?}, AX trusted {}",
            element.as_ref().and_then(AxElement::role),
            ax::is_trusted(false)
        );
        let range = element.as_ref().and_then(Self::selected_range);
        if let Some(element) = element.as_ref().filter(|_| !force_clipboard()) {
            match element.string_attribute("AXSelectedText") {
                Ok(Some(text)) if !text.is_empty() => {
                    return Ok(Capture {
                        source: text,
                        app: app_name,
                        target: MacTarget { pid, element: Some(element.clone()), range, via_ax: true },
                    });
                }
                // Le champ dit clairement que rien n'est sélectionné.
                Ok(_) if range.is_some_and(|range| range.length == 0) => {
                    return Err(CaptureError::NoSelection);
                }
                _ => {}
            }
        }

        log::info!("lecture AX impossible, repli sur la copie");
        let source = copy_selection()?.ok_or(CaptureError::NoSelection)?;
        Ok(Capture {
            source,
            app: app_name,
            target: MacTarget { pid, element, range, via_ax: false },
        })
    }

    fn still_targeted(&self, target: &MacTarget, source: &str) -> bool {
        let (app, pid) = frontmost();
        if pid != Some(target.pid) {
            return false;
        }
        let current = app.as_ref().and_then(ax::focused_element);
        if let Some(element) = &target.element {
            if current.as_ref() != Some(element) {
                return false;
            }
            if target.range.is_some() && Self::selected_range(element) != target.range {
                return false;
            }
        }
        if target.via_ax {
            let element = target.element.as_ref().expect("lecture AX sans élément");
            return Self::selected_text(element).is_some_and(|text| normalize(&text) == normalize(source));
        }
        // Voie presse-papiers : même app et même champ suffisent. Recopier la
        // sélection pour la comparer coûtait jusqu'à une seconde dans les
        // apps Electron.
        let _ = source;
        target.element.is_some() || current.is_none()
    }

    fn replace(&self, target: &MacTarget, source: &str, replacement: &str) -> Option<MacApplied> {
        let start = target.range.map(|range| range.location);
        let applied = || MacApplied { pid: target.pid, element: target.element.clone(), start };

        if let (true, Some(element)) = (target.via_ax, &target.element) {
            if element.is_settable("AXSelectedText")
                && element.set_string_attribute("AXSelectedText", replacement).is_ok()
            {
                // Certaines apps acceptent l'écriture sans l'appliquer : on ne
                // retombe sur le collage que si le texte d'origine est
                // visiblement toujours là, pour ne jamais remplacer deux fois.
                thread::sleep(Duration::from_millis(30));
                let unchanged = match (start, element.string_attribute("AXValue")) {
                    (Some(start), Ok(Some(value))) => {
                        let inserted = utf16_slice(&value, start, utf16_len(replacement));
                        if inserted.is_some_and(|slice| normalize(&slice) == normalize(replacement)) {
                            false
                        } else {
                            utf16_slice(&value, start, utf16_len(source))
                                .is_some_and(|slice| normalize(&slice) == normalize(source))
                        }
                    }
                    _ => Self::selected_text(element)
                        .is_some_and(|text| normalize(&text) == normalize(source)),
                };
                if !unchanged {
                    return Some(applied());
                }
                log::info!("remplacement AX ignoré par l'app, repli sur le collage");
            }
        }

        let observe = match (&target.element, start) {
            (Some(element), Some(start)) => Some((element, start, source)),
            _ => None,
        };
        paste_text(replacement, observe).then(applied)
    }

    fn restore(&self, applied: &MacApplied, replacement: &str, original: &str) -> bool {
        let (Some(element), Some(start)) = (&applied.element, applied.start) else {
            return false;
        };
        apps::activate(applied.pid);
        let deadline = Instant::now() + ACTIVATE_TIMEOUT;
        while frontmost().1 != Some(applied.pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(40));
        }
        thread::sleep(Duration::from_millis(120));

        let (app, pid) = frontmost();
        if pid != Some(applied.pid) || app.as_ref().and_then(ax::focused_element).as_ref() != Some(element) {
            return false;
        }
        // On resélectionne exactement le texte inséré, et on vérifie que
        // c'est bien lui avant d'y toucher.
        let range = CFRange { location: start, length: utf16_len(replacement) };
        if element.set_range_attribute("AXSelectedTextRange", range).is_err() {
            return false;
        }
        thread::sleep(Duration::from_millis(40));
        let selected = Self::selected_text(element);
        if !selected.is_some_and(|text| normalize(&text) == normalize(replacement)) {
            return false;
        }
        let target = MacTarget { pid: applied.pid, element: Some(element.clone()), range: Some(range), via_ax: true };
        self.replace(&target, replacement, original).is_some()
    }

    fn copy(&self, text: &str) {
        pasteboard::write_user_text(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_slicing_matches_accessibility_ranges() {
        // « é » : 1 unité UTF-16 ; l'émoji : 2.
        let text = "é🙂 bonjour";
        assert_eq!(utf16_len("🙂"), 2);
        assert_eq!(utf16_slice(text, 4, 7).as_deref(), Some("bonjour"));
        assert_eq!(utf16_slice(text, 0, 3).as_deref(), Some("é🙂"));
        assert_eq!(utf16_slice(text, 20, 2), None);
    }

    #[test]
    fn normalizes_line_endings() {
        assert_eq!(normalize("a\r\nb\rc"), "a\nb\nc");
    }
}
