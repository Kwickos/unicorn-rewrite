# Signing, notarization and updates

## Updates

Releases are published with `scripts/release.sh`. It builds the app, signs the
update archive with the key in `~/.tauri/unicorn-rewrite.key` (never
committed), writes `latest.json` and creates the GitHub release. The app reads
`https://github.com/Kwickos/unicorn-rewrite/releases/latest/download/latest.json`
at launch and every six hours, verifies the signature against the public key in
`tauri.conf.json`, installs in the background and offers a restart.

Losing the private key means existing installs can no longer update: keep a
backup.

## Notarization

Current builds are signed with an Apple Development certificate, so Gatekeeper
warns on other Macs. To remove that warning:

1. Join the Apple Developer Program and create a **Developer ID Application**
   certificate.
2. Build with notarization credentials:
   ```sh
   export APPLE_SIGNING_IDENTITY="Developer ID Application: Name (TEAMID)"
   export APPLE_API_ISSUER=… APPLE_API_KEY=… APPLE_API_KEY_PATH=/path/AuthKey_XXXX.p8
   scripts/release.sh
   ```
   Tauri signs with the hardened runtime, submits to Apple and staples the
   ticket.
3. Check: `spctl -a -vvv -t install "src-tauri/target/release/bundle/macos/Unicorn Rewrite.app"`.

No entitlement is needed: Accessibility is a permission the user grants, not an
entitlement. The app can't be sandboxed (it reads other apps' selections), so
the Mac App Store is not an option.

Changing the signing identity resets the Accessibility permission once for
existing users.

## Intel Macs

`rustup target add x86_64-apple-darwin`, then build with
`--target universal-apple-darwin` and add a `darwin-x86_64` entry to
`latest.json`.
