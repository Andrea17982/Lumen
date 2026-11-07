import { Circle, X } from 'lucide-react';
import {
  formatResetCountdown,
  usageStatus,
  usageStatusLabel,
  usageWindowLabel,
} from '../lib/usage';
import { IconButton, nameOf } from '../lib/util';
import type { Account, AccountUsage, UsageWindow } from '../lib/types';

function UsageRow({ label, window, now }: { label: string; window?: UsageWindow; now: number }) {
  const usedPercent = window && window.resetsAt <= now ? 0 : window?.usedPercent;
  return (
    <div className="usage-meter">
      <span>{label}</span>
      {window ? (
        <>
          <div className="usage-track" aria-hidden="true">
            <i style={{ width: `${usedPercent}%` }} />
          </div>
          <strong>{Math.round(usedPercent ?? 0)}%</strong>
          <small>Resets in {formatResetCountdown(window.resetsAt, now)}</small>
        </>
      ) : (
        <small className="usage-unavailable">Unavailable</small>
      )}
    </div>
  );
}

export default function UsagePopover({
  accounts,
  usage,
  errors,
  now,
  position,
  onRefresh,
  onClose,
}: {
  accounts: Account[];
  usage: AccountUsage[];
  errors: Record<string, string>;
  now: number;
  position: { left: number; bottom: number };
  onRefresh: () => void;
  onClose: () => void;
}) {
  return (
    <>
      <button className="usage-scrim" aria-label="Close usage" onClick={onClose} />
      <section
        className="usage-popover"
        role="dialog"
        aria-label="Provider usage"
        style={{ left: position.left, bottom: position.bottom }}
      >
        <header>
          <strong>Usage</strong>
          <IconButton className="icon-button" label="Refresh usage" onClick={onRefresh}>
            <Circle size={14} />
          </IconButton>
          <IconButton className="icon-button" label="Close usage" onClick={onClose}>
            <X size={16} />
          </IconButton>
        </header>
        {accounts.every((account) => account.provider === 'ollama') && (
          <p>No cloud providers configured.</p>
        )}
        {accounts
          .filter((account) => account.provider !== 'ollama')
          .map((account) => {
            const row = usage.find((item) => item.accountId === account.id);
            const groups = [...new Set(row?.windows.map((window) => window.group ?? '') ?? [])];
            const label = usageStatusLabel(row, now);
            return (
              <div className="usage-provider" key={account.id}>
                <strong>
                  {nameOf(account.provider)} <small>{account.name}</small>
                </strong>
                {groups.length === 0 && <small className="usage-unavailable">Unavailable</small>}
                {groups.map((group) => {
                  const windows = row?.windows.filter((window) => (window.group ?? '') === group);
                  return (
                    <div key={group}>
                      {group && <small>{group}</small>}
                      {windows?.map((window, index) => (
                        <UsageRow
                          key={`${window.windowMinutes}-${index}`}
                          label={usageWindowLabel(window.windowMinutes)}
                          window={window}
                          now={now}
                        />
                      ))}
                    </div>
                  );
                })}
                <small
                  className={`usage-state ${label.toLowerCase().replace(' ', '-')}`}
                  title={usageStatus(row, now)}
                  aria-label={usageStatus(row, now)}
                >
                  <span aria-hidden="true" />
                  {label}
                </small>
                {errors[account.id] && (
                  <small className="usage-updated" role="status">
                    Refresh failed: {errors[account.id]}. Showing the last available snapshot.
                  </small>
                )}
                {account.provider === 'claude' && (
                  <small className="usage-updated">
                    Updated when Claude responds in Lumen; use /usage in Claude Code for an
                    independent check.
                  </small>
                )}
              </div>
            );
          })}
        <small className="usage-updated">
          Codex and Antigravity refresh is attempted every 30 seconds while the app is open. Claude
          updates when it responds in Lumen; reset countdowns run locally.
        </small>
      </section>
    </>
  );
}
