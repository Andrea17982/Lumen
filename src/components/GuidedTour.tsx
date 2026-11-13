import { useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ArrowLeft, ArrowRight, X } from 'lucide-react';
import { useI18n } from '../lib/i18n';

export const TOUR_KEY = 'lumen-tour-v1';
const steps = [
  {
    target: '.sidebar-toggle',
    title: 'Your projects, close at hand',
    text: 'Click the Lumen logo to show or hide projects and conversations. Choose a project folder to give the agent the right workspace.',
  },
  {
    target: '[data-tour="settings"]',
    title: 'Bring your subscriptions together',
    text: 'Connect your accounts in Settings → Accounts using each provider’s official sign-in. You can connect more than one account.',
  },
  {
    target: '.mode-control',
    title: 'Choose who works with you',
    text: 'Pick a provider directly, or use Auto to choose for the task. Auto offers Budget, Default and Quality. Enable Orchestra in Settings for coordinated work across agents.',
  },
  {
    target: '[data-tour="permissions"]',
    title: 'Decide what the agent can do',
    text: 'Permissions controls actions, not the model. Start in Read only, allow file edits, approve individual actions, or choose Autopilot. Opening this menu does not grant access.',
  },
  {
    target: '.composer',
    title: 'Ask, then follow the work',
    text: 'Describe the result you want and attach useful files. Enter sends; Shift+Enter starts a new line. During a task, expand the progress summary to see tools, agents and checks. You can stop a response at any time.',
  },
];

/** Nonmodal coachmarks on real controls; the tour never invokes them. */
export default function GuidedTour({
  step,
  onStep,
  onClose,
}: {
  step: number;
  onStep: (step: number) => void;
  onClose: (completed: boolean) => void;
}) {
  const { t } = useI18n();
  const card = useRef<HTMLElement>(null);
  const title = useRef<HTMLHeadingElement>(null);
  const [layout, setLayout] = useState<{ target: DOMRect; left: number; top: number } | null>(null);
  const current = steps[step];
  useLayoutEffect(() => {
    const target = document.querySelector<HTMLElement>(current.target);
    function update() {
      if (!target || !card.current) return;
      const rect = target.getBoundingClientRect();
      const width = card.current.offsetWidth;
      const height = card.current.offsetHeight;
      const left = Math.max(12, Math.min(rect.left, innerWidth - width - 12));
      const top =
        rect.top > height + 28
          ? rect.top - height - 16
          : Math.min(rect.bottom + 16, innerHeight - height - 12);
      setLayout({ target: rect, left, top: Math.max(12, top) });
    }
    update();
    const focus = requestAnimationFrame(() => title.current?.focus());
    const observer = new ResizeObserver(update);
    if (target) observer.observe(target);
    if (card.current) observer.observe(card.current);
    const settle = window.setTimeout(update, 280);
    window.addEventListener('resize', update);
    window.addEventListener('scroll', update, true);
    return () => {
      observer.disconnect();
      cancelAnimationFrame(focus);
      clearTimeout(settle);
      window.removeEventListener('resize', update);
      window.removeEventListener('scroll', update, true);
    };
  }, [current]);
  return createPortal(
    <div
      className="guided-tour"
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.preventDefault();
          e.stopPropagation();
          onClose(false);
        }
      }}
    >
      {layout && (
        <div
          className="tour-highlight"
          aria-hidden="true"
          style={{
            left: layout.target.left - 5,
            top: layout.target.top - 5,
            width: layout.target.width + 10,
            height: layout.target.height + 10,
          }}
        />
      )}
      <section
        ref={card}
        className="tour-card"
        role="dialog"
        aria-modal="false"
        aria-labelledby="tour-title"
        aria-describedby="tour-description"
        style={{ left: layout?.left ?? 16, top: layout?.top ?? 80 }}
      >
        <button
          type="button"
          className="icon-button tour-close"
          aria-label={t('Close tutorial')}
          onClick={() => onClose(false)}
        >
          <X size={16} />
        </button>
        <h2 ref={title} tabIndex={-1} id="tour-title">
          {t(current.title)}
        </h2>
        <p id="tour-description">{t(current.text)}</p>
        <footer>
          <span className="tour-count" aria-label={t('Tutorial progress')}>
            {step + 1} / {steps.length}
          </span>
          <button type="button" className="text-button" onClick={() => onClose(false)}>
            {t('Skip tour')}
          </button>
          {step > 0 && (
            <button
              type="button"
              className="icon-button"
              aria-label={t('Previous step')}
              onClick={() => onStep(step - 1)}
            >
              <ArrowLeft size={16} />
            </button>
          )}
          <button
            type="button"
            className="primary"
            onClick={() => (step === steps.length - 1 ? onClose(true) : onStep(step + 1))}
          >
            {t(step === steps.length - 1 ? 'Start exploring' : 'Next')}
            <ArrowRight size={14} />
          </button>
        </footer>
      </section>
    </div>,
    document.body,
  );
}
