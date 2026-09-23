/**
 * Accès aux tokens de mouvement définis dans `index.css`, pour le peu
 * d'animation qui passe par JavaScript (Web Animations API). Sans ça, les
 * durées se dédoubleraient entre CSS et JS.
 */

function readToken(name: string, fallback: string): string {
  if (typeof window === 'undefined') return fallback
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim()
  return value || fallback
}

/** Durée d'un token, en millisecondes. */
export function motionDuration(name: string, fallbackMs: number): number {
  const raw = readToken(name, `${fallbackMs}ms`)
  const parsed = Number.parseFloat(raw)
  if (Number.isNaN(parsed)) return fallbackMs
  return raw.endsWith('ms') ? parsed : parsed * 1000
}

export function motionEasing(name: string, fallback: string): string {
  return readToken(name, fallback)
}

/** Le réglage système d'accessibilité prime sur toute animation. */
export function prefersReducedMotion(): boolean {
  return (
    typeof window !== 'undefined' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  )
}

export type Bezier = [x1: number, y1: number, x2: number, y2: number]

/** Points de contrôle d'un token `cubic-bezier(...)` ; `null` pour un
 *  mot-clé comme `linear` ou `ease-out`. */
function readBezier(name: string, fallback: string): Bezier | null {
  const match = readToken(name, fallback).match(
    /cubic-bezier\(\s*([\d.-]+)\s*,\s*([\d.-]+)\s*,\s*([\d.-]+)\s*,\s*([\d.-]+)\s*\)/,
  )
  return match ? (match.slice(1).map(Number) as Bezier) : null
}

/** Courbe d'un token en points de contrôle, pour les animations confiées à
 *  AppKit : la fenêtre du panneau suit la même courbe que le CSS. */
export function motionBezier(name: string, fallback: Bezier): Bezier {
  return readBezier(name, `cubic-bezier(${fallback.join(', ')})`) ?? fallback
}

/**
 * Évalue une courbe `cubic-bezier(...)` telle que la déclare un token, pour
 * que les animations pilotées en JavaScript suivent exactement la même courbe
 * que celles écrites en CSS.
 */
export function motionEasingFn(name: string, fallback: string): (t: number) => number {
  const bezier = readBezier(name, fallback)
  if (!bezier) return (t) => t // `linear`, `ease-out`… : approximation suffisante ici
  const [x1, y1, x2, y2] = bezier

  const axis = (a: number, b: number, t: number) => {
    const u = 1 - t
    return 3 * u * u * t * a + 3 * u * t * t * b + t * t * t
  }

  return (progress: number) => {
    if (progress <= 0) return 0
    if (progress >= 1) return 1
    // Bissection sur le paramètre de la courbe : une dizaine d'itérations
    // suffisent largement à la précision d'un pixel.
    let low = 0
    let high = 1
    let t = progress
    for (let i = 0; i < 12; i += 1) {
      const x = axis(x1, x2, t)
      if (x < progress) low = t
      else high = t
      t = (low + high) / 2
    }
    return axis(y1, y2, t)
  }
}
