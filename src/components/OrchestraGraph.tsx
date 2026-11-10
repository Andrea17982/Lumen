import type { Account, OrchestraSubtask } from '../lib/types';
import { useI18n } from '../lib/i18n';
export default function OrchestraGraph({
  tasks,
  accounts,
  assign,
  onAssign,
  disabled,
}: {
  tasks: OrchestraSubtask[];
  accounts: Account[];
  assign: Record<string, string>;
  onAssign: (id: string, account: string) => void;
  disabled: boolean;
}) {
  const { t } = useI18n();
  const depth = new Map<string, number>();
  const visit = (id: string, seen: Set<string>): number => {
    if (seen.has(id)) return 0;
    const cached = depth.get(id);
    if (cached !== undefined) return cached;
    seen.add(id);
    const task = tasks.find((task) => task.id === id);
    const value = Math.min(
      12,
      Math.max(0, ...(task?.dependsOn || []).map((dep) => visit(dep, new Set(seen)) + 1)),
    );
    depth.set(id, value);
    return value;
  };
  tasks.forEach((task) => visit(task.id, new Set()));
  const ranks = new Map<number, OrchestraSubtask[]>();
  for (const task of tasks) {
    const rank = depth.get(task.id) || 0;
    ranks.set(rank, [...(ranks.get(rank) || []), task]);
  }
  const width = Math.max(1, ranks.size) * 200;
  const height = Math.max(1, ...[...ranks.values()].map((r) => r.length)) * 96;
  const position = (id: string) => {
    const rank = depth.get(id) || 0;
    return {
      x: rank * 200,
      y:
        Math.max(
          0,
          (ranks.get(rank) || []).findIndex((t) => t.id === id),
        ) * 96,
    };
  };
  return (
    <section className="orchestra-graph" aria-label={t('Task graph')}>
      <p className="panel-hint">
        {t(
          'Drag a task onto an agent to assign its implementation worktree. Keyboard users can use the agent selectors below.',
        )}
      </p>
      <div className="task-graph-scroll">
        <div style={{ width, height, position: 'relative' }}>
          <svg width={width} height={height} aria-hidden="true" className="task-edges">
            {tasks.flatMap((task) =>
              task.dependsOn
                .filter((id) => tasks.some((t) => t.id === id))
                .map((id) => {
                  const a = position(id),
                    b = position(task.id);
                  return (
                    <path
                      key={`${id}-${task.id}`}
                      d={`M${a.x + 180},${a.y + 36} C${a.x + 190},${a.y + 36} ${b.x - 10},${b.y + 36} ${b.x},${b.y + 36}`}
                    />
                  );
                }),
            )}
          </svg>
          {tasks.map((task) => {
            const p = position(task.id);
            const account = accounts.find((a) => a.id === (assign[task.id] || task.accountId));
            return (
              <div
                key={task.id}
                className="task-node"
                draggable={!disabled}
                onDragStart={(e) => {
                  e.dataTransfer.setData('application/x-lumen-task', task.id);
                  e.dataTransfer.effectAllowed = 'move';
                }}
                style={{ left: p.x, top: p.y }}
              >
                <strong>{task.title}</strong>
                <small>{account?.name || task.agent}</small>
                <small>{task.writes ? t('Implementation worktree') : t('Read-only task')}</small>
              </div>
            );
          })}
        </div>
      </div>
      <div className="task-drop-targets">
        {accounts.map((account) => (
          <div
            key={account.id}
            className="task-drop-target"
            aria-label={`${t('Assign to')} ${account.name}`}
            onDragOver={(e) => {
              if (!disabled) e.preventDefault();
            }}
            onDrop={(e) => {
              e.preventDefault();
              const id = e.dataTransfer.getData('application/x-lumen-task');
              const task = tasks.find((t) => t.id === id);
              if (!disabled && task && !(task.writes && account.provider === 'ollama'))
                onAssign(id, account.id);
            }}
          >
            {account.name}
          </div>
        ))}
      </div>
    </section>
  );
}
