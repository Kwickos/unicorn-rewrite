# Compatibilité

L'app choisit une des deux voies selon ce que l'app cible expose.

**Accessibility** : lecture et remplacement directs de la sélection. Le
presse-papiers n'est pas touché. Avant de remplacer, l'app vérifie que le champ,
la plage sélectionnée et le texte n'ont pas changé.

**Copier/coller** : quand la sélection n'est pas lisible (apps Electron, certains
éditeurs web).
- L'app envoie ⌘C, puis vérifie que le presse-papiers a bien changé. Sinon,
  elle considère qu'il n'y a pas de sélection et ne traite pas l'ancien contenu.
- Elle colle le résultat avec ⌘V. Le contenu collé est marqué transitoire, pour
  que les gestionnaires d'historique l'ignorent.
- Votre presse-papiers est remis tel quel juste après (tous les formats), sauf
  si vous avez copié autre chose entre-temps.
- La vérification avant remplacement se limite à « même app, même champ ».

## Apps

| App | Voie | Remplacement | ↺ Restaurer | État |
| --- | --- | --- | --- | --- |
| Discord | copier/coller | oui | non (⌘Z de l'app) | observé |
| TextEdit | Accessibility | attendu | attendu | à confirmer |
| Notes, Mail | Accessibility | attendu (texte brut) | attendu | à confirmer |
| Safari, `<textarea>` | Accessibility | attendu | attendu | à confirmer |
| Chrome, Arc | Accessibility ou copier/coller | attendu | selon l'app | à confirmer |
| Slack | copier/coller | attendu | non (⌘Z de l'app) | à confirmer |
| Champs de mot de passe | — | refusé | — | par conception |

Si vous testez une app qui n'est pas dans la liste, ouvrez une issue avec ce
que vous avez observé et les lignes du journal
(`~/Library/Logs/com.digitalunicorn.unicorn-rewrite/diagnostic.log`).

## Tester soi-même

Une build de test pilote TextEdit, Safari et Arc par Accessibility :
remplacement, restauration, absence de sélection, changement de focus, texte
modifié pendant la requête, double raccourci, annulation, copie concurrente.

```bash
APPLE_SIGNING_IDENTITY="…" pnpm tauri build --debug --features selftest --bundles app
open "src-tauri/target/debug/bundle/macos/Unicorn Rewrite.app" \
  --args --selftest=textedit,safari,arc --selftest-out=$PWD/selftest.txt
```

Accordez d'abord l'Accessibilité à cette build, puis ne touchez à rien pendant
environ deux minutes.
