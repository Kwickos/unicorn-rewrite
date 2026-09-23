import { useEffect } from 'react'
import { isDesktop, quitApp } from '@/lib/desktop'

/**
 * ⌘Q depuis le panneau. L'app tourne en mode accessoire (pas d'icône dans le
 * Dock, pas de menu applicatif), donc macOS ne fournit pas ce raccourci.
 */
export function useQuitShortcut(): void {
  useEffect(() => {
    if (!isDesktop) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.metaKey && event.key.toLowerCase() === 'q') {
        event.preventDefault()
        void quitApp()
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])
}
