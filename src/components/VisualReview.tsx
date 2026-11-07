import { Camera, ImagePlus, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { call } from '../lib/bridge';
import { errorText, IconButton } from '../lib/util';
import type { Screenshot } from '../lib/types';

const shotName = (s: Screenshot) => `${s.label} · ${s.width}×${s.height}`;

function Shot({ folder, shot, title }: { folder: string; shot: Screenshot; title: string }) {
  const [src, setSrc] = useState('');
  useEffect(() => {
    let gone = false;
    void call<string>('screenshot_data', { folder, path: shot.path })
      .then((data) => !gone && setSrc(data))
      .catch(() => !gone && setSrc(''));
    return () => {
      gone = true;
    };
  }, [folder, shot.path]);
  return (
    <figure className="review-shot">
      <figcaption>
        <strong>{title}</strong> {shotName(shot)}
        <small>
          {shot.source} · {new Date(shot.captured).toLocaleString()}
        </small>
      </figcaption>
      {src ? <img src={src} alt={`${title}: ${shot.label}`} /> : <div className="review-empty" />}
    </figure>
  );
}

/**
 * Labelled screenshots saved in the project, with a baseline and a side-by-side comparison.
 * Captures and imports happen only on request; no game or runtime service is started.
 */
export default function VisualReview({
  folder,
  onError,
}: {
  folder: string;
  onError: (s: string) => void;
}) {
  const [shots, setShots] = useState<Screenshot[]>([]);
  const [baseline, setBaseline] = useState('');
  const [candidate, setCandidate] = useState('');
  const load = useCallback(
    () =>
      call<Screenshot[]>('list_screenshots', { folder })
        .then(setShots)
        .catch((e) => onError(errorText(e))),
    [folder, onError],
  );
  useEffect(() => {
    setBaseline('');
    setCandidate('');
    void load();
  }, [load]);

  async function capture() {
    try {
      const shot = await call<Screenshot>('capture_screenshot', { folder, label: '' });
      setCandidate(shot.path);
      await load();
    } catch (e) {
      onError(errorText(e));
    }
  }

  async function importImage() {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const file = await open({
        multiple: false,
        filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
      });
      if (typeof file !== 'string') return;
      const shot = await call<Screenshot>('import_screenshot', { folder, file, label: '' });
      setCandidate(shot.path);
      await load();
    } catch (e) {
      onError(errorText(e));
    }
  }

  async function rename(shot: Screenshot, label: string) {
    if (!label.trim() || label === shot.label) return;
    try {
      await call('rename_screenshot', { folder, path: shot.path, label });
      await load();
    } catch (e) {
      onError(errorText(e));
    }
  }

  async function remove(shot: Screenshot) {
    try {
      await call('delete_screenshot', { folder, path: shot.path });
      if (baseline === shot.path) setBaseline('');
      if (candidate === shot.path) setCandidate('');
      await load();
    } catch (e) {
      onError(errorText(e));
    }
  }

  const base = shots.find((s) => s.path === baseline);
  const next = shots.find((s) => s.path === candidate);
  return (
    <section className="tools-section" aria-label="Visual review">
      <div className="panel-label">
        Visual review <small>Saved in .lumen/screenshots · not committed</small>
      </div>
      <div className="panel-row">
        <button className="text-button" onClick={() => void capture()}>
          <Camera size={12} /> Capture screen
        </button>
        <button className="text-button" onClick={() => void importImage()}>
          <ImagePlus size={12} /> Import image
        </button>
      </div>
      {shots.length === 0 && (
        <p className="panel-hint">
          No screenshots yet. Capture a screen or window, or import a device capture.
        </p>
      )}
      {shots.map((s) => (
        <div key={s.path} className="tool-row review-row">
          <input
            aria-label="Screenshot label"
            defaultValue={s.label}
            onBlur={(e) => void rename(s, e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') e.currentTarget.blur();
            }}
          />
          <small>
            {s.width}×{s.height} · {s.source}
          </small>
          <div className="panel-row">
            <label>
              <input
                type="radio"
                name="review-baseline"
                checked={baseline === s.path}
                onChange={() => setBaseline(s.path)}
              />{' '}
              Baseline
            </label>
            <label>
              <input
                type="radio"
                name="review-candidate"
                checked={candidate === s.path}
                onChange={() => setCandidate(s.path)}
              />{' '}
              Compare
            </label>
            <IconButton label={`Delete ${s.label}`} onClick={() => void remove(s)}>
              <Trash2 size={12} />
            </IconButton>
          </div>
        </div>
      ))}
      {base && next && base.path !== next.path && (
        <div className="review-compare" role="group" aria-label="Side-by-side comparison">
          <Shot folder={folder} shot={base} title="Baseline" />
          <Shot folder={folder} shot={next} title="Compare" />
          {(base.width !== next.width || base.height !== next.height) && (
            <small className="tool-conflict">
              Resolutions differ ({base.width}×{base.height} and {next.width}×{next.height}).
            </small>
          )}
        </div>
      )}
    </section>
  );
}
