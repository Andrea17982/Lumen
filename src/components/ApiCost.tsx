import type { Activity } from '../lib/types';
export default function ApiCost({ activity }: { activity: Activity[] }) {
  const requests = new Map<string, number>();
  for (const row of activity.filter((a) => a.kind === 'api-cost')) {
    try {
      const v = JSON.parse(row.text);
      if (typeof v.requestId === 'string' && Number.isFinite(v.estimatedUsd) && v.estimatedUsd >= 0)
        requests.set(v.requestId, v.estimatedUsd);
    } catch {}
  }
  if (!requests.size) return null;
  const total = [...requests.values()].reduce((a, b) => a + b, 0);
  return (
    <small
      className="assignment-note"
      title="Based on your confirmed model rates. Uncertain requests retain their reserved estimate; this is not the provider’s invoice."
    >
      API estimate · ${total.toFixed(6)}
    </small>
  );
}
