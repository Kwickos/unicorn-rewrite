import { useState } from 'react'
import { Check, CircleAlert, LoaderCircle } from 'lucide-react'
import { cn } from 'cn'
import { isDesktop, native, type Status } from '@/lib/desktop'
import { useT, type MessageKey } from '@/lib/i18n'
import { formatShortcut } from '@/lib/shortcut'
import { describeStatus, type Tone } from '@/lib/status'
import { primaryAction, quietAction, secondaryAction } from '@/lib/styles'

function ToneIcon({ tone }: { tone: Tone }) {
  // Même emplacement pour tous les états : seule l'icône change, en fondu.
  return (
    <span className="relative flex size-4 shrink-0 items-center justify-center" aria-hidden>
      {tone === 'busy' && (
        <LoaderCircle className="size-4 animate-spin text-muted-foreground motion-reduce:animate-none" />
      )}
      {tone === 'success' && <Check className="view-enter size-4 text-emerald-600 dark:text-emerald-400" />}
      {tone === 'error' && <CircleAlert className="view-enter size-4 text-destructive" />}
      {tone === 'idle' && <span className="size-1.5 rounded-full bg-muted-foreground/60" />}
    </span>
  )
}

type Props = {
  status: Status
  shortcut: string
  onAllow: () => void
  onAddKey: () => void
}

/** Ligne d'état en tête du panneau, annoncée aux lecteurs d'écran. */
export function StatusLine({ status, shortcut, onAllow, onAddKey }: Props) {
  const t = useT()
  const label = formatShortcut(shortcut, t('spaceKey'))
  const view = describeStatus(status, label)

  return (
    <div className="flex items-center gap-2.5 px-2 py-1.5" role="status" aria-live="polite">
      <ToneIcon tone={view.tone} />
      <div key={`${status.phase}-${status.error}`} className="view-enter min-w-0 flex-1">
        <p className="text-sm leading-snug text-pretty">{t(view.message, view.params)}</p>
      </div>
      {view.action === 'cancel' && isDesktop && (
        <button type="button" onClick={() => void native.cancelOperation()} className={quietAction}>
          {t('cancel')}
        </button>
      )}
      {view.action === 'allow' && (
        <button type="button" onClick={onAllow} className={primaryAction}>
          {t('actionAllow')}
        </button>
      )}
      {view.action === 'addKey' && (
        <button type="button" onClick={onAddKey} className={primaryAction}>
          {t('actionAddKey')}
        </button>
      )}
    </div>
  )
}

/** Résultat prêt mais pas appliqué : on le montre, on propose de le copier. */
export function ResultCard({ status }: { status: Status }) {
  const t = useT()
  const [copied, setCopied] = useState<'result' | 'original' | null>(null)

  const copy = async (original: boolean) => {
    if (!isDesktop) return
    if (await native.copyText(original)) setCopied(original ? 'original' : 'result')
  }

  const label = (which: 'result' | 'original', key: MessageKey) =>
    copied === which ? t('copied') : t(key)

  return (
    <div className="view-enter flex flex-col gap-1.5 px-1.5 pb-1.5">
      {status.result && (
        <div className="rounded-lg bg-muted px-2.5 py-2">
          <p className="mb-1 text-xs text-muted-foreground">{t('resultTitle')}</p>
          <p className="max-h-36 overflow-y-auto text-sm whitespace-pre-wrap select-text">{status.result}</p>
        </div>
      )}
      <div className="flex flex-wrap items-center gap-1">
        {status.result && (
          <button type="button" onClick={() => void copy(false)} className={primaryAction}>
            {label('result', 'copyResult')}
          </button>
        )}
        {status.hasOriginal && (
          <button type="button" onClick={() => void copy(true)} className={secondaryAction}>
            {label('original', 'copyOriginal')}
          </button>
        )}
        <button
          type="button"
          onClick={() => isDesktop && void native.dismissResult()}
          className={cn(quietAction, 'ml-auto')}
        >
          {t('dismiss')}
        </button>
      </div>
    </div>
  )
}
