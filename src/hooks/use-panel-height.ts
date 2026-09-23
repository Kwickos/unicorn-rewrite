import { useEffect, useRef, useState } from 'react'
import { isDesktop, resizePanel } from '@/lib/desktop'
import { motionBezier, motionDuration, prefersReducedMotion } from '@/lib/motion'

const MIN_HEIGHT = 120
const MAX_HEIGHT = 560

/**
 * Hauteur du panneau, animée.
 *
 * Sur macOS, c'est la fenêtre qui s'anime, et côté natif : le fond en verre
 * est une vue AppKit qui la remplit, et lui seul dessine le bord visible du
 * panneau. On envoie la hauteur cible une fois par changement, AppKit anime
 * le cadre. Piloter la fenêtre depuis JavaScript, un `setSize` par image, ne
 * marche pas : les allers-retours avec Rust s'empilent et la fenêtre saute
 * directement à la valeur finale.
 *
 * Sur le web, le panneau est une carte dans la page : sa hauteur s'anime en
 * CSS, où le compositeur fait le travail.
 */
export function usePanelHeight<T extends HTMLElement>() {
  const contentRef = useRef<T>(null)
  const [height, setHeight] = useState<number | null>(null)
  /** Pas de transition sur la toute première mesure, sinon le panneau se
   *  déplierait depuis zéro à l'ouverture. */
  const [ready, setReady] = useState(false)

  useEffect(() => {
    const element = contentRef.current
    if (!element) return

    let current: number | null = null

    const observer = new ResizeObserver(() => {
      const measured = Math.round(element.getBoundingClientRect().height)
      const clamped = Math.min(Math.max(measured, MIN_HEIGHT), MAX_HEIGHT)
      if (clamped === current) return
      const first = current === null
      current = clamped

      if (isDesktop) {
        void resizePanel(
          clamped,
          first || prefersReducedMotion()
            ? null
            : {
                duration: motionDuration('--duration-fast', 250),
                easing: motionBezier('--ease-smooth-out', [0.22, 1, 0.36, 1]),
              },
        )
        return
      }

      setHeight(clamped)
      setReady(true)
    })
    observer.observe(element)

    return () => observer.disconnect()
  }, [])

  return { contentRef, height, animated: ready }
}
