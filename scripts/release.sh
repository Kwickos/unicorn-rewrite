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
ARCH=$(uname -m | sed 's/arm64/aarch64/')

[ -f "$KEY_FILE" ] || { echo "Clé de mise à jour absente : $KEY_FILE" >&2; exit 1; }
gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1 && { echo "$TAG existe déjà." >&2; exit 1; }

export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_FILE")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
pnpm tauri build

BUNDLE=src-tauri/target/release/bundle
OUT=$(mktemp -d)
# GitHub remplace les espaces des noms de fichiers : on les nomme nous-mêmes.
cp "$BUNDLE/macos/Unicorn Rewrite.app.tar.gz" "$OUT/UnicornRewrite_${ARCH}.app.tar.gz"
cp "$BUNDLE/dmg/Unicorn Rewrite_${VERSION}_${ARCH}.dmg" "$OUT/UnicornRewrite_${VERSION}_${ARCH}.dmg"
SIGNATURE=$(cat "$BUNDLE/macos/Unicorn Rewrite.app.tar.gz.sig")

cat > "$OUT/latest.json" <<JSON
{
  "version": "$VERSION",
  "notes": "Voir https://github.com/$REPO/releases/tag/$TAG",
  "pub_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "platforms": {
    "darwin-$ARCH": {
      "signature": "$SIGNATURE",
      "url": "https://github.com/$REPO/releases/download/$TAG/UnicornRewrite_${ARCH}.app.tar.gz"
    }
  }
}
JSON

gh release create "$TAG" --repo "$REPO" --title "$TAG" --generate-notes \
  "$OUT/UnicornRewrite_${VERSION}_${ARCH}.dmg" \
  "$OUT/UnicornRewrite_${ARCH}.app.tar.gz" \
  "$OUT/latest.json"
echo "Publié : https://github.com/$REPO/releases/tag/$TAG"
