/**
 * Raccourcis au format natif (`Control+Alt+KeyR`) : lecture d'une frappe dans
 * l'enregistreur, et libellé à la manière des menus macOS (⌃⌥R).
 * La validation (conflits) est faite côté natif.
 */

const MODIFIER_SYMBOLS: Record<string, string> = {
  Control: '⌃',
  Alt: '⌥',
  Shift: '⇧',
  Super: '⌘',
}

/** Ordre des menus macOS. */
const MODIFIER_ORDER = ['Control', 'Alt', 'Shift', 'Super']

const KEY_LABELS: Record<string, string> = {
  Space: 'Espace',
  Enter: '↩',
  Tab: '⇥',
  Backspace: '⌫',
  Escape: 'Échap',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Comma: ',',
  Period: '.',
  Slash: '/',
  Semicolon: ';',
  Quote: "'",
  BracketLeft: '[',
  BracketRight: ']',
  Backslash: '\\',
  Minus: '-',
  Equal: '=',
  Backquote: '`',
}

export function keyLabel(code: string, spaceLabel = 'Espace'): string {
  if (code === 'Space') return spaceLabel
  if (code.startsWith('Key')) return code.slice(3)
  if (code.startsWith('Digit')) return code.slice(5)
  return KEY_LABELS[code] ?? code
}

/** `Control+Alt+KeyR` → `⌃⌥R`. */
export function formatShortcut(spec: string, spaceLabel?: string): string {
  const parts = spec.split('+')
  const key = parts.pop() ?? ''
  const modifiers = MODIFIER_ORDER.filter((modifier) => parts.includes(modifier))
  return modifiers.map((modifier) => MODIFIER_SYMBOLS[modifier]).join('') + keyLabel(key, spaceLabel)
}

/** Libellé lisible par un lecteur d'écran : « Contrôle Option R ». */
export function describeShortcut(spec: string, names: Record<string, string>): string {
  const parts = spec.split('+')
  const key = parts.pop() ?? ''
  const modifiers = MODIFIER_ORDER.filter((modifier) => parts.includes(modifier))
  return [...modifiers.map((modifier) => names[modifier] ?? modifier), keyLabel(key)].join(' ')
}

const MODIFIER_CODES = new Set([
  'ControlLeft',
  'ControlRight',
  'AltLeft',
  'AltRight',
  'ShiftLeft',
  'ShiftRight',
  'MetaLeft',
  'MetaRight',
  'CapsLock',
  'Fn',
])

type KeyLike = Pick<KeyboardEvent, 'code' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'>

/**
 * Frappe → raccourci. `null` tant qu'on ne tient que des modificateurs.
 * On lit `code` (position physique), pas `key` : avec ⌥ enfoncé, `key`
 * vaut « ® » au lieu de « R ».
 */
export function shortcutFromEvent(event: KeyLike): string | null {
  if (!event.code || MODIFIER_CODES.has(event.code)) return null
  const modifiers: string[] = []
  if (event.ctrlKey) modifiers.push('Control')
  if (event.altKey) modifiers.push('Alt')
  if (event.shiftKey) modifiers.push('Shift')
  if (event.metaKey) modifiers.push('Super')
  return [...modifiers, event.code].join('+')
}

/** Modificateurs tenus pendant l'enregistrement, pour un aperçu en direct. */
export function heldModifiers(event: KeyLike): string {
  const held: string[] = []
  if (event.ctrlKey) held.push('Control')
  if (event.altKey) held.push('Alt')
  if (event.shiftKey) held.push('Shift')
  if (event.metaKey) held.push('Super')
  return held.map((modifier) => MODIFIER_SYMBOLS[modifier]).join('')
}
