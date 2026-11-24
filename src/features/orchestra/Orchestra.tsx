import { useRef, useState } from 'react';
import { Badge, Button, ConfirmDialog, Select, TextField, type Tone } from '../../design/components';
import { useT } from '../../i18n';
import type { Message, RunOptions } from '../../protocol/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import { visibleText } from '../attention/requests';
import { overLimit, planEdits, tail, type OrchestraState, type Subtask } from './state';
import s from './Orchestra.module.css';

const tones: Record<string, Tone> = { running: 'accent', done: 'success', failed: 'danger', interrupted: 'warning', queued: 'neutral', pending: 'neutral', skipped: 'neutral' };
const OPEN = new Set(['questions', 'plan', 'running', 'interrupted', 'failed']);

/**
 * An Orchestra run on its message: questions, the plan to approve, live lanes and recovery. Every
 * button sends one daemon command; the card re-renders from the state the daemon publishes next.
 */
export function OrchestraCard({ sessionId, message, state }: { sessionId: string; message: Message; state: OrchestraState }) {
  const t = useT();
  const sync = useSyncClient();
  const access = useSyncState((st) => st.sessions.find((x) => x.id === sessionId)?.accessMode);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const sending = useRef(false);
  // Orchestra runs only from Auto, so the next step goes on with Auto and this conversation's access.
  const options: RunOptions = { mode: 'auto', effort: '', autoPolicy: 'default', accessMode: access, attachments: [] };
  async function send(command: string, args: Record<string, unknown>) {
    if (sending.current) return;
    sending.current = true;
    setBusy(true);
    setError(null);
    try {
      await sync.call(command, { id: sessionId, message: message.id, ...args });
    } catch (e) {
      setError(visibleText(String(e)));
    } finally {
      sending.current = false;
      setBusy(false);
    }
  }
  const done = state.subtasks.filter((x) => x.status === 'done').length;
  const open = OPEN.has(state.stage) && !(state.stage === 'interrupted' && !state.resumable);
  const extra = overLimit(state);
  return (
    <section className={s.card} data-state={open ? 'open' : 'closed'} aria-label={t.orchestra.title}>
      <header className={s.head}>
        <h3>{t.orchestra.title}</h3>
        {state.subtasks.length > 0 && !['questions', 'plan'].includes(state.stage) && <span className={s.quiet}>{t.orchestra.progress(done, state.subtasks.length)}</span>}
        <Badge tone={tones[state.stage] ?? 'neutral'}>{t.orchestra.stage[state.stage] ?? state.stage}</Badge>
      </header>
      {state.stage === 'questions' && <Questions state={state} busy={busy} onAnswer={(args) => send('orchestra_answer', { ...args, options })} onCancel={() => send('orchestra_cancel', {})} />}
      {state.stage === 'plan' && <Plan state={state} busy={busy} onApprove={(edits) => send('orchestra_approve', { edits, options })} onCancel={() => send('orchestra_cancel', {})} />}
      {!['questions', 'plan', 'answered', 'cancelled'].includes(state.stage) && state.subtasks.length > 0 && <Lanes state={state} live={message.live ?? {}} />}
      {state.steps.length > 0 && (
        <details className={s.disclosure}>
          <summary>{t.orchestra.steps}</summary>
          <ol className={s.steps}>
            {state.steps.map((step, i) => (
              <li key={i}>
                <Badge tone={tones[step.status] ?? 'neutral'}>{t.orchestra.status[step.status] ?? step.status}</Badge>
                <span>{visibleText(step.name)}</span>
                {step.detail && <small>{visibleText(step.detail)}</small>}
              </li>
            ))}
          </ol>
        </details>
      )}
      {state.stage === 'running' && extra > 0 && (
        <div className={s.actions}>
          <Button variant="secondary" isDisabled={busy} onPress={() => sync.call('orchestra_raise', { messageId: message.id, extra }).catch((e) => setError(visibleText(String(e))))}>
            {t.orchestra.raise(extra)}
          </Button>
        </div>
      )}
      {state.stage === 'interrupted' && state.resumable && <Resume state={state} busy={busy} onResume={(force) => send('orchestra_resume', { force })} onDiscard={() => send('orchestra_lanes', { keep: false })} />}
      {state.stage === 'failed' && state.lanes.length > 0 && <Branches busy={busy} onKeep={() => send('orchestra_lanes', { keep: true })} onDiscard={() => send('orchestra_lanes', { keep: false })} />}
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
    </section>
  );
}

function Questions({ state, busy, onAnswer, onCancel }: { state: OrchestraState; busy: boolean; onAnswer: (args: { answers: string[]; note: string | null; skip: boolean }) => void; onCancel: () => void }) {
  const t = useT();
  const [answers, setAnswers] = useState<string[]>(() => state.questions.map(() => ''));
  const [note, setNote] = useState('');
  return (
    <form
      className={s.body}
      onSubmit={(e) => {
        e.preventDefault();
        onAnswer({ answers, note: note.trim() || null, skip: false });
      }}
    >
      {state.questions.length > 0 ? (
        <>
          <p className={s.lead}>{t.orchestra.questions}</p>
          {state.questions.map((q, i) => (
            <TextField
              key={i}
              label={visibleText(q.q)}
              placeholder={visibleText(q.recommended)}
              description={q.recommended ? t.orchestra.suggested(visibleText(q.recommended)) : undefined}
              value={answers[i]}
              onChange={(v) => setAnswers(answers.map((a, j) => (j === i ? v : a)))}
            />
          ))}
        </>
      ) : (
        <>
          <p className={s.lead}>{t.orchestra.confirm}</p>
          <p>{visibleText(state.confirm || state.task)}</p>
          <TextField label={t.orchestra.change} value={note} onChange={setNote} multiline />
        </>
      )}
      <div className={s.actions}>
        <Button variant="primary" type="submit" isDisabled={busy}>
          {t.orchestra.continue}
        </Button>
        {state.questions.length > 0 && (
          <Button variant="secondary" isDisabled={busy} onPress={() => onAnswer({ answers: [], note: null, skip: true })}>
            {t.orchestra.useSuggested}
          </Button>
        )}
        <Button variant="plain" isDisabled={busy} onPress={onCancel}>
          {t.orchestra.cancel}
        </Button>
      </div>
    </form>
  );
}

function Plan({ state, busy, onApprove, onCancel }: { state: OrchestraState; busy: boolean; onApprove: (edits: ReturnType<typeof planEdits>) => void; onCancel: () => void }) {
  const t = useT();
  const accounts = useSyncState((st) => st.accounts);
  const [agents, setAgents] = useState<Record<string, string>>({});
  const [removed, setRemoved] = useState<Set<string>>(new Set());
  const kept = state.subtasks.length - removed.size;
  const titles = Object.fromEntries(state.subtasks.map((x) => [x.id, x.title]));
  return (
    <div className={s.body}>
      {state.summary && <p className={s.lead}>{visibleText(state.summary)}</p>}
      <p className={s.quiet}>{t.orchestra.parts(kept)}</p>
      {state.readOnly && <p className={s.quiet}>{t.orchestra.readOnly}</p>}
      <ol className={s.plan}>
        {state.subtasks.map((task) => {
          const out = removed.has(task.id);
          // Ollama answers but cannot edit files: the daemon refuses it for a writing part.
          const choices = accounts.filter((a) => !(task.writes && a.provider === 'ollama'));
          const value = agents[task.id] ?? task.accountId;
          return (
            <li key={task.id} data-removed={out || undefined} aria-label={visibleText(task.title)}>
              <div className={s.planHead}>
                <strong>{visibleText(task.title)}</strong>
                {out && <Badge>{t.orchestra.left}</Badge>}
                <span className={s.spacer} />
                <Button
                  variant="plain"
                  isDisabled={busy || (!out && kept === 1)}
                  onPress={() => {
                    const next = new Set(removed);
                    if (out) next.delete(task.id);
                    else next.add(task.id);
                    setRemoved(next);
                  }}
                >
                  {out ? t.orchestra.restore : t.orchestra.remove}
                </Button>
              </div>
              {task.detail && <p className={s.quiet}>{visibleText(task.detail)}</p>}
              {(task.files.length > 0 || task.dependsOn.length > 0) && (
                <p className={s.meta}>
                  {task.files.map((f) => (
                    <code key={f}>{visibleText(f)}</code>
                  ))}
                  {task.dependsOn.length > 0 && <span>{t.orchestra.after(task.dependsOn.map((d) => visibleText(titles[d] ?? d)).join(', '))}</span>}
                </p>
              )}
              {!out && choices.length > 0 && (
                <Select
                  compact
                  label={t.orchestra.agent}
                  value={value || ''}
                  isDisabled={busy}
                  options={[...(value ? [] : [{ id: '', label: t.orchestra.lumenChooses }]), ...choices.map((a) => ({ id: a.id, label: a.name }))]}
                  onChange={(id) => setAgents({ ...agents, [task.id]: id })}
                />
              )}
            </li>
          );
        })}
      </ol>
      {state.planReview && (
        <details className={s.disclosure}>
          <summary>{t.orchestra.review}</summary>
          <p className={s.pre}>{visibleText(state.planReview)}</p>
        </details>
      )}
      <div className={s.actions}>
        <Button variant="primary" isDisabled={busy} onPress={() => onApprove(planEdits(state.subtasks, agents, removed))}>
          {t.orchestra.approve}
        </Button>
        <Button variant="plain" isDisabled={busy} onPress={onCancel}>
          {t.orchestra.cancel}
        </Button>
      </div>
    </div>
  );
}

function Lanes({ state, live }: { state: OrchestraState; live: Record<string, string> }) {
  const t = useT();
  const waiting = Object.fromEntries((state.queue?.waiting ?? []).map((w) => [w.id, w.reason]));
  return (
    <ol className={s.lanes}>
      {state.subtasks.map((task) => (
        <Lane key={task.id} task={task} live={live[task.id] ?? ''} waiting={waiting[task.id] ? (t.orchestra.waiting[waiting[task.id]] ?? waiting[task.id]) : ''} />
      ))}
    </ol>
  );
}

function Lane({ task, live, waiting }: { task: Subtask; live: string; waiting: string }) {
  const t = useT();
  const running = task.status === 'running';
  const text = running ? live : task.output;
  return (
    <li className={s.lane} aria-label={visibleText(task.title)}>
      <div className={s.planHead}>
        <Badge tone={tones[task.status] ?? 'neutral'}>{t.orchestra.status[task.status] ?? task.status}</Badge>
        <strong>{visibleText(task.title)}</strong>
        <span className={s.spacer} />
        {task.agent && <span className={s.quiet}>{[task.agent, task.model].filter(Boolean).join(' · ')}</span>}
      </div>
      {waiting && !running && task.status !== 'done' && <p className={s.quiet}>{waiting}</p>}
      {text && (
        <details className={s.disclosure} open={running || undefined}>
          <summary>{running ? t.orchestra.live : t.orchestra.output}</summary>
          <pre className={s.output} tabIndex={0}>
            {visibleText(tail(text))}
          </pre>
        </details>
      )}
    </li>
  );
}

function Resume({ state, busy, onResume, onDiscard }: { state: OrchestraState; busy: boolean; onResume: (force: boolean) => void; onDiscard: () => void }) {
  const t = useT();
  const [force, setForce] = useState(false);
  return (
    <div className={s.body}>
      <p className={s.quiet}>{t.orchestra.resumeHelp}</p>
      {state.notice && (
        <p className={s.notice} role="note">
          {visibleText(state.notice)}
        </p>
      )}
      <div className={s.actions}>
        <Button variant="primary" isDisabled={busy} onPress={() => onResume(false)}>
          {t.orchestra.resume}
        </Button>
        {state.notice && (
          <Button variant="secondary" isDisabled={busy} onPress={() => setForce(true)}>
            {t.orchestra.resumeAnyway}
          </Button>
        )}
        <Discard busy={busy} onDiscard={onDiscard} />
      </div>
      <ConfirmDialog isOpen={force} onOpenChange={setForce} title={t.orchestra.resumeAnywayTitle} body={t.orchestra.resumeAnywayBody} confirmLabel={t.orchestra.resumeAnyway} cancelLabel={t.cancel} danger onConfirm={() => onResume(true)} />
    </div>
  );
}

function Branches({ busy, onKeep, onDiscard }: { busy: boolean; onKeep: () => void; onDiscard: () => void }) {
  const t = useT();
  return (
    <div className={s.body}>
      <p className={s.quiet}>{t.orchestra.keepHelp}</p>
      <div className={s.actions}>
        <Button variant="secondary" isDisabled={busy} onPress={onKeep}>
          {t.orchestra.keep}
        </Button>
        <Discard busy={busy} onDiscard={onDiscard} />
      </div>
    </div>
  );
}

function Discard({ busy, onDiscard }: { busy: boolean; onDiscard: () => void }) {
  const t = useT();
  return (
    <ConfirmDialog
      trigger={
        <Button variant="plain" isDisabled={busy}>
          {t.orchestra.discard}
        </Button>
      }
      title={t.orchestra.discardTitle}
      body={t.orchestra.discardBody}
      confirmLabel={t.orchestra.discard}
      cancelLabel={t.cancel}
      danger
      onConfirm={onDiscard}
    />
  );
}

/** A complex task ran with a single agent because the folder has no Git: offer to set it up. */
export function NoGitCard({ sessionId }: { sessionId: string }) {
  const t = useT();
  const sync = useSyncClient();
  const [state, setState] = useState<'idle' | 'busy' | 'ready'>('idle');
  const [error, setError] = useState<string | null>(null);
  return (
    <section className={s.card} data-state="closed" aria-label={t.orchestra.title}>
      <p>{state === 'ready' ? t.orchestra.gitReady : t.orchestra.noGit}</p>
      {state !== 'ready' && (
        <div className={s.actions}>
          <Button
            variant="secondary"
            isDisabled={state === 'busy'}
            onPress={() => {
              setState('busy');
              setError(null);
              sync.call('orchestra_init_git', { id: sessionId }).then(
                () => setState('ready'),
                (e) => {
                  setState('idle');
                  setError(visibleText(String(e)));
                },
              );
            }}
          >
            {t.orchestra.initGit}
          </Button>
        </div>
      )}
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
