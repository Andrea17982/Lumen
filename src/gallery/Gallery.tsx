import { useEffect, useState } from 'react';
import { I18nProvider, systemLocale, useT, type Locale } from '../i18n';
import { useAppearance, type Style } from '../design/appearance';
import { AppMock } from './AppMock';
import { AttentionStates } from './AttentionStates';
import { Board } from './Board';
import { Components } from './Components';
import { Button } from '../design/components';
import { AppearancePreview, WelcomeFrame } from '../features/welcome/Welcome';
import s from './Gallery.module.css';

type View = 'app' | 'board' | 'components' | 'attention' | 'welcome';
const views: View[] = ['app', 'board', 'components', 'attention', 'welcome'];
// Tests and links can open a given state: ?view=components&style=satin&theme=dark&locale=it
const query = new URLSearchParams(location.search);

/**
 * Development gallery: the app mock, the style board and the components.
 * Keys: 1 Ink, 2 Satin, L/D/S theme, E/I language, V next view, H hide toolbar.
 */
export function Gallery() {
  const [appearance, setAppearance] = useAppearance();
  const [locale, setLocale] = useState<Locale>(() => (query.get('locale') === 'it' || query.get('locale') === 'en' ? (query.get('locale') as Locale) : systemLocale()));
  const [view, setView] = useState<View>(() => (views.includes(query.get('view') as View) ? (query.get('view') as View) : 'app'));
  const [toolbar, setToolbar] = useState(query.get('toolbar') !== '0');

  useEffect(() => {
    const style = query.get('style');
    const theme = query.get('theme');
    if (style === 'ink' || style === 'satin') setAppearance({ style });
    if (theme === 'light' || theme === 'dark' || theme === 'system') setAppearance({ theme });
  }, []);

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  useEffect(() => {
    const keys: Record<string, () => void> = {
      '1': () => setAppearance({ style: 'ink' }),
      '2': () => setAppearance({ style: 'satin' }),
      l: () => setAppearance({ theme: 'light' }),
      d: () => setAppearance({ theme: 'dark' }),
      s: () => setAppearance({ theme: 'system' }),
      e: () => setLocale('en'),
      i: () => setLocale('it'),
      v: () => setView((v) => views[(views.indexOf(v) + 1) % views.length]),
      h: () => setToolbar((t) => !t),
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLElement && e.target.closest('input, textarea')) return;
      keys[e.key.toLowerCase()]?.();
    };
    addEventListener('keydown', onKey);
    return () => removeEventListener('keydown', onKey);
  }, []);

  return (
    <I18nProvider locale={locale}>
      {view === 'app' ? <AppMock /> : view === 'board' ? <Board style={appearance.style} /> : view === 'attention' ? <AttentionStates /> : view === 'welcome' ? <WelcomeGallery {...{ appearance, setAppearance, locale, setLocale }} /> : <Components appearance={appearance} setAppearance={setAppearance} setLocale={setLocale} />}
      {toolbar && (
        <div className={s.toolbar} role="toolbar" aria-label="Gallery">
          <Segment<Style> value={appearance.style} options={[['ink', 'Ink'], ['satin', 'Satin']]} onChange={(style) => setAppearance({ style })} />
          <Segment value={appearance.theme} options={[['system', 'System'], ['light', 'Light'], ['dark', 'Dark']]} onChange={(theme) => setAppearance({ theme })} />
          <Segment value={locale} options={[['en', 'EN'], ['it', 'IT']]} onChange={setLocale} />
          <Segment value={view} options={[['app', 'App'], ['board', 'Board'], ['components', 'Components'], ['attention', 'Attention'], ['welcome', 'Welcome']]} onChange={setView} />
        </div>
      )}
    </I18nProvider>
  );
}

function WelcomeGallery({ appearance, setAppearance, locale, setLocale }: { appearance: ReturnType<typeof useAppearance>[0]; setAppearance: ReturnType<typeof useAppearance>[1]; locale: Locale; setLocale: (locale: Locale) => void }) {
  const t = useT().welcome;
  const [preview, setPreview] = useState(query.get('step') === 'appearance');
  return (
    <WelcomeFrame step={preview ? 1 : 0} footer={<Button variant="primary" onPress={() => setPreview(!preview)}>{preview ? t.back : t.begin}</Button>}>
      {preview && <AppearancePreview preferences={{ appearance, setAppearance, language: locale, setLanguage: (language) => setLocale(language === 'system' ? systemLocale() : language) }} />}
    </WelcomeFrame>
  );
}

function Segment<T extends string>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <div className={s.segment}>
      {options.map(([id, label]) => (
        <button key={id} aria-pressed={value === id} onClick={() => onChange(id)}>
          {label}
        </button>
      ))}
    </div>
  );
}
