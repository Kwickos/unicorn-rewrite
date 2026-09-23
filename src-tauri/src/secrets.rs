//! La clé API vit dans le Trousseau macOS, nulle part ailleurs : ni fichier
//! de réglages, ni localStorage, ni journal. Le panneau ne la relit jamais ;
//! il sait seulement si une clé est enregistrée.

use std::sync::Mutex;

const SERVICE: &str = "com.digitalunicorn.unicorn-rewrite";
const ACCOUNT: &str = "api-key";
/// Compte de la première version (clé Gemini), toujours lu.
const LEGACY_ACCOUNT: &str = "gemini-api-key";

/// Copie en mémoire, pour ne pas interroger le Trousseau à chaque appel.
static CACHE: Mutex<Option<String>> = Mutex::new(None);

#[cfg(target_os = "macos")]
fn read_keychain() -> Option<String> {
    let bytes = security_framework::passwords::get_generic_password(SERVICE, ACCOUNT)
        .or_else(|_| security_framework::passwords::get_generic_password(SERVICE, LEGACY_ACCOUNT))
        .ok()?;
    String::from_utf8(bytes).ok().filter(|key| !key.is_empty())
}

#[cfg(not(target_os = "macos"))]
fn read_keychain() -> Option<String> {
    None
}

pub fn api_key() -> Option<String> {
    let mut cache = CACHE.lock().ok()?;
    if cache.is_none() {
        *cache = read_keychain();
    }
    cache.clone()
}

pub fn has_api_key() -> bool {
    api_key().is_some()
}

#[cfg(target_os = "macos")]
pub fn set_api_key(key: &str) -> Result<(), String> {
    security_framework::passwords::set_generic_password(SERVICE, ACCOUNT, key.as_bytes())
        .map_err(|error| format!("Trousseau : {}", error.code()))?;
    // Une seule clé à la fois : l'ancienne entrée ne doit pas reprendre le dessus.
    let _ = security_framework::passwords::delete_generic_password(SERVICE, LEGACY_ACCOUNT);
    if let Ok(mut cache) = CACHE.lock() {
        *cache = Some(key.to_owned());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn delete_api_key() -> Result<(), String> {
    let _ = security_framework::passwords::delete_generic_password(SERVICE, LEGACY_ACCOUNT);
    match security_framework::passwords::delete_generic_password(SERVICE, ACCOUNT) {
        // -25300 : errSecItemNotFound, déjà absente.
        Err(error) if error.code() != -25300 => {
            return Err(format!("Trousseau : {}", error.code()));
        }
        _ => {}
    }
    if let Ok(mut cache) = CACHE.lock() {
        *cache = None;
    }
    Ok(())
}

