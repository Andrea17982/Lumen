import { useState, type ReactNode } from 'react';
import { IconButton, Segmented } from '../../design/components';
import { useT } from '../../i18n';
import { Accounts } from './Accounts';
import { Extensions } from './Extensions';
import { General, type Preferences } from './General';
import s from './Settings.module.css';

type Section = 'accounts' | 'general' | 'extensions';
const SECTIONS: Section[] = ['accounts', 'general', 'extensions'];
const VIEWS: Record<Section, (p: Preferences) => ReactNode> = { accounts: () => <Accounts />, general: (p) => <General preferences={p} />, extensions: () => <Extensions /> };

/** Settings replace the conversation in the main area: a few sections, few options each. */
export function Settings({ onClose, leading, preferences }: { onClose: () => void; leading?: ReactNode; preferences: Preferences }) {
  const t = useT();
  const [section, setSection] = useState<Section>('accounts');
  return (
    <section className={s.settings} aria-labelledby="settings-title">
      <header className={s.header}>
        {leading}
        <IconButton icon="close" label={t.settings.close} onPress={onClose} />
        <h1 id="settings-title">{t.settings.title}</h1>
      </header>
      <div className={s.scroll}>
        <div className={s.content}>
          {SECTIONS.length > 1 && (
            <Segmented
              label={t.settings.title}
              value={section}
              options={SECTIONS.map((id) => ({
                id,
                label: t.settings.sections[id],
              }))}
              onChange={setSection}
            />
          )}
          <h2 className={s.title}>{t.settings.sections[section]}</h2>
          {VIEWS[section](preferences)}
        </div>
      </div>
    </section>
  );
}
