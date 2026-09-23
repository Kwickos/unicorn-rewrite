import { useCallback, useEffect, useState } from 'react'
import { isDesktop } from '@/lib/desktop'

/**
 * Lancement à l'ouverture de session.
 *
 * L'état ne vit pas dans les préférences : la vérité est le fichier
 * LaunchAgent posé par macOS, que l'utilisateur peut retirer depuis les
 * Réglages Système. On le lit donc à chaque ouverture du panneau plutôt que
 * d'afficher une valeur mémorisée qui pourrait mentir.
 */
export function useAutostart() {
  /** `null` tant qu'on ne sait pas — le réglage reste inactif d'ici là. */
  const [enabled, setEnabled] = useState<boolean | null>(null)

  useEffect(() => {
    if (!isDesktop) return
    let cancelled = false

    void (async () => {
      try {
        const { isEnabled } = await import('@tauri-apps/plugin-autostart')
        const current = await isEnabled()
        if (!cancelled) setEnabled(current)
      } catch {
        // Plugin indisponible : on laisse le réglage inactif plutôt que de
        // proposer un interrupteur qui ne ferait rien.
      }
    })()

    return () => {
      cancelled = true
    }
  }, [])

  const toggle = useCallback(async (next: boolean) => {
    if (!isDesktop) return
    // Optimiste : l'interrupteur suit le doigt, et on se recale sur l'état
    // réel juste après.
    setEnabled(next)
    try {
      const { enable, disable, isEnabled } = await import(
        '@tauri-apps/plugin-autostart'
      )
      if (next) await enable()
      else await disable()
      setEnabled(await isEnabled())
    } catch {
      setEnabled(!next)
    }
  }, [])

  return { enabled, toggle }
}
