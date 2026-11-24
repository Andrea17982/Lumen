import { createContext, useCallback, useContext, useRef, useState } from 'react';
import { Button, ConfirmDialog } from '../../design/components';
import { useT } from '../../i18n';
import type { RewindPreview } from '../../protocol/types';
import s from './Conversation.module.css';

/** Rewind is offered on the user's own messages while no turn runs in the conversation. */
export interface RewindHost {
  enabled: boolean;
  start(messageId: string): void;
}
export const RewindContext = createContext<RewindHost>({ enabled: false, start: () => {} });

export function RewindButton({ messageId }: { messageId: string }) {
  const t = useT();
  const host = useContext(RewindContext);
  if (!host.enabled) return null;
  return (
    <Button variant="plain" onPress={() => host.start(messageId)}>
      {t.recovery.rewind}
    </Button>
  );
}

/**
 * Asks the daemon what a rewind would undo, shows it, and rewinds only on confirmation. The
 * daemon refuses when another conversation changed the project since; that reason is shown.
 */
export function useRewind(sessionId: string, call: <T>(command: string, args: Record<string, unknown>) => Promise<T>, onDone: (prompt: string) => void) {
  const t = useT();
  const [target, setTarget] = useState<{ messageId: string; preview: RewindPreview } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const latest = useRef({ call, onDone });
  latest.current = { call, onDone };

  // Stable, so the buttons on every user message do not re-render while a reply streams.
  const start = useCallback(
    (messageId: string) => {
      setError(null);
      latest.current.call<RewindPreview>('rewind_preview', { session: sessionId, message: messageId }).then(
        (preview) => setTarget({ messageId, preview }),
        (e) => setError(String(e)),
      );
    },
    [sessionId],
  );

  const dialog = (
    <>
      {error && (
        <p className={s.rewindError} role="alert">
          {error}
        </p>
      )}
      <ConfirmDialog
        isOpen={!!target}
        onOpenChange={(open) => !open && setTarget(null)}
        title={t.recovery.rewindTitle}
        danger
        body={
          target && (
            <div className={s.rewindBody}>
              <p>{t.recovery.rewindBody(target.preview.laterMessages)}</p>
              {target.preview.restorable && target.preview.files.length > 0 ? (
                <>
                  <p>{t.recovery.rewindFiles(target.preview.files.length)}</p>
                  <ul>
                    {target.preview.files.map((f) => (
                      <li key={f}>
                        <code>{f}</code>
                      </li>
                    ))}
                  </ul>
                </>
              ) : (
                <p>{t.recovery.rewindNoFiles}</p>
              )}
            </div>
          )
        }
        confirmLabel={t.recovery.rewindConfirm}
        cancelLabel={t.cancel}
        onConfirm={() => {
          const messageId = target?.messageId;
          setTarget(null);
          if (!messageId) return;
          latest.current.call<string>('rewind', { session: sessionId, message: messageId }).then(latest.current.onDone, (e) => setError(String(e)));
        }}
      />
    </>
  );
  return { start, dialog };
}
