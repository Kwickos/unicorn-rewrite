//! Le panneau de la barre de menu : son fond en verre, et sa hauteur animée
//! par AppKit.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::{available, rc::Retained, MainThreadMarker};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSAutoresizingMaskOptions,
    NSGlassEffectView, NSWindow, NSWindowOrderingMode,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_quartz_core::{kCACornerCurveContinuous, CAMediaTimingFunction};
use serde::Deserialize;
use tauri::{
    window::{Effect, EffectState, EffectsBuilder},
    WebviewWindow,
};

/// Rayon des coins du verre, concentrique avec le contenu : les lignes du
/// panneau, arrondies à `--radius` (7,2 px), sont posées à 6 px du bord.
const CORNER_RADIUS: f64 = 13.0;

fn native_window(window: &WebviewWindow) -> Option<Retained<NSWindow>> {
    let pointer = window.ns_window().ok()?;
    // SAFETY : Tauri renvoie la NSWindow de cette fenêtre ; on la retient le
    // temps de s'en servir.
    unsafe { Retained::retain(pointer.cast()) }
}

/// Pose le fond du panneau derrière la page : Liquid Glass depuis macOS 26,
/// le matériau translucide des popovers avant.
pub fn install_backdrop(window: &WebviewWindow) -> tauri::Result<()> {
    let (Some(mtm), Some(ns_window)) = (MainThreadMarker::new(), native_window(window)) else {
        return Ok(());
    };
    // wry installe son conteneur comme vue de contenu de la fenêtre, avec le
    // WebView dedans.
    let Some(content) = ns_window.contentView() else {
        return Ok(());
    };

    // Tout le contenu est découpé à la forme du verre : macOS dessine l'ombre
    // de la fenêtre, et son liseré sombre, d'après ce qui y est opaque. Sans
    // cette découpe, le verre compte comme un rectangle plein et un contour
    // noir à angles droits entoure les coins arrondis.
    content.setWantsLayer(true);
    if let Some(layer) = content.layer() {
        layer.setCornerRadius(CORNER_RADIUS);
        // SAFETY : constante exportée par Core Animation.
        layer.setCornerCurve(unsafe { kCACornerCurveContinuous });
        layer.setMasksToBounds(true);
    }

    if available!(macos = 26.0) {
        // Le verre se glisse sous la page, dans le conteneur de wry, et suit
        // la fenêtre quand elle change de taille.
        let glass = NSGlassEffectView::new(mtm);
        glass.setFrame(content.bounds());
        glass.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        glass.setCornerRadius(CORNER_RADIUS);
        content.addSubview_positioned_relativeTo(&glass, NSWindowOrderingMode::Below, None);
    } else {
        window.set_effects(
            EffectsBuilder::new()
                .effect(Effect::Popover)
                .state(EffectState::Active)
                .radius(CORNER_RADIUS)
                .build(),
        )?;
    }

    ns_window.invalidateShadow();
    Ok(())
}

/// Animation de hauteur, lue par la page dans les tokens de mouvement.
#[derive(Deserialize)]
pub struct Motion {
    /// En millisecondes.
    duration: f64,
    /// Points de contrôle de la courbe `cubic-bezier`.
    easing: [f32; 4],
}

/// Donne au panneau la hauteur de son contenu, bord supérieur fixe sous
/// l'icône.
///
/// C'est la fenêtre qu'on anime, et non un bloc dans la page : le verre la
/// remplit, et c'est lui qui dessine le bord visible du panneau.
#[tauri::command]
pub fn resize_panel(window: WebviewWindow, height: f64, motion: Option<Motion>) {
    let handle = window.clone();
    let _ = window.run_on_main_thread(move || {
        let Some(ns_window) = native_window(&handle) else {
            return;
        };

        let frame = ns_window.frame();
        // L'origine d'une fenêtre AppKit est son coin inférieur gauche : on
        // garde le haut, collé sous la barre de menu.
        let top = frame.origin.y + frame.size.height;
        let target = NSRect::new(
            NSPoint::new(frame.origin.x, top - height),
            NSSize::new(frame.size.width, height),
        );

        // Panneau fermé : personne ne verrait l'animation.
        let Some(Motion {
            duration,
            easing: [x1, y1, x2, y2],
        }) = motion.filter(|_| ns_window.isVisible())
        else {
            ns_window.setFrame_display(target, true);
            ns_window.invalidateShadow();
            return;
        };

        let changes = RcBlock::new(|context: NonNull<NSAnimationContext>| {
            // SAFETY : AppKit passe le contexte du groupe, valide pendant l'appel.
            let context = unsafe { context.as_ref() };
            context.setDuration(duration / 1000.0);
            context.setTimingFunction(Some(&CAMediaTimingFunction::functionWithControlPoints(
                x1, y1, x2, y2,
            )));
            ns_window.animator().setFrame_display(target, true);
        });
        // L'ombre d'une fenêtre transparente se calcule d'après son contenu :
        // on la redessine une fois la taille définitive atteinte.
        let done = RcBlock::new({
            let ns_window = ns_window.clone();
            move || ns_window.invalidateShadow()
        });
        NSAnimationContext::runAnimationGroup_completionHandler(&changes, Some(&*done));
    });
}
