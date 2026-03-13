import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

// https://vitejs.dev/config/
// Configuration for Tauri (Electron migration in progress)
export default defineConfig({
  // Required for Tauri: all asset URLs must be relative because the webview
  // loads via file:// in production.
  base: './',
  plugins: [
    react(),
    // Electron plugins removed - migrating to Tauri
  ],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      // Note: @electron alias removed for Tauri
    },
  },
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
});
