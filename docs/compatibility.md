# Compatibility

The app picks one of two paths depending on what the target app exposes.

**Accessibility.** The selection is read and replaced directly; the clipboard
is untouched. Before replacing, the app checks that the field, the selected
range and the text haven't changed.

**Copy and paste.** Used when the selection isn't readable (Electron apps,
some web editors).
- The app sends ⌘C and checks that the clipboard actually changed. If it
  didn't, there's no selection, and the old clipboard content is never used.
- The result is pasted with ⌘V and marked as transient, so clipboard managers
  (Maccy, Raycast, Paste…) ignore it.
- Your clipboard is restored right after, in every format, unless you copied
  something new in the meantime.
- The check before replacing is limited to "same app, same field".

## Apps

| App | Path | Replace | ↺ Restore | Status |
| --- | --- | --- | --- | --- |
| Discord | copy/paste | yes | no (use ⌘Z) | observed |
| TextEdit | Accessibility | expected | expected | to confirm |
| Notes, Mail | Accessibility | expected (plain text) | expected | to confirm |
| Safari `<textarea>` | Accessibility | expected | expected | to confirm |
| Chrome, Arc | Accessibility or copy/paste | expected | depends | to confirm |
| Slack | copy/paste | expected | no (use ⌘Z) | to confirm |
| Password fields | — | refused | — | by design |

Tried an app that isn't listed? Open an issue with what happened and the
matching lines from
`~/Library/Logs/com.digitalunicorn.unicorn-rewrite/diagnostic.log` (timings
and error codes only, no text).

## Automated check

A test build drives TextEdit, Safari and Arc through Accessibility: replace,
restore, no selection, focus change, text edited during the request, double
shortcut, cancel, concurrent copy.

```sh
APPLE_SIGNING_IDENTITY="…" pnpm tauri build --debug --features selftest --bundles app
open "src-tauri/target/debug/bundle/macos/Unicorn Rewrite.app" \
  --args --selftest=textedit,safari,arc --selftest-out=$PWD/selftest.txt
```

Grant Accessibility to that build first, then leave the keyboard and mouse
alone for about two minutes.
