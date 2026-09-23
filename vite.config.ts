import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  // Chemins relatifs : le build se dépose tel quel dans n'importe quel
  // sous-dossier (ou dans la coquille Tauri) sans reconfiguration.
  base: './',
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': new URL('./src', import.meta.url).pathname,
    },
  },
  // Tauri attend un port fixe et ne doit pas masquer les erreurs Rust
  clearScreen: false,
  server: {
    // IPv4 explicite : sur macOS un `localhost` nu n'écoute que sur ::1,
    // et les clients qui résolvent en 127.0.0.1 se font refuser.
    host: '127.0.0.1',
    port: 1421,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
})
