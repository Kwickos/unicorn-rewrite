import { useCallback, useEffect, useState } from 'react'
import {
  demoParams,
  IDLE_STATUS,
  isDesktop,
  listenNative,
  native,
  WEB_SNAPSHOT,
  type Settings,
  type Snapshot,
  type Status,
} from '@/lib/desktop'

/**
 * État de l'app, tenu côté natif. La page s'abonne aux changements, et
 * relit tout à chaque ouverture du panneau : pendant qu'il était fermé,
 * macOS a pu suspendre la page et lui faire manquer des événements.
 */
export function useApp() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(isDesktop ? null : WEB_SNAPSHOT)
  const [status, setStatus] = useState<Status>(() =>
    demoParams.get('view') === 'error' ? { ...IDLE_STATUS, phase: 'error', error: 'noSelection' } : IDLE_STATUS,
  )

  const refresh = useCallback(async () => {
    if (!isDesktop) return
    try {
      const [nextSnapshot, nextStatus] = await Promise.all([native.getState(), native.getStatus()])
      setSnapshot(nextSnapshot)
      setStatus(nextStatus)
    } catch {
      // L'app native répondra au prochain événement.
    }
  }, [])

  useEffect(() => {
    // Synchronisation avec l'app native, système externe par excellence.
    // oxlint-disable-next-line react/set-state-in-effect
    void refresh()
    const unlisteners = [
      listenNative<Status>('status', (next) =>
        // Un état plus ancien que celui affiché (événement en retard) est ignoré.
        setStatus((current) => (next.operation >= current.operation ? next : current)),
      ),
      listenNative('panel-shown', () => void refresh()),
    ]
    const onFocus = () => void refresh()
    window.addEventListener('focus', onFocus)
    return () => {
      window.removeEventListener('focus', onFocus)
      for (const unlisten of unlisteners) void unlisten.then((stop) => stop())
    }
  }, [refresh])

  const updateSettings = useCallback(
    async (patch: Partial<Omit<Settings, 'shortcut'>>) => {
      // Optimiste : le choix s'affiche tout de suite, le natif confirme.
      setSnapshot((current) =>
        current ? { ...current, settings: { ...current.settings, ...patch } } : current,
      )
      if (!isDesktop) return
      try {
        const settings = await native.updateSettings(patch)
        setSnapshot((current) => (current ? { ...current, settings } : current))
      } catch {
        void refresh()
      }
    },
    [refresh],
  )

  const patchSnapshot = useCallback((patch: Partial<Snapshot>) => {
    setSnapshot((current) => (current ? { ...current, ...patch } : current))
  }, [])

  return { snapshot, status, refresh, updateSettings, patchSnapshot }
}

export type AppModel = ReturnType<typeof useApp>
