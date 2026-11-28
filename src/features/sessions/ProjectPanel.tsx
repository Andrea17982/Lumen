import { useEffect, useState } from 'react';
import { Button, IconButton } from '../../design/components';
import { useT } from '../../i18n';
import type { Session } from '../../protocol/types';
import { useSyncClient } from '../../sync/react';
import { Checks, Routines } from './ProjectExtras';
import s from './ProjectPanel.module.css';

export function ProjectPanel({ session, folder, routines, onClose }: { session: Session; folder: string; routines: boolean; onClose: () => void }) {
  const t = useT();
  const title = routines ? t.routines.title : t.project.open;
  return (
    <aside className={s.panel} aria-label={title}>
      <header className={s.head}>
        <h2>{title}</h2>
        <IconButton icon="close" label={t.project.close} onPress={onClose} />
      </header>
      {routines ? <Routines session={session} /> : <><Notes folder={folder} /><Checks folder={folder} /></>}
    </aside>
  );
}

function Notes({ folder }: { folder: string }) {
  const t = useT();
  const sync = useSyncClient();
  const [notes, setNotes] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    sync.call<{ notes: string }>('project_context', { folder }).then(({ notes }) => {
      if (!live) return;
      setNotes(notes);
      setSaved(notes);
    }, (e) => { if (live) setError(String(e)); });
    return () => { live = false; };
  }, [sync, folder]);

  return (
    <section className={s.section}>
      <label className={s.label} htmlFor="project-notes">{t.project.notes}</label>
      <p className={s.help}>{t.project.notesHelp}</p>
      <textarea id="project-notes" className={s.notes} value={notes ?? ''} disabled={notes === null} onChange={(e) => setNotes(e.target.value)} rows={8} />
      <div className={s.actions}>
        {saved !== null && notes === saved && notes !== '' && <span className={s.help}>{t.project.saved}</span>}
        <Button variant="secondary" isDisabled={notes === null || notes === saved} onPress={() => {
          setError(null);
          sync.call('save_project_context', { folder, notes }).then(() => setSaved(notes), (e) => setError(String(e)));
        }}>
          {t.project.save}
        </Button>
      </div>
      {error && <p className={s.error} role="alert">{error}</p>}
    </section>
  );
}
