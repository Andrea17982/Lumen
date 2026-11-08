import { visibleInterval } from '../../lib/visible';
import { useEffect, useState } from 'react';
import { decisionBlocker, sameScope, visibleText, whitelistCommands } from './contracts';
import type { Action, ApprovalRequest, Decision, ExactRule, Scope, SessionView } from './contracts';
import './gatekeeper.css';
import { errorText } from '../../lib/util';

export interface GatekeeperPanelProps {
  requests: ApprovalRequest[];
  sessions: SessionView[];
  rules?: ExactRule[];
  onDecide: (decision: Decision) => Promise<void>;
  onAddRule?: (scope: Scope, command: string, cwd: string) => Promise<void>;
  onRemoveRule?: (scope: Scope, ruleId: string) => Promise<void>;
}

const labels = {
  pending: 'Waiting for your decision',
  approved: 'Approved once',
  rejected: 'Rejected',
  cancelled: 'Cancelled',
  timedOut: 'Timed out',
  stale: 'Recovered or changed · new request required',
  resolved: 'Cleared by provider',
};

function RequestCard({
  request,
  sessions,
  onDecide,
  now,
}: Pick<GatekeeperPanelProps, 'sessions' | 'onDecide'> & {
  request: ApprovalRequest;
  now: number;
}) {
  const [working, setWorking] = useState(false);
  const [error, setError] = useState('');
  const [note, setNote] = useState('');
  const blocker = decisionBlocker(request, sessions, now);
  const disabled = working || !!blocker;
  async function decide(action: Action) {
    if (disabled || (action === 'approve' && !request.canApprove)) return;
    setWorking(true);
    setError('');
    try {
      await onDecide({
        requestId: request.id,
        scope: request.scope,
        action,
        note: note.trim() || null,
      });
    } catch (e) {
      setError(errorText(e));
    } finally {
      setWorking(false);
    }
  }
  return (
    <article
      className="gatekeeper-request"
      aria-label={`${request.scope.provider} ${request.tool} approval`}
    >
      <div className="gatekeeper-heading">
        <strong>{visibleText(request.tool)}</strong>
        <span role="status">{labels[request.status]}</span>
      </div>
      <p className="gatekeeper-identity">
        {request.scope.provider} · Session {visibleText(request.scope.sessionId)} · Turn{' '}
        {visibleText(request.scope.turnId)}
      </p>
      {request.command !== null && <pre aria-label="Command">{visibleText(request.command)}</pre>}
      <p>
        <strong>{request.scope.provider === 'claude' ? 'Session directory' : 'Directory'}:</strong>{' '}
        <code>{visibleText(request.cwd || 'Not provided')}</code>
      </p>
      {request.scope.provider === 'claude' && (
        <p className="gatekeeper-detail">
          Claude MCP does not report the current shell directory. Review any directory changes in
          the command.
        </p>
      )}
      <p>
        <strong>Reason:</strong> {visibleText(request.reason || 'Not provided by the CLI')}
      </p>
      {request.diffs.map((change, index) => (
        <details key={`${index}:${change.path}`} open>
          <summary>{visibleText(change.path)}</summary>
          <pre aria-label={`Change preview ${index + 1}`}>{visibleText(change.diff)}</pre>
        </details>
      ))}
      <details>
        <summary>Exact request details</summary>
        <pre>{visibleText(JSON.stringify(request.input, null, 2))}</pre>
        <p>
          Request {request.id} · Connection {request.scope.connectionId}
        </p>
      </details>
      {request.unavailableReason && <p role="status">Unavailable: {request.unavailableReason}</p>}
      {request.delivery === 'uncertain' && (
        <p role="status">
          Response delivery is uncertain. Verify provider state before running this operation again.
        </p>
      )}
      {request.status === 'pending' && (
        <>
          <label className="gatekeeper-note">
            Decision note (optional)
            <textarea
              value={note}
              maxLength={4096}
              disabled={disabled}
              onChange={(e) => setNote(e.target.value)}
            />
          </label>
          <div className="gatekeeper-actions">
            <button
              type="button"
              disabled={disabled || !request.canApprove}
              onClick={() => void decide('approve')}
            >
              Approve once
            </button>
            <button type="button" disabled={disabled} onClick={() => void decide('reject')}>
              Reject
            </button>
            <button type="button" disabled={disabled} onClick={() => void decide('cancel')}>
              Cancel
            </button>
            <span>{Math.max(0, Math.ceil((request.expiresAtMs - now) / 1000))}s remaining</span>
          </div>
        </>
      )}
      {blocker && request.status === 'pending' && <p role="status">{blocker}</p>}
      {request.note && request.status !== 'pending' && <p>{visibleText(request.note)}</p>}
      {error && <p role="alert">{error}</p>}
    </article>
  );
}

function Whitelist({
  session,
  rules,
  onAddRule,
  onRemoveRule,
}: {
  session: SessionView;
  rules: ExactRule[];
} & Pick<GatekeeperPanelProps, 'onAddRule' | 'onRemoveRule'>) {
  const [command, setCommand] = useState<string>(whitelistCommands[0]);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState('');
  async function change(action: () => Promise<void>) {
    if (working) return;
    setWorking(true);
    setError('');
    try {
      await action();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setWorking(false);
    }
  }
  return (
    <details className="gatekeeper-whitelist">
      <summary>Exact command whitelist · {visibleText(session.scope.sessionId)}</summary>
      <p>
        Applies only to this connection and directory: <code>{visibleText(session.cwd)}</code>.
        Rules expire when it disconnects. Extra arguments require a new decision.
      </p>
      {onAddRule && (
        <div className="gatekeeper-actions">
          <select
            aria-label="Exact command to whitelist"
            value={command}
            disabled={working}
            onChange={(e) => setCommand(e.target.value)}
          >
            {whitelistCommands.map((value) => (
              <option key={value}>{value}</option>
            ))}
          </select>
          <button
            type="button"
            disabled={working}
            onClick={() => void change(() => onAddRule(session.scope, command, session.cwd))}
          >
            Add exact rule
          </button>
        </div>
      )}
      <ul>
        {rules
          .filter((rule) => sameScope(rule.scope, session.scope))
          .map((rule) => (
            <li key={rule.id}>
              <code>{visibleText(rule.command)}</code>{' '}
              {onRemoveRule && (
                <button
                  type="button"
                  disabled={working}
                  onClick={() => void change(() => onRemoveRule(session.scope, rule.id))}
                >
                  Remove rule
                </button>
              )}
            </li>
          ))}
      </ul>
      {error && <p role="alert">{error}</p>}
    </details>
  );
}

/** Pure review surface. Mounting, countdowns and recovered data never authorize work. */
export default function GatekeeperPanel({
  requests,
  sessions,
  rules = [],
  onDecide,
  onAddRule,
  onRemoveRule,
}: GatekeeperPanelProps) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    return visibleInterval(() => setNow(Date.now()), 500);
  }, []);
  return (
    <section className="gatekeeper-panel" aria-label="Gatekeeper approvals">
      <h3>Gatekeeper</h3>
      {sessions.map((session) => (
        <div key={session.scope.connectionId}>
          {session.availability.state === 'unavailable' ? (
            <p role="status">
              {session.scope.provider} · Unavailable: {session.availability.reason}
            </p>
          ) : (
            <>
              <p className="gatekeeper-detail">
                {session.scope.provider} · {session.availability.version}
              </p>
              {session.scope.provider === 'codex' && (onAddRule || rules.length > 0) && (
                <Whitelist
                  session={session}
                  rules={rules}
                  onAddRule={onAddRule}
                  onRemoveRule={onRemoveRule}
                />
              )}
            </>
          )}
        </div>
      ))}
      {requests.length === 0 && <p>No pending approval requests.</p>}
      {requests.map((request) => (
        <RequestCard
          key={request.id}
          request={request}
          sessions={sessions}
          onDecide={onDecide}
          now={now}
        />
      ))}
    </section>
  );
}
