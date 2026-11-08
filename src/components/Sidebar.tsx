import { useState } from 'react';
import { Folder, MessageSquare, MoreHorizontal, Pin } from 'lucide-react';
import { useI18n } from '../lib/i18n';
import type { Session, Workspace } from '../lib/types';
import { IconButton } from '../lib/util';

const DRAG = 'text/lumen-session';

/** Project filter chips; dropping a conversation on a project moves it there. */
export function ProjectList({
  workspaces,
  filter,
  onFilter,
  onDropSession,
}: {
  workspaces: Workspace[];
  /** `*` for all conversations, `''` for unsorted ones. */
  filter: string;
  onFilter: (id: string) => void;
  onDropSession: (sessionId: string, workspaceId: string) => void;
}) {
  const { t } = useI18n();
  const items = [
    { id: '*', name: t('All conversations') },
    ...(workspaces.length ? [{ id: '', name: t('Unsorted') }] : []),
    ...workspaces,
  ];
  return (
    <div className="project-list" aria-label="Projects">
      {items.map((w) => (
        <button
          key={w.id}
          className={filter === w.id ? 'selected' : ''}
          onClick={() => onFilter(w.id)}
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => {
            e.preventDefault();
            const id = e.dataTransfer.getData(DRAG);
            if (id && w.id !== '*') onDropSession(id, w.id);
          }}
        >
          <Folder size={13} />
          {w.name}
        </button>
      ))}
    </div>
  );
}

/** The conversation list: select, drag to reorder, and open each row's action menu. */
export function SessionList({
  sessions,
  empty,
  searching,
  selected,
  menuOpenFor,
  hasMore,
  onSelect,
  onReorder,
  onMenu,
  onLoadMore,
}: {
  sessions: Session[];
  /** No conversations at all (before the project filter). */
  empty: boolean;
  searching: boolean;
  selected: string | null;
  menuOpenFor?: string;
  hasMore: boolean;
  onSelect: (id: string) => void;
  onReorder: (id: string, beforeId: string) => void;
  onMenu: (session: Session, x: number, y: number, trigger: HTMLElement) => void;
  onLoadMore: () => void;
}) {
  const { t } = useI18n();
  const [dragged, setDragged] = useState<string | null>(null);
  return (
    <nav className="session-list" aria-label="Conversations">
      {empty && (
        <div className="sidebar-empty">
          <MessageSquare size={20} />
          <p>
            {searching ? t('No conversations found.') : t('A little room for your next big idea.')}
          </p>
        </div>
      )}
      {sessions.map((s) => (
        <div className="session-item" key={s.id}>
          <button
            draggable
            onDragStart={(e) => {
              setDragged(s.id);
              e.dataTransfer.setData(DRAG, s.id);
            }}
            onDragEnd={() => setDragged(null)}
            onDragOver={(e) => {
              if (dragged) e.preventDefault();
            }}
            onDrop={(e) => {
              e.preventDefault();
              const id = e.dataTransfer.getData(DRAG);
              if (id && id !== s.id) onReorder(id, s.id);
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              onMenu(
                s,
                Math.min(e.clientX, window.innerWidth - 245),
                Math.min(e.clientY, window.innerHeight - 130),
                e.currentTarget,
              );
            }}
            className={`session-row ${selected === s.id ? 'selected' : ''}`}
            onClick={() => onSelect(s.id)}
          >
            <span className={`status-dot ${s.status}`} title={s.status} />
            <span className="session-title">{s.title}</span>
            {s.pinned && <Pin size={12} />}
            <span className="sr-only">{s.status}</span>
          </button>
          <IconButton
            className="session-menu-button"
            label={`Actions for ${s.title}`}
            aria-haspopup="menu"
            aria-expanded={menuOpenFor === s.id}
            onClick={(e) => {
              const rect = e.currentTarget.getBoundingClientRect();
              onMenu(
                s,
                Math.max(0, Math.min(rect.right, window.innerWidth - 245)),
                Math.max(0, Math.min(rect.bottom, window.innerHeight - 180)),
                e.currentTarget,
              );
            }}
          >
            <MoreHorizontal size={16} />
          </IconButton>
        </div>
      ))}
      {hasMore && (
        <button className="text-button load-more" onClick={onLoadMore}>
          {t('Load more conversations')}
        </button>
      )}
    </nav>
  );
}
