import { useEffect, useRef, useState } from 'react';
import { call } from '../lib/bridge';
import { useI18n } from '../lib/i18n';
import { errorText } from '../lib/util';
import type { Activity } from '../lib/types';

interface Question {
  id: string;
  header: string;
  question: string;
  options: { label: string; description: string }[];
  multiSelect: boolean;
  secret: boolean;
}
interface Request {
  id: string;
  questions: Question[];
}
function parse<T>(text: string): T | null {
  try {
    return JSON.parse(text) as T;
  } catch {
    return null;
  }
}
export default function NativeQuestions({
  activity,
  live,
}: {
  activity: Activity[];
  live: boolean;
}) {
  return (
    <>
      {activity
        .filter((a) => a.kind === 'interaction')
        .map((a) => {
          const request = parse<Request>(a.text);
          if (!request?.id || !Array.isArray(request.questions)) return null;
          const result = activity
            .filter((a) => a.kind === 'interaction-result')
            .map((a) =>
              parse<{ id: string; answers: Record<string, string[]>; cancelled?: boolean }>(a.text),
            )
            .find((r) => r?.id === request.id);
          return (
            <QuestionForm
              key={request.id}
              request={request}
              result={result?.answers}
              cancelled={!!result?.cancelled}
              live={live}
            />
          );
        })}
    </>
  );
}
function QuestionForm({
  request,
  result,
  live,
  cancelled,
}: {
  request: Request;
  result?: Record<string, string[]>;
  live: boolean;
  cancelled: boolean;
}) {
  const { t } = useI18n();
  const [answers, setAnswers] = useState<Record<string, string[]>>({});
  const [custom, setCustom] = useState<Record<string, string>>({});
  const [pending, setPending] = useState<boolean | null>(null);
  const [statusFailed, setStatusFailed] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const status = useRef<HTMLElement>(null);
  const restoreAnswerFocus = useRef(false);
  const [sent, setSent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    if (!restoreAnswerFocus.current || busy || (!sent && !result && !error)) return;
    if (
      document.activeElement === document.body ||
      form.current?.contains(document.activeElement)
    ) {
      if (error) form.current?.querySelector<HTMLButtonElement>('button[type="submit"]')?.focus();
      else status.current?.focus();
    }
    restoreAnswerFocus.current = false;
  }, [sent, result, error, busy]);
  useEffect(() => {
    if (!live || sent || result) {
      setPending(false);
      return;
    }
    setPending(null);
    setStatusFailed(false);
    let mounted = true;
    const check = () =>
      void call<string[]>('pending_questions')
        .then((ids) => {
          if (mounted) setPending(ids.includes(request.id));
          if (mounted) setStatusFailed(false);
        })
        .catch(() => {
          if (mounted) {
            setPending(false);
            setStatusFailed(true);
          }
        });
    check();
    const timer = setInterval(check, 1500);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, [request.id, live, sent, result]);
  const enabled = pending === true && !sent && !result;
  const finalAnswers = Object.fromEntries(
    request.questions.map((q) => [
      q.id,
      custom[q.id]?.trim() ? [...(answers[q.id] || []), custom[q.id].trim()] : answers[q.id] || [],
    ]),
  );
  return (
    <form
      ref={form}
      className="native-questions"
      aria-label={t('Questions from the agent')}
      onSubmit={(e) => {
        e.preventDefault();
        if (!enabled || busy) return;
        restoreAnswerFocus.current = true;
        setBusy(true);
        setError('');
        void call('answer_question', { id: request.id, answers: finalAnswers })
          .then(() => {
            setSent(true);
            setCustom({});
          })
          .catch((e) => setError(errorText(e)))
          .finally(() => setBusy(false));
      }}
    >
      {request.questions.map((q) => (
        <fieldset key={q.id} disabled={!enabled || busy}>
          <legend>
            {q.header && <small>{q.header}</small>}
            {q.question}
          </legend>
          <div className="native-options">
            {q.options.map((o) => (
              <label key={o.label}>
                <input
                  type={q.multiSelect ? 'checkbox' : 'radio'}
                  name={`${request.id}-${q.id}`}
                  value={o.label}
                  checked={(answers[q.id] || []).includes(o.label)}
                  onChange={() => {
                    setAnswers((prev) => ({
                      ...prev,
                      [q.id]: q.multiSelect
                        ? (prev[q.id] || []).includes(o.label)
                          ? prev[q.id].filter((s) => s !== o.label)
                          : [...(prev[q.id] || []), o.label]
                        : [o.label],
                    }));
                    if (!q.multiSelect) setCustom((prev) => ({ ...prev, [q.id]: '' }));
                  }}
                />
                <span>
                  <strong>{o.label}</strong>
                  {o.description && <small>{o.description}</small>}
                </span>
              </label>
            ))}
          </div>
          <label className="native-custom">
            {q.options.length ? t('Write a different answer') : t('Your answer')}
            <input
              type={q.secret ? 'password' : 'text'}
              autoComplete="off"
              value={custom[q.id] || ''}
              onChange={(e) => {
                setCustom((prev) => ({ ...prev, [q.id]: e.target.value }));
                if (!q.multiSelect) setAnswers((prev) => ({ ...prev, [q.id]: [] }));
              }}
            />
          </label>
          {result?.[q.id] && <p className="native-answer">{result[q.id].join(', ')}</p>}
        </fieldset>
      ))}
      {enabled ? (
        <button
          className="primary-button"
          type="submit"
          disabled={busy || Object.values(finalAnswers).some((a) => !a.length)}
        >
          {t(busy ? 'Sending…' : 'Send answers')}
        </button>
      ) : (
        <small ref={status} role="status" tabIndex={-1}>
          {t(
            !cancelled && (sent || result)
              ? 'Answers sent'
              : live && pending === null
                ? 'Checking question status…'
                : live && statusFailed
                  ? 'Question status unavailable. Retrying…'
                  : 'This question is no longer waiting for an answer',
          )}
        </small>
      )}
      {error && <p role="alert">{error}</p>}
    </form>
  );
}
