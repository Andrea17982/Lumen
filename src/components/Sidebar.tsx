import { createPortal } from 'react-dom';
import { useState } from 'react';
import { Folder, MessageSquare, Pin } from 'lucide-react';
import { useI18n } from '../lib/i18n';
import type { Session, Workspace } from '../lib/types';

const DRAG = 'text/lumen-session';

/** Project filter chips; dropping a conversation on a project moves it there. */
export function ProjectList({
  workspaces,
  filter,
  onFilter,
  onDropSession,
  onRename,
  onDelete,
}: {
  workspaces: Workspace[];
  /** `*` for all conversations, `''` for unsorted ones. */
  filter: string;
  onFilter: (id: string) => void;
  onDropSession: (sessionId: string, workspaceId: string) => void;
  onRename: (id: string, name: string) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
}) {
  const { t } = useI18n();
  const [menu, setMenu] = useState<{ project: Workspace; x: number; y: number } | null>(null);
  const [edit, setEdit] = useState<{ project: Workspace; action: 'rename' | 'delete' } | null>(
    null,
  );
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const items = workspaces.length ? [{ id: '*', name: t('All conversations') }, ...workspaces] : [];
  const submit = async () => {
    if (!edit || busy) return;
    setBusy(true);
    try {
      if (edit.action === 'rename') await onRename(edit.project.id, name.trim());
      else await onDelete(edit.project.id);
      setEdit(null);
    } catch {
      /* The parent shows the error; keep this dialog available. */
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="project-list" aria-label="Projects">
      {items.map((w) => (
        <button
          key={w.id}
          className={filter === w.id ? 'selected' : ''}
          onClick={() => onFilter(w.id)}
          onContextMenu={(e) => {
            if (w.id === '*') return;
            e.preventDefault();
            setMenu({
              project: w as Workspace,
              x: Math.min(e.clientX, window.innerWidth - 220),
              y: Math.min(e.clientY, window.innerHeight - 110),
            });
          }}
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
      {menu &&
        createPortal(
          <>
            <button
              className="context-scrim"
              aria-label={t('Close project menu')}
              onClick={() => setMenu(null)}
            />
            <div
              className="popup context-menu"
              role="menu"
              style={{ left: menu.x, top: menu.y }}
              onKeyDown={(e) => {
                if (e.key === 'Escape') setMenu(null);
              }}
            >
              {(['rename', 'delete'] as const).map((action) => (
                <button
                  key={action}
                  role="menuitem"
                  autoFocus={action === 'rename'}
                  onClick={() => {
                    setEdit({ project: menu.project, action });
                    setName(menu.project.name);
                    setMenu(null);
                  }}
                >
                  {t(action === 'rename' ? 'Rename project' : 'Delete project')}
                </button>
              ))}
            </div>
          </>,
          document.body,
        )}
      {edit &&
        createPortal(
          <div className="dialog-scrim">
            <form
              className="popup conversation-dialog"
              role="dialog"
              aria-label={t(edit.action === 'rename' ? 'Rename project' : 'Delete project')}
              onSubmit={(e) => {
                e.preventDefault();
                void submit();
              }}
              onKeyDown={(e) => {
                if (e.key === 'Escape' && !busy) setEdit(null);
              }}
            >
              {edit.action === 'rename' ? (
                <label>
                  {t('Project name')}
                  <input
                    autoFocus
                    value={name}
                    maxLength={100}
                    onChange={(e) => setName(e.target.value)}
                  />
                </label>
              ) : (
                <p>
                  {t('Delete project')} “{edit.project.name}”?{' '}
                  {t(
                    'All conversations in this project will be deleted. Move any conversations you want to keep first. Files in the project folder are kept.',
                  )}
                </p>
              )}
              <div className="rename-actions">
                <button
                  type="button"
                  autoFocus={edit.action === 'delete'}
                  disabled={busy}
                  onClick={() => setEdit(null)}
                >
                  {t('Cancel')}
                </button>
                <button type="submit" disabled={busy || (edit.action === 'rename' && !name.trim())}>
                  {t(edit.action === 'rename' ? 'Save' : 'Delete project')}
                </button>
              </div>
            </form>
          </div>,
          document.body,
        )}
    </div>
  );
}

/** The conversation list: select, drag to reorder, and open each row's action menu. */
export function SessionList({
  sessions,
  empty,
  searching,
  selected,
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
            onKeyDown={(e) => {
              if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
                e.preventDefault();
                const r = e.currentTarget.getBoundingClientRect();
                onMenu(
                  s,
                  Math.min(r.left, window.innerWidth - 245),
                  Math.min(r.bottom, window.innerHeight - 180),
                  e.currentTarget,
                );
              }
            }}
            className={`session-row ${selected === s.id ? 'selected' : ''}`}
            onClick={() => onSelect(s.id)}
          >
            <span className={`status-dot ${s.status}`} title={s.status} />
            <span className="session-title">{s.title}</span>
            {s.pinned && <Pin size={12} />}
            <span className="sr-only">{s.status}</span>
          </button>
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
