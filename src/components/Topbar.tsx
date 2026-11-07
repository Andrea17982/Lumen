import { Folder, Menu, Settings2 } from 'lucide-react';
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
  onOpenSidebar: () => void;
  onOpenSettings: () => void;
}

/** Breadcrumb on the left; provider status, project context and settings on the right. */
export default function Topbar({
  desktop,
  workspaceLabel,
  workspaceTitle,
  conversationTitle,
  pills,
  onOpenSidebar,
  onOpenSettings,
}: TopbarProps) {
  const { t } = useI18n();
  return (
    <header className="topbar">
      <div className="topbar-left">
        <IconButton
          className="icon-button mobile-menu"
          label={t('Open sidebar')}
          onClick={onOpenSidebar}
        >
          <Menu size={19} />
        </IconButton>
        <span className="workspace-name" title={workspaceTitle}>
          <Folder size={15} />
          {workspaceLabel}
        </span>
        <span className="topbar-divider" />
        <span className="conversation-label">{conversationTitle}</span>
      </div>
      <div className="topbar-actions">
        {!desktop && <span className="preview-label">{t('Browser preview')}</span>}
        {pills.map((pill) => (
          <span key={pill.key} className={`health-pill ${pill.state}`} title={pill.title}>
            <span className={`health-dot ${pill.state}`} />
            {pill.label}
          </span>
        ))}
        <IconButton label={t('Settings')} onClick={onOpenSettings}>
          <Settings2 size={17} />
        </IconButton>
      </div>
    </header>
  );
}
