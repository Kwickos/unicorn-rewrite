import type { CatalogModel } from './desktop.ts'

export type ModelFilter = {
  fastOnly: boolean
  /** Coût maximal d'une reformulation, en centimes ; `null` : pas de limite. */
  maxCents: number | null
  query: string
}

export const PRICE_CAPS: (number | null)[] = [0.05, 0.2, null]

/**
 * Tri par date de sortie, le plus récent d'abord. « Rapides » garde les
 * modèles en tête des classements de vitesse d'OpenRouter.
 */
export function filterModels(models: CatalogModel[], filter: ModelFilter): CatalogModel[] {
  const query = filter.query.trim().toLowerCase()
  return models
    .filter((model) => filter.maxCents === null || model.costCents <= filter.maxCents)
    .filter((model) => !filter.fastOnly || model.fast)
    .filter(
      (model) =>
        !query || model.name.toLowerCase().includes(query) || model.id.toLowerCase().includes(query),
    )
    .sort((a, b) => b.created - a.created)
}

/** « Qwen: Qwen3.8 Flash » → « Qwen3.8 Flash ». */
export function shortName(name: string): string {
  const index = name.indexOf(': ')
  return index >= 0 ? name.slice(index + 2) : name
}

export function formatCents(cents: number, locale: string): string {
  const digits = cents < 0.01 ? 3 : 2
  return `${cents.toLocaleString(locale, { maximumFractionDigits: digits, minimumFractionDigits: 0 })} ¢`
}

export function formatMonth(created: number, locale: string): string {
  return new Date(created * 1000).toLocaleDateString(locale, { month: 'short', year: 'numeric' })
}
