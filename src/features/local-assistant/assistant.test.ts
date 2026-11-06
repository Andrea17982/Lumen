import { describe, expect, it, vi } from 'vitest';
import { LocalAssistant } from './assistant';
import { Draft } from './draft';
import { cleanOutput } from './prompts';
import { assertLoopback, httpTransport } from './transport';
import {
  DEFAULT_CONFIG,
  TransportError,
  type GenerateRequest,
  type LocalAssistantConfig,
  type OllamaTransport,
} from './types';

const on: LocalAssistantConfig = { ...DEFAULT_CONFIG, enabled: true, model: 'qwen2.5:7b' };

function fake(generate: (r: GenerateRequest) => Promise<string>): OllamaTransport & {
  generate: ReturnType<typeof vi.fn>;
} {
  return { generate: vi.fn(generate), models: vi.fn(async () => ['qwen2.5:7b']) };
}
const hang = (r: GenerateRequest) =>
  new Promise<string>((_, rej) =>
    r.signal.addEventListener('abort', () => rej(new Error('aborted'))),
  );

describe('triggers', () => {
  const chat = { title: '', titleEdited: false, messageCount: 2 };
  it('does nothing while disabled or without a model', async () => {
    const t = fake(async () => 'x');
    const a = new LocalAssistant(DEFAULT_CONFIG, t);
    expect(a.shouldAutoTitle(chat)).toBe(false);
    expect((await a.title('hi')).ok).toBe(false);
    expect((await a.commitMessage('diff')).ok).toBe(false);
    a.configure({ enabled: true });
    expect(await a.handoffSummary('chat')).toMatchObject({ ok: false, reason: 'no-model' });
    expect(t.generate).not.toHaveBeenCalled();
    expect(t.models).not.toHaveBeenCalled();
  });
  it('auto-titles only untitled, unedited chats with messages', () => {
    const a = new LocalAssistant(
      on,
      fake(async () => 'x'),
    );
    expect(a.shouldAutoTitle(chat)).toBe(true);
    expect(a.shouldAutoTitle({ ...chat, title: 'Mine' })).toBe(false);
    expect(a.shouldAutoTitle({ ...chat, titleEdited: true })).toBe(false);
    expect(a.shouldAutoTitle({ ...chat, messageCount: 0 })).toBe(false);
    a.configure({ autoTitles: false });
    expect(a.shouldAutoTitle(chat)).toBe(false);
  });
  it('rejects empty input without calling the model', async () => {
    const t = fake(async () => 'x');
    expect(await new LocalAssistant(on, t).commitMessage('  ')).toMatchObject({
      reason: 'invalid-input',
    });
    expect(t.generate).not.toHaveBeenCalled();
  });
});

describe('failures keep the manual flow', () => {
  it('maps service, model, empty, timeout and cancel', async () => {
    const down = new LocalAssistant(
      on,
      fake(async () => Promise.reject(new TypeError('boom'))),
    );
    expect(await down.title('x')).toMatchObject({ ok: false, reason: 'unavailable' });
    const missing = new LocalAssistant(
      on,
      fake(async () => Promise.reject(new TransportError('model-missing', 'no'))),
    );
    expect(await missing.title('x')).toMatchObject({ reason: 'model-missing' });
    expect(
      await new LocalAssistant(
        on,
        fake(async () => '  '),
      ).title('x'),
    ).toMatchObject({ reason: 'empty' });

    vi.useFakeTimers();
    const slow = new LocalAssistant({ ...on, timeoutMs: 1000 }, fake(hang));
    const p = slow.title('x');
    await vi.advanceTimersByTimeAsync(1000);
    expect(await p).toMatchObject({ reason: 'timeout' });
    vi.useRealTimers();

    const ctl = new AbortController();
    const p2 = new LocalAssistant(on, fake(hang)).title('x', ctl.signal);
    ctl.abort();
    expect(await p2).toMatchObject({ reason: 'cancelled' });
  });
  it('sends the configured model and cleans the title', async () => {
    const t = fake(async () => '<think>hm</think>Title: "Fix the login bug."\nextra');
    const r = await new LocalAssistant(on, t).title('user: login fails');
    expect(r).toEqual({ ok: true, text: 'Fix the login bug', model: 'qwen2.5:7b' });
    expect(t.generate.mock.calls[0][0].model).toBe('qwen2.5:7b');
  });
  it('keeps multi-line commit messages intact', () => {
    expect(cleanOutput('commit', '```\nAdd x\n\nBody\n```')).toBe('Add x\n\nBody');
  });
});

describe('drafts', () => {
  it('only generate on request, stay editable and respect typing during generation', async () => {
    let release!: (v: string) => void;
    const t = fake(() => new Promise<string>((r) => (release = r)));
    const d = Draft.commit(new LocalAssistant(on, t), () => 'diff');
    expect(t.generate).not.toHaveBeenCalled();
    expect(d.take()).toBeNull();
    const run = d.generate();
    d.edit('my own message');
    release('Generated');
    await run;
    expect(d.getState().text).toBe('my own message');
    expect(d.take()).toBe('my own message');
  });
  it('fills the text, then marks it as edited once touched', async () => {
    const d = Draft.handoff(
      new LocalAssistant(
        on,
        fake(async () => '- goal'),
      ),
      () => 'chat',
    );
    await d.generate();
    expect(d.getState()).toMatchObject({ text: '- goal', generated: true, status: 'ready' });
    d.edit('- goal\n- mine');
    expect(d.getState().generated).toBe(false);
  });
  it('reports errors and cancel without losing manual text', async () => {
    const d = Draft.commit(new LocalAssistant(on, fake(hang)), () => 'diff');
    const run = d.generate();
    d.cancel();
    await run;
    expect(d.getState().status).toBe('idle');
    const off = Draft.commit(
      new LocalAssistant(
        DEFAULT_CONFIG,
        fake(async () => 'x'),
      ),
      () => 'd',
    );
    d.edit('typed');
    await off.generate();
    expect(off.getState().status).toBe('error');
    expect(d.take()).toBe('typed');
  });
});

describe('no remote calls', () => {
  it('refuses redirects for both generation and model discovery', async () => {
    const fetcher = vi.fn(async (_url: string | URL | Request, init?: RequestInit) => {
      expect(init?.redirect).toBe('error');
      throw new TypeError('redirect refused');
    });
    const a = new LocalAssistant(on, httpTransport(undefined, fetcher));
    expect(await a.title('x')).toMatchObject({ reason: 'unavailable' });
    expect(await a.installedModels()).toMatchObject({ reason: 'unavailable' });
    expect(fetcher).toHaveBeenCalledTimes(2);
  });

  it('refuses non-loopback endpoints', () => {
    for (const u of [
      'https://api.openai.com',
      'http://example.com:11434',
      'http://192.168.1.5:11434',
    ])
      expect(() => assertLoopback(u)).toThrow();
    expect(() => httpTransport('https://api.anthropic.com')).toThrow();
    expect(() => assertLoopback('http://localhost:11434')).not.toThrow();
  });
  it('only ever fetches the loopback daemon, and a dead daemon is "unavailable"', async () => {
    const urls: string[] = [];
    const f = vi.fn(async (url: string) => {
      urls.push(url);
      return new Response(JSON.stringify({ response: 'Hello' }));
    }) as unknown as typeof fetch;
    const a = new LocalAssistant(on, httpTransport(undefined, f));
    expect(await a.title('x')).toMatchObject({ ok: true, text: 'Hello' });
    await a.installedModels();
    expect(urls.every((u) => u.startsWith('http://127.0.0.1:11434/'))).toBe(true);

    const dead = vi.fn(async () =>
      Promise.reject(new TypeError('fetch failed')),
    ) as unknown as typeof fetch;
    expect(await new LocalAssistant(on, httpTransport(undefined, dead)).title('x')).toMatchObject({
      reason: 'unavailable',
    });
    const nf = vi.fn(async () => new Response('', { status: 404 })) as unknown as typeof fetch;
    expect(await new LocalAssistant(on, httpTransport(undefined, nf)).title('x')).toMatchObject({
      reason: 'model-missing',
    });
  });
});
