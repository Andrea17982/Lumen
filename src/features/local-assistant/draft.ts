import type { LocalAssistant } from './assistant';
import type { AssistantResult } from './types';

export type DraftStatus = 'idle' | 'generating' | 'ready' | 'error';

export interface DraftState {
  status: DraftStatus;
  text: string;
  error: string;
  /** True while the text is exactly what the model produced. */
  generated: boolean;
}

/**
 * An editable draft. Generation only starts from `generate()` (a user action),
 * never overwrites text the user typed while it was running, and `take()` is the
 * only way text leaves, so nothing is committed or sent behind the user's back.
 */
export class Draft {
  private state: DraftState = { status: 'idle', text: '', error: '', generated: false };
  private listeners = new Set<() => void>();
  private ctl: AbortController | null = null;
  private edited = false;
  private readonly produce: (signal: AbortSignal) => Promise<AssistantResult>;

  constructor(produce: (signal: AbortSignal) => Promise<AssistantResult>) {
    this.produce = produce;
  }

  static commit(a: LocalAssistant, diff: () => string) {
    return new Draft((s) => a.commitMessage(diff(), s));
  }
  static handoff(a: LocalAssistant, conversation: () => string) {
    return new Draft((s) => a.handoffSummary(conversation(), s));
  }

  getState = () => this.state;
  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };
  private set(patch: Partial<DraftState>) {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((l) => l());
  }

  edit(text: string) {
    this.edited = true;
    this.set({ text, generated: false, error: '' });
  }

  async generate() {
    if (this.state.status === 'generating') return;
    const ctl = new AbortController();
    this.ctl = ctl;
    this.edited = false;
    this.set({ status: 'generating', error: '' });
    const r = await this.produce(ctl.signal);
    if (this.ctl !== ctl) return;
    this.ctl = null;
    if (r.ok) {
      if (this.edited) this.set({ status: 'ready' });
      else this.set({ status: 'ready', text: r.text, generated: true });
    } else if (r.reason === 'cancelled') {
      this.set({ status: 'idle' });
    } else {
      this.set({ status: 'error', error: r.message });
    }
  }

  cancel() {
    this.ctl?.abort();
  }

  /** The text to use, or null when empty. Does not clear it. */
  take(): string | null {
    const t = this.state.text.trim();
    return t || null;
  }

  reset() {
    this.ctl?.abort();
    this.ctl = null;
    this.edited = false;
    this.set({ status: 'idle', text: '', error: '', generated: false });
  }
}
