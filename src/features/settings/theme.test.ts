import { describe, expect, it } from 'vitest';
import { themeCss } from './theme';

const theme = (tokens: Record<string, Record<string, string>>) => ({ plugin: 'p', id: 't', name: 'T', tokens });

describe('plugin themes', () => {
  it('writes tokens for each scheme', () => {
    expect(themeCss(theme({ any: { '--accent': '#b5562f' }, dark: { '--accent': '#e08a5c' } }))).toBe([':root { --accent: #b5562f; }', ":root[data-theme='dark'] { --accent: #e08a5c; }"].join('\n'));
  });
  it('drops anything that could leave the block, load a resource or restyle status colours', () => {
    const css = themeCss(
      theme({ any: { '--a': 'red; } body { display: none', '--b': 'url(https://x)', '--status-danger': 'green', '--p-success': 'red', '--focus-ring': 'none', '--c': 'blue /* x', color: 'red', '--ok': '4px' } }),
    );
    expect(css).toBe(':root { --ok: 4px; }');
  });
});
