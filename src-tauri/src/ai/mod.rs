//! IA via OpenRouter, derrière une interface minimale : un texte et un profil en
//! entrée, un texte en sortie. Changer de modèle ou de fournisseur, c'est
//! écrire une autre implémentation de `Rewriter`.

pub mod mock;
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

use std::sync::RwLock;
use std::time::Duration;

use openrouter::OpenRouter;

/// Une seule nouvelle tentative, sur erreur temporaire uniquement.
const MAX_ATTEMPTS: u32 = 2;
const RETRY_DELAY: Duration = Duration::from_millis(300);

/// Une clé OpenRouter : `sk-or-…`.
pub fn is_openrouter_key(key: &str) -> bool {
    key.trim().starts_with("sk-or-")
}

/// Le fournisseur : OpenRouter, avec le modèle choisi dans les réglages.
pub struct Router {
    pub openrouter: OpenRouter,
    model: RwLock<String>,
}

impl Router {
    pub fn new() -> Self {
        Self { openrouter: OpenRouter::new(), model: RwLock::new(openrouter::FALLBACK_MODEL.into()) }
    }

    pub fn set_model(&self, model: &str) {
        if let Ok(mut current) = self.model.write() {
            *current = model.to_owned();
        }
    }

    pub fn model(&self) -> String {
        self.model.read().map(|model| model.clone()).unwrap_or_default()
    }

    pub async fn verify_key(&self, key: &str) -> Result<(), AiError> {
        if !is_openrouter_key(key) {
            return Err(AiError::InvalidKey);
        }
        self.openrouter.verify_key(key).await
    }
}

impl Rewriter for Router {
    fn prepare(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            if let Some(key) = crate::secrets::api_key() {
                self.openrouter.warm_up(&key).await;
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
            let model = self.model();
            let mut attempt = 1;
            loop {
                match self.openrouter.rewrite(&key, &model, text, profile, custom).await {
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
    fn only_openrouter_keys_are_accepted() {
        assert!(is_openrouter_key(" sk-or-v1-abc "));
        assert!(!is_openrouter_key("sk-abc"));
        assert!(!is_openrouter_key("AIzaSy"));
    }
}
