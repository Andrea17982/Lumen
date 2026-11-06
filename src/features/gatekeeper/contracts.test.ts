import { describe, expect, it } from 'vitest';
import { decisionBlocker, sameScope, visibleText, whitelistCommands } from './contracts';
import type { ApprovalRequest, Scope, SessionView } from './contracts';

export const testScope: Scope = {
  sessionId: 'chat-1',
  connectionId: 'connection-1',
  provider: 'codex',
  providerSessionId: 'thread-1',
  turnId: 'turn-1',
};
export const testSession: SessionView = {
  scope: testScope,
  cwd: '/repo',
  availability: { state: 'available', version: 'fixture', evidence: 'fixture' },
};
export const testRequest: ApprovalRequest = {
  id: 'approval-1',
  scope: testScope,
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
  createdAtMs: 1000,
  expiresAtMs: 5000,
  status: 'pending',
  delivery: 'none',
  decision: null,
  decisionSource: null,
  note: null,
};

describe('approval review contracts', () => {
  it('enables only a live, matching, unexpired request', () => {
    expect(decisionBlocker(testRequest, [testSession], 2000)).toBeNull();
    expect(decisionBlocker(testRequest, [testSession], 5000)).toContain('expired');
    expect(decisionBlocker(testRequest, [], 2000)).toContain('unavailable');
    expect(decisionBlocker({ ...testRequest, status: 'stale' }, [testSession], 2000)).toContain(
      'closed',
    );
  });

  it.each(['sessionId', 'connectionId', 'providerSessionId', 'turnId'] as const)(
    'does not match a changed %s',
    (field) => {
      const wrong = { ...testScope, [field]: 'other' };
      expect(sameScope(testScope, wrong)).toBe(false);
      expect(decisionBlocker(testRequest, [{ ...testSession, scope: wrong }], 2000)).toContain(
        'unavailable',
      );
    },
  );

  it('does not accept another provider or unavailable capability', () => {
    expect(sameScope(testScope, { ...testScope, provider: 'claude' })).toBe(false);
    expect(
      decisionBlocker(
        testRequest,
        [{ ...testSession, availability: { state: 'unavailable', reason: 'Old CLI' } }],
        2000,
      ),
    ).toContain('unavailable');
  });

  it('reveals invisible controls and bidi formatting while preserving line breaks', () => {
    expect(visibleText('git status\u202eEvil\u001b[0m\nnext\r')).toBe(
      'git status\\u202eEvil\\u001b[0m\nnext\\u000d',
    );
    expect(visibleText('normal path /repo/file.ts')).toBe('normal path /repo/file.ts');
  });

  it('offers finite exact recipes without shell syntax or executable arguments', () => {
    expect(whitelistCommands).toEqual([
      'git status --short',
      'git diff --stat',
      'git diff --check',
      'git rev-parse --show-toplevel',
    ]);
    expect(whitelistCommands.every((command) => /^[a-z -]+$/.test(command))).toBe(true);
  });
});
