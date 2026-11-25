// Line icons drawn for Ink: 24 px grid, 1.7 px stroke, round caps. Decorative unless labelled.
const paths = {
  plus: 'M12 5v14M5 12h14',
  search: 'M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4-4',
  up: 'M12 19V5M6 11l6-6 6 6',
  chevron: 'M9 6l6 6-6 6',
  chevronDown: 'M6 9l6 6 6-6',
  terminal: 'M5 7l5 5-5 5M12 17h7',
  file: 'M7 3h7l5 5v13H7zM14 3v5h5',
  check: 'M5 12l5 5 9-10',
  shield: 'M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z',
  sidebar: 'M4 5h16v14H4zM10 5v14',
  folder: 'M3 7h7l2 2h9v10H3z',
  close: 'M6 6l12 12M18 6L6 18',
  more: 'M5 12h.01M12 12h.01M19 12h.01',
  stop: 'M7 7h10v10H7z',
  copy: 'M9 9h10v10H9zM5 15V5h10',
  alert: 'M12 4l9 16H3zM12 10v4M12 17h.01',
  info: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v5M12 8h.01',
  paperclip: 'M20 11l-8.5 8.5a5 5 0 0 1-7-7L13 4a3.3 3.3 0 0 1 4.7 4.7l-8.5 8.5a1.7 1.7 0 0 1-2.4-2.4L14 7',
  repeat: 'M17 2l3 3-3 3M4 11V9a4 4 0 0 1 4-4h12M7 22l-3-3 3-3M20 13v2a4 4 0 0 1-4 4H4',
  gear: 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19 12l2-1-1-3-2 .2-1.3-1.5.4-2-3-1-1 1.8h-2L10 3.7l-3 1 .4 2L6 8.2 4 8l-1 3 2 1-.1 2-1.9 1 1 3 2-.2 1.3 1.5-.4 2 3 1 1-1.8h2l1 1.8 3-1-.4-2 1.4-1.5 2 .2 1-3-2-1z',
};
export type IconName = keyof typeof paths;
export const iconNames = Object.keys(paths) as IconName[];

export function Icon({ name, size = 16, label }: { name: IconName; size?: number; label?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <path d={paths[name]} />
    </svg>
  );
}
