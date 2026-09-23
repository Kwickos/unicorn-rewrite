#!/usr/bin/env bash
# Construit, signe et publie une version sur GitHub Releases.
#
#   APPLE_SIGNING_IDENTITY="Apple Development: …" scripts/release.sh
#
# La version vient de src-tauri/tauri.conf.json. La clé privée de mise à jour
# est lue dans ~/.tauri/unicorn-rewrite.key (jamais dans le dépôt).
set -euo pipefail

cd "$(dirname "$0")/.."
REPO="Kwickos/unicorn-rewrite"
KEY_FILE="${TAURI_KEY_FILE:-$HOME/.tauri/unicorn-rewrite.key}"
VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
TAG="v$VERSION"
# App universelle : Apple Silicon et Intel dans un seul binaire.
TARGET="universal-apple-darwin"

[ -f "$KEY_FILE" ] || { echo "Clé de mise à jour absente : $KEY_FILE" >&2; exit 1; }
gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1 && { echo "$TAG existe déjà." >&2; exit 1; }

export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_FILE")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
pnpm tauri build --target "$TARGET"

BUNDLE="src-tauri/target/$TARGET/release/bundle"
OUT=$(mktemp -d)
# GitHub remplace les espaces des noms de fichiers : on les nomme nous-mêmes.
cp "$BUNDLE/macos/Unicorn Rewrite.app.tar.gz" "$OUT/UnicornRewrite_universal.app.tar.gz"
cp "$BUNDLE/dmg/Unicorn Rewrite_${VERSION}_universal.dmg" "$OUT/UnicornRewrite_${VERSION}_universal.dmg"
URL="https://github.com/$REPO/releases/download/$TAG/UnicornRewrite_universal.app.tar.gz"
SIGNATURE=$(cat "$BUNDLE/macos/Unicorn Rewrite.app.tar.gz.sig")

cat > "$OUT/latest.json" <<JSON
{
  "version": "$VERSION",
  "notes": "Voir https://github.com/$REPO/releases/tag/$TAG",
  "pub_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "platforms": {
    "darwin-aarch64": { "signature": "$SIGNATURE", "url": "$URL" },
    "darwin-x86_64": { "signature": "$SIGNATURE", "url": "$URL" }
  }
}
JSON

gh release create "$TAG" --repo "$REPO" --title "$TAG" --generate-notes \
  "$OUT/UnicornRewrite_${VERSION}_universal.dmg" \
  "$OUT/UnicornRewrite_universal.app.tar.gz" \
  "$OUT/latest.json"
echo "Publié : https://github.com/$REPO/releases/tag/$TAG"
