import { useEffect, useRef, useState, type RefObject } from 'react';
import { Button } from '../../design/components';
import { Icon, type IconName } from '../../design/icons';
import { useT } from '../../i18n';
import { useSyncClient } from '../../sync/react';
import s from './Welcome.module.css';

const targets = ['composer', 'agent', 'auto', 'access', 'stop', 'task', 'orchestra', 'maintenance'];
const icons: IconName[] = ['file', 'terminal', 'info', 'shield', 'stop', 'folder', 'sidebar', 'gear'];

export function Tour({ index, onStep, root, onClose }: { index: number; onStep: (index: number) => void; root: RefObject<HTMLDivElement | null>; onClose: () => void }) {
  const t = useT();
  const w = t.welcome;
  const sync = useSyncClient();
  const heading = useRef<HTMLHeadingElement>(null);
  const [found, setFound] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const { title, body } = w.tourSteps[index];

  useEffect(() => heading.current?.focus(), [index]);
  useEffect(() => {
    const area = root.current;
    if (!area) return;
    let target: HTMLElement | null = null;
    const find = () => {
      const next = area.querySelector<HTMLElement>(`[data-tour-target="${targets[index]}"]`);
      if (target === next) return;
      target?.removeAttribute('data-tour-highlight');
      target = next;
      target?.setAttribute('data-tour-highlight', '');
      setFound(target !== null);
    };
    setFound(false);
    find();
    const observer = new MutationObserver(find);
    observer.observe(area, { childList: true, subtree: true });
    return () => {
      observer.disconnect();
      target?.removeAttribute('data-tour-highlight');
    };
  }, [index, root]);

  async function finish(skipped: boolean) {
    setBusy(true);
    setFailed(false);
    try {
      await sync.updateOnboarding(skipped ? 'skipTour' : 'finishTour');
      onClose();
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className={s.tour} aria-labelledby="tour-title">
      <div>
        <h2 tabIndex={-1} ref={heading} id="tour-title">{title}</h2>
        <p>{body}</p>
        {!found && <p className={s.example}><Icon name={icons[index]} />{w.example} · {[t.composerPlaceholder, t.composer.agent, t.modeAuto, t.composer.access, t.stop, w.tourSteps[5].title, t.general.orchestra, t.settings.sections.maintenance][index]}</p>}
        {failed && <p role="alert" className={s.error}>{w.saveError}</p>}
      </div>
      <div className={s.tourActions}>
        <span>{w.progress(index + 1, targets.length)}</span>
        {failed && <Button variant="plain" onPress={onClose}>{w.leave}</Button>}
        <Button variant="plain" isDisabled={busy} onPress={() => void finish(true)}>{w.skipTour}</Button>
        {index > 0 && <Button variant="secondary" isDisabled={busy} onPress={() => onStep(index - 1)}>{w.back}</Button>}
        <Button variant="primary" isDisabled={busy} onPress={() => index === targets.length - 1 ? void finish(false) : onStep(index + 1)}>{index === targets.length - 1 ? w.endTour : w.next}</Button>
      </div>
    </section>
  );
}
