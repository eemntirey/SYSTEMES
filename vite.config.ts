import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    watch: {
      ignored: [
        '**/native-core/src/obj/**',
        '**/native-core/src/bin/**',
        '**/*.assets.cache',
        '**/native/**/obj/**',
        '**/native/**/bin/**',
        '**/*.WebView2/**',
        '**/EBWebView/**',
      ]
    }
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    globals: true,
  },
})
