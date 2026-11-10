import type { OrchestraLimits } from '../features/orchestra/scheduling/limits';
import type { LocalAssistantConfig } from '../features/local-assistant/types';
export type Provider = 'claude' | 'gpt' | 'gemini' | 'antigravity' | 'ollama';
export type Mode = Provider | 'auto' | 'both';
export type Status = 'idle' | 'waiting' | 'running' | 'completed' | 'interrupted' | 'error';
export interface Session {
  id: string;
  title: string;
  pinned: boolean;
  status: Status;
  workspace: string;
  updatedAt: number;
  workspaceId?: string | null;
  accessMode?: AccessMode;
}
export interface Workspace {
  id: string;
  name: string;
  folder: string;
}
export type AccessMode = 'read-only' | 'file-edits' | 'autopilot' | 'ask';
export interface ModelInfo {
  id: string;
  name: string;
  description: string;
  efforts: string[];
  defaultEffort: string;
  isDefault: boolean;
}
export interface Account {
  id: string;
  name: string;
  provider: Provider;
}
export interface Activity {
  kind: string;
  text: string;
}
export interface Message {
  id: string;
  role: 'user' | 'assistant';
  text: string;
  activity: Activity[];
  /** Live output per Orchestra subtask while it runs (not persisted). */
  live?: Record<string, string>;
}
export interface OrchestraSubtask {
  id: string;
  title: string;
  detail: string;
  files: string[];
  difficulty: string;
  writes: boolean;
  dependsOn: string[];
  accountId: string;
  agent: string;
  model: string;
  effort: string;
  pinned?: boolean;
  /** pending · queued · running · done · failed · skipped · interrupted */
  status: string;
  output: string;
  startedAt: number;
  endedAt: number;
  attempts?: number;
}
export interface OrchestraState {
  stage: string;
  task: string;
  questions: { q: string; recommended: string }[];
  confirm: string;
  answers: string;
  orchestrator: string;
  orchestratorAgent: string;
  orchestratorModel: string;
  effort: string;
  summary: string;
  subtasks: OrchestraSubtask[];
  planReview: string;
  readOnly: boolean;
  steps: { name: string; status: string; detail: string }[];
  lanes: [string, string, string][];
  queue?: OrchestraQueue | null;
  /** Interrupted while subtasks ran: completed work is kept and the rest can resume. */
  resumable?: boolean;
  /** Why the last resume was refused. */
  notice?: string;
}
export type WaitReason = 'concurrency' | 'total-cap' | 'probe' | 'cooldown' | 'quota';
/** Why queued Orchestra subtasks wait, while a run is in progress. */
export interface OrchestraQueue {
  running: number;
  admitted: number;
  concurrent: number;
  total: number | null;
  waiting: { id: string; reason: WaitReason }[];
  wakeAt: number | null;
}
export interface RunOptions {
  mode: Mode;
  accountId: string | null;
  model: string | null;
  effort: string;
  accessMode: AccessMode;
  autoPolicy?: 'budget' | 'default' | 'quality';
  balanceUsage?: boolean;
  autoReview?: boolean;
  attachments?: string[];
  auditOf?: string | null;
}
export interface Plan {
  provider: Provider;
  accountId: string;
  model: string | null;
  effort: string;
  strategy: string;
  reason: string;
}
export interface UsageWindow {
  group?: string;
  usedPercent: number;
  resetsAt: number;
  windowMinutes: number;
}
export interface AccountUsage {
  accountId: string;
  windows: UsageWindow[];
  updatedAt: number;
  source: string;
}
export interface RunEvent {
  sessionId: string;
  messageId: string;
  kind: string;
  text: string;
}
export interface Diagnostics {
  claude: boolean;
  gpt: boolean;
  gemini: boolean;
  antigravity: boolean;
  ollama?: boolean;
  dataDir: string;
}
export interface OllamaStatus {
  installed: boolean;
  running: boolean;
  models: { name: string; size: string; bytes: number }[];
  selected: string | null;
}
export interface AppSettings {
  verifyChanges?: boolean;
  captureHotkey?: boolean;
  autoFallback: boolean;
  selfHeal: boolean;
  reviewRounds: number;
  orchestra: boolean;
  closeToTray: boolean;
  openAtLogin: boolean;
  ollamaModel: string;
  orchestraLimits: OrchestraLimits;
  localAssistant: LocalAssistantConfig;
  repoContext: boolean;
}
export interface Health {
  accountId: string;
  state: 'active' | 'near' | 'cooldown';
  until: number | null;
  turns: number;
}
export interface FileEntry {
  path: string;
  size: number;
  tokens: number;
  noise: boolean;
}
export interface ProjectContext {
  notes: string;
  files: FileEntry[];
  verify: string | null;
  git: boolean;
}
export interface McpServer {
  name: string;
  enabled: boolean;
  healthy: boolean;
  target: string;
}
export interface RewindPreview {
  files: string[];
  restorable: boolean;
  laterMessages: number;
}
export interface ProjectTool {
  kind: 'skill' | 'mcp';
  name: string;
  provider: Provider;
  origin: string;
  detail: string;
  trust: string;
  conflict: boolean;
}
export interface GodotDiagnostics {
  executable: string | null;
  projects: string[];
  mcpServers: string[];
  editorConnection: string;
  servers: string[];
}
export interface ProjectCheck {
  id: string;
  label: string;
  command: string;
  cwd: string;
  source: string;
  playtest: boolean;
  layers: string[];
}
export interface CheckResult {
  ok: boolean;
  exitCode: number | null;
  output: string;
  truncated: boolean;
  millis: number;
  links: { file: string; line: number }[];
}

export interface GodotConnection {
  server: string;
  started: boolean;
  serverInfo: string;
  toolCount: number;
  editorConnected: boolean;
  detail: string;
}

export interface Screenshot {
  path: string;
  label: string;
  width: number;
  height: number;
  source: string;
  captured: number;
}
export interface CaptureTarget {
  id: number;
  title: string;
  app: string;
}

export interface RepoSymbol {
  name: string;
  kind: string;
  file: string;
  line: number;
}
export interface RepoMapStats {
  files: number;
  symbols: number;
  refreshed: number;
  removed: number;
  millis: number;
  partial: boolean;
}
export interface RepoReference {
  file: string;
  /** 0 when a scene or resource uses the file rather than mentioning a line. */
  line: number;
  text: string;
}

export interface ChangeLayer {
  name: string;
  files: { path: string; status: string }[];
  checks: string[];
}
