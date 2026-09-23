//! Fournisseur simulé : sert à valider le parcours natif (sélection →
//! traitement → remplacement) sans clé ni réseau.
//!
//! Actif seulement en développement, avec `UNICORN_REWRITE_MOCK=1`, et dans
//! l'auto-test.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::{AiError, BoxFuture, Rewriter};
use crate::profiles::ProfileId;

pub struct Mock {
    /// Latence simulée, en millisecondes : de quoi changer de fenêtre ou
    /// modifier le texte pendant « la requête ».
    delay_ms: AtomicU64,
}

impl Mock {
    pub fn new(delay: Duration) -> Self {
        Self { delay_ms: AtomicU64::new(delay.as_millis() as u64) }
    }

    pub fn set_delay(&self, delay: Duration) {
        self.delay_ms.store(delay.as_millis() as u64, Ordering::Relaxed);
    }

    /// Transformation visible et réversible à l'œil : « [naturel] texte ».
    pub fn transform(text: &str, profile: ProfileId) -> String {
        let tag = match profile {
            ProfileId::Correct => "corrigé",
            ProfileId::Natural => "naturel",
            ProfileId::Professional => "pro",
            ProfileId::Warm => "chaleureux",
            ProfileId::Concise => "concis",
            ProfileId::Custom => "perso",
        };
        format!("[{tag}] {}", text.trim())
    }

    pub fn enabled_by_env() -> bool {
        cfg!(debug_assertions) && std::env::var("UNICORN_REWRITE_MOCK").is_ok_and(|value| value == "1")
    }
}

impl Rewriter for Mock {
    fn rewrite<'a>(
        &'a self,
        text: &'a str,
        profile: ProfileId,
        _custom: &'a str,
    ) -> BoxFuture<'a, Result<String, AiError>> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(self.delay_ms.load(Ordering::Relaxed))).await;
            Ok(Self::transform(text, profile))
        })
    }
}
