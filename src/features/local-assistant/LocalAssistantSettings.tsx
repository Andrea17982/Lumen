import { useEffect, useState } from 'react';
import type { LocalAssistant } from './assistant';
import type { LocalAssistantConfig } from './types';

interface Props {
  assistant: LocalAssistant;
  config: LocalAssistantConfig;
  onChange: (next: LocalAssistantConfig) => void;
}

/** Opt-in switch, model picker and title toggle. */
export default function LocalAssistantSettings({ assistant, config, onChange }: Props) {
  const [models, setModels] = useState<string[]>([]);
  const [note, setNote] = useState('');
  useEffect(() => {
    if (!config.enabled) return;
    const ctl = new AbortController();
    void assistant.installedModels(ctl.signal).then((r) => {
      if (ctl.signal.aborted) return;
      setModels(r.ok ? r.models : []);
      setNote(
        r.ok
          ? r.models.length
            ? ''
            : 'No models installed. Run `ollama pull llama3.2`.'
          : r.message,
      );
    });
    return () => ctl.abort();
  }, [assistant, config.enabled]);
  const set = (p: Partial<LocalAssistantConfig>) => onChange({ ...config, ...p });
  return (
    <section className="local-assistant-settings">
      <label>
        <input
          type="checkbox"
          checked={config.enabled}
          onChange={(e) => set({ enabled: e.target.checked })}
        />
        Local assistant (Ollama)
      </label>
      {config.enabled && (
        <>
          <label>
            Model
            <select value={config.model} onChange={(e) => set({ model: e.target.value })}>
              <option value="">Choose…</option>
              {config.model && !models.includes(config.model) && (
                <option value={config.model}>{config.model}</option>
              )}
              {models.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </label>
          <label>
            <input
              type="checkbox"
              checked={config.autoTitles}
              onChange={(e) => set({ autoTitles: e.target.checked })}
            />
            Name new chats automatically
          </label>
          <p className="muted">
            Commit messages and handoff summaries are only written when you ask, and you can edit
            them first. Nothing is ever sent to a remote provider.
          </p>
          {note && <p role="status">{note}</p>}
        </>
      )}
    </section>
  );
}
