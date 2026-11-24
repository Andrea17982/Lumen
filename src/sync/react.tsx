import { createContext, useContext, useEffect, useSyncExternalStore, type ReactNode } from 'react';
import type { Sync, SyncState } from './sync';

const Context = createContext<Sync | null>(null);

export function SyncProvider({ sync, children }: { sync: Sync; children: ReactNode }) {
  useEffect(() => {
    void sync.start();
    return () => sync.stop();
  }, [sync]);
  return <Context value={sync}>{children}</Context>;
}

export function useSyncClient(): Sync {
  const sync = useContext(Context);
  if (!sync) throw new Error('useSyncClient needs a SyncProvider');
  return sync;
}

/** Re-renders only when the selected slice changes by reference. */
export function useSyncState<T>(select: (state: SyncState) => T): T {
  const sync = useSyncClient();
  return useSyncExternalStore(sync.subscribe, () => select(sync.getState()));
}
