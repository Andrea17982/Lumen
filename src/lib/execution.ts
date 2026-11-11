import type { Message, Session } from './types';
import { orchestraOf } from './messages';

export interface ExecutionTrace {
  version: 1;
  id: string;
  provider: string;
  phase: string;
  startedAt: number;
  lastEventAt: number | null;
  processStartedAt: number | null;
  firstProviderEventAt: number | null;
  firstTextAt: number | null;
  endedAt: number | null;
  events: number;
  toolEvents: number;
  pendingTools: number;
  pendingQuestions: number;
  lastKind: string;
  timeline: [string, number][];
}
export function jsonValue(text: string): Record<string, unknown> | null {
  try {
    const value = JSON.parse(text);
    return value && typeof value === 'object' && !Array.isArray(value) ? value : null;
  } catch {
    return null;
  }
}
export function executionState(
  messages: Message[],
  session: Session | undefined,
  approvalPending: boolean,
) {
  const userIndex = messages.map((m) => m.role).lastIndexOf('user');
  const turn = messages.slice(Math.max(userIndex, 0));
  const activity = turn.flatMap((m) => m.activity);
  const traces = new Map<string, ExecutionTrace>();
  const phases: { phase: string; at: number }[] = [];
  let marker: { phase: string; at: number } | undefined;
  for (const item of activity) {
    if (!['execution', 'execution-phase'].includes(item.kind)) continue;
    const value = jsonValue(item.text);
    if (
      item.kind === 'execution' &&
      value?.version === 1 &&
      typeof value.id === 'string' &&
      typeof value.phase === 'string' &&
      typeof value.startedAt === 'number' &&
      Array.isArray(value.timeline) &&
      value.timeline.length <= 32 &&
      value.timeline.every(
        (entry: unknown) =>
          Array.isArray(entry) && typeof entry[0] === 'string' && Number.isFinite(entry[1]),
      ) &&
      typeof value.provider === 'string' &&
      ['startedAt', 'events', 'toolEvents', 'pendingTools', 'pendingQuestions'].every(
        (key) =>
          typeof value[key] === 'number' &&
          Number.isFinite(value[key]) &&
          (value[key] as number) >= 0,
      ) &&
      ['lastEventAt', 'processStartedAt', 'firstProviderEventAt', 'firstTextAt', 'endedAt'].every(
        (key) =>
          value[key] === null || (typeof value[key] === 'number' && Number.isFinite(value[key])),
      ) &&
      typeof value.lastKind === 'string'
    ) {
      traces.set(value.id, value as unknown as ExecutionTrace);
    }
    if (
      item.kind === 'execution-phase' &&
      typeof value?.phase === 'string' &&
      typeof value.at === 'number' &&
      (!marker || value.at >= marker.at)
    ) {
      marker = { phase: value.phase, at: value.at };
      phases.push(marker);
    }
  }
  const stages = [...traces.values()].sort((a, b) => a.startedAt - b.startedAt);
  const activeStages = stages.filter((s) => s.endedAt === null);
  const last = activeStages.at(-1) ?? stages.at(-1);
  const lastAt = last
    ? Math.max(
        last.lastEventAt ?? last.startedAt,
        last.timeline.at(-1)?.[1] ?? 0,
        last.endedAt ?? 0,
      )
    : 0;
  const running = session?.status === 'running' || session?.status === 'waiting';
  const questionResults = new Set(
    activity.filter((a) => a.kind === 'interaction-result').map((a) => jsonValue(a.text)?.id),
  );
  const question =
    running &&
    activity.some(
      (a) =>
        a.kind === 'interaction' &&
        typeof jsonValue(a.text)?.id === 'string' &&
        !questionResults.has(jsonValue(a.text)?.id),
    );
  const orchestra = turn
    .map(orchestraOf)
    .filter((s) => s !== null)
    .at(-1);
  const phaseMarker = running && marker?.phase.startsWith('turn-') ? undefined : marker;
  let phase = phaseMarker && phaseMarker.at >= lastAt ? phaseMarker.phase : last?.phase;
  if (running && last?.endedAt)
    phase = phaseMarker && phaseMarker.at > lastAt ? phaseMarker.phase : 'waiting-next-stage';
  if (running && activeStages.length > 1) phase = 'parallel';
  if (session?.status === 'completed') phase = 'turn-returned';
  if (orchestra?.stage === 'questions') phase = 'question';
  if (orchestra?.stage === 'plan') phase = 'plan-approval';
  if (question) phase = 'question';
  if (approvalPending && running) phase = 'approval';
  if (session?.status === 'interrupted') phase = 'turn-interrupted';
  if (session?.status === 'error') phase = 'turn-error';
  if (!phase && running) phase = 'waiting-start';
  const checks = new Map<string, string>();
  const childStates = new Map<string, string>();
  for (const item of activity) {
    if (!['validation', 'child'].includes(item.kind)) continue;
    const value = jsonValue(item.text);
    if (
      item.kind === 'validation' &&
      typeof value?.label === 'string' &&
      typeof value.status === 'string'
    )
      checks.set(`${String(value.workspace ?? '')}:${value.label}`, value.status);
    if (item.kind === 'child' && typeof value?.id === 'string' && typeof value.status === 'string')
      childStates.set(value.id, value.status);
  }
  const childActive = [...childStates.values()].filter((s) =>
    ['running', 'inProgress', 'in_progress', 'working'].includes(s),
  ).length;
  const workers = orchestra?.subtasks.filter((s) => s.status === 'running').length ?? 0;
  // Output-only child streams do not establish an active autonomous session.
  const terminalAt = Math.max(
    0,
    ...phases.filter((p) => p.phase.startsWith('turn-')).map((p) => p.at),
  );
  const durationIncomplete = !running && activeStages.length > 0 && terminalAt === 0;
  return {
    phase,
    running,
    stages,
    last,
    marker,
    phases,
    durationIncomplete,
    startedAt:
      stages.length || phases.length
        ? Math.min(...stages.map((s) => s.startedAt), ...phases.map((s) => s.at))
        : undefined,
    endedAt: running
      ? undefined
      : Math.max(
          ...stages.map((s) => s.endedAt ?? s.lastEventAt ?? s.startedAt),
          ...phases.filter((p) => p.phase.startsWith('turn-')).map((p) => p.at),
        ) || undefined,
    lastEventAt: last?.lastEventAt ?? last?.processStartedAt ?? last?.startedAt,
    silent:
      running &&
      !question &&
      !approvalPending &&
      !!last &&
      !last.endedAt &&
      [
        'starting-provider',
        'waiting-provider',
        'reasoning',
        'response',
        'tool',
        'tool-activity',
      ].includes(last.phase),
    agents: running ? workers || childActive : 0,
    checks: [...checks.values()],
  };
}

export const phaseLabel = (phase?: string): string =>
  ({
    catalog: 'Loading model catalog',
    capacity: 'Waiting for an execution slot',
    'starting-provider': 'Starting provider',
    'waiting-provider': 'Waiting for provider',
    response: 'Receiving response',
    reasoning: 'Reasoning received',
    tool: 'Tool operation open',
    'tool-activity': 'Tool activity received',
    question: 'Waiting for your answer',
    approval: 'Waiting for permission',
    'plan-approval': 'Waiting for plan approval',
    workspace: 'Waiting for project access',
    context: 'Preparing context',
    verification: 'Running checks',
    returned: 'Provider turn returned',
    cancelled: 'Provider call cancelled',
    error: 'Provider call failed',
    'turn-returned': 'Turn ended',
    'turn-error': 'Turn failed',
    'turn-interrupted': 'Turn interrupted',
    'waiting-start': 'Waiting to start',
    'waiting-next-stage': 'Between execution stages',
    parallel: 'Providers active',
    'preparing-request': 'Preparing request',
    'delivery-uncertain': 'Delivery not confirmed',
  })[phase ?? ''] ?? 'Execution status';

export function secondsBetween(from: number, to: number) {
  return Math.max(0, Math.floor((to - from) / 1000));
}
