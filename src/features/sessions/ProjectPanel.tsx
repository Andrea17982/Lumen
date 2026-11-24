import { useEffect, useState } from 'react';
import { Button, IconButton } from '../../design/components';
import { useT } from '../../i18n';
import type { Session } from '../../protocol/types';
import { useSyncClient } from '../../sync/react';
import s from './ProjectPanel.module.css';

interface Fact {
  source: string;
  text: string;
}
interface WorkState {
  goal: Fact | null;
  decisions: Fact[];
  completed: Fact[];
  pending: Fact[];
  checks: Fact[];
}
interface AuditEntry {
  message: string;
  class: 'reversible' | 'risky' | 'external';
  action: string;
  detail: string;
}

/**
 * The project beside the conversation: the notes every agent reads (LUMEN.md), what this
 * conversation has settled so far, and the actions agents took. Read from the daemon on open.
 */
export function ProjectPanel({ session, onClose }: { session: Session; onClose: () => void }) {
  const t = useT();
  const sync = useSyncClient();
  const [notes, setNotes] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [git, setGit] = useState(true);
  const [work, setWork] = useState<WorkState | null>(null);
  const [log, setLog] = useState<AuditEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    const fail = (e: unknown) => live && setError(String(e));
    sync.call<{ notes: string; git: boolean }>('project_context', { folder: session.workspace }).then((c) => {
      if (!live) return;
      setNotes(c.notes);
      setSaved(c.notes);
      setGit(c.git);
    }, fail);
    sync.call<WorkState>('session_work_state', { id: session.id }).then((w) => live && setWork(w), fail);
    sync.call<AuditEntry[]>('audit_log', { id: session.id }).then((l) => live && setLog(l), fail);
    return () => {
      live = false;
    };
  }, [sync, session.id, session.workspace]);

  const sections: [string, Fact[]][] = work
    ? [
        [t.project.goal, work.goal ? [work.goal] : []],
        [t.project.decisions, work.decisions],
        [t.project.pending, work.pending],
        [t.project.done, work.completed],
        [t.project.checks, work.checks],
      ]
    : [];

  return (
    <aside className={s.panel} aria-label={t.project.open}>
      <header className={s.head}>
        <h2>{t.project.open}</h2>
        <IconButton icon="close" label={t.project.close} onPress={onClose} />
      </header>
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
      {!git && <p className={s.note}>{t.project.noGit}</p>}
      <section className={s.section}>
        <label className={s.label} htmlFor="project-notes">
          {t.project.notes}
        </label>
        <p className={s.help}>{t.project.notesHelp}</p>
        <textarea id="project-notes" className={s.notes} value={notes ?? ''} disabled={notes === null} onChange={(e) => setNotes(e.target.value)} rows={8} />
        <div className={s.actions}>
          {saved !== null && notes === saved && notes !== '' && <span className={s.help}>{t.project.saved}</span>}
          <Button
            variant="secondary"
            isDisabled={notes === null || notes === saved}
            onPress={() =>
              sync.call('save_project_context', { folder: session.workspace, notes }).then(
                () => setSaved(notes),
                (e) => setError(String(e)),
              )
            }
          >
            {t.project.save}
          </Button>
        </div>
      </section>
      <section className={s.section}>
        <h3>{t.project.work}</h3>
        {work && sections.every(([, facts]) => facts.length === 0) ? (
          <p className={s.help}>{t.project.nothing}</p>
        ) : (
          <dl className={s.facts}>
            {sections
              .filter(([, facts]) => facts.length)
              .map(([title, facts]) => (
                <div key={title}>
                  <dt>{title}</dt>
                  {facts.map((f, i) => (
                    <dd key={i}>{f.text}</dd>
                  ))}
                </div>
              ))}
          </dl>
        )}
      </section>
      <section className={s.section}>
        <h3>{t.project.log}</h3>
        {log && log.length === 0 && <p className={s.help}>{t.project.noLog}</p>}
        {log && log.length > 0 && (
          <ul className={s.log}>
            {log.map((e, i) => (
              <li key={i} data-class={e.class}>
                <span className={s.class}>{t.project.classes[e.class] ?? e.class}</span>
                <span className={s.action}>{[e.action, e.detail].filter(Boolean).join(' · ')}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </aside>
  );
}
