//! Ce que le moteur attend du système : lire la sélection de l'app au premier
//! plan, vérifier qu'elle n'a pas bougé, la remplacer, la restaurer.
//!
//! L'implémentation réelle est `macos::MacPlatform` ; les tests du moteur en
//! utilisent une fausse, pour rejouer les cas qui peuvent perdre du texte.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureError {
    /// Rien n'est sélectionné (ou la copie n'a rien produit).
    NoSelection,
    /// Champ de mot de passe ou saisie sécurisée active.
    SecureField,
    /// Le focus est dans notre propre panneau.
    SelfTarget,
    /// L'app ne laisse lire ni sa sélection ni son presse-papiers.
    ReadFailed,
}

/// Une sélection lue, et de quoi retrouver sa cible.
pub struct Capture<T> {
    pub source: String,
    /// Nom de l'app, pour l'état affiché dans le panneau.
    pub app: String,
    pub target: T,
}

pub trait Platform: Send + Sync + 'static {
    /// Tout ce qu'il faut pour revenir à la cible : app, champ, sélection.
    type Target: Send + Sync + 'static;
    /// Trace d'un remplacement effectué, pour pouvoir le défaire.
    type Applied: Send + Sync + 'static;

    fn is_trusted(&self) -> bool;

    /// Lit la sélection de l'app au premier plan, sans toucher au focus.
    fn capture(&self) -> Result<Capture<Self::Target>, CaptureError>;

    /// Même app, même champ, même sélection, même texte qu'au départ ? En
    /// cas de doute, la réponse est non.
    fn still_targeted(&self, target: &Self::Target, source: &str) -> bool;

    /// Remplace la sélection. `None` si rien n'a pu être appliqué.
    fn replace(&self, target: &Self::Target, source: &str, replacement: &str) -> Option<Self::Applied>;

    /// Remet le texte d'origine à la place du remplacement, si la cible est
    /// encore identifiable.
    fn restore(&self, applied: &Self::Applied, replacement: &str, original: &str) -> bool;

    /// Copie demandée par l'utilisateur depuis le panneau.
    fn copy(&self, text: &str);
}
