import { useEffect, useMemo, useState } from 'react';
import { Check, FileText, Search, X } from 'lucide-react';
import { call } from '../lib/bridge';
import type { FileEntry, ProjectContext } from '../lib/types';

const formatTokens = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n));

/** Shared project memory (LUMEN.md) and the files attached to the next message. */
export default function ProjectPanel({
  folder,
  attached,
  setAttached,
  onFiles,
  onClose,
  onError,
}: {
  folder: string;
  attached: string[];
  setAttached: (files: string[]) => void;
  onFiles: (files: FileEntry[]) => void;
  onClose: () => void;
  onError: (s: string) => void;
}) {
  const [data, setData] = useState<ProjectContext | null>(null);
  const [notes, setNotes] = useState('');
  const [filter, setFilter] = useState('');
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    if (!folder) return;
    let gone = false;
    void call<ProjectContext>('project_context', { folder })
      .then((d) => {
        if (gone) return;
        setData(d);
        setNotes(d.notes);
        onFiles(d.files);
      })
      .catch((e) => onError(String(e)));
    return () => {
      gone = true;
    };
  }, [folder, onError, onFiles]);
  const shown = useMemo(() => {
    const q = filter.toLowerCase();
    const files = data?.files || [];
    const list = q ? files.filter((f) => f.path.toLowerCase().includes(q)) : files;
    return list.slice(0, 300);
  }, [data, filter]);
  const toggle = (f: FileEntry) =>
    setAttached(
      attached.includes(f.path) ? attached.filter((p) => p !== f.path) : [...attached, f.path],
    );
  return (
    <aside className="project-panel" aria-label="Project context">
      <header>
        <strong>Project context</strong>
        <button className="icon-button" aria-label="Close project context" onClick={onClose}>
          <X size={16} />
        </button>
      </header>
      {!folder ? (
        <p className="panel-hint">Choose a folder to share notes and files with both assistants.</p>
      ) : (
        <>
          <section>
            <label className="panel-label" htmlFor="lumen-notes">
              Shared notes <small>LUMEN.md · read by Claude and ChatGPT every turn</small>
            </label>
            <textarea
              id="lumen-notes"
              className="notes-editor"
              placeholder={'Rules, decisions and conventions for this project.\nverify: npm test'}
              value={notes}
              spellCheck={false}
              onChange={(e) => {
                setNotes(e.target.value);
                setSaved(false);
              }}
            />
            <div className="panel-row">
              <small>
                {data?.verify ? (
                  <>
                    Self-healing check: <code>{data.verify}</code>
                  </>
                ) : (
                  'Add “verify: <command>” to enable self-healing checks.'
                )}
              </small>
              <button
                className="text-button"
                disabled={notes === (data?.notes ?? '')}
                onClick={() =>
                  void call('save_project_context', { folder, notes })
                    .then(() => {
                      setData((d) => (d ? { ...d, notes } : d));
                      setSaved(true);
                    })
                    .catch((e) => onError(String(e)))
                }
              >
                {saved ? (
                  <>
                    <Check size={13} /> Saved
                  </>
                ) : (
                  'Save'
                )}
              </button>
            </div>
          </section>
          <section className="file-section">
            <div className="panel-label">
              Attach files{' '}
              <small>
                {attached.length ? `${attached.length} attached` : 'Sent with your next message'}
              </small>
            </div>
            <label className="search panel-search">
              <Search size={14} />
              <input
                aria-label="Filter files"
                placeholder="Filter files"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
              />
            </label>
            <div className="file-list">
              {shown.map((f) => (
                <label key={f.path} className={`file-row ${f.noise ? 'noise' : ''}`}>
                  <input
                    type="checkbox"
                    checked={attached.includes(f.path)}
                    disabled={f.noise && !attached.includes(f.path)}
                    onChange={() => toggle(f)}
                  />
                  <FileText size={13} />
                  <span title={f.path}>{f.path}</span>
                  <small>{f.noise ? 'skipped' : formatTokens(f.tokens)}</small>
                </label>
              ))}
              {data && data.files.length === 0 && <p className="panel-hint">No files found.</p>}
            </div>
          </section>
        </>
      )}
    </aside>
  );
}
