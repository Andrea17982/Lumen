import { createContext, useContext, type ReactNode } from 'react';
import { I18nProvider as AriaI18nProvider } from 'react-aria-components';
import { en, type Catalog } from './en';
import { it } from './it';
import { languageTag } from './format';

export type Locale = 'en' | 'it';
export const catalogs: Record<Locale, Catalog> = { en, it };

/** The system language decides until the user picks one. */
export function systemLocale(): Locale {
  return navigator.languages.some((l) => l.toLowerCase().startsWith('it')) ? 'it' : 'en';
}

const Context = createContext<{ locale: Locale; t: Catalog }>({ locale: 'en', t: en });

export function I18nProvider({ locale, children }: { locale: Locale; children: ReactNode }) {
  // React Aria gets the same locale for its built-in labels, collation and number parsing.
  return (
    <Context value={{ locale, t: catalogs[locale] }}>
      <AriaI18nProvider locale={languageTag(locale)}>{children}</AriaI18nProvider>
    </Context>
  );
}

export function useT() {
  return useContext(Context).t;
}
export function useLocale() {
  return useContext(Context).locale;
}
