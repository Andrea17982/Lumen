import type { Provider } from './types';

export const REPO = 'https://github.com/Andrea17982/Lumen';
export const PAYPAL =
  'https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen';
export const nameOf = (p: Provider) =>
  ({
    claude: 'Claude',
    gpt: 'GPT',
    gemini: 'Gemini CLI',
    antigravity: 'Antigravity',
    ollama: 'Ollama',
  })[p];
export function errorText(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}
export function Mark({ small = false }: { small?: boolean }) {
  return (
    <span className={`lumen-mark ${small ? 'small' : ''}`} aria-hidden="true">
      <span />
      <span />
      <span />
    </span>
  );
}
