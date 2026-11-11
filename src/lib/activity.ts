import type { Activity } from './types';

/** Match the backend's bounded snapshots, so child tokens cannot evict questions/checkpoints. */
export function recordActivity(rows: Activity[], incoming: Activity): Activity[] {
  const next = [...rows];
  if (incoming.kind === 'execution') {
    try {
      const value = JSON.parse(incoming.text);
      const index =
        typeof value.id === 'string'
          ? next.findIndex((a) => {
              try {
                return a.kind === 'execution' && JSON.parse(a.text).id === value.id;
              } catch {
                return false;
              }
            })
          : -1;
      if (index >= 0) {
        next[index] = incoming;
        return next;
      }
    } catch {
      /* Preserve unsupported telemetry as bounded activity. */
    }
    if (next.filter((a) => a.kind === 'execution').length >= 64)
      next.splice(
        next.findIndex((a) => a.kind === 'execution'),
        1,
      );
  }
  if (
    incoming.kind === 'execution-phase' &&
    next.filter((a) => a.kind === 'execution-phase').length >= 64
  )
    next.splice(
      next.findIndex((a) => a.kind === 'execution-phase'),
      1,
    );
  if (incoming.kind === 'child') {
    try {
      const child = JSON.parse(incoming.text);
      if (typeof child.id === 'string' && child.id) {
        const index = next.findIndex((a) => {
          try {
            return a.kind === 'child' && JSON.parse(a.text).id === child.id;
          } catch {
            return false;
          }
        });
        const value =
          index < 0
            ? { id: child.id, kind: 'snapshot', text: '', title: '', status: '' }
            : JSON.parse(next[index].text);
        const text = typeof child.text === 'string' ? child.text : '';
        if (child.kind === 'snapshot') Object.assign(value, child);
        else if (child.kind === 'delta') value.text += text;
        else if (child.kind === 'message' && !value.text.endsWith(text)) value.text += '\n' + text;
        else if (child.kind === 'status') {
          if (text) value.title = text;
          if (child.status) value.status = child.status;
        }
        if (child.sessionId) value.sessionId = child.sessionId;
        if (value.text.length > 20000) {
          value.text = value.text.slice(-20000);
          value.truncated = true;
        }
        incoming = { kind: 'child', text: JSON.stringify(value) };
        if (index >= 0) {
          next[index] = incoming;
          return next;
        }
      }
    } catch {
      /* Preserve unknown protocol events as ordinary bounded activity. */
    }
  }
  next.push(incoming);
  if (next.length > 500) {
    const index = next.findIndex((a) =>
      [
        'tool',
        'file',
        'thinking',
        'child',
        'model',
        'resume',
        'tool-start',
        'tool-end',
        'execution',
      ].includes(a.kind),
    );
    next.splice(index < 0 ? 0 : index, 1);
  }
  return next;
}
