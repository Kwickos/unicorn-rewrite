//! Les quelques fonctions C d'Accessibility, Core Graphics et Carbon dont
//! l'app a besoin, déclarées à la main : une douzaine de signatures stables
//! depuis des années, plus lisibles qu'une dépendance de bindings générés.

#![allow(non_upper_case_globals, non_snake_case, dead_code)]

use std::ffi::c_void;

use core_foundation::base::{Boolean, CFTypeRef};
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::string::CFStringRef;

pub type AXUIElementRef = *const c_void;
pub type AXValueRef = *const c_void;
pub type AXError = i32;
pub type AXValueType = u32;
pub type PidT = i32;

pub const kAXErrorSuccess: AXError = 0;
pub const kAXErrorAPIDisabled: AXError = -25211;
pub const kAXErrorNoValue: AXError = -25212;
pub const kAXErrorAttributeUnsupported: AXError = -25205;

pub const kAXValueTypeCFRange: AXValueType = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CFRange {
    pub location: isize,
    pub length: isize,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub static kAXTrustedCheckOptionPrompt: CFStringRef;

    pub fn AXIsProcessTrusted() -> Boolean;
    pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
    pub fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    pub fn AXUIElementCreateApplication(pid: PidT) -> AXUIElementRef;
    pub fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    pub fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    pub fn AXUIElementIsAttributeSettable(
        element: AXUIElementRef,
        attribute: CFStringRef,
        settable: *mut Boolean,
    ) -> AXError;
    pub fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut PidT) -> AXError;
    pub fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, seconds: f32) -> AXError;
    pub fn AXUIElementGetTypeID() -> usize;
    pub fn AXValueCreate(value_type: AXValueType, value: *const c_void) -> AXValueRef;
    pub fn AXValueGetValue(value: AXValueRef, value_type: AXValueType, out: *mut c_void) -> Boolean;
    pub fn AXValueGetTypeID() -> usize;
}

// --- Core Graphics : événements clavier synthétiques ---------------------

pub type CGEventRef = *mut c_void;
pub type CGEventSourceRef = *mut c_void;
pub type CGKeyCode = u16;
pub type CGEventFlags = u64;

pub const kCGEventSourceStateCombinedSessionState: i32 = 0;
pub const kCGEventSourceStateHIDSystemState: i32 = 1;
pub const kCGHIDEventTap: u32 = 0;

pub const kCGEventFlagMaskShift: CGEventFlags = 0x0002_0000;
pub const kCGEventFlagMaskControl: CGEventFlags = 0x0004_0000;
pub const kCGEventFlagMaskAlternate: CGEventFlags = 0x0008_0000;
pub const kCGEventFlagMaskCommand: CGEventFlags = 0x0010_0000;
pub const kCGEventFlagMaskSecondaryFn: CGEventFlags = 0x0080_0000;

/// `kVK_ANSI_C` et `kVK_ANSI_V` : positions physiques des touches, identiques
/// en QWERTY et AZERTY.
pub const kVK_ANSI_C: CGKeyCode = 0x08;
pub const kVK_ANSI_V: CGKeyCode = 0x09;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub fn CGEventSourceCreate(state: i32) -> CGEventSourceRef;
    pub fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        key: CGKeyCode,
        key_down: bool,
    ) -> CGEventRef;
    pub fn CGEventSetFlags(event: CGEventRef, flags: CGEventFlags);
    pub fn CGEventPost(tap: u32, event: CGEventRef);
    pub fn CGEventSourceFlagsState(state: i32) -> CGEventFlags;
}

// --- Carbon : saisie sécurisée (champs de mot de passe) ------------------

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    /// Vrai quand une app a activé la saisie sécurisée : un champ de mot de
    /// passe a le focus, ou « Saisie sécurisée au clavier » d'un terminal.
    pub fn IsSecureEventInputEnabled() -> Boolean;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub fn CFEqual(a: CFTypeRef, b: CFTypeRef) -> Boolean;
    pub fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    pub fn CFRelease(cf: CFTypeRef);
    pub fn CFGetTypeID(cf: CFTypeRef) -> usize;
}
