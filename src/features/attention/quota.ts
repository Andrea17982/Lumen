import type { Catalog } from '../../i18n/en';
import type { Interaction, Provider } from '../../protocol/types';
import { providerName } from '../settings/Accounts';
import { visibleText } from './requests';

const REASONS = ['observed', 'fallback-off', 'no-alternative', 'model-access'] as const;

/** The `quota-stop` activity: why a reply stopped on a quota and did not continue by itself. */
export interface QuotaStop {
  provider: Provider;
  model?: string | null;
  reason: (typeof REASONS)[number];
  /** The provider's own message, in the provider's language. */
  detail: string;
}

/** The `quota-alternative` activity: the equivalent model Lumen proposes on another account. */
export interface QuotaAlternative {
  accountId: string;
  account: string;
  provider: Provider;
  model: string;
  modelName: string;
  effort: string;
  weaker: boolean;
}

/** The facts behind the quota-fallback question asked before any work was observed. */
export type QuotaSwitch = NonNullable<Interaction['quota']>;

const isProvider = (value: unknown): value is Provider => typeof value === 'string' && Object.hasOwn(providerName, value);
const record = (value: unknown): Record<string, unknown> | null => (value && typeof value === 'object' ? (value as Record<string, unknown>) : null);

/** A `quota-stop` row as the daemon writes it, or null: a malformed row keeps the daemon's own error visible. */
export function quotaStopOf(value: unknown): QuotaStop | null {
  const v = record(value);
  if (!v || !isProvider(v.provider) || !REASONS.includes(v.reason as QuotaStop['reason'])) return null;
  return {
    provider: v.provider,
    model: typeof v.model === 'string' ? v.model : null,
    reason: v.reason as QuotaStop['reason'],
    detail: typeof v.detail === 'string' ? v.detail : '',
  };
}

/** A `quota-alternative` row with every field the button names, or null. */
export function alternativeOf(value: unknown): QuotaAlternative | null {
  const v = record(value);
  if (!v || !isProvider(v.provider)) return null;
  const { accountId, account, model, modelName, effort, weaker } = v;
  if (typeof accountId !== 'string' || typeof account !== 'string' || typeof model !== 'string' || typeof modelName !== 'string' || typeof effort !== 'string' || typeof weaker !== 'boolean') return null;
  return { accountId, account, provider: v.provider, model, modelName, effort, weaker };
}

export const agentLabel = (provider: Provider) => providerName[provider] ?? provider;

// Names come from catalogues and the user's account names: hidden or reordering characters
// show as escapes, so the target the user approves is the target that runs.
const shown = (text: string) => visibleText(text.length > 120 ? `${text.slice(0, 119)}…` : text);

/** What the user reads about a quota stop, in their language. The provider's words come second. */
export function quotaText(t: Catalog, stop: QuotaStop): { summary: string; detail: string } {
  const name = agentLabel(stop.provider);
  const summary =
    stop.reason === 'model-access'
      ? t.quota.modelAccess(stop.model ? shown(stop.model) : t.quota.selectedModel)
      : `${t.quota.stopped(name)} ${stop.reason === 'observed' ? t.quota.observed : stop.reason === 'fallback-off' ? t.quota.fallbackOff : t.quota.noAlternative}`;
  const detail = stop.detail.length > 600 ? `${stop.detail.slice(0, 599)}…` : stop.detail;
  return { summary, detail: detail ? t.quota.detail(name, visibleText(detail)) : '' };
}

/** The quota-fallback question in the user's language. */
export function switchText(t: Catalog, q: QuotaSwitch): string {
  const question = t.quota.switchQuestion(agentLabel(q.provider), shown(q.model), q.effort ? shown(q.effort) : t.quota.defaultEffort, shown(q.toAccount));
  return q.weaker ? `${question} ${t.quota.switchWeaker}` : question;
}

/** The label of the button that continues on the proposed model. */
export function elsewhereLabel(t: Catalog, a: QuotaAlternative): string {
  return t.recovery.elsewhere(a.effort ? `${shown(a.modelName)} · ${shown(a.effort)}` : shown(a.modelName), shown(a.account));
}
