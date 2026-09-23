import { useState } from 'react'
import { isDesktop, native, type ErrorCode } from '@/lib/desktop'
import { useT } from '@/lib/i18n'
import { errorMessage } from '@/lib/status'
import { fieldClass, primaryAction, quietAction } from '@/lib/styles'

type Props = {
  hasKey: boolean
  onChange: (hasKey: boolean) => void
  autoFocus?: boolean
}

/**
 * Clé enregistrée : masquée, avec « Modifier ». Sinon un champ. Le natif la
 * vérifie et la range dans le Trousseau ; la page ne la relit jamais.
 */
export function ApiKeyField({ hasKey, onChange, autoFocus }: Props) {
  const t = useT()
  const [editing, setEditing] = useState(!hasKey)
  const [draft, setDraft] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<ErrorCode | null>(null)

  const save = async () => {
    if (!draft.trim() || !isDesktop) return
    setBusy(true)
    setError(null)
    try {
      await native.saveApiKey(draft)
      setDraft('')
      setEditing(false)
      onChange(true)
    } catch (code) {
      setError(code as ErrorCode)
    } finally {
      setBusy(false)
    }
  }

  if (!editing) {
    return (
      <div className="flex items-center gap-1">
        <span className="min-w-0 flex-1 truncate text-sm tracking-widest text-muted-foreground">••••••••••••</span>
        <button type="button" onClick={() => setEditing(true)} className={quietAction}>
          {t('apiKeyReplace')}
        </button>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-1">
      <form
        className="flex gap-1"
        onSubmit={(event) => {
          event.preventDefault()
          void save()
        }}
      >
        <input
          type="password"
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder={t('apiKeyPlaceholder')}
          aria-label={t('apiKey')}
          aria-invalid={error !== null}
          autoComplete="off"
          spellCheck={false}
          autoFocus={autoFocus}
          className={fieldClass}
        />
        <button type="submit" disabled={!draft.trim() || busy} className={primaryAction}>
          {t('apiKeySave')}
        </button>
        {hasKey && (
          <button type="button" onClick={() => setEditing(false)} className={quietAction}>
            {t('cancel')}
          </button>
        )}
      </form>
      {error ? (
        <span role="alert" className="text-xs text-destructive">
          {t(errorMessage(error))}
        </span>
      ) : (
        !hasKey && (
          <button
            type="button"
            onClick={() => isDesktop && void native.openKeyPage()}
            className="self-start text-xs text-muted-foreground underline-offset-2 outline-none hover:text-foreground hover:underline focus-visible:ring-2 focus-visible:ring-ring/60"
          >
            {t('apiKeyGet')}
          </button>
        )
      )}
    </div>
  )
}
