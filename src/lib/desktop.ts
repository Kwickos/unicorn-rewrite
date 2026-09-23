import type { Bezier } from './motion.ts'

/**
 * Pont vers l'app native. Tout ce qui compte (raccourci, sélection,
 * remplacement, clé, appel IA) se passe côté Rust ; la page ne fait
 * qu'afficher l'état et transmettre des choix. L'API Tauri n'est chargée que
 * sur le bureau : la version web n'embarque jamais ce code.
 */

/** Vrai uniquement dans la coquille Tauri (application macOS). */
export const isDesktop =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export type ProfileId = 'correct' | 'natural' | 'professional' | 'warm' | 'concise' | 'custom'
export type Theme = 'system' | 'light' | 'dark'
export type Locale = 'fr' | 'en'

export type Settings = {
  profile: ProfileId
  customInstruction: string
  shortcut: string
  theme: Theme
  locale: Locale | null
  onboarded: boolean
  model: string | null
  upgradedFrom: string | null
}

export type Snapshot = {
  settings: Settings
  hasKey: boolean
  trusted: boolean
  model: string
  /** La clé permet de choisir le modèle (OpenRouter). */
  modelChoice: boolean
  mock: boolean
  /** Nouvelle version installée, active au redémarrage. */
  updateReady: string | null
  version: string
}

export type CatalogModel = {
  id: string
  name: string
  /** Secondes Unix. */
  created: number
  /** Coût d'une reformulation type, en centimes de dollar. */
  costCents: number
  /** Parmi les plus rapides d'OpenRouter (débit et délai de réponse). */
  fast: boolean
  /** Jetons/s mesurés par OpenRouter, quand il les publie. */
  throughput: number | null
  reasoningMandatory: boolean
}

export type Phase =
  | 'idle'
  | 'reading'
  | 'rewriting'
  | 'replacing'
  | 'done'
  | 'unchanged'
  | 'restored'
  | 'cancelled'
  | 'error'
  | 'review'

export type ErrorCode =
  | 'permission'
  | 'noSelection'
  | 'secureField'
  | 'selfTarget'
  | 'readFailed'
  | 'tooLong'
  | 'missingKey'
  | 'invalidKey'
  | 'quota'
  | 'network'
  | 'timeout'
  | 'blocked'
  | 'truncated'
  | 'empty'
  | 'unusable'
  | 'provider'
  | 'targetChanged'
  | 'replaceFailed'
  | 'restoreFailed'

export type Status = {
  phase: Phase
  error: ErrorCode | null
  app: string | null
  profile: ProfileId | null
  /** Résultat non appliqué, à copier. */
  result: string | null
  canRestore: boolean
  hasOriginal: boolean
  operation: number
}

export type ShortcutProblem =
  | 'invalid'
  | 'needsModifier'
  | 'typesCharacter'
  | 'appShortcut'
  | 'system'
  | 'unavailable'

export type KeyCheck = 'valid' | 'unverified'

export const IDLE_STATUS: Status = {
  phase: 'idle',
  error: null,
  app: null,
  profile: null,
  result: null,
  canRestore: false,
  hasOriginal: false,
  operation: 0,
}

/** État de démonstration pour la version web : aucun appel natif. */
export const WEB_SNAPSHOT: Snapshot = {
  settings: {
    profile: 'natural',
    customInstruction: '',
    shortcut: 'Control+Alt+KeyR',
    theme: 'system',
    locale: null,
    onboarded: true,
    model: null,
    upgradedFrom: null,
  },
  hasKey: false,
  trusted: false,
  model: 'qwen/qwen3.8-flash',
  modelChoice: true,
  mock: false,
  updateReady: null,
  version: '0.1.0',
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(command, args)
}

export const native = {
  getState: () => call<Snapshot>('get_state'),
  getStatus: () => call<Status>('get_status'),
  updateSettings: (patch: Partial<Omit<Settings, 'shortcut'>>) =>
    call<Settings>('update_settings', { patch }),
  /** Rejette avec un `ShortcutProblem`. */
  setShortcut: (spec: string) => call<Settings>('set_shortcut', { spec }),
  pauseShortcut: (paused: boolean) => call<void>('pause_shortcut', { paused }),
  /** Rejette avec un `ErrorCode`. */
  saveApiKey: (key: string) => call<KeyCheck>('save_api_key', { key }),
  checkApiKey: () => call<KeyCheck>('check_api_key'),
  deleteApiKey: () => call<void>('delete_api_key'),
  requestAccessibility: () => call<boolean>('request_accessibility'),
  openAccessibilitySettings: () => call<void>('open_accessibility_settings'),
  openKeyPage: () => call<void>('open_key_page'),
  cancelOperation: () => call<boolean>('cancel_operation'),
  restoreLast: () => call<boolean>('restore_last'),
  copyText: (original: boolean) => call<boolean>('copy_text', { original }),
  dismissResult: () => call<void>('dismiss_result'),
  quitApp: () => call<void>('quit_app'),
  listModels: () => call<CatalogModel[]>('list_models'),
  setModel: (model: string) => call<Settings>('set_model', { model }),
  restartApp: () => call<void>('restart_app'),
}

/** Écoute un événement émis par l'app native. Renvoie de quoi se désabonner. */
export async function listenNative<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<() => void> {
  if (!isDesktop) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<T>(event, (message) => handler(message.payload))
}

/** Animation confiée à AppKit : durée en millisecondes, courbe en points de contrôle. */
export type PanelMotion = { duration: number; easing: Bezier }

/**
 * Ajuste la hauteur de la fenêtre macOS au contenu affiché, bord supérieur
 * fixe. Avec `motion`, AppKit anime la fenêtre : un seul appel, pas un par image.
 */
export async function resizePanel(height: number, motion: PanelMotion | null): Promise<void> {
  if (!isDesktop) return
  try {
    await call('resize_panel', { height, motion })
  } catch {
    // La fenêtre garde la taille définie dans la config.
  }
}

/** Apparence native de l'app : le verre du panneau suit le thème choisi. */
export async function setWindowTheme(theme: Theme): Promise<void> {
  if (!isDesktop) return
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().setTheme(theme === 'system' ? null : theme)
  } catch {
    // Le verre garde l'apparence du système.
  }
}

/** Compatibilité avec `use-quit-shortcut`, repris d'Unicorn Time. */
export async function quitApp(): Promise<void> {
  if (!isDesktop) return
  try {
    await native.quitApp()
  } catch {
    // L'app reste ouverte.
  }
}
