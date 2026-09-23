//! OpenRouter : une clé, tous les modèles récents.
//!
//! - Le catalogue (`/models`) donne date de sortie, prix, date de retrait et
//!   si le raisonnement peut être coupé ; la vitesse mesurée par hébergeur
//!   (`/models/{id}/endpoints`) n'est visible qu'avec une clé.
//! - Chaque requête part chez l'hébergeur le plus rapide du modèle choisi
//!   (`provider.sort: throughput`), raisonnement coupé.
//! - Le modèle ne change jamais au hasard : seulement pour la version plus
//!   récente de la même famille (`qwen3.8-flash` → `qwen3.9-flash`), ou quand
//!   il est retiré.

use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Value};

use super::{openai, prompt, AiError};
use crate::profiles::ProfileId;

const API: &str = "https://openrouter.ai/api/v1";
/// Modèle de départ si le catalogue est injoignable.
pub const FALLBACK_MODEL: &str = "qwen/qwen3.8-flash";
/// Au-delà, un modèle n'est plus « récent » pour le catalogue.
const MAX_AGE_DAYS: u64 = 540;
/// « Rapide » : dans les N premiers du classement par débit, et dans les M
/// premiers par délai de première réponse (sur ~450 modèles).
const FAST_THROUGHPUT_RANK: usize = 120;
const FAST_LATENCY_RANK: usize = 200;
const CATALOG_TTL: Duration = Duration::from_secs(10 * 60);
/// Reformulation type, pour estimer un coût lisible : consignes + message
/// d'une centaine de mots, et sa réécriture.
const TYPICAL_INPUT_TOKENS: f64 = 800.0;
const TYPICAL_OUTPUT_TOKENS: f64 = 300.0;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    /// Sortie, en secondes Unix.
    pub created: u64,
    /// Coût estimé d'une reformulation type, en centimes de dollar.
    pub cost_cents: f64,
    /// Parmi les plus rapides d'OpenRouter, en débit comme en délai de
    /// première réponse (classements publics, mis à jour en continu).
    pub fast: bool,
    /// Débit mesuré par OpenRouter (jetons/s, médiane sur 30 min, meilleur
    /// hébergeur). Publié seulement aux requêtes authentifiées, et pas
    /// toujours : absent, rien n'est affiché.
    pub throughput: Option<f64>,
    /// Le raisonnement ne peut pas être coupé : plus lent.
    pub reasoning_mandatory: bool,
    #[serde(skip)]
    pub expires: Option<String>,
}

pub struct OpenRouter {
    client: reqwest::Client,
    catalog: Mutex<Option<(Instant, Vec<CatalogModel>)>>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default()
}

fn price(value: &Value, field: &str) -> Option<f64> {
    value.pointer(&format!("/pricing/{field}"))?.as_str()?.parse().ok()
}

/// Modèles utilisables pour réécrire du texte, d'après la fiche du catalogue.
fn parse_model(value: &Value) -> Option<CatalogModel> {
    let id = value.get("id")?.as_str()?.to_owned();
    // Variantes gratuites : limitées et lentes. Alias « ~…-latest » : ils
    // changent de modèle sans prévenir, ce qu'on ne veut justement pas.
    if id.contains(':') || id.starts_with('~') {
        return None;
    }
    let text_only = |field: &str| {
        value
            .pointer(&format!("/architecture/{field}"))
            .and_then(Value::as_array)
            .is_some_and(|modalities| modalities.iter().any(|m| m == "text"))
    };
    if !text_only("input_modalities") || !text_only("output_modalities") {
        return None;
    }
    let output = value
        .pointer("/architecture/output_modalities")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if output != 1 {
        return None;
    }
    let created = value.get("created")?.as_u64()?;
    if now_secs().saturating_sub(created) > MAX_AGE_DAYS * 86_400 {
        return None;
    }
    let (input, completion) = (price(value, "prompt")?, price(value, "completion")?);
    if input < 0.0 || completion <= 0.0 {
        return None; // prix variables (-1) ou routeurs
    }
    let reasoning_mandatory = value
        .pointer("/reasoning/mandatory")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(CatalogModel {
        name: value.get("name").and_then(Value::as_str).unwrap_or(&id).to_owned(),
        id,
        created,
        cost_cents: (input * TYPICAL_INPUT_TOKENS + completion * TYPICAL_OUTPUT_TOKENS) * 100.0,
        fast: false,
        throughput: None,
        reasoning_mandatory,
        expires: value.get("expiration_date").and_then(Value::as_str).map(str::to_owned),
    })
}

/// Famille d'un modèle : son identifiant sans numéro de version ni date,
/// mais avec sa taille (`27b`) et son palier (`flash`, `lite`). Deux modèles
/// de la même famille sont interchangeables, le plus récent en mieux.
pub fn family(id: &str) -> String {
    let id = id.to_lowercase();
    let mut out = String::with_capacity(id.len());
    let bytes: Vec<char> = id.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == '.') {
                i += 1;
            }
            // Une taille (27b, 2.4t, 120b) fait partie de la famille.
            if i < bytes.len() && matches!(bytes[i], 'b' | 't' | 'm') && bytes.get(i + 1).is_none_or(|c| !c.is_ascii_alphabetic()) {
                out.extend(&bytes[start..=i]);
                i += 1;
            } else {
                out.push('#');
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    // Suffixes de date ou de révision : « -0902 », « -20260902 ».
    while let Some(stripped) = out.strip_suffix("-#") {
        out = stripped.to_owned();
    }
    out
}

/// Le successeur à adopter pour `current`, s'il y en a un : même famille,
/// plus récent, pas plus lent à cause d'un raisonnement obligatoire, et pas
/// plus de deux fois plus cher. Aussi quand `current` a disparu ou expire.
pub fn successor<'a>(current: &str, catalog: &'a [CatalogModel]) -> Option<&'a CatalogModel> {
    let key = family(current);
    let present = catalog.iter().find(|model| model.id == current);
    let current_created = present.map(|model| model.created).unwrap_or(0);
    let current_cost = present.map(|model| model.cost_cents);
    let expiring = present.is_none_or(|model| model.expires.is_some());
    catalog
        .iter()
        .filter(|model| model.id != current && family(&model.id) == key)
        .filter(|model| !model.reasoning_mandatory && model.expires.is_none())
        .filter(|model| model.created > current_created || expiring)
        .filter(|model| current_cost.is_none_or(|cost| model.cost_cents <= cost * 2.0 + 0.001))
        .max_by_key(|model| model.created)
}

impl OpenRouter {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(300))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .expect("client HTTP");
        Self { client, catalog: Mutex::new(None) }
    }

    fn request(&self, builder: reqwest::RequestBuilder, key: &str) -> reqwest::RequestBuilder {
        builder
            .bearer_auth(key)
            .header("X-OpenRouter-Title", "Unicorn Rewrite")
            .header("HTTP-Referer", "https://github.com/digital-unicorn/unicorn-rewrite")
    }

    pub async fn verify_key(&self, key: &str) -> Result<(), AiError> {
        let response = self
            .request(self.client.get(format!("{API}/key")), key)
            .send()
            .await
            .map_err(|error| if error.is_timeout() { AiError::Timeout } else { AiError::Network })?;
        match response.status().as_u16() {
            200 => Ok(()),
            status => Err(openai::map_status(status)),
        }
    }

    pub async fn warm_up(&self, key: &str) {
        let _ = self.verify_key(key).await;
    }

    pub fn request_body(model: &str, reasoning_mandatory: bool, text: &str, profile: ProfileId, custom: &str) -> Value {
        let prompt = prompt::build(text, profile, custom);
        let reasoning = if reasoning_mandatory {
            json!({ "effort": "minimal", "exclude": true })
        } else {
            json!({ "effort": "none" })
        };
        json!({
            "model": model,
            "messages": [
                { "role": "system", "content": prompt.system },
                { "role": "user", "content": prompt.user }
            ],
            "temperature": prompt.temperature,
            "max_tokens": prompt::output_budget(text),
            "reasoning": reasoning,
            // Hébergeur le plus rapide pour ce modèle.
            "provider": { "sort": "throughput" },
            "stream": false
        })
    }

    pub async fn rewrite(&self, key: &str, model: &str, text: &str, profile: ProfileId, custom: &str) -> Result<String, AiError> {
        let mandatory = self
            .cached()
            .and_then(|catalog| catalog.into_iter().find(|entry| entry.id == model))
            .is_some_and(|entry| entry.reasoning_mandatory);
        let body = Self::request_body(model, mandatory, text, profile, custom);
        let response = self
            .request(self.client.post(format!("{API}/chat/completions")), key)
            .json(&body)
            .send()
            .await
            .map_err(|error| if error.is_timeout() { AiError::Timeout } else { AiError::Network })?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|_| AiError::Network)?;
        if status != 200 {
            return Err(openai::map_status(status));
        }
        let value: Value = serde_json::from_str(&text).map_err(|_| AiError::Provider)?;
        // OpenRouter peut renvoyer une erreur d'hébergeur dans un 200.
        if value.get("error").is_some() {
            return Err(AiError::Unavailable);
        }
        openai::parse_response(&value)
    }

    fn cached(&self) -> Option<Vec<CatalogModel>> {
        let guard = self.catalog.lock().ok()?;
        guard
            .as_ref()
            .filter(|(at, _)| at.elapsed() < CATALOG_TTL)
            .map(|(_, models)| models.clone())
    }

    /// Catalogue des modèles récents, avec la vitesse mesurée des plus
    /// récents. Mis en cache dix minutes.
    pub async fn catalog(&self, key: &str) -> Result<Vec<CatalogModel>, AiError> {
        if let Some(models) = self.cached() {
            return Ok(models);
        }
        let response = self
            .request(self.client.get(format!("{API}/models")), key)
            .send()
            .await
            .map_err(|_| AiError::Network)?;
        if !response.status().is_success() {
            return Err(openai::map_status(response.status().as_u16()));
        }
        let value: Value = response.json().await.map_err(|_| AiError::Provider)?;
        let mut models: Vec<CatalogModel> = value
            .get("data")
            .and_then(Value::as_array)
            .map(|models| models.iter().filter_map(parse_model).collect())
            .unwrap_or_default();
        models.sort_by(|a, b| b.created.cmp(&a.created));

        // Vitesse : classements publics d'OpenRouter.
        let (throughput, latency) =
            tokio::join!(self.ranking(key, "throughput-high-to-low"), self.ranking(key, "latency-low-to-high"));
        let rank = |ranking: &[String], id: &str| ranking.iter().position(|entry| entry == id);
        for model in &mut models {
            model.fast = !model.reasoning_mandatory
                && rank(&throughput, &model.id).is_some_and(|rank| rank < FAST_THROUGHPUT_RANK)
                && rank(&latency, &model.id).is_some_and(|rank| rank < FAST_LATENCY_RANK);
        }

        // Débit chiffré des modèles rapides, en parallèle.
        let mut speeds = tokio::task::JoinSet::new();
        for model in models.iter().filter(|model| model.fast) {
            let (client, key, id) = (self.client.clone(), key.to_owned(), model.id.clone());
            speeds.spawn(async move {
                let throughput = measured_throughput(&client, &key, &id).await;
                (id, throughput)
            });
        }
        while let Some(Ok((id, throughput))) = speeds.join_next().await {
            if let Some(model) = models.iter_mut().find(|model| model.id == id) {
                model.throughput = throughput;
            }
        }

        if let Ok(mut cache) = self.catalog.lock() {
            *cache = Some((Instant::now(), models.clone()));
        }
        Ok(models)
    }

    /// Identifiants du catalogue dans l'ordre d'un classement.
    async fn ranking(&self, key: &str, sort: &str) -> Vec<String> {
        let Ok(response) = self.request(self.client.get(format!("{API}/models?sort={sort}")), key).send().await else {
            return Vec::new();
        };
        let Ok(value) = response.json::<Value>().await else { return Vec::new() };
        value
            .get("data")
            .and_then(Value::as_array)
            .map(|models| {
                models
                    .iter()
                    .filter_map(|model| model.get("id").and_then(Value::as_str).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Meilleur débit médian parmi les hébergeurs d'un modèle.
async fn measured_throughput(client: &reqwest::Client, key: &str, id: &str) -> Option<f64> {
    let response = client
        .get(format!("{API}/models/{id}/endpoints"))
        .bearer_auth(key)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    let value: Value = response.json().await.ok()?;
    value
        .pointer("/data/endpoints")?
        .as_array()?
        .iter()
        .filter_map(|endpoint| endpoint.pointer("/throughput_last_30m/p50")?.as_f64())
        .max_by(f64::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str, created: u64, cost: f64) -> CatalogModel {
        CatalogModel {
            id: id.into(),
            name: id.into(),
            created,
            cost_cents: cost,
            fast: false,
            throughput: None,
            reasoning_mandatory: false,
            expires: None,
        }
    }

    #[test]
    fn family_ignores_versions_and_dates_but_keeps_size_and_tier() {
        assert_eq!(family("qwen/qwen3.8-flash"), family("qwen/qwen3.9-flash"));
        assert_eq!(family("qwen/qwen3.8-max-0902"), family("qwen/qwen3.8-max"));
        assert_eq!(family("google/gemini-3.5-flash-lite"), family("google/gemini-4-flash-lite"));
        assert_ne!(family("google/gemini-3.5-flash-lite"), family("google/gemini-3.5-flash"));
        assert_ne!(family("openai/gpt-oss-20b"), family("openai/gpt-oss-120b"));
        assert_ne!(family("qwen/qwen3.8-27b"), family("qwen/qwen3.8-flash"));
    }

    #[test]
    fn upgrades_only_within_the_family() {
        let catalog = vec![
            model("qwen/qwen3.8-flash", 100, 0.02),
            model("qwen/qwen3.9-flash", 200, 0.03),
            model("qwen/qwen3.9-max", 300, 0.5),
            model("mistral/ministral-9b", 400, 0.01),
        ];
        assert_eq!(successor("qwen/qwen3.8-flash", &catalog).unwrap().id, "qwen/qwen3.9-flash");
        assert!(successor("qwen/qwen3.9-flash", &catalog).is_none());
        assert!(successor("mistral/ministral-9b", &catalog).is_none());
    }

    #[test]
    fn a_much_pricier_successor_is_not_adopted() {
        let catalog = vec![model("x/y1-flash", 100, 0.02), model("x/y2-flash", 200, 0.5)];
        assert!(successor("x/y1-flash", &catalog).is_none());
    }

    #[test]
    fn a_removed_model_falls_back_to_its_family() {
        let catalog = vec![model("qwen/qwen3.9-flash", 200, 0.03)];
        assert_eq!(successor("qwen/qwen3.8-flash", &catalog).unwrap().id, "qwen/qwen3.9-flash");
    }

    #[test]
    fn catalog_keeps_recent_text_models_only() {
        let recent = now_secs() - 86_400;
        let text = json!({
            "id": "qwen/qwen3.8-flash", "name": "Qwen: Qwen3.8 Flash", "created": recent,
            "architecture": { "input_modalities": ["text"], "output_modalities": ["text"] },
            "pricing": { "prompt": "0.00000015", "completion": "0.00000047" },
            "reasoning": { "mandatory": false }
        });
        let parsed = parse_model(&text).unwrap();
        assert!((parsed.cost_cents - 0.0261).abs() < 0.001);
        let free = json!({ "id": "qwen/qwen3.8-27b:free", "created": recent });
        assert!(parse_model(&free).is_none());
        let alias = json!({ "id": "~deepseek/deepseek-flash-latest", "created": recent });
        assert!(parse_model(&alias).is_none());
        let old = json!({
            "id": "old/model", "created": 1_000_000,
            "architecture": { "input_modalities": ["text"], "output_modalities": ["text"] },
            "pricing": { "prompt": "0.0000001", "completion": "0.0000001" }
        });
        assert!(parse_model(&old).is_none());
    }

    #[test]
    fn request_routes_to_the_fastest_host_without_reasoning() {
        let body = OpenRouter::request_body("qwen/qwen3.8-flash", false, "Bonjour", ProfileId::Correct, "");
        assert_eq!(body["provider"]["sort"], "throughput");
        assert_eq!(body["reasoning"]["effort"], "none");
    }
}
