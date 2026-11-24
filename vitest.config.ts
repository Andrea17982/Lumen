import { defineConfig, mergeConfig } from 'vitest/config';
import vite from './vite.config.ts';

// Unit tests live next to the code; tests/ui belongs to Playwright. The token tests read
// tokens.css as text, which Vitest only serves when CSS processing covers that file.
export default mergeConfig(vite, defineConfig({ test: { include: ['src/**/*.test.ts'], css: { include: [/tokens\.css/] } } }));
