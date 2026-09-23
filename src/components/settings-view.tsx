import { useEffect, useRef, useState } from 'react'
import { ChevronRight, Power, X } from 'lucide-react'
import { cn } from 'cn'
import { ApiKeyField } from '@/components/api-key-field'
import { ShortcutRecorder } from '@/components/shortcut-recorder'
import { Switch } from '@/components/ui/switch'
import { useAutostart } from '@/hooks/use-autostart'
import type { AppModel } from '@/hooks/use-app'
import { isDesktop, native, type Snapshot, type Theme } from '@/lib/desktop'
import { LOCALES, useT, type MessageKey } from '@/lib/i18n'
import { fieldClass, iconButton, quietAction, rowButton } from '@/lib/styles'

const THEMES: { value: Theme; label: MessageKey }[] = [
  { value: 'system', label: 'themeSystem' },
  { value: 'light', label: 'themeLight' },
  { value: 'dark', label: 'themeDark' },
]

const CUSTOM_MAX = 280

/** `qwen/qwen3.8-flash` → `qwen3.8-flash`. */
function modelLabel(id: string): string {
  return id.slice(id.indexOf('/') + 1)
}

/** Puce d'un choix parmi peu d'options : même forme que dans Unicorn Time. */
function Pill({
  active,
  onClick,
  children,
}: {
  active: boolean
  onClick: () => void
  children: React.ReactNode
}) {
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

function Row({ label, htmlFor, children }: { label: string; htmlFor?: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg px-2 py-1.5">
      {htmlFor ? (
        <label htmlFor={htmlFor} className="text-sm">
          {label}
        </label>
      ) : (
        <span className="text-sm">{label}</span>
      )}
      {children}
    </div>
  )
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-1.5 px-2 py-1.5">
      <h3 className="text-sm">{title}</h3>
      {children}
    </section>
  )
}

/** Consigne du profil personnalisé, enregistrée après une courte pause de frappe. */
function CustomInstruction({ value, onSave }: { value: string; onSave: (value: string) => void }) {
  const t = useT()
  const [draft, setDraft] = useState(value)
  const timer = useRef<number | undefined>(undefined)

  useEffect(() => () => window.clearTimeout(timer.current), [])

  const schedule = (next: string) => {
    setDraft(next)
    window.clearTimeout(timer.current)
    timer.current = window.setTimeout(() => onSave(next), 500)
  }

  return (
    <div className="flex flex-col gap-1">
      <textarea
        id="custom-instruction"
        value={draft}
        maxLength={CUSTOM_MAX}
        rows={2}
        placeholder={t('customInstructionPlaceholder')}
        onChange={(event) => schedule(event.target.value)}
        onBlur={() => {
          window.clearTimeout(timer.current)
          if (draft !== value) onSave(draft)
        }}
        className={cn(fieldClass, 'resize-none leading-snug')}
      />
    </div>
  )
}

type Props = {
  app: AppModel
  snapshot: Snapshot
  focusKey?: boolean
  onOpenModels: () => void
  onClose: () => void
}

/**
 * Une vue du panneau, pas une fenêtre flottante : un popover réintroduirait
 * une boîte avec bordure et ombre là où le reste de l'app n'en a pas.
 */
export function SettingsView({ app, snapshot, focusKey, onOpenModels, onClose }: Props) {
  const t = useT()
  const autostart = useAutostart()
  const { settings } = snapshot
  const locale = settings.locale ?? 'fr'

  return (
    <div className="flex flex-col p-1.5">
      <div className="flex items-center justify-between px-2 py-1.5">
        <h2 className="text-sm font-medium">{t('settings')}</h2>
        <button type="button" onClick={onClose} aria-label={t('closeSettings')} className={cn(iconButton, 'size-6 rounded-md')}>
          <X className="size-3.5" />
        </button>
      </div>

      <div className="scrollbar-none flex max-h-[30rem] flex-col overflow-y-auto">
        <Row label={t('shortcut')}>
          <ShortcutRecorder value={settings.shortcut} onSaved={(next) => app.patchSnapshot({ settings: next })} />
        </Row>

        <Section title={t('customInstruction')}>
          <CustomInstruction
            value={settings.customInstruction}
            onSave={(customInstruction) => void app.updateSettings({ customInstruction })}
          />
        </Section>

        <Section title={t('apiKey')}>
          <ApiKeyField
            hasKey={snapshot.hasKey}
            autoFocus={focusKey}
            onChange={(hasKey) => {
              app.patchSnapshot({ hasKey })
              void app.refresh()
            }}
          />
        </Section>

        {snapshot.hasKey && (
          <div className="flex flex-col px-2 py-1.5">
            <div className="flex items-center justify-between gap-3">
              <span className="text-sm">{t('model')}</span>
              {snapshot.modelChoice ? (
                <button type="button" onClick={onOpenModels} className={cn(quietAction, 'max-w-52 truncate')}>
                  {modelLabel(snapshot.model)}
                  <ChevronRight className="size-3.5 shrink-0" />
                </button>
              ) : (
                <span className="truncate text-xs text-muted-foreground">{modelLabel(snapshot.model)}</span>
              )}
            </div>
            {settings.upgradedFrom && (
              <span className="mt-0.5 self-end text-[0.7rem] text-muted-foreground">
                {t('modelUpgraded', { model: modelLabel(settings.upgradedFrom) })}
              </span>
            )}
          </div>
        )}

        {/* Utile seulement quand l'accès manque. */}
        {!snapshot.trusted && (
          <Row label={t('permission')}>
            <span className="flex items-center gap-1.5">
              <span className="text-xs text-destructive">{t('permissionMissing')}</span>
              {isDesktop && (
                <button
                  type="button"
                  onClick={() => void native.requestAccessibility().then(() => app.refresh())}
                  className={quietAction}
                >
                  {t('permissionOpen')}
                </button>
              )}
            </span>
          </Row>
        )}

        {isDesktop && (
          <Row label={t('launchAtLogin')} htmlFor="autostart">
            <Switch
              id="autostart"
              checked={autostart.enabled ?? false}
              disabled={autostart.enabled === null}
              onCheckedChange={(checked) => void autostart.toggle(checked)}
            />
          </Row>
        )}

        <Row label={t('appearance')}>
          <div className="flex gap-0.5">
            {THEMES.map((theme) => (
              <Pill
                key={theme.value}
                active={settings.theme === theme.value}
                onClick={() => void app.updateSettings({ theme: theme.value })}
              >
                {t(theme.label)}
              </Pill>
            ))}
          </div>
        </Row>

        <Row label={t('language')}>
          <div className="flex gap-0.5">
            {LOCALES.map((option) => (
              <Pill
                key={option.value}
                active={locale === option.value}
                onClick={() => void app.updateSettings({ locale: option.value })}
              >
                {option.label}
              </Pill>
            ))}
          </div>
        </Row>

      </div>

      {isDesktop && (
        <button type="button" onClick={() => void native.quitApp()} className={cn(rowButton, 'mt-1')}>
          <Power className="size-3.5" />
          {t('quit')}
        </button>
      )}
    </div>
  )
}
