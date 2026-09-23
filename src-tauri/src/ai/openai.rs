//! Fournisseurs compatibles avec l'API « chat completions » d'OpenAI :
//! Cerebras et Groq, choisis pour leur vitesse (inférence sur puces dédiées).
//!
//! Cerebras `qwen-3.8-27b`, raisonnement coupé : ~1 500–1 850 jetons/s
//! annoncés, bon niveau en français. Groq `openai/gpt-oss-20b` en secours.

use std::time::Duration;

use serde_json::{json, Value};

use super::{prompt, AiError};
use crate::profiles::ProfileId;

const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(8);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);

pub struct OpenAiCompatible {
    client: reqwest::Client,
    base: &'static str,
    pub model: &'static str,
    /// Paramètres propres au fournisseur (raisonnement coupé).
    extra: fn() -> Value,
}

impl OpenAiCompatible {
    fn new(base: &'static str, model: &'static str, extra: fn() -> Value) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(ATTEMPT_TIMEOUT)
            // Connexion gardée ouverte entre deux reformulations : on
            // économise la poignée de main TLS.
            .pool_idle_timeout(Duration::from_secs(300))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .expect("client HTTP");
        Self { client, base, model, extra }
    }

    pub fn cerebras() -> Self {
        Self::new("https://api.cerebras.ai/v1", "qwen-3.8-27b", || {
            json!({ "reasoning_effort": "none" })
        })
    }

    pub fn groq() -> Self {
        Self::new("https://api.groq.com/openai/v1", "openai/gpt-oss-20b", || {
            json!({ "reasoning_effort": "low", "include_reasoning": false })
        })
    }

    /// Qwen Cloud (Alibaba), `qwen3.8-flash`. Les clés « Token Plan »
    /// (`sk-sp-…`) ont leur propre hôte.
    pub fn qwen(token_plan: bool) -> Self {
        let base = if token_plan {
            "https://token-plan.maas.qwencloudapi.com/compatible-mode/v1"
        } else {
            "https://maas.qwencloudapi.com/compatible-mode/v1"
        };
        Self::new(base, "qwen3.8-flash", || json!({ "enable_thinking": false }))
    }

    pub async fn verify_key(&self, key: &str) -> Result<(), AiError> {
        let response = self
            .client
            .get(format!("{}/models", self.base))
            .bearer_auth(key)
            .send()
            .await
            .map_err(map_transport_error)?;
        match response.status().as_u16() {
            // 404 : liste des modèles non exposée, mais la clé a passé
            // l'authentification.
            200 | 404 => Ok(()),
            status => Err(map_status(status)),
        }
    }

    /// Préchauffe la connexion (DNS, TLS) sans consommer de jetons.
    pub async fn warm_up(&self, key: &str) {
        let _ = self.verify_key(key).await;
    }

    pub fn request_body(&self, text: &str, profile: ProfileId, custom: &str) -> Value {
        let prompt = prompt::build(text, profile, custom);
        let mut body = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": prompt.system },
                { "role": "user", "content": prompt.user }
            ],
            "temperature": prompt.temperature,
            "max_completion_tokens": prompt::output_budget(text),
            "stream": false
        });
        if let (Some(body), Value::Object(extra)) = (body.as_object_mut(), (self.extra)()) {
            body.extend(extra);
        }
        body
    }

    pub async fn rewrite(&self, key: &str, text: &str, profile: ProfileId, custom: &str) -> Result<String, AiError> {
        let body = self.request_body(text, profile, custom);
        let response = self
            .client
            .post(format!("{}/chat/completions", self.base))
            .bearer_auth(key)
            .json(&body)
            .send()
            .await
            .map_err(map_transport_error)?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(map_transport_error)?;
        if status != 200 {
            return Err(map_status(status));
        }
        let value: Value = serde_json::from_str(&text).map_err(|_| AiError::Provider)?;
        parse_response(&value)
    }
}

fn map_transport_error(error: reqwest::Error) -> AiError {
    if error.is_timeout() {
        AiError::Timeout
    } else {
        AiError::Network
    }
}

pub fn map_status(status: u16) -> AiError {
    match status {
        401 | 403 => AiError::InvalidKey,
        402 | 429 => AiError::Quota,
        408 | 500..=599 => AiError::Unavailable,
        _ => AiError::Provider,
    }
}

/// Seule une réponse terminée normalement (`stop`) est acceptée.
pub fn parse_response(value: &Value) -> Result<String, AiError> {
    let Some(choice) = value.pointer("/choices/0") else {
        return Err(AiError::Empty);
    };
    match choice.get("finish_reason").and_then(Value::as_str) {
        Some("stop") => {}
        Some("length") => return Err(AiError::Truncated),
        Some("content_filter") => return Err(AiError::Blocked),
        _ => return Err(AiError::Truncated),
    }
    let text = choice
        .pointer("/message/content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err(AiError::Empty);
    }
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cerebras_request_turns_reasoning_off() {
        let body = OpenAiCompatible::cerebras().request_body("Bonjour", ProfileId::Natural, "");
        assert_eq!(body["model"], "qwen-3.8-27b");
        assert_eq!(body["reasoning_effort"], "none");
        assert_eq!(body["messages"].as_array().unwrap().len(), 2, "aucun historique");
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn qwen_request_turns_thinking_off() {
        let body = OpenAiCompatible::qwen(false).request_body("Bonjour", ProfileId::Correct, "");
        assert_eq!(body["model"], "qwen3.8-flash");
        assert_eq!(body["enable_thinking"], false);
    }

    #[test]
    fn parses_answers_and_rejects_truncated_ones() {
        let ok = json!({ "choices": [{ "message": { "content": "Salut." }, "finish_reason": "stop" }] });
        assert_eq!(parse_response(&ok).unwrap(), "Salut.");
        let cut = json!({ "choices": [{ "message": { "content": "Sal" }, "finish_reason": "length" }] });
        assert_eq!(parse_response(&cut), Err(AiError::Truncated));
        assert_eq!(parse_response(&json!({})), Err(AiError::Empty));
        assert_eq!(map_status(401), AiError::InvalidKey);
        assert_eq!(map_status(429), AiError::Quota);
    }
}
