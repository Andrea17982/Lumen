import { useEffect, useRef, useState } from 'react';
import { X } from 'lucide-react';
import { call } from '../lib/bridge';
import { useI18n } from '../lib/i18n';
import { errorText } from '../lib/util';
import ProjectTools from './ProjectTools';
import VisualReview from './VisualReview';
import LayoutCanvas from './LayoutCanvas';
import GameControls from './GameControls';
import VariantComparison from './VariantComparison';
interface Table {
  columns: string[];
  rows: unknown[][];
  truncated: boolean;
}
export default function ProjectInspector({
  folder,
  session,
  onClose,
  onError,
}: {
  folder: string;
  session?: string;
  onClose: () => void;
  onError: (s: string) => void;
}) {
  const { t } = useI18n();
  const [tab, setTab] = useState('web');
  const [imageFile, setImageFile] = useState('');
  const close = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const opener = document.activeElement as HTMLElement;
    close.current?.focus();
    return () => {
      if (opener?.isConnected) opener.focus();
    };
  }, []);
  const tabs = [
    'web',
    'image',
    'data',
    'changes',
    'visual',
    'layout',
    'game',
    ...(session ? ['variants'] : []),
  ];
  return (
    <aside
      className="project-inspector popup"
      aria-label={t('Project inspector')}
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <header>
        <strong>{t('Project inspector')}</strong>
        <button
          ref={close}
          className="icon-button"
          aria-label={t('Close inspector')}
          onClick={onClose}
        >
          <X size={17} />
        </button>
      </header>
      <nav aria-label={t('Inspector tools')}>
        {tabs.map((id) => (
          <button key={id} aria-pressed={tab === id} onClick={() => setTab(id)}>
            {t(
              {
                web: 'Web preview',
                image: 'Image',
                data: 'Data',
                changes: 'Changes',
                visual: 'Visual review',
                layout: 'Layout canvas',
                game: 'Game controls',
                variants: 'Compare versions',
              }[id] || id,
            )}
          </button>
        ))}
      </nav>
      <div className="inspector-content" key={folder}>
        {tab === 'web' && <WebPreview />}
        {tab === 'image' && <ImageInspector folder={folder} initialFile={imageFile} />}
        {tab === 'data' && <DataInspector folder={folder} />}
        {tab === 'changes' && <ProjectTools folder={folder} onError={onError} />}
        {tab === 'visual' && <VisualReview folder={folder} onError={onError} />}
        {tab === 'layout' && <LayoutCanvas folder={folder} />}
        {tab === 'game' && (
          <GameControls
            folder={folder}
            onInspect={(file) => {
              setImageFile(file);
              setTab('image');
            }}
          />
        )}
        {tab === 'variants' && session && <VariantComparison key={session} session={session} />}
      </div>
    </aside>
  );
}
function WebPreview() {
  const { t } = useI18n();
  const [draft, setDraft] = useState('http://localhost:3000');
  const [url, setUrl] = useState('');
  const [error, setError] = useState('');
  const [version, setVersion] = useState(0);
  return (
    <>
      <form
        className="inspector-form"
        onSubmit={(e) => {
          e.preventDefault();
          try {
            const u = new URL(draft);
            if (
              !['http:', 'https:'].includes(u.protocol) ||
              !['localhost', '127.0.0.1'].includes(u.hostname) ||
              u.username ||
              u.password
            )
              throw Error(t('Enter a localhost web address.'));
            setUrl(u.href);
            setError('');
            setVersion((n) => n + 1);
          } catch (e) {
            setError(errorText(e));
          }
        }}
      >
        <label>
          {t('Preview address')}
          <input type="url" value={draft} onChange={(e) => setDraft(e.target.value)} />
        </label>
        <button type="submit">{t('Load preview')}</button>
      </form>
      {error && <p role="alert">{error}</p>}
      {url ? (
        <iframe
          key={version}
          title={t('Project web preview')}
          src={url}
          sandbox="allow-scripts allow-forms"
          referrerPolicy="no-referrer"
        />
      ) : (
        <p className="panel-hint">{t('Start the project server, then load its address.')}</p>
      )}
    </>
  );
}
function ImageInspector({ folder, initialFile = '' }: { folder: string; initialFile?: string }) {
  const { t } = useI18n();
  const [file, setFile] = useState(initialFile);
  const [src, setSrc] = useState('');
  const [error, setError] = useState('');
  const [zoom, setZoom] = useState(4);
  const [grid, setGrid] = useState(true);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    if (!initialFile) return;
    let active = true;
    void call<string>('response_image_data', { folder, href: initialFile })
      .then((data) => {
        if (active) setSrc(data);
      })
      .catch((e) => {
        if (active) setError(errorText(e));
      });
    return () => {
      active = false;
    };
  }, [folder, initialFile]);
  return (
    <>
      <form
        className="inspector-form"
        onSubmit={(e) => {
          e.preventDefault();
          setError('');
          void call<string>('response_image_data', { folder, href: file })
            .then(setSrc)
            .catch((e) => setError(errorText(e)));
        }}
      >
        <label>
          {t('Project image path')}
          <input value={file} onChange={(e) => setFile(e.target.value)} required />
        </label>
        <button type="submit">{t('Open image')}</button>
      </form>
      {error && <p role="alert">{error}</p>}
      {src && (
        <>
          <div className="panel-row">
            <label>
              {t('Zoom')} {zoom}×{' '}
              <input
                aria-label={t('Image zoom')}
                type="range"
                min="1"
                max="16"
                value={zoom}
                onChange={(e) => setZoom(Number(e.target.value))}
              />
            </label>
            <label>
              <input type="checkbox" checked={grid} onChange={(e) => setGrid(e.target.checked)} />
              {t('Pixel grid')}
            </label>
          </div>
          <p>
            {size.width} × {size.height} px
          </p>
          <div className="sprite-viewport">
            <div
              className={grid ? 'sprite-grid' : ''}
              style={{
                width: size.width * zoom,
                height: size.height * zoom,
                backgroundSize: `${zoom}px ${zoom}px`,
              }}
            >
              <img
                src={src}
                alt={file}
                onLoad={(e) =>
                  setSize({
                    width: e.currentTarget.naturalWidth,
                    height: e.currentTarget.naturalHeight,
                  })
                }
                style={{ width: size.width * zoom, height: size.height * zoom }}
              />
            </div>
          </div>
        </>
      )}
    </>
  );
}
function DataInspector({ folder }: { folder: string }) {
  const { t } = useI18n();
  const [file, setFile] = useState('');
  const [sql, setSql] = useState('SELECT name, type FROM sqlite_schema ORDER BY name');
  const [json, setJson] = useState('');
  const [table, setTable] = useState<Table | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const sqlite = /\.(db|sqlite3?|db3)$/i.test(file);
  return (
    <>
      <form
        className="inspector-form"
        onSubmit={(e) => {
          e.preventDefault();
          setBusy(true);
          setError('');
          setTable(null);
          setJson('');
          void (
            sqlite
              ? call<Table>('inspect_sqlite', { folder, file, sql }).then(setTable)
              : call<string>('inspect_json', { folder, file }).then(setJson)
          )
            .catch((e) => setError(errorText(e)))
            .finally(() => setBusy(false));
        }}
      >
        <label>
          {t('JSON or SQLite project path')}
          <input required value={file} onChange={(e) => setFile(e.target.value)} />
        </label>
        {sqlite && (
          <label>
            {t('Read-only SQL')}
            <textarea value={sql} onChange={(e) => setSql(e.target.value)} rows={4} />
          </label>
        )}
        <button disabled={busy}>{t(busy ? 'Loading…' : 'Inspect data')}</button>
      </form>
      {error && <p role="alert">{error}</p>}
      {json && <pre className="inspector-data">{json}</pre>}
      {table && (
        <>
          <div className="inspector-table">
            <table>
              <thead>
                <tr>
                  {table.columns.map((c, i) => (
                    <th key={i}>{c}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {table.rows.map((r, i) => (
                  <tr key={i}>
                    {r.map((v, j) => (
                      <td key={j}>{v === null ? 'NULL' : String(v)}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p role="status">
            {table.rows.length} {t('rows')}
            {table.truncated ? ` · ${t('Preview truncated')}` : ''}
          </p>
        </>
      )}
    </>
  );
}
