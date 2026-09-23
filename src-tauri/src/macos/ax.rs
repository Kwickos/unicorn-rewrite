//! Accès Accessibility : l'élément qui a le focus, sa sélection, son texte.
//!
//! Tout passe par des requêtes IPC vers l'app cible ; un délai court évite
//! qu'une app figée ne bloque l'opération.

#![allow(non_upper_case_globals)]

use std::ffi::c_void;
use std::ptr;

use core_foundation::array::CFArray;
use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;

use super::ffi::*;

/// Délai maximal d'une requête Accessibility vers une autre app.
const MESSAGING_TIMEOUT_SECONDS: f32 = 1.0;

/// Référence retenue sur un élément d'interface d'une autre app.
///
/// Deux références désignent le même champ si `CFEqual` le dit : c'est ainsi
/// qu'on vérifie, avant de remplacer, que le focus n'a pas bougé.
pub struct AxElement(AXUIElementRef);

// SAFETY : un AXUIElementRef est un objet Core Foundation immuable, et les
// fonctions AX sont utilisables depuis n'importe quel thread.
unsafe impl Send for AxElement {}
unsafe impl Sync for AxElement {}

impl Drop for AxElement {
    fn drop(&mut self) {
        // SAFETY : on possède une référence (Create/Copy ou Retain).
        unsafe { CFRelease(self.0) }
    }
}

impl Clone for AxElement {
    fn clone(&self) -> Self {
        // SAFETY : l'élément est valide tant que `self` vit.
        unsafe { CFRetain(self.0) };
        Self(self.0)
    }
}

impl PartialEq for AxElement {
    fn eq(&self, other: &Self) -> bool {
        // SAFETY : deux objets CF valides.
        unsafe { CFEqual(self.0, other.0) != 0 }
    }
}

impl std::fmt::Debug for AxElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AxElement({:p})", self.0)
    }
}

/// Erreur d'une requête Accessibility, telle que la renvoie l'app cible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxError(pub i32);

impl AxElement {
    /// Prend possession d'une référence obtenue par Create/Copy.
    fn owned(raw: AXUIElementRef) -> Option<Self> {
        (!raw.is_null()).then_some(Self(raw))
    }

    pub fn system_wide() -> Self {
        // SAFETY : fonction sans précondition ; renvoie une référence possédée.
        let element = unsafe { AXUIElementCreateSystemWide() };
        unsafe { AXUIElementSetMessagingTimeout(element, MESSAGING_TIMEOUT_SECONDS) };
        Self(element)
    }

    fn copy_attribute(&self, name: &str) -> Result<Option<CFType>, AxError> {
        let attribute = CFString::new(name);
        let mut value: CFTypeRef = ptr::null();
        // SAFETY : élément valide, attribut CFString valide, sortie non nulle.
        let status = unsafe {
            AXUIElementCopyAttributeValue(self.0, attribute.as_concrete_TypeRef(), &mut value)
        };
        match status {
            kAXErrorSuccess if value.is_null() => Ok(None),
            // SAFETY : « Copy » → on possède la valeur renvoyée.
            kAXErrorSuccess => Ok(Some(unsafe { CFType::wrap_under_create_rule(value) })),
            kAXErrorNoValue => Ok(None),
            other => Err(AxError(other)),
        }
    }

    pub fn element_attribute(&self, name: &str) -> Result<Option<AxElement>, AxError> {
        let Some(value) = self.copy_attribute(name)? else {
            return Ok(None);
        };
        // SAFETY : on vérifie le type avant de réinterpréter la référence.
        if value.type_of() as usize != unsafe { AXUIElementGetTypeID() } {
            return Ok(None);
        }
        let raw = value.as_CFTypeRef();
        unsafe { CFRetain(raw) };
        Ok(AxElement::owned(raw))
    }

    pub fn string_attribute(&self, name: &str) -> Result<Option<String>, AxError> {
        let Some(value) = self.copy_attribute(name)? else {
            return Ok(None);
        };
        Ok(value.downcast::<CFString>().map(|string| string.to_string()))
    }

    pub fn range_attribute(&self, name: &str) -> Result<Option<CFRange>, AxError> {
        let Some(value) = self.copy_attribute(name)? else {
            return Ok(None);
        };
        // SAFETY : type vérifié, puis lecture dans une CFRange de même layout.
        if value.type_of() as usize != unsafe { AXValueGetTypeID() } {
            return Ok(None);
        }
        let mut range = CFRange { location: 0, length: 0 };
        let ok = unsafe {
            AXValueGetValue(
                value.as_CFTypeRef(),
                kAXValueTypeCFRange,
                &mut range as *mut CFRange as *mut c_void,
            )
        };
        Ok((ok != 0).then_some(range))
    }

    pub fn is_settable(&self, name: &str) -> bool {
        let attribute = CFString::new(name);
        let mut settable: u8 = 0;
        // SAFETY : pointeurs valides pour la durée de l'appel.
        let status = unsafe {
            AXUIElementIsAttributeSettable(self.0, attribute.as_concrete_TypeRef(), &mut settable)
        };
        status == kAXErrorSuccess && settable != 0
    }

    pub fn set_string_attribute(&self, name: &str, value: &str) -> Result<(), AxError> {
        let attribute = CFString::new(name);
        let value = CFString::new(value);
        // SAFETY : références CF valides pendant l'appel.
        let status = unsafe {
            AXUIElementSetAttributeValue(
                self.0,
                attribute.as_concrete_TypeRef(),
                value.as_CFTypeRef(),
            )
        };
        if status == kAXErrorSuccess {
            Ok(())
        } else {
            Err(AxError(status))
        }
    }

    pub fn set_range_attribute(&self, name: &str, range: CFRange) -> Result<(), AxError> {
        let attribute = CFString::new(name);
        // SAFETY : `AXValueCreate` copie la CFRange ; on libère la valeur après usage.
        let value = unsafe {
            AXValueCreate(kAXValueTypeCFRange, &range as *const CFRange as *const c_void)
        };
        if value.is_null() {
            return Err(AxError(-1));
        }
        let value = unsafe { CFType::wrap_under_create_rule(value) };
        let status = unsafe {
            AXUIElementSetAttributeValue(self.0, attribute.as_concrete_TypeRef(), value.as_CFTypeRef())
        };
        if status == kAXErrorSuccess {
            Ok(())
        } else {
            Err(AxError(status))
        }
    }

    #[cfg_attr(not(feature = "selftest"), allow(dead_code))]
    pub fn set_bool_attribute(&self, name: &str, value: bool) -> Result<(), AxError> {
        let attribute = CFString::new(name);
        let value = if value { CFBoolean::true_value() } else { CFBoolean::false_value() };
        // SAFETY : références CF valides pendant l'appel.
        let status = unsafe {
            AXUIElementSetAttributeValue(self.0, attribute.as_concrete_TypeRef(), value.as_CFTypeRef())
        };
        if status == kAXErrorSuccess {
            Ok(())
        } else {
            Err(AxError(status))
        }
    }

    #[cfg_attr(not(feature = "selftest"), allow(dead_code))]
    pub fn children(&self) -> Vec<AxElement> {
        let Ok(Some(value)) = self.copy_attribute("AXChildren") else {
            return Vec::new();
        };
        let Some(array) = value.downcast::<CFArray>() else {
            return Vec::new();
        };
        array
            .iter()
            .filter_map(|item| {
                let raw: CFTypeRef = *item;
                // SAFETY : type vérifié, puis référence retenue pour nous.
                if unsafe { CFGetTypeID(raw) != AXUIElementGetTypeID() } {
                    return None;
                }
                unsafe { CFRetain(raw) };
                AxElement::owned(raw)
            })
            .collect()
    }

    pub fn pid(&self) -> Option<i32> {
        let mut pid = 0;
        // SAFETY : élément valide, sortie non nulle.
        let status = unsafe { AXUIElementGetPid(self.0, &mut pid) };
        (status == kAXErrorSuccess).then_some(pid)
    }

    pub fn role(&self) -> Option<String> {
        self.string_attribute("AXRole").ok().flatten()
    }

    pub fn subrole(&self) -> Option<String> {
        self.string_attribute("AXSubrole").ok().flatten()
    }

    /// Champ de mot de passe : on n'y touche jamais.
    pub fn is_secure(&self) -> bool {
        let secure = |value: Option<String>| {
            value.is_some_and(|value| value.contains("SecureTextField"))
        };
        secure(self.subrole()) || secure(self.role())
    }
}

/// L'app a-t-elle la permission Accessibilité ? Avec `prompt`, macOS affiche
/// sa propre invitation (une seule fois par lancement).
pub fn is_trusted(prompt: bool) -> bool {
    if !prompt {
        // SAFETY : fonction sans précondition.
        return unsafe { AXIsProcessTrusted() != 0 };
    }
    // SAFETY : constante exportée par ApplicationServices.
    let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0 }
}

/// L'app au premier plan, vue par Accessibility.
pub fn focused_application() -> Option<AxElement> {
    AxElement::system_wide()
        .element_attribute("AXFocusedApplication")
        .ok()
        .flatten()
}

/// Le champ qui a le focus clavier dans une app.
pub fn focused_element(app: &AxElement) -> Option<AxElement> {
    app.element_attribute("AXFocusedUIElement").ok().flatten()
}

pub fn secure_input_enabled() -> bool {
    // SAFETY : fonction sans précondition.
    unsafe { IsSecureEventInputEnabled() != 0 }
}
