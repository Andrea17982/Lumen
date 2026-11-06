import { useState } from 'react';
import { saveSettings, useAppSettings } from '../lib/settings';
import type { AppSettings } from '../lib/types';
import { Switch } from './controls';

/** Settings → Teamwork: how the providers help each other. */
export default function TeamworkSettings({
  advanced,
  onError,
}: {
  advanced: boolean;
  onError: (s: string) => void;
}) {
  const s = useAppSettings(onError);
  const [busy, setBusy] = useState(false);
  async function save(next: AppSettings) {
    setBusy(true);
    try {
      await saveSettings(next);
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  }
  if (!s) return null;
  return (
    <section className="settings-section">
      <h3>Teamwork</h3>
      <p>How Claude, ChatGPT and the other connected assistants help each other.</p>
      <Switch
        label="Orchestra"
        hint="For complex tasks, plan and split work across agents. Works with Auto."
        checked={s.orchestra}
        disabled={busy}
        onChange={(v) => void save({ ...s, orchestra: v })}
      />
      <Switch
        label="Switch on rate limit"
        hint="If one provider hits its limit, another continues the same message."
        checked={s.autoFallback}
        disabled={busy}
        onChange={(v) => void save({ ...s, autoFallback: v })}
      />
      {advanced && (
        <>
          <Switch
            label="Self-healing"
            hint="After edits, run the project check and fix failures automatically (up to 3 times)."
            checked={s.selfHeal}
            disabled={busy}
            onChange={(v) => void save({ ...s, selfHeal: v })}
          />
          <div className="switch-row">
            <span>
              <strong>Cross-audit rounds</strong>
              <small>How many review and revise cycles an audit may run.</small>
            </span>
            <div className="auto-policy" role="group" aria-label="Cross-audit rounds">
              {[1, 2, 3].map((n) => (
                <button
                  key={n}
                  aria-pressed={s.reviewRounds === n}
                  disabled={busy}
                  onClick={() => void save({ ...s, reviewRounds: n })}
                >
                  {n}
                </button>
              ))}
            </div>
          </div>
        </>
      )}
    </section>
  );
}
