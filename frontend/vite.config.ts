import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
// Fail clearly instead of silently moving to 5174, which would violate the
// backend's configured CORS origin and leave the interface empty.
export default defineConfig({ plugins: [react()], server: { port: 5174, strictPort: true } })
