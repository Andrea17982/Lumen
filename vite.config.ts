import { defineConfig } from 'vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';

// Only the config reads the environment; the client itself never sees Node types.
declare const process: { env: Record<string, string | undefined> };

// Tauri expects a fixed port in development and loads the built files from dist/ in release.
// The development gallery is a second page, left out when the Tauri CLI runs the build.
export default defineConfig({
  base: './',
  clearScreen: false,
  plugins: [react(), babel({ presets: [reactCompilerPreset()] })],
  server: { host: '127.0.0.1', port: 1420, strictPort: true },
  build: {
    outDir: 'dist',
    target: 'es2023',
    rolldownOptions: { input: process.env.TAURI_ENV_PLATFORM ? { app: 'index.html' } : { app: 'index.html', gallery: 'gallery.html' } },
  },
});
