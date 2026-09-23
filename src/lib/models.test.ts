import assert from 'node:assert/strict'
import { test } from 'node:test'
import type { CatalogModel } from './desktop.ts'
import { filterModels, shortName } from './models.ts'

const model = (id: string, created: number, costCents: number, fast: boolean, reasoningMandatory = false): CatalogModel => ({
  id,
  name: `X: ${id}`,
  created,
  costCents,
  fast,
  throughput: null,
  reasoningMandatory,
})

const catalog = [
  model('a/old-fast', 100, 0.01, true),
  model('a/new-fast', 300, 0.03, true),
  model('a/new-slow', 400, 0.02, false),
  model('a/thinker', 500, 0.01, false, true),
  model('a/pricey', 200, 0.9, true),
]

test('les plus récents d’abord, filtrés par vitesse et par prix', () => {
  const ids = filterModels(catalog, { fastOnly: true, maxCents: 0.05, query: '' }).map((m) => m.id)
  assert.deepEqual(ids, ['a/new-fast', 'a/old-fast'])
})

test('sans filtre, tout le catalogue par date', () => {
  const ids = filterModels(catalog, { fastOnly: false, maxCents: null, query: '' }).map((m) => m.id)
  assert.deepEqual(ids, ['a/thinker', 'a/new-slow', 'a/new-fast', 'a/pricey', 'a/old-fast'])
})

test('recherche par nom', () => {
  assert.equal(filterModels(catalog, { fastOnly: false, maxCents: null, query: 'PRICEY' }).length, 1)
  assert.equal(shortName('Qwen: Qwen3.8 Flash'), 'Qwen3.8 Flash')
})
