import { Lexer, type Token, type Tokens } from 'marked';

/**
 * Markdown blocks for agent replies. Agent text is untrusted: tokens become React elements and
 * raw HTML stays visible text, so nothing is ever parsed as markup by the WebView.
 */
export type Block = Exclude<Token, Tokens.Space | Tokens.Def>;

const options = { gfm: true, breaks: false };
const finished = new Map<string, Block[]>();
const FINISHED_LIMIT = 2_000;

const visible = (tokens: Token[]) => tokens.filter((t): t is Block => t.type !== 'space' && t.type !== 'def');

/** Blocks of a finished message, parsed once per distinct text. */
export function blocks(text: string): Block[] {
  let cached = finished.get(text);
  if (!cached) {
    cached = visible(Lexer.lex(text, options));
    if (finished.size >= FINISHED_LIMIT) finished.delete(finished.keys().next().value!);
    finished.set(text, cached);
  }
  return cached;
}

/**
 * Incremental parse for a message that is still streaming. Everything before the last block is
 * closed and keeps its token objects, so React skips it; only the open tail is lexed again.
 * The last block is always re-read because more text can still change it (a list item, an
 * unclosed code fence, a table row).
 */
export class StreamingBlocks {
  private stable: Token[] = [];
  private stableLength = 0;
  private text = '';
  private result: Block[] = [];

  update(text: string): Block[] {
    if (text === this.text) return this.result;
    if (!text.startsWith(this.text)) {
      // A `replace` event rewrote the reply: start over.
      this.stable = [];
      this.stableLength = 0;
    }
    this.text = text;
    const tail = Lexer.lex(text.slice(this.stableLength), options);
    // Keep all but the last non-space token of the tail as closed.
    let last = tail.length - 1;
    while (last > 0 && tail[last].type === 'space') last--;
    for (const token of tail.slice(0, Math.max(0, last))) {
      this.stable.push(token);
      this.stableLength += token.raw.length;
    }
    this.result = visible([...this.stable, ...tail.slice(Math.max(0, last))]);
    return this.result;
  }
}

/** Only these schemes leave Lumen; the daemon checks them again before opening anything. */
export function linkKind(href: string): 'web' | 'file' | 'anchor' | 'blocked' {
  if (/^https?:\/\//i.test(href)) return 'web';
  if (href.startsWith('#')) return 'anchor';
  if (/^[a-z][a-z0-9+.-]*:/i.test(href) && !/^[a-z]:[\\/]/i.test(href)) return 'blocked';
  return 'file';
}
