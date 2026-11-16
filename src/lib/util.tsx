import type { Provider } from './types';
import type { ButtonHTMLAttributes, ReactNode } from 'react';

export const REPO = 'https://github.com/Andrea17982/Lumen';
export const PAYPAL =
  'https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen';
export const nameOf = (p: Provider) =>
  ({
    claude: 'Claude',
    gpt: 'GPT',
    antigravity: 'Antigravity',
    ollama: 'Ollama',
    opencode: 'OpenCode',
    cursor: 'Cursor',
    openaiapi: 'OpenAI API',
    anthropicapi: 'Anthropic API',
    googleapi: 'Google API',
  })[p];
export function errorText(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}
export function Mark({ small = false }: { small?: boolean }) {
  return (
    <svg
      className={`lumen-mark ${small ? 'small' : ''}`}
      viewBox="224 65 1333 766"
      aria-hidden="true"
    >
      <image href="/lumen.png" width="1774" height="887" />
    </svg>
  );
}

export function IconButton({
  label,
  children,
  className = 'icon-button',
  ...props
}: Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'aria-label' | 'children'> & {
  label: string;
  children: ReactNode;
}) {
  return (
    <button
      {...props}
      type={props.type ?? 'button'}
      className={className}
      aria-label={label}
      title={props.title ?? label}
    >
      {children}
    </button>
  );
}
