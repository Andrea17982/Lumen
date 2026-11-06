import type { Provider } from '../../../lib/types';

/**
 * Optional Orchestra caps. `null` means "no cap of my own": concurrency then
 * falls back to the backend default and the total is unbounded.
 */
export interface OrchestraLimits {
  /** Sub-agents running at the same time. */
  maxConcurrent: number | null;
  /** Subtasks admitted per run. Finishing one never gives its place back. */
  maxSubtasks: number | null;
  /** Used percentage of the tightest quota window at which a provider gets no new work. */
  quotaThresholds: Partial<Record<Provider, number>>;
}

/** Matches `DEFAULT_CONCURRENT` in `src-tauri/src/scheduling.rs` and `MAX_SUBTASKS` in `orchestra.rs`. */
export const DEFAULT_CONCURRENT = 6;
export const PLAN_MAX_SUBTASKS = 12;
export const NO_LIMITS: OrchestraLimits = {
  maxConcurrent: null,
  maxSubtasks: null,
  quotaThresholds: {},
};

const PROVIDERS: Provider[] = ['claude', 'gpt', 'gemini', 'antigravity'];

function count(v: unknown, max: number): number | null {
  const n = typeof v === 'string' && v.trim() !== '' ? Number(v) : v;
  if (typeof n !== 'number' || !Number.isFinite(n) || n < 1) return null;
  return Math.min(max, Math.floor(n));
}

/** Clamp untrusted input (settings file, form fields) into a valid configuration. */
export function normalizeLimits(raw: unknown): OrchestraLimits {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const thresholds = (
    r.quotaThresholds && typeof r.quotaThresholds === 'object' ? r.quotaThresholds : {}
  ) as Record<string, unknown>;
  const quotaThresholds: OrchestraLimits['quotaThresholds'] = {};
  for (const p of PROVIDERS) {
    const v = thresholds[p];
    if (typeof v === 'number' && Number.isFinite(v) && v > 0 && v < 100) {
      quotaThresholds[p] = Math.round(v);
    }
  }
  return {
    maxConcurrent: count(r.maxConcurrent, 32),
    maxSubtasks: count(r.maxSubtasks, 999),
    quotaThresholds,
  };
}

export const concurrencyOf = (l: OrchestraLimits) => l.maxConcurrent ?? DEFAULT_CONCURRENT;

/**
 * Raise the total cap so `extra` more subtasks can be admitted. The total only
 * ever grows through an explicit call like this one.
 */
export function raiseTotal(l: OrchestraLimits, admitted: number, extra: number): OrchestraLimits {
  if (l.maxSubtasks === null || extra < 1) return l;
  return { ...l, maxSubtasks: Math.max(l.maxSubtasks, admitted) + Math.floor(extra) };
}
