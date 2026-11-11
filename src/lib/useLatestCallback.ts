import { useCallback, useLayoutEffect, useRef } from 'react';

/** Stable event identity for memoized children, with the latest committed chat/permissions. */
export function useLatestCallback<Args extends unknown[], Result>(
  callback: (...args: Args) => Result,
): (...args: Args) => Result {
  const current = useRef(callback);
  useLayoutEffect(() => {
    current.current = callback;
  });
  return useCallback((...args: Args) => current.current(...args), []);
}
