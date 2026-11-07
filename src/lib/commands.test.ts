import { describe, expect, it } from 'vitest';
import { filterCommands, runCommand, type Command } from './commands';

const make = (id: string, label: string, extra: Partial<Command> = {}): Command => ({
  id,
  label,
  group: 'General',
  run: () => undefined,
  ...extra,
});

describe('command registry', () => {
  const commands = [
    make('new', 'New conversation'),
    make('settings', 'Open settings', { keywords: 'preferences' }),
    make('compact', 'Compact context', { available: false }),
  ];
  it('returns every available command for an empty query', () => {
    expect(filterCommands(commands, '').map((c) => c.id)).toEqual(['new', 'settings']);
  });
  it('prefers contiguous label matches and falls back to keywords', () => {
    expect(filterCommands(commands, 'set')[0].id).toBe('settings');
    expect(filterCommands(commands, 'prefer').map((c) => c.id)).toEqual(['settings']);
  });
  it('matches subsequences and excludes non-matches', () => {
    expect(filterCommands(commands, 'ncv').map((c) => c.id)).toEqual(['new']);
    expect(filterCommands(commands, 'zzz')).toEqual([]);
  });
  it('runs a command by id only when it is available', () => {
    const seen: string[] = [];
    const registry = [
      make('new', 'New conversation', { run: () => seen.push('new') }),
      make('compact', 'Compact context', { available: false, run: () => seen.push('compact') }),
    ];
    expect(runCommand(registry, 'new')).toBe(true);
    expect(runCommand(registry, 'compact')).toBe(false);
    expect(runCommand(registry, 'missing')).toBe(false);
    expect(seen).toEqual(['new']);
  });
});
