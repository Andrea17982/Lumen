import { useEffect, useMemo, useRef, useState, type DragEvent, type KeyboardEvent, type ReactNode } from 'react';
import { Mark, Wordmark } from '../../brand/Logo';
import { Button, ConfirmDialog, IconButton, Menu, SearchField, Select, TextField } from '../../design/components';
import { Icon } from '../../design/icons';
import { useT } from '../../i18n';
import type { Session, Workspace } from '../../protocol/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import { daemonAnchor, folderName, groups, inProject, moved, toneOf } from './sessions';
import s from './Sidebar.module.css';

type Dialog = { kind: 'delete' | 'move'; session: Session } | { kind: 'renameProject' | 'removeProject'; project: Workspace } | null;

/**
 * Projects and their conversations. One project is open at a time, as in the gallery mock: its
 * conversations are listed by day, and the project itself is switched at the bottom.
 */
export function Sidebar({ project, selected, onSelect, onProject, onHide, onSettings, footer }: { project: Workspace | null; selected: string | null; onSelect: (id: string | null) => void; onProject: (id: string | null) => void; onHide: () => void; onSettings: () => void; footer: ReactNode }) {
  const t = useT();
  const sync = useSyncClient();
  const all = useSyncState((st) => st.sessions);
  const workspaces = useSyncState((st) => st.workspaces);
  const complete = useSyncState((st) => st.sessionsComplete);
  const connected = useSyncState((st) => st.connection.state === 'connected');
  const [search, setSearch] = useState('');
  const [dialog, setDialog] = useState<Dialog>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const root = useRef<HTMLElement>(null);
  const sessions = useMemo(() => (project ? all.filter((x) => inProject(x, project)) : []), [all, project]);
  const list = useMemo(() => groups(sessions, Date.now()), [sessions]);

  // The daemon filters by title; a short pause keeps typing smooth.
  useEffect(() => {
    // The list is already read for this search (on start, or a page further with Show more).
    if (search === sync.getState().search) return;
    const id = setTimeout(() => void sync.searchSessions(search).catch((e) => setError(String(e))), 200);
    return () => clearTimeout(id);
  }, [sync, search]);

  // Ctrl+K searches, Ctrl+N starts a task in the open project.
  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return;
      if (e.key.toLowerCase() === 'k') {
        e.preventDefault();
        root.current?.querySelector<HTMLInputElement>('[role=search] input, input[type=search]')?.focus();
      } else if (e.key.toLowerCase() === 'n') {
        e.preventDefault();
        void newTask();
      }
    };
    addEventListener('keydown', onKey);
    return () => removeEventListener('keydown', onKey);
  });

  const run = <T,>(work: Promise<T>) => work.catch((e) => setError(String(e)));

  async function newTask() {
    if (!project || !connected) return;
    setError(null);
    const created = await run(sync.mutate<Session>('create_session', { workspace: project.folder }));
    if (created) onSelect(created.id);
  }

  async function addProject() {
    setError(null);
    const folder = await sync.pickFolder();
    if (!folder) return;
    const added = await run(sync.mutate<Workspace>('add_workspace', { name: folderName(folder), folder }));
    if (added) onProject(added.id);
  }

  // `target`: the conversation `id` goes right above; null puts it last.
  const reorder = (id: string, target: string | null) => run(sync.mutate('reorder_session', { id, beforeId: daemonAnchor(all, id, target) }, (list) => moved(list, id, target)));

  function onRowKey(e: KeyboardEvent, session: Session, group: Session[]) {
    if (!e.altKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
    e.preventDefault();
    const i = group.indexOf(session);
    if (e.key === 'ArrowUp' && i > 0) void reorder(session.id, group[i - 1].id);
    if (e.key === 'ArrowDown' && i < group.length - 1) void reorder(session.id, group[i + 2]?.id ?? null);
  }

  const dragged = useRef<string | null>(null);
  const onDrop = (e: DragEvent, target: Session) => {
    e.preventDefault();
    const id = dragged.current;
    dragged.current = null;
    if (id && id !== target.id) void reorder(id, target.id);
  };

  return (
    <aside ref={root} className={s.sidebar} aria-label={t.sessions}>
      <div className={s.brand}>
        <Mark size={24} />
        <Wordmark />
        <span className={s.spacer} />
        <IconButton icon="gear" label={t.settings.open} onPress={onSettings} />
        <IconButton icon="sidebar" label={t.sidebar.hide} onPress={onHide} />
      </div>
      <Button variant="secondary" icon="plus" isDisabled={!project || !connected} onPress={() => void newTask()}>
        {t.newTask}
      </Button>
      <SearchField label={t.search} clearLabel={t.clearSearch} value={search} onChange={setSearch} />
      <nav className={s.sessions} aria-label={t.sessions} aria-describedby="reorder-help">
        <p id="reorder-help" className={s.hidden}>
          {t.sidebar.reorderHelp}
        </p>
        {!project ? (
          <div className={s.empty}>
            <p>{t.sidebar.noProjects}</p>
            <Button variant="primary" icon="folder" isDisabled={!connected} onPress={() => void addProject()}>
              {t.sidebar.addProject}
            </Button>
          </div>
        ) : sessions.length === 0 ? (
          <p className={s.quiet}>{search ? t.sidebar.noResults : t.noSessions}</p>
        ) : (
          list.map((group) => (
            <section key={group.key} aria-labelledby={`group-${group.key}`}>
              <h2 id={`group-${group.key}`} className={s.group}>
                {{ pinned: t.sidebar.pinned, today: t.today, yesterday: t.yesterday, earlier: t.sidebar.earlier }[group.key]}
              </h2>
              <ul className={s.list}>
                {group.sessions.map((x) => (
                  <li key={x.id} className={s.row} data-current={x.id === selected || undefined} draggable={renaming !== x.id} onDragStart={() => (dragged.current = x.id)} onDragOver={(e) => e.preventDefault()} onDrop={(e) => onDrop(e, x)}>
                    {renaming === x.id ? (
                      <Rename
                        label={t.sidebar.title}
                        value={x.title}
                        onDone={(title) => {
                          setRenaming(null);
                          if (title !== null && title.trim() && title !== x.title) void run(sync.mutate('update_session', { id: x.id, title: title.trim(), pinned: x.pinned }, (l) => l.map((y) => (y.id === x.id ? { ...y, title: title.trim() } : y))));
                        }}
                      />
                    ) : (
                      <button className={s.session} aria-current={x.id === selected ? 'page' : undefined} onClick={() => onSelect(x.id)} onKeyDown={(e) => onRowKey(e, x, group.sessions)}>
                        <Status status={x.status} />
                        <span className={s.title}>{x.title || t.untitled}</span>
                        {toneOf(x.status) === 'needsYou' && (
                          <span className={s.badge} title={t.needsYou}>
                            {t.needsYouShort}
                          </span>
                        )}
                      </button>
                    )}
                    <span className={s.more}>
                      <Menu
                        label={t.moreActions}
                        items={[
                          { id: 'rename', label: t.rename },
                          { id: 'pin', label: x.pinned ? t.sidebar.unpin : t.pin },
                          { id: 'move', label: t.sidebar.moveTo, isDisabled: workspaces.length < 2 },
                          { id: 'delete', label: t.delete, danger: true, separatorBefore: true },
                        ]}
                        onAction={(id) => {
                          if (id === 'rename') setRenaming(x.id);
                          if (id === 'pin') void run(sync.mutate('update_session', { id: x.id, title: x.title, pinned: !x.pinned }, (l) => l.map((y) => (y.id === x.id ? { ...y, pinned: !x.pinned } : y))));
                          if (id === 'move' || id === 'delete') setDialog({ kind: id, session: x });
                        }}
                      />
                    </span>
                  </li>
                ))}
              </ul>
            </section>
          ))
        )}
        {project && !complete && (
          <Button variant="plain" onPress={() => void run(sync.showMore())}>
            {t.sidebar.showMore}
          </Button>
        )}
      </nav>
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
      {project && (
        <div className={s.project}>
          <Icon name="folder" />
          <div className={s.projectText}>
            <strong>{project.name}</strong>
            <small title={project.folder}>{project.folder}</small>
          </div>
          <Menu
            label={t.sidebar.switchProject(project.name)}
            icon="chevron"
            items={[
              ...workspaces.map((w) => ({ id: `open:${w.id}`, label: w.name, icon: w.id === project.id ? ('check' as const) : undefined })),
              { id: 'add', label: t.sidebar.addProject, separatorBefore: true },
              { id: 'rename', label: t.sidebar.renameProject },
              { id: 'remove', label: t.sidebar.removeProject, danger: true },
            ]}
            onAction={(id) => {
              if (id.startsWith('open:')) return onProject(id.slice(5));
              if (id === 'add') return void addProject();
              setDialog({ kind: id === 'rename' ? 'renameProject' : 'removeProject', project });
            }}
          />
        </div>
      )}
      {footer}
      <Dialogs
        dialog={dialog}
        workspaces={workspaces}
        onClose={() => setDialog(null)}
        onDelete={(x) => {
          if (x.id === selected) onSelect(null);
          void run(sync.mutate('delete_session', { id: x.id }, (l) => l.filter((y) => y.id !== x.id)));
        }}
        onMove={(x, w) => void run(sync.mutate('set_session_workspace', { id: x.id, workspaceId: w.id, folder: w.folder }))}
        onRenameProject={(w, name) => void run(sync.mutate('rename_workspace', { id: w.id, name }))}
        onRemoveProject={(w) => {
          onProject(null);
          onSelect(null);
          void run(sync.mutate('delete_workspace', { id: w.id }));
        }}
      />
    </aside>
  );
}

function Status({ status }: { status: string }) {
  const t = useT();
  const tone = toneOf(status);
  const label = tone === 'idle' ? undefined : (t.status[status] ?? status);
  return <span className={s.status} data-tone={tone} role={label ? 'img' : undefined} aria-label={label} aria-hidden={label ? undefined : true} />;
}

function Rename({ label, value, onDone }: { label: string; value: string; onDone: (title: string | null) => void }) {
  const [text, setText] = useState(value);
  return (
    <input
      className={s.rename}
      aria-label={label}
      value={text}
      maxLength={200}
      autoFocus
      onChange={(e) => setText(e.target.value)}
      onBlur={() => onDone(text)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') onDone(text);
        if (e.key === 'Escape') onDone(null);
      }}
    />
  );
}

function Dialogs({ dialog, workspaces, onClose, onDelete, onMove, onRenameProject, onRemoveProject }: { dialog: Dialog; workspaces: Workspace[]; onClose: () => void; onDelete: (s: Session) => void; onMove: (s: Session, w: Workspace) => void; onRenameProject: (w: Workspace, name: string) => void; onRemoveProject: (w: Workspace) => void }) {
  const t = useT();
  const [name, setName] = useState('');
  const [target, setTarget] = useState('');
  useEffect(() => {
    if (dialog?.kind === 'renameProject') setName(dialog.project.name);
    if (dialog?.kind === 'move') setTarget(workspaces.find((w) => !inProject(dialog.session, w))?.id ?? '');
  }, [dialog, workspaces]);
  const open = (o: boolean) => !o && onClose();
  return (
    <>
      <ConfirmDialog isOpen={dialog?.kind === 'delete'} onOpenChange={open} title={t.deleteSession} body={t.deleteSessionBody} confirmLabel={t.delete} cancelLabel={t.cancel} danger onConfirm={() => dialog?.kind === 'delete' && onDelete(dialog.session)} />
      <ConfirmDialog
        isOpen={dialog?.kind === 'move'}
        onOpenChange={open}
        title={t.sidebar.moveTitle}
        body={target ? <Select label={t.sidebar.project} value={target} options={workspaces.filter((w) => dialog?.kind !== 'move' || !inProject(dialog.session, w)).map((w) => ({ id: w.id, label: w.name }))} onChange={setTarget} /> : ''}
        confirmLabel={t.sidebar.move}
        cancelLabel={t.cancel}
        onConfirm={() => {
          const w = workspaces.find((x) => x.id === target);
          if (dialog?.kind === 'move' && w) onMove(dialog.session, w);
        }}
      />
      <ConfirmDialog
        isOpen={dialog?.kind === 'renameProject'}
        onOpenChange={open}
        title={t.sidebar.renameProject}
        body={<TextField label={t.sidebar.projectName} value={name} onChange={setName} />}
        confirmLabel={t.sidebar.save}
        cancelLabel={t.cancel}
        onConfirm={() => dialog?.kind === 'renameProject' && name.trim() && onRenameProject(dialog.project, name.trim())}
      />
      <ConfirmDialog
        isOpen={dialog?.kind === 'removeProject'}
        onOpenChange={open}
        title={dialog?.kind === 'removeProject' ? t.sidebar.removeProjectTitle(dialog.project.name) : ''}
        body={t.sidebar.removeProjectBody}
        confirmLabel={t.sidebar.removeProject}
        cancelLabel={t.cancel}
        danger
        onConfirm={() => dialog?.kind === 'removeProject' && onRemoveProject(dialog.project)}
      />
    </>
  );
}
