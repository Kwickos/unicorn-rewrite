# Signature et notarisation (distribution future)

Aujourd'hui : usage personnel, signature **Apple Development** (certificat
local). Le Mac qui a construit l'app l'ouvre sans avertissement ; un autre Mac
la bloquerait (Gatekeeper).

Pour distribuer :

1. **Compte Apple Developer Program** (99 $/an), puis certificat
   **Developer ID Application** (Xcode → Settings → Accounts → Manage
   Certificates, ou developer.apple.com).
2. **Identifiant d'app** : `com.digitalunicorn.unicorn-rewrite` (déjà dans
   `tauri.conf.json`).
3. **Runtime renforcé** : Tauri l'active à la signature. Aucun droit
   particulier n'est nécessaire : l'Accessibilité est une autorisation TCC
   accordée par l'utilisateur, pas un *entitlement*. Pas de sandbox App Store :
   une app sandboxée ne peut ni lire la sélection d'une autre app ni
   synthétiser ⌘C/⌘V. **Pas de Mac App Store** donc, distribution directe
   seulement.
4. **Build signé et notarisé** :

   ```bash
   export APPLE_SIGNING_IDENTITY="Developer ID Application: Nom (TEAMID)"
   # Notarisation par clé API App Store Connect (recommandé)…
   export APPLE_API_ISSUER=… APPLE_API_KEY=… APPLE_API_KEY_PATH=/chemin/AuthKey_XXXX.p8
   # …ou par identifiant Apple
   # export APPLE_ID=… APPLE_PASSWORD=<mot de passe d'app> APPLE_TEAM_ID=…
   pnpm app:build
   ```

   Tauri signe l'app, l'envoie au service de notarisation, puis agrafe le
   ticket (*staple*). Vérification :

   ```bash
   spctl -a -vvv -t install "src-tauri/target/release/bundle/macos/Unicorn Rewrite.app"
   xcrun stapler validate "src-tauri/target/release/bundle/dmg/Unicorn Rewrite_0.1.0_aarch64.dmg"
   ```

5. **Universel Intel + Apple Silicon** (facultatif) :
   `rustup target add x86_64-apple-darwin` puis
   `pnpm tauri build --target universal-apple-darwin`.
6. **Mises à jour** : `tauri-plugin-updater`, avec une clé de signature des mises
   à jour (`pnpm tauri signer generate`) et une URL HTTPS.
7. **Avant de publier** : passer à une clé Gemini avec facturation (conditions
   EEE/UK/CH), rédiger une politique de confidentialité, et remplacer
   « clé fournie par l'utilisateur » par un proxy si l'app doit fonctionner
   sans compte Google.

Changer d'identité de signature invalide l'autorisation Accessibilité déjà
accordée : les utilisateurs devront la réaccorder une fois.
