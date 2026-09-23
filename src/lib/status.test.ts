import assert from 'node:assert/strict'
import { test } from 'node:test'
import { IDLE_STATUS, type ErrorCode } from './desktop.ts'
import { translate } from './i18n.ts'
import { describeStatus, errorMessage } from './status.ts'

test('chaque erreur native a un message, dans les deux langues', () => {
  const codes: ErrorCode[] = [
    'permission', 'noSelection', 'secureField', 'selfTarget', 'readFailed', 'tooLong',
    'missingKey', 'invalidKey', 'quota', 'network', 'timeout', 'blocked', 'truncated',
    'empty', 'unusable', 'provider', 'targetChanged', 'replaceFailed', 'restoreFailed',
  ]
  for (const code of codes) {
    for (const locale of ['fr', 'en'] as const) {
      assert.ok(translate(locale, errorMessage(code)).length > 5, `${locale}:${code}`)
    }
  }
})

test('les erreurs actionnables proposent la bonne action', () => {
  const permission = describeStatus({ ...IDLE_STATUS, phase: 'error', error: 'permission' }, '⌃⌥R')
  assert.equal(permission.action, 'allow')
  const key = describeStatus({ ...IDLE_STATUS, phase: 'error', error: 'invalidKey' }, '⌃⌥R')
  assert.equal(key.action, 'addKey')
  const network = describeStatus({ ...IDLE_STATUS, phase: 'error', error: 'network' }, '⌃⌥R')
  assert.equal(network.action, null)
})

test('le traitement peut être annulé, un remplacement restauré', () => {
  assert.equal(describeStatus({ ...IDLE_STATUS, phase: 'rewriting', app: 'Mail' }, '').action, 'cancel')
  const done = describeStatus({ ...IDLE_STATUS, phase: 'done', app: 'Mail', canRestore: true }, '')
  assert.equal(done.tone, 'success')
  assert.equal(done.action, 'restore')
  assert.equal(translate('fr', done.message, done.params), 'Remplacé')
})
