import type { Locale } from './index';

// Intl formatters per locale, created once. Lumen shows absent values as absent, never as zero.
const tags: Record<Locale, string> = { en: 'en-US', it: 'it-IT' };
const cache = new Map<string, Intl.NumberFormat | Intl.DateTimeFormat | Intl.RelativeTimeFormat>();
function get<T>(key: string, make: () => T): T {
  let f = cache.get(key) as T | undefined;
  if (!f) cache.set(key, (f = make()) as never);
  return f;
}

export const languageTag = (locale: Locale) => tags[locale];

export function formatNumber(locale: Locale, n: number | null | undefined) {
  return n == null ? '—' : get(`n:${locale}`, () => new Intl.NumberFormat(tags[locale])).format(n);
}

/** `used` is a 0–100 percentage, as the daemon reports quota windows. */
export function formatPercent(locale: Locale, used: number | null | undefined) {
  return used == null ? '—' : get(`p:${locale}`, () => new Intl.NumberFormat(tags[locale], { style: 'percent', maximumFractionDigits: 0 })).format(used / 100);
}

export function formatTime(locale: Locale, ms: number) {
  return get(`t:${locale}`, () => new Intl.DateTimeFormat(tags[locale], { hour: '2-digit', minute: '2-digit' })).format(ms);
}

export function formatDate(locale: Locale, ms: number) {
  return get(`d:${locale}`, () => new Intl.DateTimeFormat(tags[locale], { day: 'numeric', month: 'short', year: 'numeric' })).format(ms);
}

/** "in 3 hours", "tra 3 ore", "5 minutes ago": picks the largest whole unit. */
export function formatRelative(locale: Locale, ms: number, now = Date.now()) {
  const rtf = get(`r:${locale}`, () => new Intl.RelativeTimeFormat(tags[locale], { numeric: 'auto' }));
  const diff = (ms - now) / 1000;
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ['day', 86400],
    ['hour', 3600],
    ['minute', 60],
  ];
  for (const [unit, size] of units) if (Math.abs(diff) >= size) return rtf.format(Math.round(diff / size), unit);
  return rtf.format(Math.round(diff), 'second');
}
