import { fileURLToPath } from 'node:url'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// The kit package is served from wherever a host puts it, so every asset path is relative.
export default defineConfig({
  base: './',
  plugins: [react()],
  resolve: {
    // @kition/sdk may be linked from a kition-sdk checkout (KITION_LINK=1); one React for the page.
    dedupe: ['react', 'react-dom'],
  },
  optimizeDeps: {
    exclude: ['@kition/sdk'],
    // @kition/sdk's editor reaches a CommonJS shim (through Tessera's UI library); excluded above,
    // it is not scanned, so the shim is named here to be converted for the browser.
    include: ['@kition/sdk > use-sync-external-store/shim'],
  },
  worker: { format: 'es' },
  server: {
    port: 5196,
    // The kition-sdk and kition-tessera checkouts KITION_LINK=1 links.
    fs: { allow: [fileURLToPath(new URL('..', import.meta.url)), fileURLToPath(new URL('../../kition-sdk', import.meta.url)), fileURLToPath(new URL('../../kition-tessera', import.meta.url))] },
  },
  build: {
    outDir: 'dist',
    rollupOptions: { input: { index: fileURLToPath(new URL('index.html', import.meta.url)), dev: fileURLToPath(new URL('dev.html', import.meta.url)) } },
  },
})
