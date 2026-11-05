export type Provider = 'claude' | 'gpt';
export type Mode = Provider | 'auto';
export type Status = 'idle' | 'waiting' | 'running' | 'completed' | 'interrupted' | 'error';
export interface Session {
  id: string;
  title: string;
  pinned: boolean;
  status: Status;
  workspace: string;
  updatedAt: number;
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
}
export interface RunOptions {
  mode: Mode;
  accountId: string | null;
  model: string | null;
  effort: string;
  writeAccess: boolean;
}
export interface Plan {
  provider: Provider;
  accountId: string;
  model: string | null;
  effort: string;
  strategy: string;
  reason: string;
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
  dataDir: string;
}
