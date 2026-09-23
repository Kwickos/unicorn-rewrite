//! Auto-test du parcours natif dans de vraies apps macOS.
//!
//! Compilé seulement avec la feature `selftest`, jamais dans un build de
//! distribution. Lancement :
//!
//! ```sh
//! open "Unicorn Rewrite.app" --args --selftest=textedit,safari,arc \
//!   --selftest-out=/chemin/rapport.txt
//! ```
//!
//! Pour chaque app : ouvre un document de test, place une sélection par
//! Accessibility, déclenche le vrai moteur (capture → traitement simulé →
//! vérification → remplacement), puis relit le champ et le presse-papiers.
//! Le fournisseur est simulé : on valide l'intégration macOS, pas le modèle.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use tauri::async_runtime::block_on;
use tauri::{AppHandle, Manager};

use super::ax::{self, AxElement};
use super::ffi::CFRange;
use super::{apps, pasteboard, FORCE_CLIPBOARD};
use crate::ai::mock::Mock;
use crate::ai::{AiError, BoxFuture, Rewriter};
use crate::engine::{ErrorCode, Phase, Request};
use crate::profiles::ProfileId;
use crate::AppState;

const BASE: &str = "Bonjour tout le monde, ceci est un test.\nDeuxième ligne : 1 250,50 € le 3 mars.";
/// « tout le monde » : 8 → 21 (UTF-16).
const SELECTION: CFRange = CFRange { location: 8, length: 13 };
const SELECTED: &str = "tout le monde";

struct Failing;

impl Rewriter for Failing {
    fn rewrite<'a>(&'a self, _: &'a str, _: ProfileId, _: &'a str) -> BoxFuture<'a, Result<String, AiError>> {
        Box::pin(async { Err(AiError::Network) })
    }
}

#[derive(Clone, Copy)]
enum Kind {
    TextEdit,
    Browser,
}

struct Target {
    name: &'static str,
    kind: Kind,
    pid: i32,
    field: AxElement,
}

struct Report {
    lines: Vec<String>,
    failures: usize,
}

impl Report {
    fn record(&mut self, app: &str, case: &str, outcome: Result<String, String>) {
        let line = match outcome {
            Ok(detail) => format!("PASS  {app:<9} {case:<34} {detail}"),
            Err(detail) => {
                self.failures += 1;
                format!("FAIL  {app:<9} {case:<34} {detail}")
            }
        };
        log::info!("{line}");
        self.lines.push(line);
    }
}

pub fn start_if_requested(app: &AppHandle) {
    let args: Vec<String> = std::env::args().collect();
    let Some(spec) = args.iter().find_map(|arg| arg.strip_prefix("--selftest=")) else {
        return;
    };
    let out = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--selftest-out="))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("unicorn-rewrite-selftest.txt"));
    let targets: Vec<String> = spec.split(',').map(str::to_owned).collect();
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(800));
        let mut report = Report { lines: Vec::new(), failures: 0 };
        // Sans permission, macOS affiche son invitation et ajoute l'app à la
        // liste des Réglages Système.
        report.lines.push(format!("Accessibilité accordée : {}", ax::is_trusted(true)));
        if ax::is_trusted(false) {
            for name in &targets {
                run_target(&app, name, &mut report);
            }
        }
        report.lines.push(format!("Échecs : {}", report.failures));
        let _ = fs::write(&out, report.lines.join("\n") + "\n");
        crate::hide_panel(&app);
        app.exit(if report.failures == 0 { 0 } else { 1 });
    });
}

fn work_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("unicorn-rewrite-selftest");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn open_target(name: &str) -> Option<Target> {
    let (label, kind, app_name): (&'static str, Kind, &str) = match name {
        "textedit" => ("TextEdit", Kind::TextEdit, "TextEdit"),
        "safari" => ("Safari", Kind::Browser, "Safari"),
        "arc" => ("Arc", Kind::Browser, "Arc"),
        _ => return None,
    };
    let path = match kind {
        Kind::TextEdit => {
            let path = work_dir().join("selftest.txt");
            fs::write(&path, BASE).ok()?;
            path
        }
        Kind::Browser => {
            let path = work_dir().join("selftest.html");
            let html = format!(
                "<!doctype html><meta charset=utf-8><title>Unicorn Rewrite selftest</title>\
                 <textarea id=t rows=6 cols=60 autofocus>{BASE}</textarea>\
                 <input id=p type=password value=secret>"
            );
            fs::write(&path, html).ok()?;
            path
        }
    };
    Command::new("open").args(["-a", app_name]).arg(&path).status().ok()?;

    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(300));
        let Some(app) = ax::focused_application() else { continue };
        let Some(pid) = app.pid() else { continue };
        if apps::info(pid).name != label {
            continue;
        }
        if let Some(field) = ax::focused_element(&app) {
            if field.role().as_deref() == Some("AXTextArea") {
                return Some(Target { name: label, kind, pid, field });
            }
        }
    }
    None
}

fn bring_back(target: &Target) {
    apps::activate(target.pid);
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if ax::focused_application().and_then(|app| app.pid()) == Some(target.pid) {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    thread::sleep(Duration::from_millis(150));
}

fn value(target: &Target) -> String {
    target.field.string_attribute("AXValue").ok().flatten().unwrap_or_default().replace('\r', "\n")
}

fn reset(target: &Target, range: CFRange) -> Result<(), String> {
    bring_back(target);
    target
        .field
        .set_string_attribute("AXValue", BASE)
        .map_err(|error| format!("AXValue non modifiable ({})", error.0))?;
    thread::sleep(Duration::from_millis(80));
    target
        .field
        .set_range_attribute("AXSelectedTextRange", range)
        .map_err(|error| format!("sélection impossible ({})", error.0))?;
    thread::sleep(Duration::from_millis(120));
    if value(target) != BASE {
        return Err("réinitialisation du champ non appliquée".into());
    }
    Ok(())
}

fn expected(profile: ProfileId) -> String {
    BASE.replacen(SELECTED, &Mock::transform(SELECTED, profile), 1)
}

fn request() -> Request {
    Request { profile: ProfileId::Natural, custom: String::new() }
}

fn run_target(app: &AppHandle, name: &str, report: &mut Report) {
    let Some(target) = open_target(name) else {
        report.record(name, "ouverture", Err("champ de texte introuvable".into()));
        return;
    };
    let state = app.state::<AppState>();
    let engine = state.engine.clone();
    let mock = Arc::new(Mock::new(Duration::from_millis(0)));
    engine.set_rewriter(mock.clone());
    let started = Instant::now();

    for clipboard_path in [false, true] {
        FORCE_CLIPBOARD.store(clipboard_path, Ordering::SeqCst);
        let mode = if clipboard_path { "copier/coller" } else { "accessibility" };
        let case = |label: &str| format!("{label} [{mode}]");

        // 1. Parcours nominal, presse-papiers préservé.
        let outcome = (|| {
            reset(&target, SELECTION)?;
            pasteboard::write_user_text("PRESSE-PAPIERS-UTILISATEUR");
            mock.set_delay(Duration::from_millis(200));
            let begin = Instant::now();
            block_on(engine.clone().run(request()));
            let elapsed = begin.elapsed().as_millis();
            let status = engine.status();
            if status.phase != Phase::Done {
                return Err(format!("état {:?} {:?}", status.phase, status.error));
            }
            let got = value(&target);
            if got != expected(ProfileId::Natural) {
                return Err(format!("texte obtenu : {got:?}"));
            }
            let clip = pasteboard::read_text().unwrap_or_default();
            if clip != "PRESSE-PAPIERS-UTILISATEUR" {
                return Err(format!("presse-papiers modifié : {clip:?}"));
            }
            Ok(format!("{elapsed} ms (dont 200 ms simulés)"))
        })();
        report.record(target.name, &case("remplacement + presse-papiers"), outcome);

        // 2. Restauration de la dernière modification.
        let outcome = (|| {
            bring_back(&target);
            if !block_on(engine.clone().restore()) {
                return Err(format!("restauration refusée ({:?})", engine.status().error));
            }
            let got = value(&target);
            if got != BASE {
                return Err(format!("texte obtenu : {got:?}"));
            }
            Ok("original remis".into())
        })();
        report.record(target.name, &case("restauration"), outcome);

        // 3. Aucune sélection.
        let outcome = (|| {
            reset(&target, CFRange { location: 5, length: 0 })?;
            pasteboard::write_user_text("ANCIEN-CONTENU");
            block_on(engine.clone().run(request()));
            let status = engine.status();
            if status.error != Some(ErrorCode::NoSelection) {
                return Err(format!("état {:?} {:?}", status.phase, status.error));
            }
            if value(&target) != BASE {
                return Err("texte modifié".into());
            }
            if pasteboard::read_text().as_deref() != Some("ANCIEN-CONTENU") {
                return Err("presse-papiers modifié".into());
            }
            Ok("signalé, rien touché, ancien presse-papiers non traité".into())
        })();
        report.record(target.name, &case("aucune sélection"), outcome);

        // 4. Texte modifié pendant la requête.
        let outcome = (|| {
            reset(&target, SELECTION)?;
            mock.set_delay(Duration::from_millis(1_200));
            let running = tauri::async_runtime::spawn(engine.clone().run(request()));
            thread::sleep(Duration::from_millis(400));
            let edited = BASE.replace("test", "essai");
            target.field.set_string_attribute("AXValue", &edited).map_err(|e| format!("édition ({})", e.0))?;
            block_on(running).map_err(|_| "tâche interrompue")?;
            let status = engine.status();
            if status.error != Some(ErrorCode::TargetChanged) || status.result.is_none() {
                return Err(format!("état {:?} {:?}", status.phase, status.error));
            }
            if value(&target) != edited {
                return Err(format!("texte écrasé : {:?}", value(&target)));
            }
            Ok("résultat proposé à la copie, texte intact".into())
        })();
        report.record(target.name, &case("modification pendant la requête"), outcome);

        // 5. Focus déplacé vers une autre app pendant la requête.
        let outcome = (|| {
            reset(&target, SELECTION)?;
            mock.set_delay(Duration::from_millis(1_500));
            let running = tauri::async_runtime::spawn(engine.clone().run(request()));
            thread::sleep(Duration::from_millis(300));
            let _ = Command::new("open").args(["-a", "Finder"]).status();
            block_on(running).map_err(|_| "tâche interrompue")?;
            let status = engine.status();
            if status.error != Some(ErrorCode::TargetChanged) {
                return Err(format!("état {:?} {:?}", status.phase, status.error));
            }
            if value(&target) != BASE {
                return Err("texte modifié".into());
            }
            Ok("rien collé ailleurs, résultat à copier".into())
        })();
        report.record(target.name, &case("changement de focus"), outcome);

        // 6. Double déclenchement.
        let outcome = (|| {
            reset(&target, SELECTION)?;
            mock.set_delay(Duration::from_millis(600));
            let first = tauri::async_runtime::spawn(engine.clone().run(request()));
            thread::sleep(Duration::from_millis(100));
            let second = block_on(engine.clone().run(request()));
            block_on(first).map_err(|_| "tâche interrompue")?;
            if second {
                return Err("second déclenchement accepté".into());
            }
            if value(&target) != expected(ProfileId::Natural) {
                return Err(format!("texte obtenu : {:?}", value(&target)));
            }
            Ok("second appui ignoré, un seul remplacement".into())
        })();
        report.record(target.name, &case("double raccourci"), outcome);

        // 7. Annulation, puis réponse tardive.
        let outcome = (|| {
            reset(&target, SELECTION)?;
            mock.set_delay(Duration::from_millis(900));
            let running = tauri::async_runtime::spawn(engine.clone().run(request()));
            thread::sleep(Duration::from_millis(250));
            engine.cancel();
            block_on(running).map_err(|_| "tâche interrompue")?;
            thread::sleep(Duration::from_millis(900));
            if value(&target) != BASE {
                return Err("réponse tardive appliquée".into());
            }
            if engine.status().phase != Phase::Cancelled {
                return Err(format!("état {:?}", engine.status().phase));
            }
            Ok("réponse tardive jetée".into())
        })();
        report.record(target.name, &case("annulation"), outcome);

        // 8. Copie de l'utilisateur pendant la requête.
        let outcome = (|| {
            reset(&target, SELECTION)?;
            pasteboard::write_user_text("AVANT");
            mock.set_delay(Duration::from_millis(900));
            let running = tauri::async_runtime::spawn(engine.clone().run(request()));
            thread::sleep(Duration::from_millis(400));
            pasteboard::write_user_text("NOUVELLE-COPIE");
            block_on(running).map_err(|_| "tâche interrompue")?;
            if value(&target) != expected(ProfileId::Natural) {
                return Err(format!("texte obtenu : {:?}", value(&target)));
            }
            let clip = pasteboard::read_text().unwrap_or_default();
            if clip != "NOUVELLE-COPIE" {
                return Err(format!("copie de l'utilisateur perdue : {clip:?}"));
            }
            Ok("nouvelle copie conservée".into())
        })();
        report.record(target.name, &case("copie concurrente"), outcome);

        // 9. Erreur réseau.
        let outcome = (|| {
            reset(&target, SELECTION)?;
            engine.set_rewriter(Arc::new(Failing));
            block_on(engine.clone().run(request()));
            engine.set_rewriter(mock.clone());
            if engine.status().error != Some(ErrorCode::Network) || value(&target) != BASE {
                return Err(format!("état {:?}", engine.status().error));
            }
            Ok("erreur signalée, texte intact".into())
        })();
        report.record(target.name, &case("erreur réseau"), outcome);
    }
    FORCE_CLIPBOARD.store(false, Ordering::SeqCst);

    // Champ de mot de passe (navigateurs).
    if let Kind::Browser = target.kind {
        let outcome = (|| {
            bring_back(&target);
            let window = ax::focused_application()
                .and_then(|app| app.element_attribute("AXFocusedWindow").ok().flatten())
                .ok_or("fenêtre introuvable")?;
            let password = find_secure_field(&window, 0).ok_or("champ mot de passe introuvable")?;
            password.set_bool_attribute("AXFocused", true).map_err(|e| format!("focus ({})", e.0))?;
            thread::sleep(Duration::from_millis(300));
            block_on(engine.clone().run(request()));
            let status = engine.status();
            if status.error != Some(ErrorCode::SecureField) {
                return Err(format!("état {:?} {:?}", status.phase, status.error));
            }
            Ok("refusé".into())
        })();
        report.record(target.name, "champ de mot de passe", outcome);
    }

    report.lines.push(format!("{} : {} s", target.name, started.elapsed().as_secs()));
    crate::hide_panel(app);
}

fn find_secure_field(element: &AxElement, depth: usize) -> Option<AxElement> {
    if depth > 25 {
        return None;
    }
    if element.is_secure() {
        return Some(element.clone());
    }
    for child in element.children() {
        if let Some(found) = find_secure_field(&child, depth + 1) {
            return Some(found);
        }
    }
    None
}
