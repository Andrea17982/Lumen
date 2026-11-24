import { useT } from '../i18n';
import { ApprovalCard } from '../features/attention/ApprovalCard';
import { QuestionCard } from '../features/attention/QuestionCard';
import type { ApprovalRequest, ApprovalSession, Interaction } from '../protocol/types';
import s from './Gallery.module.css';

// Every attention state with fixed data, including the ones a live test cannot reach in time
// (expiry after five minutes) or at all (a request Lumen cannot approve, an uncertain delivery).
const NOW = 1_800_000_000_000;
const scope = (provider: 'claude' | 'codex', turnId = 't1') => ({ sessionId: 's1', connectionId: `c-${provider}`, provider, providerSessionId: 'p', turnId });
const live = (provider: 'claude' | 'codex'): ApprovalSession => ({ scope: scope(provider), cwd: 'C:/Projects/lumen', availability: { state: 'available', version: 'fixture', evidence: '' } });
const base = (patch: Partial<ApprovalRequest>): ApprovalRequest => ({
  id: 'r',
  scope: scope('claude'),
  providerItemId: null,
  kind: 'command',
  tool: 'Bash',
  command: 'git status --short',
  cwd: 'C:/Projects/lumen',
  reason: 'Check which files changed before the commit',
  diffs: [],
  input: { command: 'git status --short', description: 'Check which files changed' },
  canApprove: true,
  unavailableReason: null,
  whitelistEligible: false,
  createdAtMs: NOW - 28_000,
  expiresAtMs: NOW + 272_000,
  status: 'pending',
  delivery: 'none',
  decision: null,
  decisionSource: null,
  note: null,
  ...patch,
});

const requests: ApprovalRequest[] = [
  base({ id: 'command' }),
  base({
    id: 'files',
    scope: scope('codex'),
    kind: 'fileChange',
    tool: 'apply_patch',
    command: null,
    reason: 'Rename the helper and update its callers',
    diffs: [{ path: 'src/sync/sync.ts', diff: '@@ -12,3 +12,3 @@\n-const LIVE = 20_000;\n+const LIVE_LIMIT = 20_000;\n context' }],
  }),
  base({ id: 'unsupported', kind: 'unsupported', tool: 'mcp__browser__navigate', command: null, canApprove: false, unavailableReason: 'This tool requires an adapter before it can be approved', reason: null }),
  base({ id: 'expired', expiresAtMs: NOW - 1_000 }),
  base({ id: 'gone', scope: scope('claude', 'older'), worker: { agent: 'Claude', taskId: 'a', task: 'Write the tests' } }),
  base({ id: 'uncertain', status: 'approved', delivery: 'uncertain', decision: 'approve' }),
  base({ id: 'timed', status: 'timedOut' }),
  base({ id: 'hidden', command: 'echo safe\u202e && del *.txt' }),
];

const question: Interaction = {
  id: 'q',
  questions: [
    { id: 'impl', header: 'Implementation', question: 'Which implementation should I use?', options: [{ label: 'Native', description: 'Keep official CLI behavior' }, { label: 'Custom', description: 'Use a custom approach' }], multiSelect: false, secret: false, optional: false },
    { id: 'checks', header: 'Checks', question: 'Which checks should run?', options: [{ label: 'Unit tests', description: '' }, { label: 'Lint', description: '' }], multiSelect: true, secret: false, optional: true },
    { id: 'token', header: 'Token', question: 'Paste the deploy token', options: [], multiSelect: false, secret: true, optional: false },
  ],
};
const connector: Interaction = {
  id: 'c',
  kind: 'connector',
  actionId: 'action',
  urlOrigin: 'https://example.com',
  questions: [{ id: 'action', header: 'Offline GitHub', question: 'Review the request\nAllow access for this request?', options: [{ label: 'Accept', description: '' }, { label: 'Decline', description: '' }, { label: 'Cancel', description: '' }], multiSelect: false, secret: false, optional: false }],
};

const noop = async () => {};

export function AttentionStates() {
  const t = useT();
  return (
    <main className={s.states} aria-label={t.attention.label}>
      {requests.map((r) => (
        <ApprovalCard key={r.id} request={r} sessions={[live('claude'), live('codex')]} now={NOW} onDecide={noop} />
      ))}
      <QuestionCard request={question} state="open" onAnswer={noop} onOpenSignIn={noop} />
      <QuestionCard request={connector} state="open" onAnswer={noop} onOpenSignIn={noop} />
      <QuestionCard request={{ ...question, id: 'u', questions: question.questions.slice(0, 1) }} state="unknown" onAnswer={noop} onOpenSignIn={noop} />
    </main>
  );
}
