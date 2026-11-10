import { useEffect, useRef, useState } from 'react';
import { call } from '../lib/bridge';
import { useI18n } from '../lib/i18n';
import { errorText } from '../lib/util';
interface Variant {
  id: string;
  label: string;
  path: string;
}
interface Started {
  id: string;
  url: string;
}
export default function VariantComparison({ session }: { session: string }) {
  const { t } = useI18n();
  const [variants, setVariants] = useState<Variant[]>([]);
  const [selected, setSelected] = useState(['project', '']);
  const [directory, setDirectory] = useState('dist');
  const [previews, setPreviews] = useState<(Started | null)[]>([null, null]);
  const active = useRef(true);
  const owned = useRef(new Set<string>());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    active.current = true;
    void call<Variant[]>('visual_variants', { session })
      .then((v) => {
        if (active.current) setVariants(v);
      })
      .catch((e) => {
        if (active.current) setError(errorText(e));
      });
    const servers = owned.current;
    return () => {
      active.current = false;
      for (const id of servers) void call('stop_variant_preview', { id }).catch(() => {});
      servers.clear();
    };
  }, [session]);
  async function load() {
    setBusy(true);
    setError('');
    const loaded: Started[] = [];
    try {
      for (const id of owned.current) await call('stop_variant_preview', { id });
      owned.current.clear();
      setPreviews([null, null]);
      for (const variant of selected) {
        const preview = await call<Started>('start_variant_preview', {
          session,
          variant,
          directory,
        });
        loaded.push(preview);
        if (!active.current) {
          await call('stop_variant_preview', { id: preview.id });
          throw Error('Preview closed');
        }
        owned.current.add(preview.id);
      }
      setPreviews(loaded);
    } catch (e) {
      for (const preview of loaded) {
        owned.current.delete(preview.id);
        void call('stop_variant_preview', { id: preview.id }).catch(() => {});
      }
      if (active.current) setError(errorText(e));
    } finally {
      if (active.current) setBusy(false);
    }
  }
  return (
    <section className="variant-comparison" aria-label={t('Worktree visual comparison')}>
      <p className="panel-hint">
        {t(
          'Build each version in its own worktree, then choose its web output folder. Lumen serves those files locally; it does not run project commands.',
        )}
      </p>
      <div className="merge-sources">
        {selected.map((id, i) => (
          <label key={i}>
            {t(i ? 'Second version' : 'First version')}
            <select
              value={id}
              onChange={(e) =>
                setSelected((values) => values.map((v, j) => (j === i ? e.target.value : v)))
              }
            >
              <option value="">{t('Choose a version')}</option>
              {variants.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.label}
                </option>
              ))}
            </select>
          </label>
        ))}
      </div>
      <label>
        {t('Web output folder')}
        <input value={directory} onChange={(e) => setDirectory(e.target.value)} />
      </label>
      <button
        disabled={busy || selected.some((id) => !id) || selected[0] === selected[1]}
        onClick={() => void load()}
      >
        {t(busy ? 'Loading…' : 'Compare rendered versions')}
      </button>
      {error && <p role="alert">{error}</p>}
      <div className="review-compare">
        {previews.map(
          (preview, i) =>
            preview && (
              <iframe
                key={preview.id}
                title={t(i ? 'Second rendered version' : 'First rendered version')}
                src={preview.url}
                sandbox="allow-scripts allow-forms"
                referrerPolicy="no-referrer"
              />
            ),
        )}
      </div>
    </section>
  );
}
