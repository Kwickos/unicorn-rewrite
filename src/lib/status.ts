import type { ErrorCode, ProfileId, Status } from './desktop.ts'
import type { MessageKey } from './i18n.ts'

export type Tone = 'idle' | 'busy' | 'success' | 'error'

/** Action proposée à côté d'un message d'erreur. */
export type StatusAction = 'allow' | 'addKey' | 'cancel' | 'restore' | null

export type StatusView = {
  tone: Tone
  message: MessageKey
  params?: Record<string, string>
  action: StatusAction
}

const ERROR_MESSAGES: Record<ErrorCode, MessageKey> = {
  permission: 'errPermission',
  noSelection: 'errNoSelection',
  secureField: 'errSecureField',
  selfTarget: 'errSelfTarget',
  readFailed: 'errReadFailed',
  tooLong: 'errTooLong',
  missingKey: 'errMissingKey',
  invalidKey: 'errInvalidKey',
  quota: 'errQuota',
  network: 'errNetwork',
  timeout: 'errTimeout',
  blocked: 'errBlocked',
  truncated: 'errTruncated',
  empty: 'errEmpty',
  unusable: 'errUnusable',
  provider: 'errProvider',
  targetChanged: 'errTargetChanged',
  replaceFailed: 'errReplaceFailed',
  restoreFailed: 'errRestoreFailed',
}

export function errorMessage(code: ErrorCode): MessageKey {
  return ERROR_MESSAGES[code]
}

/** Traduit l'état natif en une ligne de statut. */
export function describeStatus(status: Status, shortcut: string): StatusView {
  const app = status.app ?? ''
  switch (status.phase) {
    case 'reading':
      return { tone: 'busy', message: 'reading', action: 'cancel' }
    case 'rewriting':
      return app
        ? { tone: 'busy', message: 'rewritingIn', params: { app }, action: 'cancel' }
        : { tone: 'busy', message: 'rewriting', action: 'cancel' }
    case 'replacing':
      return { tone: 'busy', message: 'replacing', action: null }
    case 'done':
      return {
        tone: 'success',
        message: app ? 'doneIn' : 'done',
        params: { app },
        action: status.canRestore ? 'restore' : null,
      }
    case 'unchanged':
      return { tone: 'success', message: 'unchanged', action: null }
    case 'restored':
      return { tone: 'success', message: 'restored', action: null }
    case 'error':
    case 'review': {
      const code = status.error ?? 'provider'
      const action: StatusAction =
        code === 'permission'
          ? 'allow'
          : code === 'missingKey' || code === 'invalidKey'
            ? 'addKey'
            : null
      return { tone: 'error', message: errorMessage(code), action }
    }
    case 'cancelled':
      return { tone: 'idle', message: 'cancelled', action: null }
    case 'idle':
      return {
        tone: 'idle',
        message: 'ready',
        params: { shortcut },
        action: status.canRestore ? 'restore' : null,
      }
  }
}

export const PROFILES: { id: ProfileId; label: MessageKey; hint: MessageKey }[] = [
  { id: 'correct', label: 'profileCorrect', hint: 'profileCorrectHint' },
  { id: 'natural', label: 'profileNatural', hint: 'profileNaturalHint' },
  { id: 'professional', label: 'profileProfessional', hint: 'profileProfessionalHint' },
  { id: 'warm', label: 'profileWarm', hint: 'profileWarmHint' },
  { id: 'concise', label: 'profileConcise', hint: 'profileConciseHint' },
  { id: 'custom', label: 'profileCustom', hint: 'profileCustomEmpty' },
]
