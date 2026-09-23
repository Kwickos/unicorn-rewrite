fn main() {
    // Les commandes de l'app sont déclarées ici pour générer une permission
    // par commande : la capacité du panneau n'autorise que celles-ci.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_state",
            "get_status",
            "update_settings",
            "set_shortcut",
            "pause_shortcut",
            "save_api_key",
            "check_api_key",
            "delete_api_key",
            "request_accessibility",
            "open_accessibility_settings",
            "open_key_page",
            "cancel_operation",
            "restore_last",
            "copy_text",
            "dismiss_result",
            "quit_app",
            "resize_panel",
            "list_models",
            "set_model",
            "restart_app",
        ]),
    ))
    .expect("échec de tauri-build");
}
