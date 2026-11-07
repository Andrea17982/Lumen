import { Search } from 'lucide-react';
import { useEffect, useState } from 'react';
import { call } from '../lib/bridge';
import { errorText } from '../lib/util';
import type { RepoMapStats, RepoReference, RepoSymbol } from '../lib/types';

/**
 * Symbol search over the local repository map, with references and links that open the file at
 * the line. The index refreshes incrementally on every search.
 */
export default function RepoMap({
  folder,
  onError,
}: {
  folder: string;
  onError: (s: string) => void;
}) {
  const [query, setQuery] = useState('');
  const [symbols, setSymbols] = useState<RepoSymbol[]>([]);
  const [stats, setStats] = useState<RepoMapStats | null>(null);
  const [refs, setRefs] = useState<{ name: string; items: RepoReference[] } | null>(null);

  useEffect(() => {
    let gone = false;
    const timer = window.setTimeout(() => {
      void call<{ symbols: RepoSymbol[]; stats: RepoMapStats }>('repo_map_search', {
        folder,
        query,
      })
        .then((r) => {
          if (gone) return;
          setSymbols(r.symbols);
          setStats(r.stats);
        })
        .catch((e) => !gone && onError(errorText(e)));
    }, 180);
    return () => {
      gone = true;
      window.clearTimeout(timer);
    };
  }, [folder, query, onError]);

  async function open(file: string, line: number) {
    try {
      await call<boolean>('open_source_location', { folder, file, line: Math.max(1, line) });
    } catch (e) {
      onError(errorText(e));
    }
  }

  async function references(name: string) {
    try {
      setRefs({
        name,
        items: await call<RepoReference[]>('repo_map_references', { folder, name }),
      });
    } catch (e) {
      onError(errorText(e));
    }
  }

  return (
    <section className="tools-section" aria-label="Repository map">
      <div className="panel-label">
        Repository map{' '}
        <small>
          {stats
            ? `${stats.symbols} declarations in ${stats.files} files${stats.partial ? ' · partial: file or time limit reached' : ''}`
            : 'C#, GDScript, TypeScript, SQL, scenes'}
        </small>
      </div>
      <label className="map-search">
        <Search size={12} />
        <input
          value={query}
          placeholder="Find a class, function, scene or table"
          aria-label="Find a declaration"
          onChange={(e) => setQuery(e.currentTarget.value)}
        />
      </label>
      {query.trim() && symbols.length === 0 && <p className="panel-hint">No declarations found.</p>}
      {symbols.map((s) => (
        <div key={`${s.file}:${s.line}:${s.name}`} className="tool-row">
          <div className="panel-row">
            <strong>{s.name}</strong>
            <button className="text-button" onClick={() => void references(s.name)}>
              References
            </button>
          </div>
          <button
            className="text-button"
            aria-label={`Open ${s.file} at line ${s.line}`}
            onClick={() => void open(s.file, s.line)}
          >
            <small>
              {s.kind} · {s.file}:{s.line}
            </small>
          </button>
          {refs?.name === s.name && (
            <div className="map-refs">
              {refs.items.length === 0 && <small>No other mentions found.</small>}
              {refs.items.map((r) => (
                <button
                  key={`${r.file}:${r.line}:${r.text}`}
                  className="text-button"
                  onClick={() => void open(r.file, r.line)}
                >
                  <small>
                    {r.file}
                    {r.line ? `:${r.line}` : ''} — {r.text}
                  </small>
                </button>
              ))}
            </div>
          )}
        </div>
      ))}
    </section>
  );
}
