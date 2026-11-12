import { motion } from 'framer-motion';
import type { ReactNode } from 'react';
import { Mark } from '../lib/util';
import { useI18n } from '../lib/i18n';

/** Empty-state hero: the floating glass mark and the product invitation. */
export default function Welcome({ reduced, children }: { reduced: boolean; children?: ReactNode }) {
  const { t } = useI18n();
  return (
    <div className="welcome">
      <motion.div
        initial={false}
        animate={{ y: [0, -5, 0] }}
        transition={
          reduced ? { duration: 0 } : { duration: 8, repeat: Infinity, ease: 'easeInOut' }
        }
      >
        <Mark />
      </motion.div>
      <h1>
        {t('A clear space')}
        <br />
        {t('for a brighter idea.')}
      </h1>
      {children}
    </div>
  );
}
