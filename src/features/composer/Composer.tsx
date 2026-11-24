import { useEffect, useRef, useState, type ClipboardEvent, type KeyboardEvent } from 'react';
import { Button, Select } from '../../design/components';
import { formatNumber } from '../../i18n/format';
import { useLocale, useT } from '../../i18n';
import type { AccessMode, AutoPolicy, ModelInfo, RunOptions, Session } from '../../protocol/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import s from './Composer.module.css';

const AUTO = 'auto';
const ACCESS: AccessMode[] = ['read-only', 'file-edits', 'autopilot', 'ask'];
const POLICIES: AutoPolicy[] = ['budget', 'default', 'quality'];
const MAX_IMAGE = 24 * 1024 * 1024;

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
  const input = useRef<HTMLTextAreaElement>(null);
  const running = session.status === 'running' || session.status === 'queued' || session.status === 'waiting';

  // Text handed back by the conversation (after a rewind) replaces what is typed.
  useEffect(() => {
    if (!draft) return;
    setText(draft.text);
    input.current?.focus();
  }, [draft]);

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
    setSending(true);
    setError(null);
    const sent = text;
    setText('');
    setAttachments([]);
    try {
      await sync.send(session.id, sent, options);
    } catch (e) {
      // The message stays in the transcript with its error; give the text back to edit.
      setText(sent);
      setError(String(e));
    } finally {
      setSending(false);
      input.current?.focus();
    }
  }

  function onKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
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
    const data = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).split(',', 2)[1] ?? '');
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });
    sync.call<string>('save_pasted_image', { data, mime: file.type }).then(
      (path) => setAttachments((list) => [...list, path]),
      (err) => setError(String(err)),
    );
  }

  const routes = [{ id: AUTO, label: t.modeAuto }, ...accounts.map((a) => ({ id: a.id, label: a.name }))];
  return (
    <form
      className={s.composer}
      aria-label={t.composer.label}
      onSubmit={(e) => {
        e.preventDefault();
        void send();
      }}
    >
      {blocked && <p className={s.notice}>{t.banner.unconfirmed}</p>}
      <div className={s.box}>
        {attachments.length > 0 && (
          <ul className={s.attachments} aria-label={t.conversationText.attachments}>
            {attachments.map((a) => (
              <li key={a}>
                <span>{a.split(/[\\/]/).at(-1)}</span>
                <button type="button" aria-label={t.composer.remove(a)} onClick={() => setAttachments((list) => list.filter((x) => x !== a))}>
                  ×
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
          onChange={(e) => setText(e.target.value)}
          onKeyDown={onKeyDown}
          onPaste={onPaste}
        />
        <div className={s.toolbar}>
          <Select compact label={t.composer.agent} value={route} options={routes} onChange={setRoute} />
          {route === AUTO ? (
            <Select compact label={t.composer.policy} value={policy} options={POLICIES.map((p) => ({ id: p, label: t.composer.policies[p] }))} onChange={setPolicy} />
          ) : (
            <>
              {models.length > 0 && model && <Select compact label={t.composer.model} value={model} options={models.map((m) => ({ id: m.id, label: m.name }))} onChange={(id) => {
                setModel(id);
                setEffort(models.find((m) => m.id === id)?.defaultEffort ?? '');
              }} />}
              {current && current.efforts.length > 0 && effort && <Select compact label={t.composer.effort} value={effort} options={current.efforts.map((e) => ({ id: e, label: e }))} onChange={setEffort} />}
            </>
          )}
          <Select
            compact
            label={t.composer.access}
            value={session.accessMode}
            options={ACCESS.map((a) => ({ id: a, label: t.access[a] }))}
            onChange={(mode) => sync.call('set_session_access_mode', { id: session.id, mode }).then(() => sync.resync(), (e) => setError(String(e)))}
          />
          <span className={s.spacer} />
          {tokens !== null && <span className={s.tokens}>{t.composer.tokens(formatNumber(locale, tokens))}</span>}
          {running ? (
            <Button variant="secondary" onPress={() => sync.call('cancel_run', { id: session.id }).catch((e) => setError(String(e)))}>
              {t.stop}
            </Button>
          ) : (
            <Button variant="primary" type="submit" isDisabled={!canSend}>
              {t.send}
            </Button>
          )}
        </div>
      </div>
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
    </form>
  );
}
