import { describe, expect, it } from 'vitest';
import { blocks, linkKind, StreamingBlocks } from './blocks';

const reply = `# Plan

I will change \`send_message\` and **test** it.

- first step
- second step

\`\`\`rust
fn main() {
    println!("hi");
}
\`\`\`

| file | change |
| --- | --- |
| a.rs | +3 |

<img src=x onerror=alert(1)>

Done.`;

const shape = (list: ReturnType<typeof blocks>) => list.map((b) => [b.type, b.raw.trim()]);

describe('markdown', () => {
  it('streams to the same blocks as a full parse, whatever the chunking', () => {
    for (const size of [1, 3, 7, 40]) {
      const stream = new StreamingBlocks();
      let last = stream.update('');
      for (let i = size; i < reply.length + size; i += size) last = stream.update(reply.slice(0, i));
      expect(shape(last), `chunks of ${size}`).toEqual(shape(blocks(reply)));
    }
  });

  it('keeps closed blocks as the same objects while the tail grows', () => {
    const stream = new StreamingBlocks();
    const first = stream.update('Intro paragraph.\n\nSecond');
    const next = stream.update('Intro paragraph.\n\nSecond paragraph grows');
    expect(next[0]).toBe(first[0]);
    expect(next[1]).not.toBe(first[1]);
  });

  it('treats an unclosed code fence as code until it closes', () => {
    const stream = new StreamingBlocks();
    expect(stream.update('```ts\nconst a = 1;').map((b) => b.type)).toEqual(['code']);
    expect(stream.update('```ts\nconst a = 1;\n```\n\nAfter').map((b) => b.type)).toEqual(['code', 'paragraph']);
  });

  it('starts over when the reply is replaced', () => {
    const stream = new StreamingBlocks();
    stream.update('Old text\n\nmore');
    expect(shape(stream.update('New'))).toEqual([['paragraph', 'New']]);
  });

  it('keeps raw HTML as an html token, rendered as text', () => {
    expect(blocks(reply).find((b) => b.type === 'html')?.raw).toContain('onerror');
  });

  it('lets only web and workspace links leave the reply', () => {
    expect(linkKind('https://example.com')).toBe('web');
    expect(linkKind('src/main.rs')).toBe('file');
    expect(linkKind('C:\\work\\a.rs')).toBe('file');
    expect(linkKind('#top')).toBe('anchor');
    for (const href of ['javascript:alert(1)', 'file:///etc/passwd', 'ms-settings:privacy', 'data:text/html,x']) expect(linkKind(href), href).toBe('blocked');
  });
});
