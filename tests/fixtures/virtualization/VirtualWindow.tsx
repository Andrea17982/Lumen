// Deliberately experimental: offscreen DOM is absent. Native find/selection/AX gates must
// decide whether this may ship; no complete-history accessibility claim is made here.
import { useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';

export default function VirtualWindow({ children }: { children: ReactNode[] }) {
  const container = useRef<HTMLDivElement>(null);
  const sizes = useRef(new Map<number, number>());
  const [revision, setRevision] = useState(0);
  const [range, setRange] = useState(() => ({
    start: Math.max(0, children.length - 40),
    end: children.length,
  }));
  const offsets = useMemo(() => {
    const values = [0];
    for (let i = 0; i < children.length; i++)
      values.push(values[i] + (sizes.current.get(i) || 320));
    return values;
  }, [children.length, revision]);
  const offsetsRef = useRef(offsets);
  offsetsRef.current = offsets;
  const keepBottom = useRef(true);
  useLayoutEffect(() => {
    const root = container.current!,
      scroll = root.closest('.conversation') as HTMLElement;
    let frame = 0;
    const update = () => {
      frame = 0;
      const p = offsetsRef.current;
      const local =
        scroll.scrollTop -
        (root.getBoundingClientRect().top - scroll.getBoundingClientRect().top + scroll.scrollTop);
      const find = (value: number) => {
        let lo = 0,
          hi = p.length - 1;
        while (lo < hi) {
          const mid = (lo + hi + 1) >> 1;
          if (p[mid] <= value) lo = mid;
          else hi = mid - 1;
        }
        return lo;
      };
      const start = Math.max(0, find(Math.max(0, local - 900)));
      const end = Math.min(children.length, find(local + scroll.clientHeight + 900) + 1);
      keepBottom.current = scroll.scrollHeight - scroll.scrollTop - scroll.clientHeight < 8;
      setRange((old) => (old.start === start && old.end === end ? old : { start, end }));
    };
    const onScroll = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };
    scroll.scrollTop = scroll.scrollHeight;
    scroll.addEventListener('scroll', onScroll, { passive: true });
    update();
    return () => {
      scroll.removeEventListener('scroll', onScroll);
      if (frame) cancelAnimationFrame(frame);
    };
  }, [children.length]);
  useLayoutEffect(() => {
    const root = container.current!,
      scroll = root.closest('.conversation') as HTMLElement;
    const observer = new ResizeObserver((entries) => {
      let changed = false;
      const follow =
        scroll.scrollHeight - scroll.scrollTop - scroll.clientHeight < 8 || keepBottom.current;
      for (const e of entries) {
        const index = Number((e.target as HTMLElement).dataset.index);
        const height = e.borderBoxSize[0]?.blockSize || e.target.getBoundingClientRect().height;
        if (height > 0 && Math.abs((sizes.current.get(index) || 320) - height) > 1) {
          sizes.current.set(index, height);
          changed = true;
        }
      }
      if (changed) {
        keepBottom.current = follow;
        setRevision((v) => v + 1);
      }
    });
    root.querySelectorAll<HTMLElement>('[data-index]').forEach((row) => observer.observe(row));
    if (keepBottom.current) scroll.scrollTop = scroll.scrollHeight;
    return () => observer.disconnect();
  }, [range, revision]);
  return (
    <div ref={container} data-virtual-window-prototype="true">
      <div aria-hidden="true" style={{ height: offsets[range.start] }} />
      {children.slice(range.start, range.end).map((child, index) => (
        <div
          key={range.start + index}
          data-index={range.start + index}
          style={{ display: 'flow-root' }}
        >
          {child}
        </div>
      ))}
      <div
        aria-hidden="true"
        style={{ height: Math.max(0, offsets[children.length] - offsets[range.end]) }}
      />
    </div>
  );
}
