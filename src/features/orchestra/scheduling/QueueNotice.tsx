import { describeWaiting, heldByTotal } from './describe';
import './scheduling.css';
import type { OrchestraQueue } from '../../../lib/types';

/** Why queued Orchestra work is not running, with the explicit way to admit more. */
export default function QueueNotice({
  queue,
  onRaise,
  disabled,
}: {
  queue: OrchestraQueue;
  /** Admit `extra` more subtasks; omitted when the caller cannot change the limit. */
  onRaise?: (extra: number) => void;
  disabled?: boolean;
}) {
  const lines = describeWaiting(queue);
  const held = heldByTotal(queue);
  if (!lines.length) return null;
  return (
    <div className="orchestra-queue" role="status">
      {lines.map((l) => (
        <small key={l}>{l}</small>
      ))}
      {onRaise && held > 0 && (
        <div className="orchestra-actions">
          <button disabled={disabled} onClick={() => onRaise(1)}>
            Allow 1 more
          </button>
          {held > 1 && (
            <button disabled={disabled} onClick={() => onRaise(held)}>
              Allow {held} more
            </button>
          )}
        </div>
      )}
    </div>
  );
}
