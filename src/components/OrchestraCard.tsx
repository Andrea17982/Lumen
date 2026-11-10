import { visibleInterval } from '../lib/visible';
import { useEffect, useState } from 'react';
import {
  Check,
  Circle,
  CircleAlert,
  CircleMinus,
  Clock,
  LoaderCircle,
  Workflow,
  X,
} from 'lucide-react';
import { QueueNotice } from '../features/orchestra/scheduling';
import type { Account, Message, OrchestraState } from '../lib/types';
import { nameOf } from '../lib/util';
import OrchestraGraph from './OrchestraGraph';

const seconds = (from: number, to: number) => {
  const s = Math.max(0, Math.round(((to || Date.now()) - from) / 1000));
  return s >= 60 ? `${Math.floor(s / 60)}m ${s % 60}s` : `${s}s`;
};

function StatusIcon({ status }: { status: string }) {
  if (status === 'running') return <LoaderCircle size={13} className="spin" aria-label="Running" />;
  if (status === 'done') return <Check size={13} aria-label="Done" />;
  if (status === 'failed') return <CircleAlert size={13} aria-label="Failed" />;
  if (status === 'queued') return <Clock size={13} aria-label="Queued" />;
  if (status === 'skipped') return <CircleMinus size={13} aria-label="Skipped" />;
  if (status === 'interrupted') return <CircleAlert size={13} aria-label="Interrupted" />;
  return <Circle size={9} aria-label="Waiting" />;
}

/** The Orchestra card: questions, plan approval and live progress, drawn from one message. */
export default function OrchestraCard({
  message,
  state,
  accounts,
  busy,
  readOnly,
  run,
}: {
  message: Message;
  state: OrchestraState;
  accounts: Account[];
  busy: boolean;
  readOnly: boolean;
  run: (command: string, args: Record<string, unknown>) => Promise<void>;
}) {
  const [answers, setAnswers] = useState<string[]>(() => state.questions.map((q) => q.recommended));
  const [changing, setChanging] = useState(false);
  const [note, setNote] = useState('');
  const [assign, setAssign] = useState<Record<string, string>>({});
  const [removed, setRemoved] = useState<string[]>([]);
  const [, tick] = useState(0);
  const live = state.stage === 'running' || state.stage === 'asking' || state.stage === 'planning';
  useEffect(() => {
    if (!live) return;
    return visibleInterval(() => tick((n) => n + 1), 1000);
  }, [live]);
  const act = (command: string, args: Record<string, unknown> = {}) =>
    void run(command, { message: message.id, ...args });

  const header = (
    <div className="orchestra-head">
      <Workflow size={14} />
      <strong>Orchestra</strong>
      <span>{state.orchestratorAgent}</span>
    </div>
  );

  if (state.stage === 'asking' || state.stage === 'planning')
    return (
      <section className="orchestra-card" aria-label="Orchestra">
        {header}
        <p className="orchestra-wait">
          <LoaderCircle size={13} className="spin" />
          {state.stage === 'asking' ? 'Reading the task…' : 'Planning the subtasks…'}
        </p>
        {state.steps.map((s) => (
          <div key={s.name} className="orchestra-row step">
            <StatusIcon status={s.status} />
            <span>{s.name}</span>
            <small>{s.detail}</small>
          </div>
        ))}
      </section>
    );

  if (state.stage === 'questions')
    return (
      <section className="orchestra-card" aria-label="Orchestra questions">
        {header}
        {state.questions.length ? (
          <>
            <p>A few questions before planning. Recommended answers are filled in.</p>
            {state.questions.map((q, i) => (
              <label key={i} className="orchestra-question">
                <span>
                  {i + 1}. {q.q}
                </span>
                <input
                  value={answers[i] ?? ''}
                  disabled={busy}
                  onChange={(e) =>
                    setAnswers((a) => a.map((x, j) => (j === i ? e.target.value : x)))
                  }
                />
              </label>
            ))}
            <div className="orchestra-actions">
              <button
                className="primary-button"
                disabled={busy}
                onClick={() => act('orchestra_answer', { answers, skip: false })}
              >
                Continue
              </button>
              <button
                className="text-button"
                disabled={busy}
                onClick={() => act('orchestra_answer', { answers: [], skip: true })}
              >
                Skip questions
              </button>
              <button
                className="text-button"
                disabled={busy}
                onClick={() => act('orchestra_cancel')}
              >
                Cancel
              </button>
            </div>
          </>
        ) : (
          <>
            <p>{state.confirm} Agree?</p>
            {changing && (
              <textarea
                className="orchestra-note"
                aria-label="What to change"
                autoFocus
                value={note}
                onChange={(e) => setNote(e.target.value)}
              />
            )}
            <div className="orchestra-actions">
              {changing ? (
                <button
                  className="primary-button"
                  disabled={busy || !note.trim()}
                  onClick={() => act('orchestra_answer', { answers: [], note, skip: false })}
                >
                  Send
                </button>
              ) : (
                <button
                  className="primary-button"
                  disabled={busy}
                  onClick={() => act('orchestra_answer', { answers: [], skip: false })}
                >
                  Agree
                </button>
              )}
              <button
                className="text-button"
                disabled={busy}
                onClick={() => setChanging(!changing)}
              >
                {changing ? 'Back' : 'Change'}
              </button>
              <button
                className="text-button"
                disabled={busy}
                onClick={() => act('orchestra_cancel')}
              >
                Cancel
              </button>
            </div>
          </>
        )}
      </section>
    );

  if (state.stage === 'plan') {
    const kept = state.subtasks.filter((t) => !removed.includes(t.id));
    return (
      <section className="orchestra-card" aria-label="Orchestra plan">
        {header}
        {state.summary && <p>{state.summary}</p>}
        <details className="orchestra-spec">
          <summary>Task specification</summary>
          <pre>{state.task}</pre>
        </details>
        <OrchestraGraph
          tasks={kept}
          accounts={accounts}
          assign={assign}
          onAssign={(id, accountId) => setAssign((current) => ({ ...current, [id]: accountId }))}
          disabled={busy}
        />
        {readOnly && (
          <p className="orchestra-warning">
            Orchestra needs File edits on to implement. With Read only, the agents only analyze.
          </p>
        )}
        {kept.map((t) => (
          <div key={t.id} className="orchestra-row plan">
            <div className="orchestra-task">
              <strong>{t.title}</strong>
              <small>
                {t.difficulty}
                {t.files.length ? ` · ${t.files.join(', ')}` : ''}
                {t.dependsOn.length ? ` · after ${t.dependsOn.join(', ')}` : ''}
              </small>
            </div>
            <select
              aria-label={`Agent for ${t.title}`}
              value={assign[t.id] ?? t.accountId}
              disabled={busy}
              onChange={(e) => setAssign((a) => ({ ...a, [t.id]: e.target.value }))}
            >
              {accounts.map((a) => (
                <option key={a.id} value={a.id} disabled={t.writes && a.provider === 'ollama'}>
                  {a.id === t.accountId && t.agent ? t.agent : `${nameOf(a.provider)} · ${a.name}`}
                </option>
              ))}
            </select>
            <button
              className="icon-button"
              aria-label={`Remove ${t.title}`}
              disabled={busy || kept.length < 2}
              onClick={() => setRemoved((r) => [...r, t.id])}
            >
              <X size={13} />
            </button>
          </div>
        ))}
        {state.planReview && (
          <details className="orchestra-review">
            <summary>Plan audit</summary>
            <pre>{state.planReview}</pre>
          </details>
        )}
        <div className="orchestra-actions">
          <button
            className="primary-button"
            disabled={busy}
            onClick={() =>
              act('orchestra_approve', {
                edits: [
                  ...removed.map((id) => ({ id, remove: true })),
                  ...Object.entries(assign).map(([id, accountId]) => ({ id, accountId })),
                ],
              })
            }
          >
            Approve
          </button>
          <button className="text-button" disabled={busy} onClick={() => act('orchestra_cancel')}>
            Cancel
          </button>
        </div>
      </section>
    );
  }

  if (
    ['answered', 'approved', 'cancelled', 'kept', 'discarded', 'superseded'].includes(state.stage)
  )
    return (
      <p className="orchestra-closed">
        <Workflow size={12} /> Orchestra ·{' '}
        {
          {
            answered: 'answered',
            approved: 'plan approved',
            cancelled: 'cancelled',
            kept: 'branches kept for manual merging',
            discarded: 'branches discarded',
            superseded: 'replaced by a newer message',
          }[state.stage]
        }
      </p>
    );

  // running · done · failed
  return (
    <section className="orchestra-card" aria-label="Orchestra progress">
      {header}
      {state.stage === 'interrupted' && !state.resumable && (
        <p className="orchestra-warning">
          Interrupted before it finished. Its temporary worktrees were removed.
        </p>
      )}
      {state.stage === 'interrupted' && state.resumable && (
        <p className="orchestra-warning">
          Interrupted when Lumen closed. Completed subtasks are kept; interrupted ones are checked
          and run again from a clean worktree.
        </p>
      )}
      {state.stage === 'resumed' && <p className="orchestra-warning">Resumed below.</p>}
      {state.subtasks.map((t) => {
        const text = message.live?.[t.id] || t.output;
        return (
          <details key={t.id} className={`orchestra-row run ${t.status}`}>
            <summary>
              <StatusIcon status={t.status} />
              <span className="orchestra-title">{t.title}</span>
              <small>{t.agent}</small>
              <small className="orchestra-time">
                {t.startedAt ? seconds(t.startedAt, t.endedAt) : 'waiting'}
              </small>
            </summary>
            {text ? <pre>{text}</pre> : <small>No output yet.</small>}
          </details>
        );
      })}
      {state.stage === 'running' && state.queue && (
        <QueueNotice
          queue={state.queue}
          disabled={busy}
          onRaise={(extra) => act('orchestra_raise', { messageId: message.id, extra })}
        />
      )}
      {state.steps.map((s) => (
        <div key={s.name} className={`orchestra-row step ${s.status}`}>
          <StatusIcon status={s.status} />
          <span>{s.name}</span>
          <small>{s.detail}</small>
        </div>
      ))}
      {state.stage === 'interrupted' && state.resumable && (
        <>
          {state.notice && <p className="orchestra-warning">{state.notice}</p>}
          <div className="orchestra-actions">
            <button
              className="primary-button"
              disabled={busy}
              onClick={() => act('orchestra_resume', { force: false })}
            >
              Resume
            </button>
            {state.notice && (
              <button
                className="text-button"
                disabled={busy}
                onClick={() => act('orchestra_resume', { force: true })}
              >
                Resume anyway
              </button>
            )}
            <button
              className="text-button danger"
              disabled={busy}
              onClick={() => act('orchestra_lanes', { keep: false })}
            >
              Discard
            </button>
          </div>
        </>
      )}
      {state.stage === 'failed' && state.lanes.length > 0 && (
        <div className="orchestra-actions">
          <button
            className="primary-button"
            disabled={busy}
            onClick={() => act('orchestra_lanes', { keep: true })}
          >
            Keep branches
          </button>
          <button
            className="text-button danger"
            disabled={busy}
            onClick={() => act('orchestra_lanes', { keep: false })}
          >
            Discard
          </button>
        </div>
      )}
    </section>
  );
}
