import { defineConfig } from 'vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';

// Tauri expects a fixed port in development and loads the built files from dist/ in release.
export default defineConfig({
  base: './',
  clearScreen: false,
  plugins: [react(), babel({ presets: [reactCompilerPreset()] })],
  server: { host: '127.0.0.1', port: 1420, strictPort: true },
  build: { outDir: 'dist', target: 'es2023' },
});
