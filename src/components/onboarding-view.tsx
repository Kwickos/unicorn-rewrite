import { useEffect } from 'react'
import { Check } from 'lucide-react'
import { cn } from 'cn'
import { ApiKeyField } from '@/components/api-key-field'
import type { AppModel } from '@/hooks/use-app'
import { isDesktop, native, type Snapshot } from '@/lib/desktop'
import { useT } from '@/lib/i18n'
import { formatShortcut } from '@/lib/shortcut'
import { kbdClass, primaryAction, secondaryAction } from '@/lib/styles'

function Step({
  index,
  done,
  title,
  children,
}: {
  index: number
  done: boolean
  title: string
  children: React.ReactNode
}) {
  return (
    <li className="flex gap-2.5 px-2 py-2">
      <span
        aria-hidden
        className={cn(
          'flex size-5 shrink-0 items-center justify-center rounded-full text-[0.7rem] font-medium tabular-nums',
          'transition-colors duration-[var(--duration-quick)] ease-[var(--ease-smooth-out)]',
          done ? 'bg-foreground text-background' : 'bg-muted text-muted-foreground',
        )}
      >
        {done ? <Check className="size-3" /> : index}
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <h3 className="text-sm leading-5 font-medium">{title}</h3>
        {children}
      </div>
    </li>
  )
}

type Props = {
  app: AppModel
  snapshot: Snapshot
  onDone: () => void
}

/**
 * Premier lancement : la permission et la clé, dans une seule vue.
 */
export function OnboardingView({ app, snapshot, onDone }: Props) {
  const t = useT()
  const { trusted } = snapshot

  // Tant que la permission manque, on relit l'état réel chaque seconde :
  // l'utilisateur l'accorde dans une autre fenêtre. Simple affichage — aucune
  // opération ne dépend de cette minuterie.
  useEffect(() => {
    if (trusted || !isDesktop) return
    const timer = window.setInterval(() => void app.refresh(), 1000)
    return () => window.clearInterval(timer)
  }, [trusted, app])

  return (
    <div className="flex flex-col p-1.5">
      <h2 className="px-2 pt-1.5 pb-1 text-sm font-medium">Unicorn Rewrite</h2>

      <ol className="flex flex-col">
        <Step index={1} done={trusted} title={t('stepPermission')}>
          {!trusted && (
              <div className="flex flex-wrap items-center gap-1">
                <button
                  type="button"
                  onClick={() => isDesktop && void native.requestAccessibility().then(() => app.refresh())}
                  className={primaryAction}
                >
                  {t('actionAllow')}
                </button>
                <button type="button" onClick={() => void app.refresh()} className={secondaryAction}>
                  {t('stepPermissionRetry')}
                </button>
              </div>
          )}
        </Step>

        <Step index={2} done={snapshot.hasKey} title={t('stepKey')}>
          <ApiKeyField hasKey={snapshot.hasKey} onChange={(hasKey) => {
              app.patchSnapshot({ hasKey })
              void app.refresh()
            }} />
        </Step>
      </ol>

      <div className="flex items-center justify-between gap-2 px-2 pt-1 pb-1.5">
        <kbd className={kbdClass}>{formatShortcut(snapshot.settings.shortcut, t('spaceKey'))}</kbd>
        <button type="button" onClick={onDone} className={primaryAction}>
          {t('start')}
        </button>
      </div>
    </div>
  )
}
