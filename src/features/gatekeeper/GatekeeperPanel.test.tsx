import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import GatekeeperPanel from './GatekeeperPanel';
import type { ApprovalRequest, SessionView } from './contracts';

const scope = {
  sessionId: 'chat-1',
  connectionId: 'connection-1',
  provider: 'codex' as const,
  providerSessionId: 'thread-1',
  turnId: 'turn-1',
};
const session: SessionView = {
  scope,
  cwd: '/repo',
  availability: { state: 'available', version: 'fixture', evidence: 'fixture' },
};
const request: ApprovalRequest = {
  id: 'approval-1',
  scope,
  providerItemId: 'item-1',
  kind: 'command',
  tool: 'Bash',
  command: 'git status --short',
  cwd: '/repo',
  reason: 'Inspect worktree',
  diffs: [],
  input: { command: 'git status --short' },
  canApprove: true,
  unavailableReason: null,
  whitelistEligible: true,
  createdAtMs: Date.now(),
  expiresAtMs: Date.now() + 60000,
  status: 'pending',
  delivery: 'none',
  decision: null,
  decisionSource: null,
  note: null,
};

describe('Gatekeeper review cards', () => {
  it('renders exact command, directory, reason and identity without deciding on mount', () => {
    const decide = vi.fn();
    const html = renderToStaticMarkup(
      <GatekeeperPanel requests={[request]} sessions={[session]} onDecide={decide} />,
    );
    expect(html).toContain('git status --short');
    expect(html).toContain('/repo');
    expect(html).toContain('Inspect worktree');
    expect(html).toContain('chat-1');
    expect(html).toContain('turn-1');
    expect(html).toContain('Approve once');
    expect(decide).not.toHaveBeenCalled();
  });

  it('disables approval when support is absent and shows why', () => {
    const html = renderToStaticMarkup(
      <GatekeeperPanel
        requests={[
          { ...request, canApprove: false, unavailableReason: 'Unsupported network grant' },
        ]}
        sessions={[session]}
        onDecide={vi.fn()}
      />,
    );
    expect(html).toMatch(/disabled="">Approve once/);
    expect(html).toContain('Unsupported network grant');
    expect(html).not.toMatch(/disabled="">Reject/);
  });

  it('renders recovered state and uncertain delivery without actionable buttons', () => {
    const decide = vi.fn();
    const html = renderToStaticMarkup(
      <GatekeeperPanel
        requests={[{ ...request, status: 'stale', delivery: 'uncertain' }]}
        sessions={[]}
        onDecide={decide}
      />,
    );
    expect(html).toContain('new request required');
    expect(html).toContain('delivery is uncertain');
    expect(html).not.toContain('Approve once');
    expect(decide).not.toHaveBeenCalled();
  });

  it('escapes hostile markup and reveals hidden command formatting in previews', () => {
    const html = renderToStaticMarkup(
      <GatekeeperPanel
        requests={[
          {
            ...request,
            command: 'git status\u202ehidden',
            diffs: [{ path: '<img onerror=alert(1)>', diff: '<script>steal()</script>' }],
          },
        ]}
        sessions={[session]}
        onDecide={vi.fn()}
      />,
    );
    expect(html).toContain('\\u202e');
    expect(html).toContain('&lt;script&gt;');
    expect(html).not.toContain('<script>');
    expect(html).not.toContain('<img');
  });

  it('labels Claude directory as session context and withholds its whitelist', () => {
    const claudeScope = { ...scope, provider: 'claude' as const };
    const html = renderToStaticMarkup(
      <GatekeeperPanel
        requests={[{ ...request, scope: claudeScope }]}
        sessions={[{ ...session, scope: claudeScope }]}
        onDecide={vi.fn()}
        onAddRule={vi.fn()}
      />,
    );
    expect(html).toContain('Session directory');
    expect(html).toContain('does not report the current shell directory');
    expect(html).not.toContain('Add exact rule');
  });

  it('shows unavailable sessions without automatic remote approval fallback', () => {
    const html = renderToStaticMarkup(
      <GatekeeperPanel
        requests={[request]}
        sessions={[
          { ...session, availability: { state: 'unavailable', reason: 'Protocol missing' } },
        ]}
        onDecide={vi.fn()}
      />,
    );
    expect(html).toContain('Unavailable: Protocol missing');
    expect(html).toMatch(/disabled="">Approve once/);
    expect(html).toMatch(/disabled="">Reject/);
  });
});
