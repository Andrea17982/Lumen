import { describe, expect, it, vi } from 'vitest';
import { LocalAssistant } from './assistant';
import { firstMessageTitle, nameNewChat } from './autoTitle';
import { DEFAULT_CONFIG, type OllamaTransport } from './types';

const call = vi.hoisted(() => vi.fn(async () => null));
vi.mock('../../lib/bridge', () => ({ call }));

const msg = (role: 'user' | 'assistant', text: string) => ({
  id: text,
  role,
  text,
  activity: [],
});

function assistantAnswering(title: string) {
  const transport: OllamaTransport = {
    generate: vi.fn(async () => title),
    models: vi.fn(async () => ['llama3.2']),
  };
  const a = new LocalAssistant({ ...DEFAULT_CONFIG, enabled: true, model: 'llama3.2' }, transport);
  return a;
}

describe('firstMessageTitle', () => {
  it('mirrors the backend: first eight words, at most 64 characters', () => {
    expect(firstMessageTitle('one two three four five six seven eight nine')).toBe(
      'one two three four five six seven eight',
    );
    expect(firstMessageTitle('  ')).toBe('');
  });
});

describe('nameNewChat', () => {
  const chat = { id: 's1', title: 'Fix the login bug please', pinned: false };
  const first = [msg('user', 'Fix the login bug please'), msg('assistant', 'Done')];

  it('names an untouched chat after its first reply', async () => {
    call.mockClear();
    const a = assistantAnswering('Login bug fix');
    const title = await nameNewChat(a, chat, first, () => chat.title);
    expect(title).toBe('Login bug fix');
    expect(call).toHaveBeenCalledWith('update_session', {
      id: 's1',
      title: 'Login bug fix',
      pinned: false,
    });
  });

  it('never overwrites a chat the user renamed', async () => {
    call.mockClear();
    const a = assistantAnswering('Login bug fix');
    const renamed = { ...chat, title: 'My own name' };
    expect(await nameNewChat(a, renamed, first, () => renamed.title)).toBeNull();
    expect(call).not.toHaveBeenCalled();
  });

  it('keeps the chat title if it was renamed while the model was answering', async () => {
    call.mockClear();
    const a = assistantAnswering('Login bug fix');
    expect(await nameNewChat(a, chat, first, () => 'Renamed meanwhile')).toBeNull();
    expect(call).not.toHaveBeenCalled();
  });

  it('only runs on the first exchange', async () => {
    call.mockClear();
    const a = assistantAnswering('Later title');
    const later = [...first, msg('user', 'And then?'), msg('assistant', 'Sure')];
    expect(await nameNewChat(a, chat, later, () => chat.title)).toBeNull();
    expect(call).not.toHaveBeenCalled();
  });

  it('does nothing while the helper is off', async () => {
    call.mockClear();
    const a = new LocalAssistant(DEFAULT_CONFIG, { generate: vi.fn(), models: vi.fn() });
    expect(await nameNewChat(a, chat, first, () => chat.title)).toBeNull();
    expect(call).not.toHaveBeenCalled();
  });
});
