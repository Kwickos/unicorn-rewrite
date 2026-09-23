import { useEffect, useRef, useState } from 'react'
import { cn } from 'cn'
import { isDesktop, native, type Settings, type ShortcutProblem } from '@/lib/desktop'
import { useT, type MessageKey } from '@/lib/i18n'
import { describeShortcut, formatShortcut, heldModifiers, shortcutFromEvent } from '@/lib/shortcut'
import { kbdClass } from '@/lib/styles'

const PROBLEMS: Record<ShortcutProblem, MessageKey> = {
  invalid: 'shortcutInvalid',
  needsModifier: 'shortcutNeedsModifier',
  typesCharacter: 'shortcutTypesCharacter',
  appShortcut: 'shortcutAppShortcut',
  system: 'shortcutSystem',
  unavailable: 'shortcutUnavailable',
}

type Props = {
  value: string
  onSaved: (settings: Settings) => void
}

/**
 * Enregistreur de raccourci. Pendant l'écoute, le raccourci global est
 * suspendu : sinon l'appuyer lancerait une reformulation au lieu d'être lu.
 */
export function ShortcutRecorder({ value, onSaved }: Props) {
  const t = useT()
  const [recording, setRecording] = useState(false)
  const [held, setHeld] = useState('')
  const [problem, setProblem] = useState<ShortcutProblem | null>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    if (!recording) return
    if (isDesktop) void native.pauseShortcut(true)

    const stop = () => {
      setRecording(false)
      setHeld('')
    }

    const onKeyDown = async (event: KeyboardEvent) => {
      event.preventDefault()
      event.stopPropagation()
      const bare = !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey
      if (event.code === 'Escape' && bare) {
        stop()
        return
      }
      setHeld(heldModifiers(event))
      const spec = shortcutFromEvent(event)
      if (!spec) return
      stop()
      if (!isDesktop) return
      try {
        onSaved(await native.setShortcut(spec))
        setProblem(null)
      } catch (error) {
        setProblem((error as ShortcutProblem) in PROBLEMS ? (error as ShortcutProblem) : 'invalid')
      }
    }
    const onKeyUp = (event: KeyboardEvent) => setHeld(heldModifiers(event))

    window.addEventListener('keydown', onKeyDown, true)
    window.addEventListener('keyup', onKeyUp, true)
    return () => {
      window.removeEventListener('keydown', onKeyDown, true)
      window.removeEventListener('keyup', onKeyUp, true)
      // Remet le raccourci enregistré (l'ancien ou le nouveau).
      if (isDesktop) void native.pauseShortcut(false)
    }
  }, [recording, onSaved])

  const names = {
    Control: t('modifierControl'),
    Alt: t('modifierAlt'),
    Shift: t('modifierShift'),
    Super: t('modifierSuper'),
  }

  return (
    <div className="flex flex-col items-end gap-1">
      <button
        ref={buttonRef}
        type="button"
        onClick={() => {
          setProblem(null)
          setRecording((current) => !current)
        }}
        onBlur={() => setRecording(false)}
        aria-label={`${t('shortcutRecord')} : ${describeShortcut(value, names)}`}
        aria-describedby={problem ? 'shortcut-problem' : undefined}
        className={cn(
          'flex h-7 min-w-16 items-center justify-center rounded-md px-2 text-sm outline-none',
          'transition-[background-color,box-shadow] duration-[var(--duration-quick)] ease-[var(--ease-smooth-out)]',
          'focus-visible:ring-2 focus-visible:ring-ring/60',
          recording ? 'bg-muted ring-2 ring-ring/60' : 'bg-muted hover:bg-muted/70',
        )}
      >
        {recording ? (
          <span className="text-xs text-muted-foreground">
            {held ? <kbd className={kbdClass}>{held}</kbd> : t('shortcutRecording')}
          </span>
        ) : (
          <kbd className={cn(kbdClass, 'bg-transparent text-sm')}>{formatShortcut(value, t('spaceKey'))}</kbd>
        )}
      </button>
      {problem && !recording && (
        <span id="shortcut-problem" role="alert" className="max-w-56 text-right text-xs text-destructive">
          {t(PROBLEMS[problem])}
        </span>
      )}
    </div>
  )
}
