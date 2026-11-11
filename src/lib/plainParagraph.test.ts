import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { describe, expect, it } from 'vitest';
import { plainParagraph } from './plainParagraph';

const oracle = (text: string) =>
  renderToStaticMarkup(createElement(ReactMarkdown, { remarkPlugins: [remarkGfm] }, text));
const output = (text: string) => (text ? renderToStaticMarkup(createElement('p', null, text)) : '');
describe('plain paragraph fast path versus independent installed Markdown parser', () => {
  it('keeps literal Unicode, punctuation, intraword identifiers and paragraph whitespace exact', () => {
    for (const text of [
      '',
      'Hello world.',
      '  A retained decision.  ',
      '   Three spaces',
      'PERF_SEARCH_4999 is a source_identifier.',
      'L’app è pronta: città, 日本語, 🚀.',
      'Price $12.50; a+b=c (result).',
      'foo___bar',
      '\u00a0text\u00a0',
    ]) {
      const plain = plainParagraph(text);
      expect(plain, JSON.stringify(text)).not.toBeUndefined();
      expect(output(plain!), JSON.stringify(text)).toBe(oracle(text));
    }
  });
  it('delegates every syntax-bearing or ambiguous case to the original renderer', () => {
    for (const text of [
      '# Title',
      'text *emphasis*',
      '_emphasis_',
      'a **strong** word',
      '~~removed~~',
      '`code`',
      '[link](counter.ts)',
      '![image](fixture.png)',
      'https://example.test',
      'www.example.test',
      'person@example.test',
      '<script>bad()</script>',
      '&amp; &#65;',
      'one\\*two',
      '- list',
      '-',
      '+',
      '1. list',
      '  + list',
      '---',
      '    code',
      'a\nb',
      'a\r\nb',
      'a\tb',
      '\ufeffhello',
      'null\u0000byte',
      'a | b',
    ])
      expect(plainParagraph(text), text).toBeUndefined();
  });
  it('matches the full parser on a fixed generated punctuation/Unicode corpus whenever eligible', () => {
    let seed = 11731;
    const chars = 'abc XYZ012_.,:;!?()/-+\"\'è日🚀*&<>[]\\\t\n';
    let eligible = 0;
    for (let i = 0; i < 1200; i++) {
      const pool = i % 2 ? chars : 'abc XYZ012.,:;!?()è日';
      let text = '';
      for (let n = 0; n < 1 + (i % 50); n++) {
        seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
        text += pool[seed % pool.length];
      }
      const plain = plainParagraph(text);
      if (plain !== undefined) {
        eligible++;
        expect(output(plain), JSON.stringify(text)).toBe(oracle(text));
      }
    }
    expect(eligible).toBeGreaterThan(100);
  });
  it('preserves very long literal messages and delegates embedded URL syntax', () => {
    for (const text of ['x'.repeat(680_000), 'prefix' + ' '.repeat(100_000) + 'suffix']) {
      const plain = plainParagraph(text);
      expect(plain).toBe(text);
      expect(output(plain!)).toBe(oracle(text));
      expect(plainParagraph(text + ' https://example.test')).toBeUndefined();
    }
  });
});
