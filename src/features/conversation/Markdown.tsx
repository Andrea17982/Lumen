import { createContext, memo, useContext, useEffect, useState, type ReactNode } from 'react';
import type { Token, Tokens } from 'marked';
import { useT } from '../../i18n';
import { linkKind, type Block } from './blocks';
import s from './Markdown.module.css';

/** What a reply can reach outside itself: links and local images go through the daemon. */
export interface MarkdownHost {
  openLink(href: string): void;
  /** Resolves a workspace image to a data URL, or rejects. */
  image(href: string): Promise<string>;
}
export const MarkdownHostContext = createContext<MarkdownHost>({ openLink: () => {}, image: () => Promise.reject('No host') });

export function Markdown({ blocks }: { blocks: Block[] }) {
  return (
    <div className={s.markdown}>
      {blocks.map((block, i) => (
        <BlockView key={i} block={block} />
      ))}
    </div>
  );
}

/** Closed blocks keep their token object, so a streaming reply re-renders only its tail. */
const BlockView = memo(function BlockView({ block }: { block: Block }): ReactNode {
  switch (block.type) {
    case 'heading': {
      const Tag = (['h3', 'h3', 'h4', 'h5', 'h6', 'h6'] as const)[block.depth - 1];
      return <Tag>{inline(block.tokens)}</Tag>;
    }
    case 'paragraph':
      return <p>{inline(block.tokens)}</p>;
    case 'text':
      return <p>{block.tokens ? inline(block.tokens) : block.text}</p>;
    case 'code':
      return <CodeBlock code={block.text} lang={block.lang} />;
    case 'blockquote':
      return <blockquote>{children(block.tokens)}</blockquote>;
    case 'list':
      return <List list={block as Tokens.List} />;
    case 'table':
      return <Table table={block as Tokens.Table} />;
    case 'hr':
      return <hr />;
    case 'html':
      // Raw HTML from an agent is shown as text, never interpreted.
      return <p className={s.raw}>{block.text}</p>;
    default:
      return 'raw' in block ? <p>{block.raw}</p> : null;
  }
});

function children(tokens: Token[] = []) {
  return tokens.filter((t) => t.type !== 'space').map((t, i) => <BlockView key={i} block={t as Block} />);
}

function List({ list }: { list: Tokens.List }) {
  const items = list.items.map((item, i) => (
    <li key={i}>
      {item.task && <input type="checkbox" checked={!!item.checked} disabled aria-readonly="true" />}
      {/* Tight list items hold inline text; loose ones hold paragraphs. */}
      {item.tokens.map((t, j) => (t.type === 'text' && !list.loose ? <span key={j}>{'tokens' in t && t.tokens ? inline(t.tokens) : t.raw}</span> : t.type === 'space' ? null : <BlockView key={j} block={t as Block} />))}
    </li>
  ));
  return list.ordered ? <ol start={list.start === '' ? undefined : Number(list.start)}>{items}</ol> : <ul>{items}</ul>;
}

function Table({ table }: { table: Tokens.Table }) {
  const align = (i: number) => table.align[i] ?? undefined;
  return (
    <div className={s.tableScroll}>
      <table>
        <thead>
          <tr>
            {table.header.map((cell, i) => (
              <th key={i} style={{ textAlign: align(i) }}>
                {inline(cell.tokens)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {table.rows.map((row, r) => (
            <tr key={r}>
              {row.map((cell, i) => (
                <td key={i} style={{ textAlign: align(i) }}>
                  {inline(cell.tokens)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function CodeBlock({ code, lang }: { code: string; lang?: string }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const copy = () =>
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  return (
    <figure className={s.code}>
      <figcaption>
        <span>{lang || t.code.plain}</span>
        <button type="button" onClick={copy}>
          {copied ? t.code.copied : t.code.copy}
        </button>
      </figcaption>
      <pre tabIndex={0}>
        <code>{code}</code>
      </pre>
    </figure>
  );
}

function inline(tokens: Token[] = []): ReactNode[] {
  return tokens.map((token, i) => <Inline key={i} token={token} />);
}

function Inline({ token }: { token: Token }): ReactNode {
  switch (token.type) {
    case 'text':
      return 'tokens' in token && token.tokens?.length ? inline(token.tokens) : token.text;
    case 'escape':
      return token.text;
    case 'strong':
      return <strong>{inline(token.tokens)}</strong>;
    case 'em':
      return <em>{inline(token.tokens)}</em>;
    case 'del':
      return <del>{inline(token.tokens)}</del>;
    case 'codespan':
      return <code className={s.inlineCode}>{token.text}</code>;
    case 'br':
      return <br />;
    case 'link':
      return <Link href={token.href}>{inline(token.tokens)}</Link>;
    case 'image':
      return <Image href={token.href} alt={token.text} />;
    case 'html':
      return token.text;
    default:
      return 'raw' in token ? token.raw : null;
  }
}

function Link({ href, children }: { href: string; children: ReactNode }) {
  const host = useContext(MarkdownHostContext);
  const kind = linkKind(href);
  if (kind === 'blocked' || kind === 'anchor') return <span className={s.inertLink}>{children}</span>;
  // No href: the WebView must never follow agent text itself (middle-click, Ctrl+click and
  // drag would bypass the daemon's checks). Only an explicit activation asks the daemon.
  return (
    <a
      role="link"
      tabIndex={0}
      title={href}
      className={s.link}
      onClick={() => host.openLink(href)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') host.openLink(href);
      }}
    >
      {children}
    </a>
  );
}

/** Workspace images load through the daemon; remote images are never fetched (privacy). */
function Image({ href, alt }: { href: string; alt: string }) {
  const host = useContext(MarkdownHostContext);
  const [src, setSrc] = useState<string | null>(null);
  const local = linkKind(href) === 'file';
  useEffect(() => {
    if (!local) return;
    let live = true;
    host.image(href).then(
      (url) => live && url.startsWith('data:image/') && setSrc(url),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [host, href, local]);
  if (src) return <img className={s.image} src={src} alt={alt} />;
  return <Link href={href}>{alt || href}</Link>;
}
