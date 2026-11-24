import { useEffect, useState } from 'react';
import { Badge, Button } from '../../design/components';
import { useLocale, useT } from '../../i18n';
import type { Session } from '../../protocol/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import { visibleText } from '../attention/requests';
import s from './ProjectPanel.module.css';

interface Routine {
  id: string;
  text: string;
  everyMs: number;
  nextAt: number;
  anchor: string | null;
}
const DAYS = ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'];

function interval(ms: number) {
  const units: [number, string][] = [
    [604_800_000, 'w'],
    [86_400_000, 'd'],
    [3_600_000, 'h'],
  ];
  for (const [size, unit] of units) if (ms % size === 0) return `${ms / size}${unit}`;
  return `${Math.round(ms / 60_000)}m`;
}

/** This conversation's recurring tasks, added with /every; each one can be stopped here. */
export function Routines({ session }: { session: Session }) {
  const t = useT();
  const locale = useLocale();
  const sync = useSyncClient();
  const version = useSyncState((st) => st.routines);
  const [list, setList] = useState<Routine[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    sync.call<Routine[]>('list_routines', { id: session.id }).then(setList, () => setList([]));
  }, [sync, session.id, version]);
  if (!list.length) return null;
  const when = (r: Routine) => {
    if (!r.anchor) return t.routines.every(interval(r.everyMs));
    const [day, time] = r.anchor.split('@');
    if (!day) return t.routines.daily(time);
    // 1 January 2024 was a Monday; only the weekday name is used.
    const name = new Date(Date.UTC(2024, 0, 1 + DAYS.indexOf(day))).toLocaleDateString(locale === 'it' ? 'it-IT' : 'en-US', { weekday: 'long', timeZone: 'UTC' });
    return t.routines.weekly(name, time);
  };
  return (
    <section className={s.section}>
      <h3>{t.routines.title}</h3>
      <ul className={s.log}>
        {list.map((r) => (
          <li key={r.id}>
            <span className={s.class}>{when(r)}</span>
            <span className={s.action}>{visibleText(r.text)}</span>
            <Button
              variant="plain"
              onPress={() =>
                sync.call('delete_routine', { routine: r.id }).then(
                  () => setList((l) => l.filter((x) => x.id !== r.id)),
                  (e) => setError(String(e)),
                )
              }
            >
              {t.routines.stop}
            </Button>
          </li>
        ))}
      </ul>
      {error && <p className={s.help}>{error}</p>}
    </section>
  );
}

interface Check {
  id: string;
  label: string;
  command: string;
  cwd: string;
  playtest: boolean;
}
interface Result {
  ok: boolean;
  output: string;
  millis: number;
  links: { file: string; line: number }[];
}

/** The project's own checks (tests, lint, build), found by the daemon and run on request. */
export function Checks({ session }: { session: Session }) {
  const t = useT();
  const sync = useSyncClient();
  const [checks, setChecks] = useState<Check[] | null>(null);
  const [running, setRunning] = useState<string | null>(null);
  const [results, setResults] = useState<Record<string, Result | string>>({});
  useEffect(() => {
    sync.call<Check[]>('project_checks', { folder: session.workspace }).then(setChecks, () => setChecks([]));
  }, [sync, session.workspace]);
  if (!checks) return null;
  const seconds = (ms: number) => `${(ms / 1000).toFixed(1)} s`;
  const shown = checks.filter((c) => !c.playtest);
  return (
    <section className={s.section}>
      <h3>{t.checks.title}</h3>
      {shown.length === 0 && <p className={s.help}>{t.checks.none}</p>}
      <ul className={s.checks}>
        {shown.map((c) => {
          const result = results[c.id];
          return (
            <li key={c.id}>
              <div className={s.checkHead}>
                <span>{visibleText(c.label)}</span>
                {typeof result === 'object' && <Badge tone={result.ok ? 'success' : 'danger'}>{result.ok ? t.checks.passed(seconds(result.millis)) : t.checks.failed(seconds(result.millis))}</Badge>}
                <Button
                  variant="plain"
                  isDisabled={running !== null}
                  onPress={() => {
                    setRunning(c.id);
                    sync
                      .call<Result>('run_project_check', { folder: session.workspace, id: c.id, command: c.command, cwd: c.cwd })
                      .then(
                        (r) => setResults((all) => ({ ...all, [c.id]: r })),
                        (e) => setResults((all) => ({ ...all, [c.id]: String(e) })),
                      )
                      .finally(() => setRunning(null));
                  }}
                >
                  {running === c.id ? t.checks.running : t.checks.run}
                </Button>
              </div>
              <code className={s.command}>{visibleText(c.command)}</code>
              {typeof result === 'string' && <p className={s.help}>{visibleText(result)}</p>}
              {typeof result === 'object' && !result.ok && (
                <>
                  <pre className={s.output} tabIndex={0}>
                    {visibleText(result.output.slice(-4000))}
                  </pre>
                  {result.links.slice(0, 8).map((l) => (
                    <button key={`${l.file}:${l.line}`} type="button" className={s.link} onClick={() => void sync.call('open_source_location', { folder: session.workspace, file: l.file, line: l.line }).catch(() => {})}>
                      {visibleText(l.file)}:{l.line}
                    </button>
                  ))}
                </>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
