/**
 * Like setInterval, but skipped while the window is hidden (minimized or in the notification
 * area). When the window shows again the callback runs at once, so data is fresh on return.
 */
export function visibleInterval(fn: () => void, ms: number): () => void {
  const tick = () => {
    if (!document.hidden) fn();
  };
  const timer = window.setInterval(tick, ms);
  document.addEventListener('visibilitychange', tick);
  return () => {
    window.clearInterval(timer);
    document.removeEventListener('visibilitychange', tick);
  };
}
