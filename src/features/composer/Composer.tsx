import { useEffect, useRef, useState, type ClipboardEvent, type DragEvent, type KeyboardEvent } from 'react';
import { Button, IconButton, ModelSelect, Select } from '../../design/components';
import { formatNumber } from '../../i18n/format';
import { useLocale, useT } from '../../i18n';
import { isCancelled, isSessionActive, type AccessMode, type AutoPolicy, type ModelInfo, type RunOptions, type Session } from '../../protocol/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import { visibleText } from '../attention/requests';
import s from './Composer.module.css';

const AUTO = 'auto';
const ACCESS: AccessMode[] = ['read-only', 'file-edits', 'autopilot', 'ask'];
const POLICIES: AutoPolicy[] = ['budget', 'default', 'quality'];
const MAX_IMAGE = 24 * 1024 * 1024;
// /every <interval> [HH:MM] <task>: the daemon parses the interval and the time.
const EVERY = /^\/every(?:\s+(\S+))?(?:\s+(\d{1,2}:\d{2})(?=\s))?(?:\s+([\s\S]+))?$/i;

/**
 * Sends work to the daemon. Choices are run options for the next message; access mode belongs to
 * the session and is saved at once. Stop asks the daemon to cancel; "stopped" shows only when
 * the daemon reports it.
 */
export function Composer({ session, draft }: { session: Session; draft?: { text: string } | null }) {
  const t = useT();
  const locale = useLocale();
  const sync = useSyncClient();
  const blocked = useSyncState((st) => st.unconfirmed.some((u) => u.sessionId === session.id));
  const connected = useSyncState((st) => st.connection.state === 'connected');
  const [text, setText] = useState('');
  const [route, setRoute] = useState<string>(AUTO);
  const [policy, setPolicy] = useState<AutoPolicy>('default');
  const accounts = useSyncState((st) => st.accounts);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [model, setModel] = useState<string | null>(null);
  const [effort, setEffort] = useState('');
  const [attachments, setAttachments] = useState<string[]>([]);
  const [tokens, setTokens] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const input = useRef<HTMLTextAreaElement>(null);
  const picker = useRef<HTMLInputElement>(null);
  // Stop is offered from the moment a message is sent: Auto may still be preparing the turn.
  const running = isSessionActive(session.status) || sending;

  // Text handed back by the conversation (after a rewind) replaces what is typed.
  useEffect(() => {
    if (!draft) return;
    setText(draft.text);
    input.current?.focus();
  }, [draft]);

  // A plugin panel's prompt.insert adds to what is typed; nothing is sent by itself.
  const insert = useSyncState((st) => st.insert);
  useEffect(() => {
    const text = insert ? sync.takeInsert() : null;
    if (text !== null) setText((current) => (current.trim() ? `${current}
${text}` : text));
  }, [sync, insert]);

  // Typing "/" offers the commands: the built-in /every and those of enabled plugins.
  const pluginCommands = useSyncState((st) => st.contributions.slashCommands);
  const slash = /^\/(\S*)$/.exec(text);
  const commands = slash
    ? [{ name: 'every', description: t.slash.every, prompt: '/every ' }, ...pluginCommands.map((c) => ({ name: c.name, description: visibleText(c.description), prompt: visibleText(c.prompt) }))].filter((c) => c.name.toLowerCase().startsWith(slash[1].toLowerCase()))
    : [];

  const account = accounts.find((a) => a.id === route);
  useEffect(() => {
    if (!account) return setModels([]);
    let live = true;
    sync.call<ModelInfo[]>('list_models', { id: account.id, workspace: session.workspace }).then(
      (list) => {
        if (!live) return;
        setModels(list);
        const chosen = list.find((m) => m.isDefault) ?? list[0];
        setModel(chosen?.id ?? null);
        setEffort(chosen?.defaultEffort ?? '');
      },
      (e) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, [sync, account, session.workspace]);

  // The token estimate follows the text, after a short pause.
  useEffect(() => {
    if (!text.trim()) return setTokens(null);
    const id = setTimeout(() => sync.call<number>('count_tokens', { text }).then(setTokens, () => setTokens(null)), 300);
    return () => clearTimeout(id);
  }, [sync, text]);

  // Grow with the text up to a limit, like a native field.
  useEffect(() => {
    const el = input.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, 240)}px`;
  }, [text]);

  const current = models.find((m) => m.id === model);
  const canSend = connected && !blocked && !sending && !running && text.trim().length > 0 && (route === AUTO || !!model);

  async function send() {
    if (!canSend) return;
    const options: RunOptions =
      route === AUTO
        ? { mode: AUTO, effort: '', autoPolicy: policy, accessMode: session.accessMode, attachments }
        : { mode: account!.provider, accountId: account!.id, model, effort, accessMode: session.accessMode, attachments };
    const routine = EVERY.exec(text.trim());
    if (routine) {
      if (!routine[1] || !routine[3]) return setError(t.routines.usage);
      setError(null);
      setSending(true);
      try {
        await sync.call('create_routine', { id: session.id, text: routine[3], every: routine[1], at: routine[2] ?? null, options: { ...options, attachments: [] } });
        setText('');
        setNotice(t.routines.created);
        sync.routinesChanged();
      } catch (e) {
        setError(String(e));
      } finally {
        setSending(false);
      }
      return;
    }
    setSending(true);
    setError(null);
    const sent = text;
    setText('');
    setAttachments([]);
    try {
      await sync.send(session.id, sent, options);
    } catch (e) {
      // The message stays in the transcript with its error; give the text back to edit.
      // Stopped before the turn started: nothing was recorded, so the text simply returns.
      // Anything typed meanwhile stays, after the returned text.
      setText((current) => (current.trim() ? `${sent}

${current}` : sent));
      setAttachments((current) => [...(options.attachments ?? []), ...current]);
      if (!isCancelled(e)) setError(String(e));
    } finally {
      setSending(false);
      input.current?.focus();
    }
  }

  const readBase64 = (file: File) =>
    new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).split(',', 2)[1] ?? '');
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });

  // Picked or dropped files of any type: the daemon keeps a copy and the message carries its path.
  async function attach(files: File[]) {
    if (files.length === 0) return;
    setError(null);
    for (const file of files) {
      if (file.size > MAX_IMAGE) {
        setError(t.composer.fileTooLarge);
        continue;
      }
      try {
        const path = await sync.call<string>('save_attachment', { data: await readBase64(file), name: file.name });
        setAttachments((list) => [...list, path]);
      } catch (e) {
        setError(String(e));
      }
    }
  }

  const hasFiles = (e: DragEvent) => [...e.dataTransfer.types].includes('Files');

  function onKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing && commands.length === 1 && commands[0].prompt !== text) {
      e.preventDefault();
      return setText(commands[0].prompt);
    }
    if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      void send();
    }
  }

  async function onPaste(e: ClipboardEvent<HTMLTextAreaElement>) {
    const file = [...e.clipboardData.files].find((f) => f.type.startsWith('image/'));
    if (!file) return;
    e.preventDefault();
    if (file.size > MAX_IMAGE) return setError(t.composer.imageTooLarge);
    const data = await readBase64(file);
    sync.call<string>('save_pasted_image', { data, mime: file.type }).then(
      (path) => setAttachments((list) => [...list, path]),
      (err) => setError(String(err)),
    );
  }

  const routes = [{ id: AUTO, label: t.modeAuto }, ...accounts.map((a) => ({ id: a.id, label: a.name }))];
  return (
    <form
      className={s.composer}
      data-tour-target="composer"
      aria-label={t.composer.label}
      onSubmit={(e) => {
        e.preventDefault();
        void send();
      }}
    >
      {blocked && <p className={s.notice}>{t.banner.unconfirmed}</p>}
      <div
        className={s.box}
        data-dragging={dragging ? '' : undefined}
        onDragOver={(e) => {
          if (!hasFiles(e)) return;
          e.preventDefault();
          setDragging(true);
        }}
        onDragLeave={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDragging(false);
        }}
        onDrop={(e) => {
          if (!hasFiles(e)) return;
          e.preventDefault();
          setDragging(false);
          void attach([...e.dataTransfer.files]);
        }}
      >
        <input
          ref={picker}
          type="file"
          multiple
          hidden
          tabIndex={-1}
          aria-hidden="true"
          onChange={(e) => {
            void attach([...(e.target.files ?? [])]);
            e.target.value = '';
          }}
        />
        {attachments.length > 0 && (
          <ul className={s.attachments} aria-label={t.conversationText.attachments}>
            {attachments.map((a) => (
              <li key={a}>
                <span>{a.split(/[\\/]/).at(-1)?.replace(/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}-/, '')}</span>
                <button type="button" aria-label={t.composer.remove(a)} onClick={() => setAttachments((list) => list.filter((x) => x !== a))}>
                  ×
                </button>
              </li>
            ))}
          </ul>
        )}
        {commands.length > 0 && (
          <ul className={s.commands} aria-label={t.composer.commands}>
            {commands.map((c) => (
              <li key={c.name}>
                <button
                  type="button"
                  onClick={() => {
                    setText(c.prompt);
                    input.current?.focus();
                  }}
                >
                  <code>/{c.name}</code>
                  <span>{c.description}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
        <textarea
          ref={input}
          className={s.input}
          rows={1}
          value={text}
          placeholder={t.composerPlaceholder}
          aria-label={t.composerPlaceholder}
          onChange={(e) => {
            setText(e.target.value);
            setNotice(null);
          }}
          onKeyDown={onKeyDown}
          onPaste={onPaste}
        />
        <div className={s.toolbar}>
          <Select compact tourTarget="agent" label={t.composer.agent} value={route} options={routes} onChange={setRoute} />
          {route === AUTO ? (
            <Select compact tourTarget="auto" label={t.composer.policy} value={policy} options={POLICIES.map((p) => ({ id: p, label: t.composer.policies[p] }))} onChange={setPolicy} />
          ) : (
            <>
              {models.length > 0 && model && <ModelSelect label={t.composer.model} value={model} options={models.map((m) => ({ id: m.id, label: m.description === 'OpenCode · console credit' ? `${m.name} · ${t.composer.consoleCredit}` : m.description === 'OpenCode · free' ? `${m.name} · ${t.composer.free}` : m.name }))} onChange={(id) => {
                setModel(id);
                setEffort(models.find((m) => m.id === id)?.defaultEffort ?? '');
              }} />}
              {current && current.efforts.length > 0 && effort && <Select compact label={t.composer.effort} value={effort} options={current.efforts.map((e) => ({ id: e, label: e }))} onChange={setEffort} />}
            </>
          )}
          <Select
            compact
            tourTarget="access"
            label={t.composer.access}
            value={session.accessMode}
            options={ACCESS.map((a) => ({ id: a, label: t.access[a] }))}
            onChange={(mode) => sync.call('set_session_access_mode', { id: session.id, mode }).then(() => sync.resync(), (e) => setError(String(e)))}
          />
          <IconButton icon="paperclip" label={t.composer.attach} isDisabled={!connected} onPress={() => {
            input.current?.focus();
            picker.current?.click();
          }} />
          <span className={s.spacer} />
          {tokens !== null && <span className={s.tokens}>{t.composer.tokens(formatNumber(locale, tokens))}</span>}
          {running ? (
            <Button variant="secondary" data-tour-target="stop" onPress={() => sync.call('cancel_run', { id: session.id }).catch((e) => setError(String(e)))}>
              {t.stop}
            </Button>
          ) : (
            <Button variant="primary" type="submit" isDisabled={!canSend}>
              {t.send}
            </Button>
          )}
        </div>
      </div>
      {notice && (
        <p className={s.hint} role="status">
          {notice}
        </p>
      )}
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
    </form>
  );
}
