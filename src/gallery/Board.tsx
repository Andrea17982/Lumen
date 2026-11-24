import { useLocale, useT } from '../i18n';
import { Mark } from '../brand/Logo';
import type { Style } from '../design/appearance';
import s from './Board.module.css';

const swatches = [
  'surface-canvas',
  'surface-sunken',
  'surface-raised',
  'text-primary',
  'text-secondary',
  'text-tertiary',
  'accent',
  'accent-subtle',
  'status-success',
  'status-warning',
  'status-danger',
  'separator',
];

const about: Record<Style, { name: string; en: string; it: string }> = {
  ink: {
    name: 'Ink',
    en: 'Lumen’s look. Editorial and quiet: warm paper neutrals, an ember accent used sparingly, small radii, a system serif for the name.',
    it: 'Il look di Lumen. Editoriale e silenzioso: neutri caldi da carta, accento brace usato con parsimonia, raggi piccoli, un serif di sistema per il nome.',
  },
  satin: {
    name: 'Satin',
    en: 'The classic alternative in Settings: cool neutrals, an indigo accent, generous radii. Same mark, same layout.',
    it: 'L’alternativa classica nelle impostazioni: neutri freddi, accento indaco, raggi ampi. Stesso marchio, stessa struttura.',
  },
};

/** One style on a page: mark, idea, palette, type and controls. */
export function Board({ style }: { style: Style }) {
  const t = useT();
  const locale = useLocale();
  return (
    <div className={s.board}>
      <section className={s.hero}>
        <Mark size={132} />
        <h1 className={s.name}>lumen</h1>
        <p className={s.direction}>{about[style].name}</p>
        <p className={s.idea}>{about[style][locale]}</p>
        <div className={s.icons}>
          {[16, 24, 32, 64].map((size) => (
            <Mark key={size} size={size} />
          ))}
        </div>
      </section>
      <section className={s.details}>
        <h2>Palette</h2>
        <div className={s.swatches}>
          {swatches.map((name) => (
            <div key={name} className={s.swatch}>
              <span style={{ background: `var(--${name})` }} />
              <small>{name}</small>
            </div>
          ))}
        </div>
        <h2>Type</h2>
        <p className={s.display}>{t.newTask}</p>
        <p className={s.body}>
          {locale === 'it'
            ? 'Scrivi l’obiettivo: Lumen sceglie gli agenti, verifica il lavoro e ti chiede solo ciò che serve.'
            : 'Write the goal: Lumen picks the agents, checks the work and asks you only what it must.'}
        </p>
        <p className={s.mono}>cargo test -p lumen-gatekeeper · 128 passed</p>
        <h2>Controls</h2>
        <div className={s.controls}>
          <button className={s.primary}>{t.approve}</button>
          <button className={s.secondary}>{t.reject}</button>
          <span className={s.badge}>{t.needsYou}</span>
          <span className={s.status} data-tone="success">
            {t.checksPassed(12)}
          </span>
          <span className={s.status} data-tone="warning">
            {t.interrupted}
          </span>
          <input className={s.input} placeholder={t.search} aria-label={t.search} />
        </div>
      </section>
    </div>
  );
}
