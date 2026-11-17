import { Repeat, X } from 'lucide-react';
import { useEffect, useState } from 'react';
import { call } from '../lib/bridge';
import { useI18n } from '../lib/i18n';

export type Routine = {
  id: string;
  text: string;
  everyMs: number;
  nextAt: number;
  untilAt?: number | null;
};

const every = (ms: number) =>
  ms % 604_800_000 === 0
    ? `${ms / 604_800_000}w`
    : ms % 86_400_000 === 0
      ? `${ms / 86_400_000}d`
      : ms % 3_600_000 === 0
        ? `${ms / 3_600_000}h`
        : `${ms / 60_000}m`;

/** Recurring tasks of this conversation, created with `/every <interval> <task>`. */
export default function Routines({
  session,
  version,
  onError,
}: {
  session?: string;
  version: number;
  onError: (text: string) => void;
}) {
  const { t } = useI18n();
  const [list, setList] = useState<Routine[]>([]);
  useEffect(() => {
    if (!session) return setList([]);
    call<Routine[]>('list_routines', { id: session })
      .then(setList)
      .catch(() => setList([]));
  }, [session, version]);
  if (!list.length) return null;
  return (
    <ul className="routines" aria-label={t('Recurring tasks')}>
      {list.map((r) => (
        <li key={r.id} title={`${t('Next run')}: ${new Date(r.nextAt).toLocaleString()}`}>
          <Repeat size={12} aria-hidden />
          <span>
            {t('Every')} {every(r.everyMs)}
            {r.untilAt
              ? ` ${t('until')} ${new Date(r.untilAt).toLocaleDateString(undefined, { timeZone: 'UTC' })}`
              : ''}
            : {r.text}
          </span>
          <button
            className="icon-button"
            aria-label={t('Stop recurring task')}
            onClick={() =>
              call('delete_routine', { routine: r.id })
                .then(() => setList((l) => l.filter((x) => x.id !== r.id)))
                .catch((e) => onError(String(e)))
            }
          >
            <X size={12} />
          </button>
        </li>
      ))}
    </ul>
  );
}
