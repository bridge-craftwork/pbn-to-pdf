import { readFileSync } from 'node:fs'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Read rather than imported: a JSON import needs an assertion whose syntax has
// moved twice, and this config is the one place that already does file IO.
const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8'))

export default defineConfig({
  plugins: [vue()],

  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },

  // Relative asset URLs, so the build works from a subpath as well as a root.
  base: './',

  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },

  build: {
    outDir: 'dist',
    rollupOptions: {
      output: {
        // The renderer is ~21 MB of wasm, almost all of it card art, and it
        // changes far less often than app code. Its own content-hashed chunk
        // means an app-code deploy does not re-download it for everyone.
        manualChunks(id) {
          if (id.includes('/src/wasm/')) return 'engine'
        },
      },
    },
    // The engine dwarfs the default 500 kB warning; it is noise here.
    chunkSizeWarningLimit: 25000,
    target: 'es2022',
  },

  worker: { format: 'es' },

  // `wasm-pack --target web` loads the binary with
  // `new URL('..._bg.wasm', import.meta.url)`. Excluding it from dep
  // optimisation keeps that URL intact in dev.
  optimizeDeps: {
    exclude: ['@/wasm/pbn_to_pdf_wasm.js'],
  },

  test: {
    environment: 'node',
    include: ['src/**/*.test.js'],
  },
})
