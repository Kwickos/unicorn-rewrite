// Chargé avant le rendu, en script classique (la CSP interdit le JavaScript
// en ligne) : sans ça, le panneau s'ouvre en blanc pendant une frame en mode
// sombre.
;(function () {
  // Sur macOS, le panneau est posé sur un verre natif : ses teintes passent
  // en translucide avant le premier rendu (voir index.css).
  document.documentElement.classList.toggle('glass', '__TAURI_INTERNALS__' in window)
  try {
    // Simple copie du thème choisi ; les réglages eux-mêmes vivent côté natif.
    var theme = localStorage.getItem('unicorn-rewrite:theme') || 'system'
    var dark =
      theme === 'dark' ||
      (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)
    document.documentElement.classList.toggle('dark', dark)
    document.documentElement.style.colorScheme = dark ? 'dark' : 'light'
  } catch {}
})()
