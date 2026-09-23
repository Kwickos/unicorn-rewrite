import { cn } from 'cn'
import { RewritePanel } from '@/components/rewrite-panel'
import { useApp } from '@/hooks/use-app'
import { usePanelHeight } from '@/hooks/use-panel-height'
import { useQuitShortcut } from '@/hooks/use-quit-shortcut'
import { isDesktop } from '@/lib/desktop'
import { detectLocale, LocaleProvider } from '@/lib/i18n'

export default function App() {
  const { contentRef, height, animated } = usePanelHeight<HTMLDivElement>()
  const app = useApp()
  useQuitShortcut()

  const snapshot = app.snapshot
  const locale = snapshot?.settings.locale ?? detectLocale()

  const panel = (
    <div
      className={cn(
        'overflow-hidden',
        animated && 'transition-[height] duration-[var(--duration-fast)] ease-[var(--ease-smooth-out)]',
      )}
      style={height === null ? undefined : { height }}
    >
      <div ref={contentRef}>
        <LocaleProvider locale={locale}>
          {/* Rien tant que le natif n'a pas répondu : quelques millisecondes,
              et pas d'état par défaut trompeur à l'écran. */}
          {snapshot && <RewritePanel app={app} snapshot={snapshot} />}
        </LocaleProvider>
      </div>
    </div>
  )

  // Sur le bureau la fenêtre est transparente et sans décoration : fond, coins
  // arrondis et bord sont un verre natif posé derrière la page
  // (`src-tauri/src/panel.rs`). La page ne dessine plus que le contenu.
  if (isDesktop) {
    return <div className="text-foreground select-none">{panel}</div>
  }

  return (
    <main className="flex min-h-dvh items-center justify-center bg-muted/40 p-4">
      <div className="w-full max-w-[360px] overflow-hidden rounded-xl border bg-background shadow-sm">
        {panel}
      </div>
    </main>
  )
}
