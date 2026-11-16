import type { Message } from '../lib/types';

export default function QuotaResume({
  message,
  running,
  busy,
  command,
}: {
  message: Message;
  running: boolean;
  busy: boolean;
  command: (name: string, args: Record<string, unknown>) => Promise<void>;
}) {
  let reset: { at: number } | undefined;
  let scheduled: { state: string } | undefined;
  try {
    const row = message.activity.filter((a) => a.kind === 'quota-reset').at(-1);
    if (row) reset = JSON.parse(row.text);
    const job = message.activity.filter((a) => a.kind === 'quota-scheduled').at(-1);
    if (job) scheduled = JSON.parse(job.text);
  } catch {
    return null;
  }
  if (!reset || !Number.isFinite(reset.at) || !message.activity.some((a) => a.kind === 'native'))
    return null;
  const waiting = running && scheduled?.state === 'waiting';
  if (running && !waiting) return null;
  return (
    <div className="quota-resume assignment-note">
      {waiting ? (
        <>
          <span>Automatic resume scheduled for {new Date(reset.at).toLocaleString()}.</span>
          <button
            className="text-button"
            disabled={busy}
            onClick={() => void command('cancel_run', {})}
          >
            Cancel scheduled resume
          </button>
        </>
      ) : (
        <button
          className="text-button"
          disabled={busy}
          onClick={() => void command('resume_at_reset', { message: message.id })}
        >
          Resume at estimated quota reset · {new Date(reset.at).toLocaleString()}
        </button>
      )}
      <small>
        Keep Lumen running or in the notification area. Resumes once with the same account, model
        and permissions.
      </small>
    </div>
  );
}
