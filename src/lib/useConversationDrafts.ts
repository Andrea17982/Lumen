import { useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';

export interface ComposerDraft {
  text: string;
  attachments: string[];
  error: string;
}
export const draftKey = (session: string | null) => session ?? 'new-conversation';
const empty = (): ComposerDraft => ({ text: '', attachments: [], error: '' });

/** Window-local drafts. Selection reads another entry; it never copies or clears a draft. */
export function useConversationDrafts(session: string | null) {
  const [drafts, setDrafts] = useState<Record<string, ComposerDraft>>({});
  const key = draftKey(session);
  const current = drafts[key] ?? empty();
  function update(target: string, change: (draft: ComposerDraft) => ComposerDraft) {
    setDrafts((all) => ({ ...all, [target]: change(all[target] ?? empty()) }));
  }
  const setText: Dispatch<SetStateAction<string>> = (value) =>
    update(key, (draft) => ({
      ...draft,
      text: typeof value === 'function' ? value(draft.text) : value,
    }));
  const setAttachments: Dispatch<SetStateAction<string[]>> = (value) =>
    update(key, (draft) => ({
      ...draft,
      attachments: typeof value === 'function' ? value(draft.attachments) : value,
    }));
  return {
    ...current,
    key,
    setText,
    setAttachments,
    clear: (target: string) => update(target, empty),
    clearTextIfUnchanged: (target: string, text: string) =>
      update(target, (draft) => (draft.text === text ? { ...draft, text: '' } : draft)),
    fail: (target: string, error: string, original?: ComposerDraft) =>
      update(target, (draft) => ({
        // A user can already be writing the next turn. Preserve both texts and all attachments.
        text: original ? [original.text, draft.text].filter(Boolean).join('\n\n') : draft.text,
        attachments: original
          ? [...new Set([...original.attachments, ...draft.attachments])]
          : draft.attachments,
        error,
      })),
    dismissError: () => update(key, (draft) => ({ ...draft, error: '' })),
    move: (source: string, target: string) =>
      setDrafts((all) => {
        const next = { ...all, [target]: all[source] ?? empty() };
        delete next[source];
        return next;
      }),
    remove: (target: string) =>
      setDrafts((all) => {
        const next = { ...all };
        delete next[target];
        return next;
      }),
  };
}
