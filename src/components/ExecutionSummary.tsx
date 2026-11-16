import { memo } from 'react';
import { executionState, secondsBetween } from '../lib/execution';
import type { Message, Session } from '../lib/types';
import { useI18n } from '../lib/i18n';
function ExecutionSummary({ messages, session }: { messages: Message[]; session?: Session }) {
  const { t } = useI18n();
  const state = executionState(messages, session, false);
  if (state.running || !state.startedAt || !state.endedAt || state.durationIncomplete) return null;
  return (
    <p className="turn-duration">
      {t('Duration')}: {secondsBetween(state.startedAt, state.endedAt)} s
    </p>
  );
}
export default memo(ExecutionSummary);
