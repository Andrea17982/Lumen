import { CircleCheck, Pencil, Shield, ShieldCheck } from 'lucide-react';
import type { AccessMode } from '../lib/types';
import { useI18n } from '../lib/i18n';

/** Order in which a click cycles through the permission modes. */
export const ACCESS_ORDER: AccessMode[] = ['read-only', 'file-edits', 'ask', 'autopilot'];

const ACCESS_LABEL: Record<AccessMode, string> = {
  'read-only': 'Ask',
  'file-edits': 'Edits',
  ask: 'Approve each action',
  autopilot: 'Autopilot',
};

/** Icon button that shows the current permission mode and cycles to the next one on click. */
export default function PermissionButton({
  accessMode,
  onCycle,
}: {
  accessMode: AccessMode;
  onCycle: (next: AccessMode) => void;
}) {
  const { t } = useI18n();
  const label = t(`Permissions: ${ACCESS_LABEL[accessMode]}`);
  const Icon =
    accessMode === 'read-only'
      ? Shield
      : accessMode === 'file-edits'
        ? Pencil
        : accessMode === 'ask'
          ? CircleCheck
          : ShieldCheck;
  return (
    <button
      className={`icon-button access-control ${accessMode === 'autopilot' ? 'autopilot' : ''}`}
      aria-label={label}
      title={`${label} - ${t('click to change')}`}
      onClick={() =>
        onCycle(ACCESS_ORDER[(ACCESS_ORDER.indexOf(accessMode) + 1) % ACCESS_ORDER.length])
      }
    >
      <Icon size={15} />
    </button>
  );
}
