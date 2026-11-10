import type { Activity } from '../lib/types';
import { useI18n } from '../lib/i18n';
interface Child {
  id: string;
  kind: string;
  text: string;
  status?: string;
  taskId?: string;
  title?: string;
}
export function childrenOf(activity: Activity[]) {
  const children = new Map<string, { id: string; title: string; text: string; status: string }>();
  for (const a of activity) {
    if (a.kind !== 'child') continue;
    try {
      const child = JSON.parse(a.text) as Child;
      if (!child.id) continue;
      const current = children.get(child.id) || { id: child.id, title: '', text: '', status: '' };
      if (child.kind === 'snapshot') {
        current.text = child.text;
        current.title = child.title || '';
        current.status = child.status || '';
      }
      if (child.kind === 'delta') current.text = (current.text + child.text).slice(-60000);
      else if (child.kind === 'message' && !current.text.endsWith(child.text))
        current.text = (current.text + '\n' + child.text).slice(-60000);
      else if (child.kind === 'status') {
        current.title = child.text || current.title;
        current.status = child.status || current.status;
      }
      children.set(child.id, current);
    } catch {
      /* Older or unsupported CLI event. */
    }
  }
  return [...children.values()];
}
export default function ChildSessions({ activity }: { activity: Activity[] }) {
  const { t } = useI18n();
  const children = childrenOf(activity);
  if (!children.length) return null;
  return (
    <section className="native-children" aria-label={t('Agent sessions')}>
      {children.map((child, i) => (
        <details key={child.id}>
          <summary>
            <span>{child.title || `${t('Agent')} ${i + 1}`}</span>
            {child.status && <small>{child.status}</small>}
          </summary>
          {child.text ? <pre>{child.text}</pre> : <p>{t('Waiting for agent output…')}</p>}
        </details>
      ))}
    </section>
  );
}
