# Unicorn Rewrite

Une petite app de barre de menu pour macOS : vous sélectionnez du texte dans
n'importe quelle app, vous appuyez sur un raccourci, et la sélection est
corrigée ou reformulée sur place.

Pas de fenêtre, pas de copier-coller à faire, pas de compte. L'app ne fait que
remplacer le texte : elle n'envoie jamais rien à votre place.

## Installation

1. Téléchargez le `.dmg` de la [dernière release](https://github.com/Kwickos/unicorn-rewrite/releases/latest)
   et glissez l'app dans Applications.
2. Au premier lancement, macOS peut bloquer l'app (elle n'est pas notarisée) :
   clic droit sur l'app → **Ouvrir**, ou
   `xattr -dr com.apple.quarantine "/Applications/Unicorn Rewrite.app"`.
3. Autorisez **Accessibilité** quand l'app le demande (Réglages Système →
   Confidentialité et sécurité → Accessibilité). C'est ce qui lui permet de
   lire la sélection et de la remplacer.
4. Collez une clé API dans le panneau.

Apple Silicon uniquement pour l'instant. Les mises à jour s'installent
d'elles-mêmes ; un bouton « Mettre à jour » apparaît dans le panneau quand une
nouvelle version est prête.

## Clé API

Le fournisseur est reconnu à la clé, il n'y a rien d'autre à régler :

| Clé | Fournisseur | Modèle |
| --- | --- | --- |
| `sk-or-…` | [OpenRouter](https://openrouter.ai/keys) | au choix, dans Réglages → Modèle |
| `csk-…` | [Cerebras](https://cloud.cerebras.ai) | `qwen-3.8-27b` |
| `gsk_…` | [Groq](https://console.groq.com/keys) | `openai/gpt-oss-20b` |
| `sk-…` | [Qwen Cloud](https://www.qwencloud.com) | `qwen3.8-flash` |
| `AIza…` | [Gemini](https://aistudio.google.com/apikey) | `gemini-3.5-flash-lite` |

La clé est stockée dans le Trousseau macOS.

Avec OpenRouter, la liste des modèles est triée par date de sortie et filtrable
par vitesse et par coût. Le modèle choisi ne change pas tout seul, sauf pour sa
propre version suivante (`qwen3.8-flash` → `qwen3.9-flash`) ou s'il est retiré.

Une reformulation coûte en général une fraction de centime.

## Utilisation

- **⌃⌥R** (modifiable dans les réglages) : reformule la sélection avec le
  profil actif.
- **Échap** pendant le traitement : annule.
- **↺** dans le panneau : remet le texte d'origine.

Profils : Corriger uniquement, Naturel, Professionnel, Chaleureux, Direct et
concis, Personnalisé (votre propre consigne).

Quel que soit le profil, le texte garde sa langue, son sens, les noms, chiffres,
dates, liens et le tutoiement ou vouvoiement. Rien n'est ajouté : ni salutation,
ni signature, ni promesse. Une instruction écrite dans le texte sélectionné est
traitée comme du texte, pas comme une consigne.

## Ce qui se passe quand vous appuyez sur le raccourci

1. L'app lit la sélection par Accessibility. Si l'app cible ne l'expose pas
   (Discord, Slack et autres apps Electron), elle la copie, puis remet votre
   presse-papiers tel qu'il était.
2. Le texte part chez le fournisseur, avec le profil. Un seul appel, sans
   historique.
3. Avant de remplacer, l'app vérifie que vous êtes toujours dans le même champ.
   Sinon, rien n'est collé : le résultat s'affiche dans le panneau avec un
   bouton Copier.

Une réponse vide, coupée ou hors sujet ne remplace jamais le texte. Les champs
de mot de passe sont ignorés. L'app ne garde aucun historique ; son journal
(`~/Library/Logs/com.digitalunicorn.unicorn-rewrite/`) contient des durées et
des codes d'erreur, jamais de texte.

## Limites

- Dans les éditeurs riches (Notes, Mail, Google Docs), le texte remplacé perd
  sa mise en forme interne (gras, liens).
- ↺ ne fonctionne que dans les apps qui exposent le texte par Accessibility.
  Ailleurs, le ⌘Z de l'app fait l'affaire.
- Dans les apps Electron, changer de sélection dans le même champ pendant le
  traitement n'est pas détecté.
- Pendant le traitement (en général moins d'une seconde), Échap est capturé par
  l'app.

Détail par app : [docs/compatibilite.md](docs/compatibilite.md).

## Développement

Tauri 2, Rust, React, TypeScript, Tailwind.

```bash
pnpm install
pnpm app                          # lance l'app en développement
UNICORN_REWRITE_MOCK=1 pnpm app   # sans clé : réponse simulée
pnpm test && pnpm lint            # tests et lint du panneau
pnpm test:rust                    # tests du natif
```

Le code natif est dans `src-tauri/src` : `engine.rs` (déroulé d'une
opération), `macos/` (Accessibility, presse-papiers), `ai/` (fournisseurs et
prompt). Le panneau est dans `src/`.

Publier une version : incrémenter `version` dans `src-tauri/tauri.conf.json`,
puis `scripts/release.sh` (voir l'en-tête du script).
