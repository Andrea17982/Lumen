import { useState } from 'react';
import { call } from '../lib/bridge';
import { useI18n } from '../lib/i18n';
import { errorText } from '../lib/util';
interface File {
  path: string;
  base: string | null;
  left: string | null;
  right: string | null;
  binary: boolean;
}
interface Preview {
  id: string;
  files: File[];
}
export default function ManualMerge({
  session,
  message,
  onApplied,
}: {
  session: string;
  message: string;
  onApplied: () => Promise<void>;
}) {
  const { t } = useI18n();
  const [preview, setPreview] = useState<Preview | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [choices, setChoices] = useState<Record<string, string>>({});
  const [texts, setTexts] = useState<Record<string, string>>({});
  async function open() {
    setBusy(true);
    setError('');
    try {
      setPreview(await call<Preview>('preview_manual_merge', { session, message }));
      setChoices({});
      setTexts({});
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }
  async function apply() {
    if (!preview) return;
    setBusy(true);
    setError('');
    try {
      await call('apply_manual_merge', {
        session,
        preview: preview.id,
        edits: preview.files.map((f) => ({
          path: f.path,
          side: choices[f.path],
          text: choices[f.path] === 'manual' ? (texts[f.path] ?? f.left ?? f.right ?? '') : null,
        })),
      });
      await onApplied();
      setPreview(null);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="manual-merge" aria-label={t('Manual merge')}>
      {!preview ? (
        <button className="text-button" disabled={busy} onClick={() => void open()}>
          {t(busy ? 'Loading versions…' : 'Merge both versions manually')}
        </button>
      ) : (
        <>
          <header>
            <strong>{t('Choose a result for each file')}</strong>
            <button className="text-button" disabled={busy} onClick={() => setPreview(null)}>
              {t('Close merge view')}
            </button>
          </header>
          {preview.files.map((f) => (
            <details key={f.path} open={preview.files.length === 1}>
              <summary>
                {f.path}
                {choices[f.path] ? ` · ${choices[f.path]}` : ''}
              </summary>
              {f.binary ? (
                <p>{t('Binary or large file: choose a complete version.')}</p>
              ) : (
                <div className="merge-sources">
                  <section>
                    <strong>{t('First version')}</strong>
                    <pre>{f.left ?? t('File absent')}</pre>
                  </section>
                  <section>
                    <strong>{t('Second version')}</strong>
                    <pre>{f.right ?? t('File absent')}</pre>
                  </section>
                </div>
              )}
              <label>
                {t('Result for')} {f.path}
                <select
                  aria-label={`${t('Result for')} ${f.path}`}
                  value={choices[f.path] || ''}
                  disabled={busy}
                  onChange={(e) => setChoices((c) => ({ ...c, [f.path]: e.target.value }))}
                >
                  <option value="" disabled>
                    {t('Choose result')}
                  </option>
                  <option value="left">{t('First version')}</option>
                  <option value="right">{t('Second version')}</option>
                  <option value="base">{t('Original project version')}</option>
                  <option value="delete">{t('Delete file')}</option>
                  {!f.binary && <option value="manual">{t('Edit merged text')}</option>}
                </select>
              </label>
              {choices[f.path] === 'manual' && (
                <label>
                  {t('Merged text')}
                  <textarea
                    spellCheck={false}
                    rows={14}
                    value={texts[f.path] ?? f.left ?? f.right ?? ''}
                    onChange={(e) => setTexts((p) => ({ ...p, [f.path]: e.target.value }))}
                  />
                </label>
              )}
            </details>
          ))}
          <button
            className="primary-button"
            disabled={busy || preview.files.some((f) => !choices[f.path])}
            onClick={() => void apply()}
          >
            {t(busy ? 'Applying merge…' : 'Apply reviewed merge')}
          </button>
        </>
      )}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}
