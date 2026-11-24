import type { Connection, Frame } from '../protocol/types';

/**
 * The only way the client reaches the daemon. The desktop shell implements it over Tauri; tests
 * use a fixture or the local test bridge. Errors are rejected as the daemon's message string.
 */
export interface Transport {
  call(command: string, args?: Record<string, unknown>): Promise<unknown>;
  /** Frames arrive in daemon order. Resolves once listening, so read `connection()` after it. */
  subscribe(listener: (frame: Frame) => void): Promise<() => void>;
  connection(): Promise<Connection>;
  /** The system folder picker; null when the user cancels or the window has none. */
  pickFolder(): Promise<string | null>;
}

export function errorText(error: unknown): string {
  return typeof error === 'string' ? error : error instanceof Error ? error.message : String(error);
}
