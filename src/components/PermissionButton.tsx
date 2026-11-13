import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ChevronDown, CircleCheck, Pencil, Shield, ShieldCheck } from 'lucide-react';
import type { AccessMode } from '../lib/types';
import { useI18n } from '../lib/i18n';

export const ACCESS_ORDER: AccessMode[] = ['read-only', 'file-edits', 'ask', 'autopilot'];
export const ACCESS_LABEL: Record<AccessMode, string> = {
  'read-only': 'Read only',
  'file-edits': 'File edits',
  ask: 'Approve each action',
  autopilot: 'Autopilot',
};
const descriptions: Record<AccessMode, string> = {
  'read-only': 'Inspect and answer without changing files.',
  'file-edits': 'Allow project file changes within the provider’s controls.',
  ask: 'Review tool actions before they run. Requires a provider that supports approvals.',
  autopilot: 'Allow available tools without approval prompts, with your OS account’s access.',
};
const icons = {
  'read-only': Shield,
  'file-edits': Pencil,
  ask: CircleCheck,
  autopilot: ShieldCheck,
};

/** Opening the explanation never changes access; selecting a mode does. */
export default function PermissionButton({
  accessMode,
  onCycle,
}: {
  accessMode: AccessMode;
  onCycle: (next: AccessMode) => void;
}) {
  const { t } = useI18n();
  const [position, setPosition] = useState<{ left: number; bottom: number } | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const close = () => {
    setPosition(null);
    trigger.current?.focus();
  };
  useEffect(() => {
    if (!position) return;
    menu.current?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
    const outside = (e: Event) => {
      if (!menu.current?.contains(e.target as Node) && !trigger.current?.contains(e.target as Node))
        setPosition(null);
    };
    const resize = () => setPosition(null);
    document.addEventListener('pointerdown', outside);
    document.addEventListener('focusin', outside);
    window.addEventListener('resize', resize);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('focusin', outside);
      window.removeEventListener('resize', resize);
    };
  }, [position]);
  const Icon = icons[accessMode];
  return (
    <>
      <button
        ref={trigger}
        type="button"
        data-tour="permissions"
        className={`icon-button access-control permission-picker ${accessMode === 'autopilot' ? 'autopilot' : ''}`}
        aria-label={`${t('Permissions')}: ${t(ACCESS_LABEL[accessMode])}`}
        aria-haspopup="menu"
        aria-expanded={!!position}
        aria-controls={position ? 'permission-menu' : undefined}
        title={t('Choose what the agent is allowed to do')}
        onClick={() => {
          if (position) return close();
          const rect = trigger.current!.getBoundingClientRect();
          setPosition({
            left: Math.max(12, Math.min(rect.right - 340, innerWidth - 352)),
            bottom: innerHeight - rect.top + 10,
          });
        }}
      >
        <Icon size={15} aria-hidden="true" />
        <span>
          <span className="permission-prefix">{t('Permissions')} · </span>
          {t(ACCESS_LABEL[accessMode])}
        </span>
        <ChevronDown size={12} aria-hidden="true" />
      </button>
      {position &&
        createPortal(
          <div
            ref={menu}
            id="permission-menu"
            className="permission-menu"
            role="menu"
            aria-label={t('Choose agent permissions')}
            style={{
              left: position.left,
              bottom: position.bottom,
              maxHeight: Math.max(80, innerHeight - position.bottom - 12),
            }}
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                e.preventDefault();
                e.stopPropagation();
                close();
              }
              const options = [...menu.current!.querySelectorAll<HTMLButtonElement>('button')];
              const current = options.indexOf(document.activeElement as HTMLButtonElement);
              const next =
                e.key === 'Home'
                  ? 0
                  : e.key === 'End'
                    ? options.length - 1
                    : e.key === 'ArrowDown'
                      ? (current + 1) % options.length
                      : e.key === 'ArrowUp'
                        ? (current - 1 + options.length) % options.length
                        : -1;
              if (next >= 0) {
                e.preventDefault();
                options[next].focus();
              }
            }}
          >
            <p>{t('Choose what the agent is allowed to do')}</p>
            {ACCESS_ORDER.map((mode) => {
              const ModeIcon = icons[mode];
              return (
                <button
                  type="button"
                  key={mode}
                  role="menuitemradio"
                  aria-checked={mode === accessMode}
                  onClick={() => {
                    close();
                    if (mode !== accessMode) onCycle(mode);
                  }}
                >
                  <ModeIcon size={16} aria-hidden="true" />
                  <span>
                    <strong>{t(ACCESS_LABEL[mode])}</strong>
                    <small>{t(descriptions[mode])}</small>
                  </span>
                  {mode === accessMode && <CircleCheck size={14} aria-hidden="true" />}
                </button>
              );
            })}
          </div>,
          document.body,
        )}
    </>
  );
}
