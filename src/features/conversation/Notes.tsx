import { useState } from 'react';
import { Button } from '../../design/components';
import { useT } from '../../i18n';
import type { Message } from '../../protocol/types';
import { useSyncClient } from '../../sync/react';
import { visibleText } from '../attention/requests';
import s from './Conversation.module.css';

const lines = (message: Message, kind: string) => message.activity.filter((a) => a.kind === kind).flatMap((a) => a.text.split('\n')).filter(Boolean);

/**
 * What a finished reply left behind: facts the agent proposed to remember, a recurring task it
 * scheduled, and a way to keep the task as a recipe. Each save is one daemon command.
 */
export function Notes({ sessionId, message }: { sessionId: string; message: Message }) {
  const t = useT();
  const sync = useSyncClient();
  const [done, setDone] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const proposed = lines(message, 'memory-proposed');
  const global = new Set(lines(message, 'memory-global'));
  const held = lines(message, 'memory-held');
  const saved = new Set(lines(message, 'memory-saved'));
  const recipe = message.activity.find((a) => a.kind === 'recipe-saved')?.text;
  const routine = message.activity.some((a) => a.kind === 'routine-created');
  const save = (key: string, command: string, args: Record<string, unknown>) => {
    setError(null);
    sync.call(command, { id: sessionId, message: message.id, ...args }).then(
      () => {
        setDone((d) => new Set(d).add(key));
        void sync.refreshTail(sessionId);
      },
      (e) => setError(visibleText(String(e))),
    );
  };
  const facts = [
    ...proposed.map((fact) => ({ fact, key: `g:${fact}`, kept: global.has(fact), ask: t.notes.remember, command: 'remember_global' })),
    ...held.filter((fact) => !saved.has(fact)).map((fact) => ({ fact, key: `p:${fact}`, kept: false, ask: t.notes.rememberHere, command: 'remember_project' })),
  ];
  return (
    <div className={s.notes}>
      {routine && <p role="status">{t.notes.routine}</p>}
      {facts.map(({ fact, key, kept, ask, command }) => (
        <p key={key}>
          {kept || done.has(key) ? t.notes.saved : ask}: {visibleText(fact)}{' '}
          {!kept && !done.has(key) && (
            <Button variant="plain" onPress={() => save(key, command, { fact })}>
              {t.notes.save}
            </Button>
          )}
        </p>
      ))}
      {recipe ? (
        <p role="status">{t.notes.recipeSaved(visibleText(recipe))}</p>
      ) : (
        !done.has('recipe') && (
          <div className={s.messageActions}>
            <Button variant="plain" onPress={() => save('recipe', 'save_recipe', {})}>
              {t.notes.recipe}
            </Button>
          </div>
        )
      )}
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
