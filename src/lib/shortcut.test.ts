import assert from 'node:assert/strict'
import { test } from 'node:test'
import { formatShortcut, heldModifiers, shortcutFromEvent } from './shortcut.ts'

const key = (code: string, mods: Partial<Record<'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey', boolean>> = {}) => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
})

test('libellé à la manière des menus macOS', () => {
  assert.equal(formatShortcut('Control+Alt+KeyR'), '⌃⌥R')
  assert.equal(formatShortcut('Super+Alt+Digit5'), '⌥⌘5')
  assert.equal(formatShortcut('Shift+Control+Space'), '⌃⇧Espace')
  assert.equal(formatShortcut('Control+Alt+Space', 'Space'), '⌃⌥Space')
})

test('la frappe lit la position physique, pas le caractère produit', () => {
  // ⌥R produit « ® » dans `key`, mais `code` reste KeyR.
  assert.equal(shortcutFromEvent(key('KeyR', { ctrlKey: true, altKey: true })), 'Control+Alt+KeyR')
  assert.equal(shortcutFromEvent(key('KeyE', { metaKey: true, altKey: true })), 'Alt+Super+KeyE')
})

test('des modificateurs seuls ne font pas un raccourci', () => {
  assert.equal(shortcutFromEvent(key('ControlLeft', { ctrlKey: true })), null)
  assert.equal(shortcutFromEvent(key('MetaRight', { metaKey: true })), null)
  assert.equal(heldModifiers(key('AltLeft', { altKey: true, ctrlKey: true })), '⌃⌥')
})
