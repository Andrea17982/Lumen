import s from './Logo.module.css';

export function Mark({ size = 24, label = 'Lumen' }: { size?: number; label?: string }) {
  return (
    <img className={s.mark} src={`${import.meta.env.BASE_URL}lumen.png`} width={size} height={size} alt={label} />
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
