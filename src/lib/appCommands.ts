import type { Command } from './commands';
import { ACCESS_ORDER } from '../components/PermissionButton';
import type { SettingsTab } from '../settings/SettingsPanel';
import { active } from './messages';
import type { AccessMode, Session } from './types';

const ACCESS_LABEL: Record<AccessMode, string> = {
  'read-only': 'Ask (read-only)',
  'file-edits': 'Edits',
  ask: 'Approve each action',
  autopilot: 'Autopilot',
};
const SETTINGS_TABS: SettingsTab[] = ['general', 'accounts', 'auto', 'teamwork', 'tools', 'about'];

/** Rename, pin and delete for one conversation; the sidebar menu and the palette share them. */
export function conversationCommands(
  s: Session,
  at = { x: window.innerWidth / 2 - 136, y: 96 },
  act: {
    rename: (s: Session, pos: { x: number; y: number }) => void;
    pin: (s: Session) => void;
    remove: (s: Session) => void;
  },
): Command[] {
  return [
    {
      id: 'session-rename',
      label: 'Rename',
      group: 'Conversation',
      run: () =>
        act.rename(s, {
          x: Math.min(at.x, window.innerWidth - 272),
          y: Math.min(at.y, window.innerHeight - 150),
        }),
    },
    {
      id: 'session-pin',
      label: s.pinned ? 'Unpin' : 'Pin conversation',
      group: 'Conversation',
      run: () => act.pin(s),
    },
    {
      id: 'session-delete',
      label: 'Delete',
      group: 'Conversation',
      available: !active(s.status),
      run: () => act.remove(s),
    },
  ];
}

/** Everything the command palette and the keyboard shortcuts can run. */
export function appCommands(c: {
  session: Session | null | undefined;
  /** A conversation is open and idle. */
  canAct: boolean;
  hasMessages: boolean;
  newChat: () => void;
  newProject: () => void;
  search: () => void;
  compact: () => void;
  handoff: () => void;
  commit: () => void;
  usage: () => void;
  settings: (tab: SettingsTab) => void;
  access: (mode: AccessMode) => void;
  theme: (name: string) => void;
  sessionCommands: (s: Session) => Command[];
}): Command[] {
  return [
    {
      id: 'new-chat',
      label: 'New conversation',
      group: 'Conversation',
      shortcut: 'Ctrl N',
      run: c.newChat,
    },
    {
      id: 'new-project',
      label: 'New project',
      group: 'Project',
      shortcut: 'Ctrl P',
      run: c.newProject,
    },
    {
      id: 'search',
      label: 'Search conversations',
      group: 'Conversation',
      shortcut: '/',
      run: c.search,
    },
    {
      id: 'compact',
      label: 'Compact context into a fresh session',
      group: 'Conversation',
      keywords: 'handoff summary shrink',
      available: c.canAct && c.hasMessages,
      run: c.compact,
    },
    {
      id: 'handoff',
      label: 'Draft a handoff summary',
      group: 'Conversation',
      available: c.canAct && c.hasMessages,
      run: c.handoff,
    },
    {
      id: 'commit',
      label: 'Draft a commit message',
      group: 'Project',
      keywords: 'git diff',
      available: c.canAct,
      run: c.commit,
    },
    {
      id: 'usage',
      label: 'Show provider usage',
      group: 'Providers',
      keywords: 'quota limits',
      run: c.usage,
    },
    ...SETTINGS_TABS.map((tab): Command => ({
      id: `settings-${tab}`,
      label: `Settings: ${tab[0].toUpperCase()}${tab.slice(1)}`,
      group: 'Settings',
      keywords: 'preferences',
      run: () => c.settings(tab),
    })),
    ...ACCESS_ORDER.map((access): Command => ({
      id: `access-${access}`,
      label: `Permission mode: ${ACCESS_LABEL[access]}`,
      group: 'Permissions',
      keywords: 'access',
      run: () => c.access(access),
    })),
    ...(['system', 'light', 'dark'] as const).map((name): Command => ({
      id: `theme-${name}`,
      label: `Theme: ${name}`,
      group: 'Appearance',
      run: () => c.theme(name),
    })),
    ...(c.session ? c.sessionCommands(c.session) : []),
  ];
}
