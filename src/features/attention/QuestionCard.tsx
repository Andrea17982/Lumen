import { useState } from 'react';
import { Button, Choices, TextField } from '../../design/components';
import { Icon } from '../../design/icons';
import { useT } from '../../i18n';
import type { Answers, Interaction } from '../../protocol/types';
import { collect, complete, visibleText, type QuestionState } from './requests';
import { switchText } from './quota';
import s from './Attention.module.css';

/**
 * Questions the agent asked through its CLI. A secret answer lives only in the field until it is
 * sent: then the field is cleared, and the daemon leaves it out of what it saves.
 */
export function QuestionCard({ request, state, onAnswer, onOpenSignIn }: { request: Interaction; state: QuestionState; onAnswer: (answers: Answers) => Promise<void>; onOpenSignIn: () => Promise<void> }) {
  const t = useT();
  const [chosen, setChosen] = useState<Record<string, string[]>>({});
  const [typed, setTyped] = useState<Record<string, string>>({});
  const [working, setWorking] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const connector = request.kind === 'connector';
  // Lumen's own quota question: worded here from its facts, answered only with its options.
  const quota = request.kind === 'quota-fallback' ? request.quota : undefined;
  const answers = collect(request, chosen, typed);
  const enabled = state === 'open' && !sent && !working;
  const server = connector ? request.questions.find((q) => q.id === request.actionId)?.header : undefined;
  const title = quota ? t.quota.switchTitle : connector ? (server ? t.attention.connector(visibleText(server)) : t.attention.connectorAny) : request.questions.length > 1 ? t.attention.questions : t.attention.question;

  async function submit() {
    if (!enabled || !complete(request, answers)) return;
    setWorking(true);
    setError(null);
    try {
      await onAnswer(answers);
      setSent(true);
    } catch (e) {
      setError(visibleText(String(e)));
    } finally {
      // Typed answers, secret ones included, leave the page once they are sent or refused.
      setTyped({});
      setWorking(false);
    }
  }

  return (
    <form
      className={s.card}
      aria-label={title}
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <header className={s.head}>
        <Icon name={connector ? 'shield' : 'info'} />
        <h3>{title}</h3>
      </header>
      {request.worker && <p className={s.meta}>{t.attention.worker(visibleText(request.worker.task))}</p>}
      {connector && <p className={s.meta}>{t.attention.connectorNote}</p>}
      {request.urlOrigin && (
        <p className={s.meta}>
          <code>{visibleText(request.urlOrigin)}</code>{' '}
          <Button variant="plain" isDisabled={!enabled} onPress={() => onOpenSignIn().catch((e) => setError(visibleText(String(e))))}>
            {t.attention.openSignIn}
          </Button>{' '}
          {t.attention.signInHelp}
        </p>
      )}
      {request.questions.map((q) => {
        const action = connector && q.id === request.actionId;
        const label = quota ? (
          <span>{switchText(t, quota)}</span>
        ) : (
          <>
            {q.header && !action && <span className={s.header}>{visibleText(q.header)} </span>}
            <span>{visibleText(action ? q.question.split('\n').slice(1).join('\n') || q.question : q.question)}</span>
            {q.optional && <span className={s.optional}> {t.attention.optional}</span>}
          </>
        );
        const optionLabel = (option: string) =>
          quota && Object.hasOwn(t.quota.options, option) ? t.quota.options[option] : action && Object.hasOwn(t.attention.actions, option) ? t.attention.actions[option] : visibleText(option);
        const optionHelp = (option: string, description: string) =>
          quota && Object.hasOwn(t.quota.optionHelp, option) ? t.quota.optionHelp[option] : description ? visibleText(description) : undefined;
        return (
          <div key={q.id} className={s.question}>
            {q.options.length > 0 ? (
              <Choices
                label={label}
                multiple={q.multiSelect}
                isDisabled={!enabled}
                value={chosen[q.id] ?? []}
                choices={q.options.map((o) => ({ id: o.label, label: optionLabel(o.label), description: optionHelp(o.label, o.description) }))}
                onChange={(v) => {
                  setChosen((c) => ({ ...c, [q.id]: v }));
                  // One answer: choosing an option replaces a typed one.
                  if (!q.multiSelect) setTyped((x) => ({ ...x, [q.id]: '' }));
                }}
              />
            ) : (
              <p className={s.questionText}>{label}</p>
            )}
            {!action && !quota && (
              <TextField
                label={q.secret ? t.attention.secretAnswer : q.options.length ? t.attention.otherAnswer : t.attention.yourAnswer}
                type={q.secret ? 'password' : 'text'}
                autoComplete="off"
                value={typed[q.id] ?? ''}
                isDisabled={!enabled}
                onChange={(v) => {
                  setTyped((x) => ({ ...x, [q.id]: v }));
                  if (!q.multiSelect && v) setChosen((c) => ({ ...c, [q.id]: [] }));
                }}
              />
            )}
          </div>
        );
      })}
      <div className={s.actions}>
        {state === 'unknown' && !sent && <span className={s.status}>{t.attention.unknown}</span>}
        {sent ? (
          <span className={s.status} role="status">
            {t.attention.sent}
          </span>
        ) : (
          <Button variant="primary" type="submit" isDisabled={!enabled || !complete(request, answers)}>
            {t.attention.sendAnswers}
          </Button>
        )}
      </div>
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
    </form>
  );
}
