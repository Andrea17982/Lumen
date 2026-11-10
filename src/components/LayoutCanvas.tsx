import { useEffect, useRef, useState } from 'react';
import { call } from '../lib/bridge';
import { errorText } from '../lib/util';
import { useI18n } from '../lib/i18n';
interface Block {
  id: string;
  label: string;
  role: string;
  x: number;
  y: number;
  width: number;
  height: number;
  notes: string;
}
interface Spec {
  version: number;
  width: number;
  height: number;
  blocks: Block[];
}
export default function LayoutCanvas({ folder }: { folder: string }) {
  const { t } = useI18n();
  const [spec, setSpec] = useState<Spec>({ version: 1, width: 800, height: 600, blocks: [] });
  const [selected, setSelected] = useState('');
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const area = useRef<HTMLDivElement>(null);
  const drag = useRef<{ id: string; x: number; y: number; left: number; top: number } | null>(null);
  useEffect(() => {
    let active = true;
    setLoaded(false);
    void call<Spec>('load_layout', { folder })
      .then((s) => {
        if (active) {
          setSpec(s);
          setLoaded(true);
        }
      })
      .catch((e) => {
        if (active) setError(errorText(e));
      });
    return () => {
      active = false;
    };
  }, [folder]);
  const block = spec.blocks.find((b) => b.id === selected);
  function update(id: string, patch: Partial<Block>) {
    setStatus('');
    setSpec((s) => ({
      ...s,
      blocks: s.blocks.map((b) => {
        if (b.id !== id) return b;
        const next = { ...b, ...patch };
        next.width = Math.max(1, Math.min(next.width, s.width));
        next.height = Math.max(1, Math.min(next.height, s.height));
        next.x = Math.max(0, Math.min(next.x, s.width - next.width));
        next.y = Math.max(0, Math.min(next.y, s.height - next.height));
        return next;
      }),
    }));
  }
  return (
    <section aria-label={t('Layout canvas')}>
      <p className="panel-hint">
        {t(
          'Move blocks with the pointer or arrow keys. Shift moves ten pixels. Save to constrain the agent’s next interface change.',
        )}
      </p>
      <div className="panel-row">
        <button
          className="text-button"
          disabled={!loaded || spec.blocks.length >= 64}
          onClick={() => {
            const id = crypto.randomUUID();
            setSpec((s) => ({
              ...s,
              blocks: [
                ...s.blocks,
                {
                  id,
                  label: `Block ${s.blocks.length + 1}`,
                  role: 'content',
                  x: 20,
                  y: 20,
                  width: 160,
                  height: 100,
                  notes: '',
                },
              ],
            }));
            setSelected(id);
            setStatus('');
          }}
        >
          {t('Add block')}
        </button>
        <button
          className="primary-button"
          disabled={!loaded || busy}
          onClick={() => {
            setBusy(true);
            setError('');
            void call<Spec>('save_layout', { folder, spec })
              .then(() =>
                setStatus(t('Layout saved. New interface tasks will receive these constraints.')),
              )
              .catch((e) => setError(errorText(e)))
              .finally(() => setBusy(false));
          }}
        >
          {t('Save layout')}
        </button>
      </div>
      <div
        ref={area}
        className="layout-canvas"
        style={{ aspectRatio: `${spec.width}/${spec.height}` }}
        aria-label={`${spec.width} × ${spec.height}`}
      >
        {spec.blocks.map((b) => (
          <button
            key={b.id}
            className={`layout-block${selected === b.id ? ' selected' : ''}`}
            style={{
              left: `${(100 * b.x) / spec.width}%`,
              top: `${(100 * b.y) / spec.height}%`,
              width: `${(100 * b.width) / spec.width}%`,
              height: `${(100 * b.height) / spec.height}%`,
            }}
            aria-label={`${b.label}, ${b.role}, x ${b.x}, y ${b.y}, ${b.width} × ${b.height}`}
            onClick={() => setSelected(b.id)}
            onKeyDown={(e) => {
              const step = e.shiftKey ? 10 : 1;
              const offset = {
                ArrowLeft: [-step, 0],
                ArrowRight: [step, 0],
                ArrowUp: [0, -step],
                ArrowDown: [0, step],
              }[e.key];
              if (offset) {
                e.preventDefault();
                update(b.id, { x: b.x + offset[0], y: b.y + offset[1] });
              }
            }}
            onPointerDown={(e) => {
              setSelected(b.id);
              drag.current = { id: b.id, x: e.clientX, y: e.clientY, left: b.x, top: b.y };
              e.currentTarget.setPointerCapture(e.pointerId);
            }}
            onPointerMove={(e) => {
              const d = drag.current;
              if (!d || d.id !== b.id || !area.current) return;
              const rect = area.current.getBoundingClientRect();
              update(b.id, {
                x: Math.round(d.left + ((e.clientX - d.x) * spec.width) / rect.width),
                y: Math.round(d.top + ((e.clientY - d.y) * spec.height) / rect.height),
              });
            }}
            onPointerUp={() => {
              drag.current = null;
            }}
            onLostPointerCapture={() => {
              drag.current = null;
            }}
          >
            {b.label}
          </button>
        ))}
      </div>
      {block && (
        <div className="layout-fields">
          <label>
            {t('Block label')}
            <input
              value={block.label}
              maxLength={200}
              onChange={(e) => update(block.id, { label: e.target.value })}
            />
          </label>
          <label>
            {t('Role')}
            <select value={block.role} onChange={(e) => update(block.id, { role: e.target.value })}>
              {['header', 'navigation', 'content', 'image', 'button', 'footer'].map((r) => (
                <option key={r}>{r}</option>
              ))}
            </select>
          </label>
          {(['x', 'y', 'width', 'height'] as const).map((key) => (
            <label key={key}>
              {key}
              <input
                type="number"
                min={key === 'width' || key === 'height' ? 1 : 0}
                value={block[key]}
                onChange={(e) => update(block.id, { [key]: Number(e.target.value) })}
              />
            </label>
          ))}
          <label>
            {t('Constraints')}
            <textarea
              value={block.notes}
              maxLength={2000}
              onChange={(e) => update(block.id, { notes: e.target.value })}
            />
          </label>
          <button
            onClick={() => {
              setSpec((s) => ({ ...s, blocks: s.blocks.filter((b) => b.id !== block.id) }));
              setSelected('');
              setStatus('');
            }}
          >
            {t('Remove block')}
          </button>
        </div>
      )}
      {status && <p role="status">{status}</p>}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}
