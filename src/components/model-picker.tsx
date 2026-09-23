import { useEffect, useMemo, useState } from 'react'
import { Check, LoaderCircle, X } from 'lucide-react'
import { cn } from 'cn'
import { isDesktop, native, type CatalogModel, type ErrorCode, type Settings } from '@/lib/desktop'
import { useLocale, useT } from '@/lib/i18n'
import { filterModels, formatCents, formatMonth, PRICE_CAPS, shortName } from '@/lib/models'
import { errorMessage } from '@/lib/status'
import { fieldClass, iconButton } from '@/lib/styles'

function Pill({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={cn(
        'rounded-full px-2 py-0.5 text-xs outline-none transition-colors',
        'duration-[var(--duration-quick)] ease-[var(--ease-smooth-out)]',
        'focus-visible:ring-2 focus-visible:ring-ring/60',
        active ? 'bg-foreground text-background' : 'text-muted-foreground hover:bg-muted',
      )}
    >
      {children}
    </button>
  )
}

type Props = {
  current: string
  onPicked: (settings: Settings) => void
  onClose: () => void
}

/**
 * Catalogue OpenRouter : les plus récents d'abord, filtrés par vitesse
 * mesurée et par coût d'une reformulation. Le choix est gardé ; seule une
 * version plus récente du même modèle le remplace ensuite.
 */
export function ModelPicker({ current, onPicked, onClose }: Props) {
  const t = useT()
  const locale = useLocale()
  const [models, setModels] = useState<CatalogModel[] | null>(null)
  const [error, setError] = useState<ErrorCode | null>(null)
  const [fastOnly, setFastOnly] = useState(true)
  const [maxCents, setMaxCents] = useState<number | null>(0.05)
  const [query, setQuery] = useState('')

  useEffect(() => {
    if (!isDesktop) return
    let cancelled = false
    native
      .listModels()
      .then((list) => !cancelled && setModels(list))
      .catch((code) => !cancelled && setError(code as ErrorCode))
    return () => {
      cancelled = true
    }
  }, [])

  const visible = useMemo(
    () => (models ? filterModels(models, { fastOnly, maxCents, query }) : []),
    [models, fastOnly, maxCents, query],
  )

  const pick = async (id: string) => {
    if (!isDesktop) return
    onPicked(await native.setModel(id))
    onClose()
  }

  return (
    <div className="flex flex-col p-1.5">
      <div className="flex items-center justify-between px-2 py-1.5">
        <h2 className="text-sm font-medium">{t('model')}</h2>
        <button type="button" onClick={onClose} aria-label={t('dismiss')} className={cn(iconButton, 'size-6 rounded-md')}>
          <X className="size-3.5" />
        </button>
      </div>

      <div className="flex flex-col gap-1.5 px-2 pb-1.5">
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('modelSearch')}
          aria-label={t('modelSearch')}
          spellCheck={false}
          className={fieldClass}
        />
        <div className="flex flex-wrap items-center gap-0.5">
          <Pill active={fastOnly} onClick={() => setFastOnly((value) => !value)}>
            {t('modelFast')}
          </Pill>
          <span className="mx-1 h-3 w-px bg-border" aria-hidden />
          {PRICE_CAPS.map((cap) => (
            <Pill key={cap ?? 'all'} active={maxCents === cap} onClick={() => setMaxCents(cap)}>
              {cap === null ? t('modelAnyPrice') : `< ${formatCents(cap, locale)}`}
            </Pill>
          ))}
        </div>
      </div>

      <div className="scrollbar-none flex max-h-[22rem] flex-col overflow-y-auto" role="listbox" aria-label={t('model')}>
        {!models && !error && (
          <div className="flex justify-center py-6 text-muted-foreground">
            <LoaderCircle className="size-4 animate-spin motion-reduce:animate-none" aria-label={t('loading')} />
          </div>
        )}
        {error && <p className="px-2 py-4 text-sm text-destructive">{t(errorMessage(error))}</p>}
        {models && visible.length === 0 && (
          <p className="px-2 py-4 text-sm text-muted-foreground">{t('modelNone')}</p>
        )}
        {visible.map((model) => {
          const selected = model.id === current
          return (
            <button
              key={model.id}
              type="button"
              role="option"
              aria-selected={selected}
              onClick={() => void pick(model.id)}
              className={cn(
                'flex items-center gap-2 rounded-lg px-2 py-1.5 text-left outline-none',
                'transition-colors duration-[var(--duration-quick)] ease-[var(--ease-smooth-out)]',
                'hover:bg-muted/60 focus-visible:ring-2 focus-visible:ring-ring/60',
                selected && 'bg-muted',
              )}
            >
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm leading-tight">{shortName(model.name)}</span>
                <span className="block truncate text-xs leading-tight text-muted-foreground tabular-nums">
                  {formatMonth(model.created, locale)} · {formatCents(model.costCents, locale)}
                  {model.throughput !== null && ` · ${Math.round(model.throughput)} t/s`}
                </span>
              </span>
              {selected && <Check className="size-3.5 shrink-0" />}
            </button>
          )
        })}
      </div>
    </div>
  )
}
