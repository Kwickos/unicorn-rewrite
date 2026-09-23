import { useEffect } from 'react'
import { setWindowTheme, type Theme } from '@/lib/desktop'
import type { Locale } from '@/lib/i18n'

const THEME_CACHE = 'unicorn-rewrite:theme'

/** Applique le thème choisi ; « system » suit la préférence de l'OS. */
export function useTheme(theme: Theme): void {
  useEffect(() => {
    // Le verre du panneau est dessiné par macOS, qui doit connaître le thème
    // choisi : sinon un texte sombre se retrouverait sur un verre sombre.
    void setWindowTheme(theme)
    try {
      // Copie pour le premier rendu (theme-init.js) ; la vérité est native.
      localStorage.setItem(THEME_CACHE, theme)
    } catch {
      // Sans stockage : un éclair clair possible à l'ouverture, rien de plus.
    }

    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const apply = () => {
      const dark = theme === 'dark' || (theme === 'system' && media.matches)
      document.documentElement.classList.toggle('dark', dark)
      document.documentElement.style.colorScheme = dark ? 'dark' : 'light'
    }
    apply()
    if (theme !== 'system') return
    media.addEventListener('change', apply)
    return () => media.removeEventListener('change', apply)
  }, [theme])
}

/** Reflète la langue choisie sur le document : lecteurs d'écran et césure s'y fient. */
export function useDocumentLocale(locale: Locale): void {
  useEffect(() => {
    document.documentElement.lang = locale
  }, [locale])
}
