//! Fournisseur IA derrière une interface minimale : un texte et un profil en
//! entrée, un texte en sortie. Changer de modèle ou de fournisseur, c'est
//! écrire une autre implémentation de `Rewriter`.

pub mod gemini;
pub mod mock;
pub mod openai;
pub mod openrouter;
pub mod prompt;

use std::future::Future;
use std::pin::Pin;

use serde::Serialize;

use crate::profiles::ProfileId;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Taille maximale d'une sélection envoyée : ~1 500 mots. Au-delà, ce n'est
/// plus un message à retoucher, et le délai comme le coût dérapent.
pub const MAX_INPUT_CHARS: usize = 8_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum AiError {
    #[error("aucune clé API")]
    MissingKey,
    #[error("clé API refusée")]
    InvalidKey,
    #[error("quota atteint")]
    Quota,
    #[error("réseau indisponible")]
    Network,
    #[error("délai dépassé")]
    Timeout,
    #[error("réponse bloquée par le fournisseur")]
    Blocked,
    #[error("réponse tronquée")]
    Truncated,
    #[error("réponse vide")]
    Empty,
    #[error("réponse inutilisable")]
    Unusable,
    #[error("erreur du fournisseur")]
    Provider,
    /// Erreur temporaire côté serveur (5xx) : la seule qu'on retente.
    #[error("service momentanément indisponible")]
    Unavailable,
}

impl AiError {
    pub fn is_retryable(self) -> bool {
        matches!(self, AiError::Unavailable | AiError::Network)
    }
}

pub trait Rewriter: Send + Sync {
    /// Appelé dès le raccourci, pendant la lecture de la sélection : ouvre la
    /// connexion à l'avance.
    fn prepare(&self) -> BoxFuture<'_, ()> {
        Box::pin(async {})
    }

    /// Renvoie la réponse brute du modèle ; le contrôle (`prompt::finalize`)
    /// est commun à tous les fournisseurs.
    fn rewrite<'a>(
        &'a self,
        text: &'a str,
        profile: ProfileId,
        custom: &'a str,
    ) -> BoxFuture<'a, Result<String, AiError>>;
}

use std::time::Duration;

use std::sync::RwLock;

use gemini::Gemini;
use openai::OpenAiCompatible;
use openrouter::OpenRouter;

/// Une seule nouvelle tentative, sur erreur temporaire uniquement.
const MAX_ATTEMPTS: u32 = 2;
const RETRY_DELAY: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Cerebras,
    Groq,
    Gemini,
    Qwen,
    QwenTokenPlan,
    OpenRouter,
}

/// Le fournisseur se déduit de la clé : pas de réglage à choisir.
pub fn detect(key: &str) -> Option<ProviderKind> {
    let key = key.trim();
    if key.starts_with("csk-") {
        Some(ProviderKind::Cerebras)
    } else if key.starts_with("gsk_") {
        Some(ProviderKind::Groq)
    } else if key.starts_with("AIza") {
        Some(ProviderKind::Gemini)
    } else if key.starts_with("sk-or-") {
        Some(ProviderKind::OpenRouter)
    } else if key.starts_with("sk-sp-") {
        Some(ProviderKind::QwenTokenPlan)
    } else if key.starts_with("sk-") {
        Some(ProviderKind::Qwen)
    } else {
        None
    }
}

pub struct Router {
    cerebras: OpenAiCompatible,
    groq: OpenAiCompatible,
    gemini: Gemini,
    qwen: OpenAiCompatible,
    qwen_token_plan: OpenAiCompatible,
    pub openrouter: OpenRouter,
    /// Modèle OpenRouter choisi dans les réglages.
    openrouter_model: RwLock<String>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            cerebras: OpenAiCompatible::cerebras(),
            groq: OpenAiCompatible::groq(),
            gemini: Gemini::new(),
            qwen: OpenAiCompatible::qwen(false),
            qwen_token_plan: OpenAiCompatible::qwen(true),
            openrouter: OpenRouter::new(),
            openrouter_model: RwLock::new(openrouter::FALLBACK_MODEL.into()),
        }
    }

    pub fn set_openrouter_model(&self, model: &str) {
        if let Ok(mut current) = self.openrouter_model.write() {
            *current = model.to_owned();
        }
    }

    pub fn openrouter_model(&self) -> String {
        self.openrouter_model.read().map(|model| model.clone()).unwrap_or_default()
    }

    /// Fournisseur compatible OpenAI pour une clé (tous sauf Gemini).
    fn compatible(&self, kind: ProviderKind) -> Option<&OpenAiCompatible> {
        match kind {
            ProviderKind::Cerebras => Some(&self.cerebras),
            ProviderKind::Groq => Some(&self.groq),
            ProviderKind::Qwen => Some(&self.qwen),
            ProviderKind::QwenTokenPlan => Some(&self.qwen_token_plan),
            ProviderKind::Gemini | ProviderKind::OpenRouter => None,
        }
    }

    pub fn model(&self, key: Option<&str>) -> String {
        match key.and_then(detect) {
            Some(ProviderKind::Gemini) => gemini::MODEL.into(),
            Some(ProviderKind::OpenRouter) => self.openrouter_model(),
            Some(kind) => self.compatible(kind).map_or(self.cerebras.model, |provider| provider.model).into(),
            None => self.cerebras.model.into(),
        }
    }

    pub async fn verify_key(&self, key: &str) -> Result<(), AiError> {
        let kind = detect(key).ok_or(AiError::InvalidKey)?;
        match (kind, self.compatible(kind)) {
            (_, Some(provider)) => provider.verify_key(key).await,
            (ProviderKind::OpenRouter, None) => self.openrouter.verify_key(key).await,
            _ => self.gemini.verify_key(key).await,
        }
    }

    async fn attempt(&self, key: &str, text: &str, profile: ProfileId, custom: &str) -> Result<String, AiError> {
        let kind = detect(key).ok_or(AiError::InvalidKey)?;
        match (kind, self.compatible(kind)) {
            (_, Some(provider)) => provider.rewrite(key, text, profile, custom).await,
            (ProviderKind::OpenRouter, None) => {
                let model = self.openrouter_model();
                self.openrouter.rewrite(key, &model, text, profile, custom).await
            }
            _ => self.gemini.rewrite(key, text, profile, custom).await,
        }
    }
}

impl Rewriter for Router {
    fn prepare(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let Some(key) = crate::secrets::api_key() else { return };
            let Some(kind) = detect(&key) else { return };
            match (kind, self.compatible(kind)) {
                (_, Some(provider)) => provider.warm_up(&key).await,
                (ProviderKind::OpenRouter, None) => self.openrouter.warm_up(&key).await,
                _ => {
                    let _ = self.gemini.verify_key(&key).await;
                }
            }
        })
    }

    fn rewrite<'a>(
        &'a self,
        text: &'a str,
        profile: ProfileId,
        custom: &'a str,
    ) -> BoxFuture<'a, Result<String, AiError>> {
        Box::pin(async move {
            let key = crate::secrets::api_key().ok_or(AiError::MissingKey)?;
            let mut attempt = 1;
            loop {
                match self.attempt(&key, text, profile, custom).await {
                    Err(error) if error.is_retryable() && attempt < MAX_ATTEMPTS => {
                        log::info!("nouvelle tentative après {error:?}");
                        attempt += 1;
                        tokio::time::sleep(RETRY_DELAY).await;
                    }
                    result => return result,
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_is_detected_from_the_key() {
        assert_eq!(detect("csk-abc"), Some(ProviderKind::Cerebras));
        assert_eq!(detect("gsk_abc"), Some(ProviderKind::Groq));
        assert_eq!(detect(" AIzaSy "), Some(ProviderKind::Gemini));
        assert_eq!(detect("sk-or-v1-abc"), Some(ProviderKind::OpenRouter));
        assert_eq!(detect("sk-abc"), Some(ProviderKind::Qwen));
        assert_eq!(detect("sk-sp-abc"), Some(ProviderKind::QwenTokenPlan));
        assert_eq!(detect("inconnue"), None);
    }
}
