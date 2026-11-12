import { ChevronDown, Folder, Settings2, PanelRight } from 'lucide-react';
import { IconButton } from '../lib/util';
import { useI18n } from '../lib/i18n';

/** Provider availability chip, derived from account health in App. */
export interface HealthPill {
  key: string;
  state: 'active' | 'near' | 'cooldown';
  label: string;
  title: string;
}

export interface TopbarProps {
  desktop: boolean;
  workspaceLabel: string;
  workspaceTitle?: string;
  conversationTitle: string;
  pills: HealthPill[];
  /** Given for a conversation that has not started: the folder name becomes a picker. */
  onChooseFolder?: () => void;
  folderBusy?: boolean;
  onOpenSettings: () => void;
  onOpenInspector?: () => void;
}

/** Breadcrumb on the left; provider status, project context and settings on the right. */
export default function Topbar({
  desktop,
  workspaceLabel,
  workspaceTitle,
  conversationTitle,
  pills,
  onChooseFolder,
  folderBusy,
  onOpenSettings,
  onOpenInspector,
}: TopbarProps) {
  const { t } = useI18n();
  return (
    <header className="topbar">
      <div className="topbar-left">
        {onChooseFolder ? (
          <button
            type="button"
            className="workspace-name workspace-picker"
            title={workspaceTitle}
            aria-label={`${t('Change folder')}: ${workspaceLabel}`}
            disabled={folderBusy}
            onClick={onChooseFolder}
          >
            <Folder size={15} />
            {workspaceLabel}
            <ChevronDown size={13} aria-hidden="true" />
          </button>
        ) : (
          <span className="workspace-name" title={workspaceTitle}>
            <Folder size={15} />
            {workspaceLabel}
          </span>
        )}
        <span className="topbar-divider" />
        <span className="conversation-label">{conversationTitle}</span>
      </div>
      <div className="topbar-actions">
        {onOpenInspector && (
          <IconButton
            className="icon-button"
            label={t('Project inspector')}
            onClick={onOpenInspector}
          >
            <PanelRight size={17} />
          </IconButton>
        )}
        {!desktop && <span className="preview-label">{t('Browser preview')}</span>}
        {pills.map((pill) => (
          <span key={pill.key} className={`health-pill ${pill.state}`} title={pill.title}>
            <span className={`health-dot ${pill.state}`} />
            {pill.label}
          </span>
        ))}
        <IconButton data-tour="settings" label={t('Settings')} onClick={onOpenSettings}>
          <Settings2 size={17} />
        </IconButton>
      </div>
    </header>
  );
}
