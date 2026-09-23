//! Une opération de reformulation, du raccourci au remplacement.
//!
//! Tout se passe ici, côté natif : la page peut être suspendue, le panneau
//! fermé, l'opération va au bout. Garanties tenues par ce module :
//! - une seule opération à la fois : un second raccourci est ignoré ;
//! - une réponse arrivée après annulation est jetée ;
//! - rien n'est remplacé tant que la réponse complète n'est pas validée ;
//! - avant de remplacer, la cible est revérifiée ; au moindre doute, le
//!   résultat est présenté avec « Copier » au lieu d'être collé à l'aveugle ;
//! - le texte d'origine reste en mémoire (jamais sur disque) un temps limité,
//!   pour restaurer ou copier.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::Notify;

use crate::ai::{prompt, AiError, Rewriter, MAX_INPUT_CHARS};
use crate::platform::{CaptureError, Platform};
use crate::profiles::ProfileId;

/// Délai total d'une reformulation, nouvelle tentative comprise.
pub const OPERATION_DEADLINE: Duration = Duration::from_secs(25);

/// Durée de conservation en mémoire de l'original et du résultat.
const RETENTION: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Reading,
    Rewriting,
    Replacing,
    /// Sélection remplacée.
    Done,
    /// Le texte était déjà bon : rien n'a été touché.
    Unchanged,
    Restored,
    Cancelled,
    Error,
    /// Résultat prêt mais pas appliqué : à copier depuis le panneau.
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    Permission,
    NoSelection,
    SecureField,
    SelfTarget,
    ReadFailed,
    TooLong,
    MissingKey,
    InvalidKey,
    Quota,
    Network,
    Timeout,
    Blocked,
    Truncated,
    Empty,
    Unusable,
    Provider,
    TargetChanged,
    ReplaceFailed,
    RestoreFailed,
}

impl From<AiError> for ErrorCode {
    fn from(error: AiError) -> Self {
        match error {
            AiError::MissingKey => ErrorCode::MissingKey,
            AiError::InvalidKey => ErrorCode::InvalidKey,
            AiError::Quota => ErrorCode::Quota,
            AiError::Network => ErrorCode::Network,
            AiError::Timeout => ErrorCode::Timeout,
            AiError::Blocked => ErrorCode::Blocked,
            AiError::Truncated => ErrorCode::Truncated,
            AiError::Empty => ErrorCode::Empty,
            AiError::Unusable => ErrorCode::Unusable,
            AiError::Provider | AiError::Unavailable => ErrorCode::Provider,
        }
    }
}

impl From<CaptureError> for ErrorCode {
    fn from(error: CaptureError) -> Self {
        match error {
            CaptureError::NoSelection => ErrorCode::NoSelection,
            CaptureError::SecureField => ErrorCode::SecureField,
            CaptureError::SelfTarget => ErrorCode::SelfTarget,
            CaptureError::ReadFailed => ErrorCode::ReadFailed,
        }
    }
}

/// L'état tel que le panneau et l'icône l'affichent.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: Phase,
    pub error: Option<ErrorCode>,
    /// App cible de la dernière opération.
    pub app: Option<String>,
    pub profile: Option<ProfileId>,
    /// Résultat non appliqué, présenté pour être copié.
    pub result: Option<String>,
    /// La dernière modification peut être défaite.
    pub can_restore: bool,
    /// Le texte d'origine peut être copié.
    pub has_original: bool,
    /// Numéro de l'opération, pour ignorer un état périmé côté page.
    pub operation: u64,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            error: None,
            app: None,
            profile: None,
            result: None,
            can_restore: false,
            has_original: false,
            operation: 0,
        }
    }
}

/// Ce que le moteur demande à l'app autour de lui.
pub trait Hooks: Send + Sync + 'static {
    fn status_changed(&self, status: &Status);
    fn show_panel(&self);
    fn hide_panel(&self);
    /// Active ou retire le raccourci Échap d'annulation, le temps du traitement.
    fn cancel_shortcut(&self, active: bool);
    /// Son d'alerte : seul retour d'une erreur quand le panneau est fermé.
    fn alert(&self);
}

#[derive(Debug, Clone)]
pub struct Request {
    pub profile: ProfileId,
    pub custom: String,
}

struct LastOperation<A> {
    applied: Option<A>,
    original: String,
    replacement: String,
    at: Instant,
}

impl<A> LastOperation<A> {
    fn fresh(&self) -> bool {
        self.at.elapsed() < RETENTION
    }
}

pub struct Engine<P: Platform> {
    platform: Arc<P>,
    rewriter: RwLock<Arc<dyn Rewriter>>,
    hooks: Arc<dyn Hooks>,
    busy: AtomicBool,
    generation: AtomicU64,
    cancel: Notify,
    status: Mutex<Status>,
    last: Mutex<Option<LastOperation<P::Applied>>>,
    deadline: Duration,
}

/// Libère le verrou « occupé » quoi qu'il arrive, même sur une panique.
struct BusyGuard<'a>(&'a AtomicBool);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl<P: Platform> Engine<P> {
    pub fn new(platform: P, rewriter: Arc<dyn Rewriter>, hooks: Arc<dyn Hooks>) -> Arc<Self> {
        Arc::new(Self {
            platform: Arc::new(platform),
            rewriter: RwLock::new(rewriter),
            hooks,
            busy: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            cancel: Notify::new(),
            status: Mutex::new(Status::default()),
            last: Mutex::new(None),
            deadline: OPERATION_DEADLINE,
        })
    }

    #[cfg(test)]
    fn with_deadline(self: Arc<Self>, deadline: Duration) -> Arc<Self> {
        let mut engine = Arc::try_unwrap(self).ok().expect("moteur non partagé");
        engine.deadline = deadline;
        Arc::new(engine)
    }

    pub fn platform(&self) -> &P {
        &self.platform
    }

    pub fn set_rewriter(&self, rewriter: Arc<dyn Rewriter>) {
        if let Ok(mut current) = self.rewriter.write() {
            *current = rewriter;
        }
    }

    pub fn status(&self) -> Status {
        let mut status = self.status.lock().map(|status| status.clone()).unwrap_or_default();
        let (can_restore, has_original) = self.retained();
        status.can_restore = can_restore;
        status.has_original = has_original;
        status
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    fn retained(&self) -> (bool, bool) {
        let Ok(mut last) = self.last.lock() else {
            return (false, false);
        };
        if last.as_ref().is_some_and(|last| !last.fresh()) {
            *last = None;
        }
        match last.as_ref() {
            Some(last) => (last.applied.is_some(), true),
            None => (false, false),
        }
    }

    fn publish(&self, change: impl FnOnce(&mut Status)) {
        let snapshot = {
            let Ok(mut status) = self.status.lock() else {
                return;
            };
            change(&mut status);
            status.clone()
        };
        let mut snapshot = snapshot;
        let (can_restore, has_original) = self.retained();
        snapshot.can_restore = can_restore;
        snapshot.has_original = has_original;
        self.hooks.status_changed(&snapshot);
    }

    fn fail(&self, generation: u64, error: ErrorCode) {
        self.publish(|status| {
            status.phase = Phase::Error;
            status.error = Some(error);
            status.result = None;
            status.operation = generation;
        });
        self.hooks.alert();
        if error == ErrorCode::Permission {
            self.hooks.show_panel();
        }
    }

    /// Point d'entrée du raccourci. Renvoie faux si une opération est déjà
    /// en cours : le second appui est ignoré.
    pub async fn run(self: Arc<Self>, request: Request) -> bool {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            log::info!("raccourci ignoré : opération déjà en cours");
            return false;
        }
        let _busy = BusyGuard(&self.busy);
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let started = Instant::now();
        self.operate(generation, request).await;
        let status = self.status();
        log::info!(
            "opération {generation} : {:?} {:?} en {} ms",
            status.phase,
            status.error,
            started.elapsed().as_millis()
        );
        true
    }

    async fn operate(self: &Arc<Self>, generation: u64, request: Request) {
        if !self.platform.is_trusted() {
            self.fail(generation, ErrorCode::Permission);
            return;
        }

        self.publish(|status| {
            *status = Status {
                phase: Phase::Reading,
                profile: Some(request.profile),
                operation: generation,
                ..Status::default()
            };
        });

        // Connexion au fournisseur ouverte pendant la lecture de la sélection.
        if let Some(rewriter) = self.rewriter.read().ok().map(|rewriter| rewriter.clone()) {
            tokio::spawn(async move { rewriter.prepare().await });
        }
        let step = Instant::now();
        let platform = self.platform.clone();
        let captured = tokio::task::spawn_blocking(move || platform.capture())
            .await
            .unwrap_or(Err(CaptureError::ReadFailed));
        let capture = match captured {
            Ok(capture) => capture,
            Err(error) => return self.fail(generation, error.into()),
        };
        log::info!(
            "capture : {} caractères dans {} en {} ms",
            capture.source.chars().count(),
            capture.app,
            step.elapsed().as_millis()
        );
        if capture.source.chars().count() > MAX_INPUT_CHARS {
            return self.fail(generation, ErrorCode::TooLong);
        }
        if self.current_generation() != generation {
            return;
        }

        self.publish(|status| {
            status.phase = Phase::Rewriting;
            status.app = Some(capture.app.clone());
        });

        // `ok()` tout de suite : le verrou ne doit pas traverser un `await`.
        let rewriter = self.rewriter.read().ok().map(|rewriter| rewriter.clone());
        let Some(rewriter) = rewriter else {
            return self.fail(generation, ErrorCode::Provider);
        };

        self.hooks.cancel_shortcut(true);
        let step = Instant::now();
        let outcome = tokio::select! {
            outcome = tokio::time::timeout(
                self.deadline,
                rewriter.rewrite(&capture.source, request.profile, &request.custom),
            ) => Some(outcome.unwrap_or(Err(AiError::Timeout))),
            _ = self.cancelled(generation) => None,
        };
        self.hooks.cancel_shortcut(false);
        log::info!("IA : {} ms", step.elapsed().as_millis());

        // Annulée pendant la requête : la réponse, même arrivée, est jetée.
        let Some(outcome) = outcome else { return };
        if self.current_generation() != generation {
            return;
        }

        let result = match outcome.and_then(|raw| prompt::finalize(&capture.source, &raw)) {
            Ok(result) => result,
            Err(error) => return self.fail(generation, error.into()),
        };

        if result == capture.source {
            self.publish(|status| status.phase = Phase::Unchanged);
            return;
        }

        self.publish(|status| status.phase = Phase::Replacing);
        let step = Instant::now();

        let engine = self.clone();
        let source = capture.source.clone();
        let replacement = result.clone();
        let target = capture.target;
        let applied = tokio::task::spawn_blocking(move || {
            if !engine.platform.still_targeted(&target, &source) {
                return Err(ErrorCode::TargetChanged);
            }
            // Dernier point de non-retour : une annulation arrivée pendant la
            // vérification l'emporte encore.
            if engine.current_generation() != generation {
                return Ok(None);
            }
            engine
                .platform
                .replace(&target, &source, &replacement)
                .map(Some)
                .ok_or(ErrorCode::ReplaceFailed)
        })
        .await
        .unwrap_or(Err(ErrorCode::ReplaceFailed));

        log::info!("vérification + remplacement : {} ms", step.elapsed().as_millis());
        match applied {
            Ok(None) => {}
            Ok(Some(applied)) => {
                self.remember(Some(applied), capture.source, result);
                self.publish(|status| status.phase = Phase::Done);
            }
            Err(reason) => {
                self.remember(None, capture.source, result.clone());
                self.publish(|status| {
                    status.phase = Phase::Review;
                    status.error = Some(reason);
                    status.result = Some(result);
                });
                self.hooks.alert();
                self.hooks.show_panel();
            }
        }
    }

    fn remember(&self, applied: Option<P::Applied>, original: String, replacement: String) {
        if let Ok(mut last) = self.last.lock() {
            *last = Some(LastOperation { applied, original, replacement, at: Instant::now() });
        }
    }

    async fn cancelled(&self, generation: u64) {
        loop {
            let notified = self.cancel.notified();
            if self.current_generation() != generation {
                return;
            }
            notified.await;
        }
    }

    /// Annule l'opération en cours. La réponse éventuelle sera ignorée.
    pub fn cancel(&self) -> bool {
        if !self.is_busy() {
            return false;
        }
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.cancel.notify_waiters();
        self.hooks.cancel_shortcut(false);
        self.publish(|status| {
            status.phase = Phase::Cancelled;
            status.error = None;
            status.result = None;
            status.operation = generation;
        });
        true
    }

    /// Défait la dernière modification si la cible est encore là ; sinon le
    /// texte d'origine reste disponible pour être copié.
    pub async fn restore(self: Arc<Self>) -> bool {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        let _busy = BusyGuard(&self.busy);
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;

        let taken = self.last.lock().ok().and_then(|mut last| {
            let fresh = last.as_ref().is_some_and(|last| last.fresh() && last.applied.is_some());
            fresh.then(|| last.take()).flatten()
        });
        let Some(last) = taken else {
            self.fail(generation, ErrorCode::RestoreFailed);
            return false;
        };

        // La cible doit reprendre le focus : on s'efface d'abord.
        self.hooks.hide_panel();
        let engine = self.clone();
        let (restored, last) = tokio::task::spawn_blocking(move || {
            let applied = last.applied.as_ref().expect("vérifié plus haut");
            let restored = engine.platform.restore(applied, &last.replacement, &last.original);
            (restored, last)
        })
        .await
        .unwrap_or_else(|_| panic!("restauration interrompue"));

        if restored {
            self.publish(|status| {
                status.phase = Phase::Restored;
                status.error = None;
                status.result = None;
                status.operation = generation;
            });
        } else {
            // On garde l'original, désormais seulement à copier.
            self.remember(None, last.original, last.replacement);
            self.fail(generation, ErrorCode::RestoreFailed);
            self.hooks.show_panel();
        }
        restored
    }

    /// Copie le résultat non appliqué, ou le texte d'origine.
    pub fn copy(&self, original: bool) -> bool {
        let text = {
            let Ok(last) = self.last.lock() else { return false };
            match last.as_ref().filter(|last| last.fresh()) {
                Some(last) if original => last.original.clone(),
                Some(last) => last.replacement.clone(),
                None => return false,
            }
        };
        self.platform.copy(&text);
        true
    }

    /// Oublie le texte retenu (fermeture du résultat par l'utilisateur).
    pub fn dismiss(&self) {
        let keep_restore = self
            .last
            .lock()
            .ok()
            .is_some_and(|last| last.as_ref().is_some_and(|last| last.applied.is_some()));
        if !keep_restore {
            if let Ok(mut last) = self.last.lock() {
                *last = None;
            }
        }
        self.publish(|status| {
            status.phase = Phase::Idle;
            status.error = None;
            status.result = None;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::BoxFuture;
    use crate::platform::Capture;

    /// Un champ de texte simulé : son contenu, sa sélection, et le focus.
    #[derive(Default)]
    struct Field {
        text: String,
        selection: (usize, usize),
        focused: bool,
    }

    #[derive(Default)]
    struct FakePlatform {
        field: Mutex<Field>,
        clipboard: Mutex<String>,
        untrusted: bool,
        refuse_replace: bool,
    }

    impl FakePlatform {
        fn with_text(text: &str, selection: (usize, usize)) -> Self {
            Self {
                field: Mutex::new(Field { text: text.into(), selection, focused: true }),
                ..Default::default()
            }
        }
        fn text(&self) -> String {
            self.field.lock().unwrap().text.clone()
        }
    }

    impl Platform for FakePlatform {
        type Target = (usize, usize);
        type Applied = (usize, usize);

        fn is_trusted(&self) -> bool {
            !self.untrusted
        }

        fn capture(&self) -> Result<Capture<Self::Target>, CaptureError> {
            let field = self.field.lock().unwrap();
            let (start, end) = field.selection;
            if start == end {
                return Err(CaptureError::NoSelection);
            }
            Ok(Capture { source: field.text[start..end].into(), app: "Fake".into(), target: (start, end) })
        }

        fn still_targeted(&self, target: &Self::Target, source: &str) -> bool {
            let field = self.field.lock().unwrap();
            field.focused && field.selection == *target && field.text.get(target.0..target.1) == Some(source)
        }

        fn replace(&self, target: &Self::Target, _source: &str, replacement: &str) -> Option<Self::Applied> {
            if self.refuse_replace {
                return None;
            }
            let mut field = self.field.lock().unwrap();
            field.text.replace_range(target.0..target.1, replacement);
            Some((target.0, target.0 + replacement.len()))
        }

        fn restore(&self, applied: &Self::Applied, replacement: &str, original: &str) -> bool {
            let mut field = self.field.lock().unwrap();
            if !field.focused || field.text.get(applied.0..applied.1) != Some(replacement) {
                return false;
            }
            field.text.replace_range(applied.0..applied.1, original);
            true
        }

        fn copy(&self, text: &str) {
            *self.clipboard.lock().unwrap() = text.into();
        }
    }

    struct FakeRewriter {
        delay: Duration,
        answer: Result<String, AiError>,
    }

    impl Rewriter for FakeRewriter {
        fn rewrite<'a>(&'a self, _text: &'a str, _profile: ProfileId, _custom: &'a str) -> BoxFuture<'a, Result<String, AiError>> {
            Box::pin(async move {
                tokio::time::sleep(self.delay).await;
                self.answer.clone()
            })
        }
    }

    #[derive(Default)]
    struct RecordingHooks {
        statuses: Mutex<Vec<Status>>,
        panel_shown: AtomicBool,
    }

    impl Hooks for RecordingHooks {
        fn status_changed(&self, status: &Status) {
            self.statuses.lock().unwrap().push(status.clone());
        }
        fn show_panel(&self) {
            self.panel_shown.store(true, Ordering::SeqCst);
        }
        fn hide_panel(&self) {}
        fn cancel_shortcut(&self, _active: bool) {}
        fn alert(&self) {}
    }

    fn engine(platform: FakePlatform, answer: Result<&str, AiError>, delay_ms: u64) -> (Arc<Engine<FakePlatform>>, Arc<RecordingHooks>) {
        let hooks = Arc::new(RecordingHooks::default());
        let rewriter = Arc::new(FakeRewriter {
            delay: Duration::from_millis(delay_ms),
            answer: answer.map(String::from),
        });
        (Engine::new(platform, rewriter, hooks.clone()), hooks)
    }

    fn request() -> Request {
        Request { profile: ProfileId::Natural, custom: String::new() }
    }

    #[tokio::test]
    async fn replaces_the_selection() {
        let (engine, _) = engine(FakePlatform::with_text("Bonjour tout le monde !", (8, 21)), Ok("à tous"), 0);
        assert!(engine.clone().run(request()).await);
        assert_eq!(engine.platform().text(), "Bonjour à tous !");
        assert_eq!(engine.status().phase, Phase::Done);
        assert!(engine.status().can_restore);
    }

    #[tokio::test]
    async fn no_selection_is_reported_and_nothing_changes() {
        let (engine, _) = engine(FakePlatform::with_text("Bonjour", (3, 3)), Ok("x"), 0);
        engine.clone().run(request()).await;
        assert_eq!(engine.status().error, Some(ErrorCode::NoSelection));
        assert_eq!(engine.platform().text(), "Bonjour");
    }

    #[tokio::test]
    async fn missing_permission_opens_the_panel() {
        let platform = FakePlatform { untrusted: true, ..FakePlatform::with_text("Bonjour", (0, 7)) };
        let (engine, hooks) = engine(platform, Ok("Salut"), 0);
        engine.clone().run(request()).await;
        assert_eq!(engine.status().error, Some(ErrorCode::Permission));
        assert!(hooks.panel_shown.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn errors_and_empty_answers_never_replace() {
        for answer in [Err(AiError::Network), Err(AiError::Truncated), Ok("   ")] {
            let (engine, _) = engine(FakePlatform::with_text("Bonjour", (0, 7)), answer, 0);
            engine.clone().run(request()).await;
            assert_eq!(engine.platform().text(), "Bonjour");
            assert_eq!(engine.status().phase, Phase::Error);
        }
    }

    #[tokio::test]
    async fn too_long_selection_is_refused_before_any_call() {
        let long = "a".repeat(MAX_INPUT_CHARS + 1);
        let (engine, _) = engine(FakePlatform::with_text(&long, (0, long.len())), Ok("b"), 0);
        engine.clone().run(request()).await;
        assert_eq!(engine.status().error, Some(ErrorCode::TooLong));
    }

    #[tokio::test]
    async fn second_trigger_during_processing_is_ignored() {
        let (engine, _) = engine(FakePlatform::with_text("Bonjour", (0, 7)), Ok("Salut"), 80);
        let (first, second) = tokio::join!(engine.clone().run(request()), async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            engine.clone().run(request()).await
        });
        assert!(first);
        assert!(!second);
        assert_eq!(engine.platform().text(), "Salut");
    }

    #[tokio::test]
    async fn cancelled_operation_drops_the_late_answer() {
        let (engine, _) = engine(FakePlatform::with_text("Bonjour", (0, 7)), Ok("Salut"), 150);
        let run = tokio::spawn(engine.clone().run(request()));
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(engine.cancel());
        run.await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(engine.platform().text(), "Bonjour");
        assert_eq!(engine.status().phase, Phase::Cancelled);
        assert!(!engine.is_busy());
    }

    #[tokio::test]
    async fn text_edited_during_the_request_is_not_overwritten() {
        let (engine, hooks) = engine(FakePlatform::with_text("Bonjour", (0, 7)), Ok("Salut"), 60);
        let run = tokio::spawn(engine.clone().run(request()));
        tokio::time::sleep(Duration::from_millis(20)).await;
        engine.platform().field.lock().unwrap().text = "Bonsoir".into();
        run.await.unwrap();
        assert_eq!(engine.platform().text(), "Bonsoir");
        let status = engine.status();
        assert_eq!(status.phase, Phase::Review);
        assert_eq!(status.error, Some(ErrorCode::TargetChanged));
        assert_eq!(status.result.as_deref(), Some("Salut"));
        assert!(hooks.panel_shown.load(Ordering::SeqCst));
        assert!(engine.copy(false));
        assert_eq!(*engine.platform().clipboard.lock().unwrap(), "Salut");
    }

    #[tokio::test]
    async fn focus_change_during_the_request_offers_a_copy() {
        let (engine, _) = engine(FakePlatform::with_text("Bonjour", (0, 7)), Ok("Salut"), 60);
        let run = tokio::spawn(engine.clone().run(request()));
        tokio::time::sleep(Duration::from_millis(20)).await;
        engine.platform().field.lock().unwrap().focused = false;
        run.await.unwrap();
        assert_eq!(engine.platform().text(), "Bonjour");
        assert_eq!(engine.status().error, Some(ErrorCode::TargetChanged));
    }

    #[tokio::test]
    async fn replace_failure_offers_a_copy() {
        let platform = FakePlatform { refuse_replace: true, ..FakePlatform::with_text("Bonjour", (0, 7)) };
        let (engine, _) = engine(platform, Ok("Salut"), 0);
        engine.clone().run(request()).await;
        assert_eq!(engine.status().phase, Phase::Review);
        assert_eq!(engine.status().error, Some(ErrorCode::ReplaceFailed));
    }

    #[tokio::test]
    async fn unchanged_text_is_left_alone() {
        let (engine, _) = engine(FakePlatform::with_text("Bonjour.", (0, 8)), Ok("Bonjour."), 0);
        engine.clone().run(request()).await;
        assert_eq!(engine.status().phase, Phase::Unchanged);
        assert!(!engine.status().can_restore);
    }

    #[tokio::test]
    async fn timeout_is_an_error() {
        let (engine, _) = engine(FakePlatform::with_text("Bonjour", (0, 7)), Ok("Salut"), 300);
        let engine = engine.with_deadline(Duration::from_millis(50));
        engine.clone().run(request()).await;
        assert_eq!(engine.status().error, Some(ErrorCode::Timeout));
        assert_eq!(engine.platform().text(), "Bonjour");
    }

    #[tokio::test]
    async fn restore_puts_the_original_back() {
        let (engine, _) = engine(FakePlatform::with_text("Bonjour tout le monde", (8, 21)), Ok("à tous"), 0);
        engine.clone().run(request()).await;
        assert_eq!(engine.platform().text(), "Bonjour à tous");
        assert!(engine.clone().restore().await);
        assert_eq!(engine.platform().text(), "Bonjour tout le monde");
        assert_eq!(engine.status().phase, Phase::Restored);
    }

    #[tokio::test]
    async fn restore_falls_back_to_copying_the_original() {
        let (engine, _) = engine(FakePlatform::with_text("Bonjour tout le monde", (8, 21)), Ok("à tous"), 0);
        engine.clone().run(request()).await;
        engine.platform().field.lock().unwrap().text = "autre chose".into();
        assert!(!engine.clone().restore().await);
        let status = engine.status();
        assert_eq!(status.error, Some(ErrorCode::RestoreFailed));
        assert!(status.has_original);
        assert!(!status.can_restore);
        assert!(engine.copy(true));
        assert_eq!(*engine.platform().clipboard.lock().unwrap(), "tout le monde");
    }
}
