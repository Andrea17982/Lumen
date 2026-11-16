import { formatResetCountdown, usageWindowLabel } from '../lib/usage';
import { nameOf } from '../lib/util';
import type { Account, AccountUsage } from '../lib/types';

export default function UsagePopover({
  accounts,
  usage,
  now,
  position,
  onClose,
}: {
  accounts: Account[];
  usage: AccountUsage[];
  now: number;
  position: { left: number; bottom: number };
  onClose: () => void;
}) {
  const rows = accounts.flatMap((account) => {
    const windows =
      usage
        .find((u) => u.accountId === account.id)
        ?.windows.filter(
          (w) =>
            [300, 10080].includes(w.windowMinutes) &&
            (account.provider !== 'antigravity' || /gemini|google/i.test(w.group ?? '')),
        ) ?? [];
    return windows.length ? [{ account, windows }] : [];
  });
  return (
    <>
      <button className="usage-scrim" aria-label="Close usage" onClick={onClose} />
      <section
        className="usage-popover"
        role="dialog"
        aria-label="Provider usage"
        style={position}
        onKeyDown={(e) => {
          if (e.key === 'Escape') onClose();
        }}
      >
        <header>
          <strong>Usage</strong>
        </header>
        {rows.map(({ account, windows }) => (
          <div className="usage-provider" key={account.id}>
            <strong>
              {nameOf(account.provider)} <small>{account.name}</small>
            </strong>
            {windows.map((window, i) => {
              const used = window.resetsAt <= now ? 0 : window.usedPercent;
              return (
                <div className="usage-meter" key={`${window.group}-${window.windowMinutes}-${i}`}>
                  <span>
                    {window.group ? `${window.group} · ` : ''}
                    {usageWindowLabel(window.windowMinutes)}
                  </span>
                  <div
                    className="usage-track"
                    role="progressbar"
                    aria-label={`${nameOf(account.provider)} ${window.group ?? ''} ${usageWindowLabel(window.windowMinutes)}`}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={used}
                  >
                    <i style={{ width: `${used}%` }} />
                  </div>
                  <strong>{Math.round(used)}%</strong>
                  <small>Resets in {formatResetCountdown(window.resetsAt, now)}</small>
                </div>
              );
            })}
          </div>
        ))}
        {!rows.length && <p>No usage limits reported yet.</p>}
      </section>
    </>
  );
}
