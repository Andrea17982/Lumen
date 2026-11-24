import { describe, expect, it } from 'vitest';
import type { Session, Workspace } from '../../protocol/types';
import { currentProject, daemonAnchor, folderName, groups, inProject, moved, toneOf } from './sessions';

const NOW = new Date(2026, 10, 24, 15, 0).getTime();
const s = (id: string, patch: Partial<Session> = {}): Session => ({ id, title: id, pinned: false, status: 'idle', workspace: 'C:\\Code\\app', updatedAt: NOW, workspaceId: null, accessMode: 'read-only', ...patch });
const app: Workspace = { id: 'w1', name: 'App', folder: 'c:/code/app/' };
const site: Workspace = { id: 'w2', name: 'Site', folder: 'C:\\Code\\site' };

describe('sessions', () => {
  it('belong to a project by id, or by folder when they have none', () => {
    expect(inProject(s('a'), app)).toBe(true);
    expect(inProject(s('a'), site)).toBe(false);
    expect(inProject(s('a', { workspaceId: 'w2' }), app)).toBe(false);
  });

  it('open the remembered project, else the one used last', () => {
    const sessions = [s('a', { updatedAt: NOW - 10 }), s('b', { workspace: 'C:\\Code\\site', updatedAt: NOW })];
    expect(currentProject([app, site], sessions, 'w1')?.id).toBe('w1');
    expect(currentProject([app, site], sessions, 'gone')?.id).toBe('w2');
    expect(currentProject([], sessions, null)).toBeNull();
  });

  it('group pinned first, then by day, keeping the daemon order inside a group', () => {
    const day = 86_400_000;
    const list = [s('t2'), s('p', { pinned: true, updatedAt: NOW - 9 * day }), s('t1', { updatedAt: NOW - 60_000 }), s('y', { updatedAt: NOW - day }), s('old', { updatedAt: NOW - 5 * day })];
    expect(groups(list, NOW).map((g) => [g.key, g.sessions.map((x) => x.id)])).toEqual([
      ['pinned', ['p']],
      ['today', ['t2', 't1']],
      ['yesterday', ['y']],
      ['earlier', ['old']],
    ]);
  });

  it('move a session before another, or to the end', () => {
    const list = [s('a'), s('b'), s('c')];
    expect(moved(list, 'c', 'a').map((x) => x.id)).toEqual(['c', 'a', 'b']);
    expect(moved(list, 'a', null).map((x) => x.id)).toEqual(['b', 'c', 'a']);
    expect(moved(list, 'a', 'a')).toBe(list);
  });

  it('name the daemon the conversation that ends up right above the moved one', () => {
    const list = [s('a'), s('b'), s('c')];
    expect(daemonAnchor(list, 'c', 'a')).toBeNull();
    expect(daemonAnchor(list, 'c', 'b')).toBe('a');
    expect(daemonAnchor(list, 'a', null)).toBe('c');
    // A pinned conversation above is ordered apart, so it is never the anchor.
    expect(daemonAnchor([s('p', { pinned: true }), ...list], 'c', 'a')).toBeNull();
  });

  it('show status as one of four tones, and folders by their last name', () => {
    expect(['running', 'waiting', 'interrupted', 'completed'].map(toneOf)).toEqual(['running', 'needsYou', 'warning', 'idle']);
    expect(folderName('C:\\Code\\app\\')).toBe('app');
  });
});
