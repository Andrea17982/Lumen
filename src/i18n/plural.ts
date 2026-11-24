const rules = new Map<string, Intl.PluralRules>();

/** Picks the CLDR plural form; `other` is required, the rest fall back to it. */
export function plural(n: number, locale: string, forms: Partial<Record<Intl.LDMLPluralRule, string>> & { other: string }) {
  let rule = rules.get(locale);
  if (!rule) rules.set(locale, (rule = new Intl.PluralRules(locale)));
  return forms[rule.select(n)] ?? forms.other;
}
