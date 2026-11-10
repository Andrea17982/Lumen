import { ArrowUp, ChevronDown, ChevronUp, Paperclip, Square, Workflow } from 'lucide-react';
import ModeControl from './ModeControl';
import PermissionButton from './PermissionButton';
import { useI18n } from '../lib/i18n';
import type { AccessMode, Mode, Provider, Workspace } from '../lib/types';

const compactTokens = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n));

/** The row under the message box: mode, project, permissions, attachments and send/stop. */
export function ComposerToolbar(p: {
  mode: Mode;
  bothAvailable: boolean;
  providers?: Provider[];
  onMode: (mode: Mode) => void;
  /** Auto mode with Orchestra turned on. */
  orchestra: boolean;
  onOrchestraOff: () => void;
  workspaces: Workspace[];
  workspaceId: string;
  workspaceName?: string;
  workspaceLocked: boolean;
  onWorkspace: (id: string) => void;
  advanced: boolean;
  onAdvanced: () => void;
  accessMode: AccessMode;
  onAccess: (mode: AccessMode) => void;
  attached: number;
  onAttach: () => void;
  running: boolean;
  canSend: boolean;
  /** Sends, or stops the running reply. */
  onSend: () => void;
}) {
  const { t } = useI18n();
  const direct = p.mode !== 'auto' && p.mode !== 'both';
  return (
    <div className="composer-toolbar">
      <ModeControl
        mode={p.mode}
        bothAvailable={p.bothAvailable}
        providers={p.providers}
        onChange={p.onMode}
      />
      {p.orchestra && (
        <button
          className="orchestra-badge"
          title={t(
            'Orchestra is on: complex tasks are planned and split across agents. Click to turn off.',
          )}
          onClick={p.onOrchestraOff}
        >
          <Workflow size={12} />
          Orchestra
        </button>
      )}
      {p.workspaces.length > 0 && (
        <div className="workspace-controls">
          <select
            aria-label="Project"
            title={p.workspaceName || t('Choose project')}
            value={p.workspaceId}
            disabled={p.workspaceLocked}
            onChange={(e) => p.onWorkspace(e.target.value)}
          >
            {!p.workspaceId && <option value="">{t('Choose project')}</option>}
            {p.workspaces.map((w) => (
              <option key={w.id} value={w.id}>
                {w.name}
              </option>
            ))}
          </select>
        </div>
      )}
      {direct && (
        <button
          className={`icon-button advanced-toggle ${p.advanced ? 'on' : ''}`}
          aria-label={t('Advanced provider options')}
          aria-expanded={p.advanced}
          onClick={p.onAdvanced}
        >
          {p.advanced ? <ChevronDown size={15} /> : <ChevronUp size={15} />}
        </button>
      )}
      <div className="composer-spacer" />
      <PermissionButton accessMode={p.accessMode} onCycle={p.onAccess} />
      <button
        className={`icon-button attach-button ${p.attached ? 'on' : ''}`}
        aria-label={t('Attach files')}
        title={p.attached ? t(`${p.attached} files attached`) : t('Attach files')}
        onClick={p.onAttach}
      >
        <Paperclip size={15} />
        {p.attached > 0 && <small>{p.attached}</small>}
      </button>
      <button
        className="send-button"
        aria-label={p.running ? t('Stop response') : t('Send message')}
        disabled={!p.running && !p.canSend}
        onClick={p.onSend}
      >
        {p.running ? <Square size={15} fill="currentColor" /> : <ArrowUp size={19} />}
      </button>
    </div>
  );
}

/** Estimated prompt size of the draft (hidden when empty) and the keyboard hint. */
export function TokenCaption({
  tokens,
  impact,
  heavy,
}: {
  tokens: number;
  impact: 'Low' | 'Medium' | 'High';
  /** The draft holds a large encoded blob. */
  heavy: boolean;
}) {
  const { t } = useI18n();
  return (
    <div className="composer-caption">
      {tokens > 0 && (
        <span
          className={`token-badge ${heavy ? 'warn' : impact.toLowerCase()}`}
          title={
            heavy
              ? 'This message contains a large encoded blob'
              : `Estimated prompt size · quota impact ${impact}`
          }
        >
          ~{compactTokens(tokens)} tokens · {heavy ? 'Heavy' : impact}
        </span>
      )}
      <span>
        {t('Enter to send')} <span className="caption-dot">·</span>{' '}
        {t('Shift Enter for a new line')}
      </span>
    </div>
  );
}
