import { useEffect, useState } from 'react';
import type { Provider } from '../../../lib/types';
import { nameOf } from '../../../lib/util';
import { DEFAULT_CONCURRENT, normalizeLimits, type OrchestraLimits } from './limits';
import './scheduling.css';

/** A number field that commits on blur or Enter; empty means "no limit". */
function LimitField({
  label,
  hint,
  value,
  placeholder,
  max,
  suffix,
  disabled,
  onCommit,
}: {
  label: string;
  hint: string;
  value: number | null | undefined;
  placeholder: string;
  max: number;
  suffix?: string;
  disabled?: boolean;
  onCommit: (v: number | null) => void;
}) {
  const [draft, setDraft] = useState(value == null ? '' : String(value));
  useEffect(() => setDraft(value == null ? '' : String(value)), [value]);
  const commit = () => {
    const n = draft.trim() === '' ? null : Number(draft);
    // Enter then blur, or moving focus away, must not save the same value again: the save
    // disables every field while it runs, and typing in the next field would be lost.
    if (n === (value ?? null)) return;
    if (n === null || (Number.isFinite(n) && n >= 1 && n <= max)) onCommit(n);
    else setDraft(value == null ? '' : String(value));
  };
  return (
    <label className="switch-row">
      <span>
        <strong>{label}</strong>
        <small>{hint}</small>
      </span>
      <span className="limit-field">
        <input
          type="number"
          inputMode="numeric"
          min={1}
          max={max}
          placeholder={placeholder}
          value={draft}
          disabled={disabled}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => e.key === 'Enter' && commit()}
        />
        {suffix && <small>{suffix}</small>}
      </span>
    </label>
  );
}

/** Settings → Teamwork → Orchestra limits. Every field is optional. */
export default function OrchestraLimitsSettings({
  value,
  providers,
  disabled,
  onChange,
}: {
  value: OrchestraLimits;
  /** Providers with a connected account; thresholds are offered only for these. */
  providers: Provider[];
  disabled?: boolean;
  onChange: (next: OrchestraLimits) => void;
}) {
  const save = (patch: Partial<OrchestraLimits>) =>
    onChange(normalizeLimits({ ...value, ...patch }));
  return (
    <div className="orchestra-limits" role="group" aria-label="Orchestra limits">
      <LimitField
        label="Simultaneous agents"
        hint="Sub-agents running at once. Others wait for a free slot."
        value={value.maxConcurrent}
        placeholder={String(DEFAULT_CONCURRENT)}
        max={32}
        disabled={disabled}
        onCommit={(maxConcurrent) => save({ maxConcurrent })}
      />
      <LimitField
        label="Subtasks per run"
        hint="Admitted in total. Finished subtasks still count; raise the limit to admit more."
        value={value.maxSubtasks}
        placeholder="No limit"
        max={999}
        disabled={disabled}
        onCommit={(maxSubtasks) => save({ maxSubtasks })}
      />
      {providers.map((p) => (
        <LimitField
          key={p}
          label={`${nameOf(p)} quota threshold`}
          hint="Stop giving new work at this usage. Missing or outdated quota data is checked with one job first."
          value={value.quotaThresholds[p]}
          placeholder="Off"
          max={99}
          suffix="%"
          disabled={disabled}
          onCommit={(v) => {
            const quotaThresholds = { ...value.quotaThresholds };
            if (v === null) delete quotaThresholds[p];
            else quotaThresholds[p] = v;
            save({ quotaThresholds });
          }}
        />
      ))}
    </div>
  );
}
