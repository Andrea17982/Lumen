import { call } from '../../lib/bridge';
import type { Message, Session } from '../../lib/types';
import type { LocalAssistant } from './assistant';

/** The title the backend gives a new chat from its first message (first 8 words, 64 chars). */
export function firstMessageTitle(text: string): string {
  return Array.from(text.split(/\s+/).filter(Boolean).slice(0, 8).join(' ')).slice(0, 64).join('');
}

/**
 * Names a chat once, after its first reply, and only while it still carries the title
 * Lumen derived from its first message. A rename made meanwhile (checked again after the
 * model answers) is never overwritten. Returns the new title, or null when nothing changed.
 */
export async function nameNewChat(
  assistant: LocalAssistant,
  chat: Pick<Session, 'id' | 'title' | 'pinned'>,
  messages: Message[],
  currentTitle: () => string | undefined,
): Promise<string | null> {
  const users = messages.filter((m) => m.role === 'user');
  if (users.length !== 1) return null;
  const first = users[0].text;
  const untouched = chat.title === firstMessageTitle(first);
  if (
    !assistant.shouldAutoTitle({
      title: untouched ? '' : chat.title,
      titleEdited: false,
      messageCount: messages.length,
    })
  )
    return null;
  const r = await assistant.title(first);
  if (!r.ok || currentTitle() !== chat.title) return null;
  await call('update_session', { id: chat.id, title: r.text, pinned: chat.pinned });
  return r.text;
}
