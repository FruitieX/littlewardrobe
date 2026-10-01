import { defineConfig } from 'vite'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [tailwindcss(), tanstackStart({ spa: { enabled: true } }), react()],
  server: {
    watch: { ignored: ['**/mockups/**'] },
    port: Number(process.env.LITTLEWARDROBE_PORT ?? 3101),
    strictPort: true,
    proxy: {
      '/api': process.env.LITTLEWARDROBE_API_URL ?? 'http://127.0.0.1:3002',
    },
  },
})
