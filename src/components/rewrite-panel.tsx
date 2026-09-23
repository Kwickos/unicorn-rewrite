import { useEffect, useState } from 'react'
import { ArrowUpCircle, RotateCcw, Settings2 } from 'lucide-react'
import { cn } from 'cn'
import { ModelPicker } from '@/components/model-picker'
import { OnboardingView } from '@/components/onboarding-view'
import { ProfileList } from '@/components/profile-list'
import { SettingsView } from '@/components/settings-view'
import { ResultCard, StatusLine } from '@/components/status-line'
import type { AppModel } from '@/hooks/use-app'
import { useDocumentLocale, useTheme } from '@/hooks/use-theme'
import { demoParams, isDesktop, native, type Snapshot } from '@/lib/desktop'
import { detectLocale, useLocale, useT } from '@/lib/i18n'
import { iconButton, rowButton } from '@/lib/styles'

type View = 'main' | 'settings' | 'onboarding' | 'models'

export function RewritePanel({ app, snapshot }: { app: AppModel; snapshot: Snapshot }) {
  const t = useT()
  const locale = useLocale()
  const { settings } = snapshot
  const { status } = app
  useTheme(settings.theme)
  useDocumentLocale(locale)

  const [chosenView, setView] = useState<View>(() => {
    const demo = demoParams.get('view')
    if (demo === 'settings' || demo === 'models' || demo === 'onboarding') return demo
    return settings.onboarded ? 'main' : 'onboarding'
  })
  const [focusKey, setFocusKey] = useState(false)

  // Permission retirée depuis le dernier lancement : l'accueil l'explique.
  const permissionLost =
    isDesktop && !snapshot.trusted && status.phase === 'error' && status.error === 'permission'
  const view: View = chosenView === 'main' && permissionLost ? 'onboarding' : chosenView

  // Premier lancement : la langue du système devient un choix explicite, que
  // changer la langue du Mac n'écrasera plus.
  useEffect(() => {
    if (settings.locale === null) void app.updateSettings({ locale: detectLocale() })
  }, [settings.locale, app])

  const finishOnboarding = () => {
    void app.updateSettings({ onboarded: true })
    setView('main')
  }

  const openKeySettings = () => {
    setFocusKey(true)
    setView('settings')
  }

  if (view === 'onboarding') {
    return (
      <div key="onboarding" className="view-enter flex flex-col">
        <OnboardingView app={app} snapshot={snapshot} onDone={finishOnboarding} />
      </div>
    )
  }

  if (view === 'models') {
    return (
      <div key="models" className="view-enter flex flex-col">
        <ModelPicker
          current={snapshot.model}
          onPicked={() => void app.refresh()}
          onClose={() => setView('settings')}
        />
      </div>
    )
  }

  if (view === 'settings') {
    return (
      <div key="settings" className="view-enter flex flex-col">
        <SettingsView
          app={app}
          snapshot={snapshot}
          focusKey={focusKey}
          onOpenModels={() => setView('models')}
          onClose={() => {
            setFocusKey(false)
            setView('main')
          }}
        />
      </div>
    )
  }

  const showResult = status.phase === 'review' || (status.error === 'restoreFailed' && status.hasOriginal)
  const visibleStatus = ['error', 'review', 'reading', 'rewriting', 'replacing'].includes(status.phase)

  return (
    <div key="main" className="view-enter flex flex-col">
      {/* Rien quand tout va bien : seulement une erreur, ou un traitement en
          cours qu'on peut annuler. */}
      {visibleStatus && (
        <div className="p-1.5 pb-0">
          <StatusLine
            status={status}
            shortcut={settings.shortcut}
            onAllow={() => setView('onboarding')}
            onAddKey={openKeySettings}
          />
        </div>
      )}
      {showResult && <ResultCard status={status} />}

      <div className="p-1.5">
        <ProfileList
          value={settings.profile}
          customInstruction={settings.customInstruction}
          onChange={(profile) => void app.updateSettings({ profile })}
        />
      </div>

      <footer className="flex shrink-0 flex-col gap-0.5 p-1.5 pt-0">
        {snapshot.updateReady && (
          <button
            type="button"
            onClick={() => isDesktop && void native.restartApp()}
            className={cn(rowButton, 'view-enter w-full text-foreground')}
          >
            <ArrowUpCircle className="size-4" />
            {t('updateReady', { version: snapshot.updateReady })}
          </button>
        )}

        <div className="flex items-center gap-0.5">
          <button type="button" onClick={() => setView('settings')} className={cn(rowButton, 'min-w-0 flex-1')}>
            <Settings2 className="size-4" />
            {t('settings')}
          </button>

          {/* N'occupe aucune place tant qu'il n'y a rien à restaurer : le
              bouton Réglages garde toute la largeur. */}
          {status.canRestore && (
            <button
              type="button"
              onClick={() => isDesktop && void native.restoreLast()}
              aria-label={t('restoreHint')}
              title={t('restoreHint')}
              className={cn(iconButton, 'view-enter')}
            >
              <RotateCcw className="size-4" />
            </button>
          )}
        </div>
      </footer>
    </div>
  )
}
