import { memo, useEffect, useState } from 'react';
import { Activity, ChevronDown } from 'lucide-react';
import { executionState, phaseLabel, secondsBetween } from '../lib/execution';
import { visibleInterval } from '../lib/visible';
import { useI18n } from '../lib/i18n';
import type { Message, Session } from '../lib/types';

/** One quiet summary for the selected turn. Provider completion is not task completion. */
function ExecutionSummary({
  messages,
  session,
  submission,
  approvalPending,
  refreshing,
  onRefresh,
}: {
  messages: Message[];
  session?: Session;
  submission?: Message;
  approvalPending: boolean;
  refreshing: boolean;
  onRefresh: () => void;
}) {
  const { t } = useI18n();
  const state = executionState(messages, session, approvalPending);
  const preparing = !!submission && state.stages.length === 0 && state.phases.length === 0;
  const active = state.running || (preparing && submission.delivery === 'sending');
  const phase = preparing
    ? submission.delivery === 'uncertain'
      ? 'delivery-uncertain'
      : 'preparing-request'
    : state.phase;
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    setNow(Date.now());
    if (active) return visibleInterval(() => setNow(Date.now()), 1000);
  }, [session?.id, active]);
  if (!phase) return null;
  const quiet = state.lastEventAt ? secondsBetween(state.lastEventAt, now) : undefined;
  const silent = state.silent && quiet !== undefined && quiet >= 30;
  const start = preparing ? submission.submittedAt : state.startedAt;
  const endpoint = active ? now : state.endedAt;
  const duration = start && endpoint ? secondsBetween(start, endpoint) : undefined;
  return (
    <details className="execution-summary" aria-label={t('Execution progress')}>
      <summary>
        <Activity size={14} aria-hidden="true" />
        <span role="status">
          {t(phaseLabel(phase))}
          {silent && <> · {t('No recent updates')}</>}
        </span>
        <span className="execution-metrics">
          {duration !== undefined && (
            <span>
              {t('Observed duration')}: {duration} s
            </span>
          )}
          {state.agents > 0 && (
            <span>
              {t('Reported active agents')}: {state.agents}
            </span>
          )}
          {state.checks.length > 0 && (
            <span>
              {t('Checks')}: {state.checks.length}
            </span>
          )}
          {state.running && quiet !== undefined && (
            <span>
              {t('Last event')}: {quiet} s
            </span>
          )}
        </span>
        <ChevronDown size={13} className="execution-chevron" aria-hidden="true" />
      </summary>
      <div className="execution-detail">
        {state.durationIncomplete && (
          <p>{t('Timing ends at the last observation; the interruption time is unknown.')}</p>
        )}
        {silent && (
          <p>
            {t(
              'No new recorded provider events for at least 30 seconds. This does not prove a stall. Silence does not trigger an automatic resend.',
            )}
          </p>
        )}
        {preparing && (
          <p>{t('Awaiting backend acceptance. Provider execution has not been confirmed.')}</p>
        )}
        {['turn-returned', 'returned'].includes(phase) && (
          <p>
            {t(
              'The turn ended. This does not certify that the requested task or its checks are complete.',
            )}
          </p>
        )}
        {['question', 'approval', 'plan-approval'].includes(phase) && (
          <p>
            {t('An input is pending. Use the question or approval controls in this conversation.')}
          </p>
        )}
        {state.last && (
          <p>
            {t('Last observed activity')}: {t(state.last.lastKind || 'No provider event yet')} ·{' '}
            {state.last.provider}
          </p>
        )}
        {state.checks.length > 0 && (
          <p>
            {t('Check results')}: {state.checks.map((s) => t(s)).join(' · ')}
          </p>
        )}
        {state.stages.length === 0 && <p>{t('Detailed timings are unavailable for this turn.')}</p>}
        {state.phases.length > 0 && (
          <ol>
            {state.phases.map((entry, i) => (
              <li key={i}>
                {t(phaseLabel(entry.phase))} · +
                {secondsBetween(state.startedAt ?? entry.at, entry.at)} s
              </li>
            ))}
          </ol>
        )}
        {state.stages.map((stage) => (
          <details key={stage.id}>
            <summary>
              {stage.provider} · {t(phaseLabel(stage.phase))}
            </summary>
            <p>
              {t('Observed events')}: {stage.events} · {t('Tool events')}: {stage.toolEvents}
            </p>
            <p>
              {t('Open tool operations')}: {stage.pendingTools} · {t('Open questions')}:{' '}
              {stage.pendingQuestions}
            </p>
            <dl className="execution-milestones">
              {(
                [
                  ['Process start', stage.processStartedAt],
                  ['First provider event', stage.firstProviderEventAt],
                  ['First text', stage.firstTextAt],
                  ['Provider return', stage.endedAt],
                ] as const
              )
                .filter(([, at]) => at !== null)
                .map(([label, at]) => (
                  <div key={label}>
                    <dt>{t(label)}</dt>
                    <dd>+{Math.max(0, at! - stage.startedAt)} ms</dd>
                  </div>
                ))}
            </dl>
            <ol>
              {stage.timeline.map(([phase, at], i) => (
                <li key={i}>
                  {t(phaseLabel(phase))} · +{secondsBetween(stage.startedAt, at)} s
                </li>
              ))}
            </ol>
          </details>
        ))}
        <button
          className="text-button"
          disabled={refreshing || !session || preparing}
          onClick={onRefresh}
        >
          {t(refreshing ? 'Refreshing status…' : 'Refresh execution status')}
        </button>
      </div>
    </details>
  );
}
export default memo(ExecutionSummary);
