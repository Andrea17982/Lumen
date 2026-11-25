import s from './Logo.module.css';

/**
 * Lumen's mark: a single stem and a point of light. It takes its colours from the active style,
 * so it stays the same identity in Ink and in Satin. Pass `label=""` where it is decorative.
 */
export function Mark({ size = 24, label = 'Lumen' }: { size?: number; label?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" role={label ? 'img' : undefined} aria-label={label || undefined} aria-hidden={label ? undefined : true}>
      <rect x="20" y="14" width="11" height="40" rx="5.5" fill="var(--text-primary)" />
      <circle cx="43.5" cy="17.5" r="6.5" fill="var(--accent)" />
    </svg>
  );
}

/** The mark as the control that hides or shows the sidebar. */
export function LogoButton({ label, onPress }: { label: string; onPress: () => void }) {
  return (
    <button type="button" className={s.button} aria-label={label} title={label} onClick={onPress}>
      <Mark size={24} label="" />
    </button>
  );
}
