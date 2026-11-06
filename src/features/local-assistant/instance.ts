import { LocalAssistant } from './assistant';
import { ollamaTransport } from './transport';
import { DEFAULT_CONFIG } from './types';

/** The app's single local helper; Settings → Teamwork and the chat screen both configure it. */
export const localAssistant = new LocalAssistant(DEFAULT_CONFIG, ollamaTransport());
