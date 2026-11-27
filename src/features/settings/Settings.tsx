import { useState, type ReactNode } from 'react';
import { Button, IconButton, Segmented } from '../../design/components';
import { useT } from '../../i18n';
import { Accounts } from './Accounts';
import { Extensions } from './Extensions';
import { General, type Preferences } from './General';
import { Maintenance } from './Maintenance';
import s from './Settings.module.css';

type Section = 'accounts' | 'general' | 'extensions' | 'maintenance';
const SECTIONS: Section[] = ['accounts', 'general', 'extensions', 'maintenance'];
const VIEWS: Record<Section, (p: Preferences) => ReactNode> = { accounts: () => <Accounts />, general: (p) => <General preferences={p} />, extensions: () => <Extensions />, maintenance: () => <Maintenance /> };

/** Settings replace the conversation in the main area: a few sections, few options each. */
export function Settings({ onClose, leading, preferences, initialSection = 'accounts', onWelcome, onTour }: { onClose: () => void; leading?: ReactNode; preferences: Preferences; initialSection?: Section; onWelcome: () => void; onTour: () => void }) {
  const t = useT();
  const [section, setSection] = useState<Section>(initialSection);
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
          <h2 className={s.title} data-tour-target={section === 'maintenance' ? 'maintenance' : undefined}>{t.settings.sections[section]}</h2>
          {VIEWS[section](preferences)}
          {section === 'general' && (
            <div className={s.fields}>
              <div><Button variant="secondary" onPress={onWelcome}>{t.welcome.reopen}</Button></div>
              <div><Button variant="secondary" onPress={onTour}>{t.welcome.startTour}</Button></div>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
