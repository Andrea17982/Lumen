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
export type AccessMode = 'read-only' | 'file-edits' | 'autopilot';
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
  status: string;
  output: string;
  startedAt: number;
  endedAt: number;
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
  autoFallback: boolean;
  selfHeal: boolean;
  reviewRounds: number;
  apiEnabled: boolean;
  apiPort: number;
  apiToken: string;
  orchestra: boolean;
  closeToTray: boolean;
  openAtLogin: boolean;
  ollamaModel: string;
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
export interface ApiStatus {
  running: boolean;
  url: string;
  log: { at: number; model: string; status: string; stream: boolean; millis: number }[];
}
export interface RewindPreview {
  files: string[];
  restorable: boolean;
  laterMessages: number;
}
