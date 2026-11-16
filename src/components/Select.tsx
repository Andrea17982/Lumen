import { Children, isValidElement, useEffect, useId, useRef, useState } from 'react';
import type { ChangeEvent, ReactNode, SelectHTMLAttributes } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronDown } from 'lucide-react';

/** One keyboard-accessible glass menu for all single-choice fields. */
export default function Select({
  children,
  onChange,
  ...props
}: SelectHTMLAttributes<HTMLSelectElement>) {
  const id = useId();
  const native = useRef<HTMLSelectElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const [position, setPosition] = useState<{
    left: number;
    top?: number;
    bottom?: number;
    width: number;
    maxHeight: number;
  } | null>(null);
  const [active, setActive] = useState(0);
  const options: { value: string; label: ReactNode; disabled?: boolean; title?: string }[] = [];
  const collect = (nodes: ReactNode) =>
    Children.forEach(nodes, (node) => {
      if (
        !isValidElement<{
          value?: string;
          children?: ReactNode;
          disabled?: boolean;
          title?: string;
        }>(node)
      )
        return;
      if (node.type === 'option')
        options.push({
          value: String(node.props.value ?? node.props.children ?? ''),
          label: node.props.children,
          disabled: node.props.disabled,
          title: node.props.title,
        });
      else collect(node.props.children);
    });
  collect(children);
  const selected = options.findIndex((o) => o.value === String(props.value ?? ''));
  const close = () => {
    setPosition(null);
    trigger.current?.focus();
  };
  useEffect(() => {
    if (!position) return;
    const dismiss = (event: Event) => {
      if (
        event.type === 'scroll' &&
        event.target instanceof Element &&
        event.target.closest('.select-menu')
      )
        return;
      setPosition(null);
    };
    window.addEventListener('resize', dismiss);
    window.addEventListener('scroll', dismiss, true);
    return () => {
      window.removeEventListener('resize', dismiss);
      window.removeEventListener('scroll', dismiss, true);
    };
  }, [position]);
  const choose = (index: number) => {
    const option = options[index];
    if (!option || option.disabled || !native.current) return;
    native.current.value = option.value;
    onChange?.({
      target: native.current,
      currentTarget: native.current,
    } as ChangeEvent<HTMLSelectElement>);
    close();
  };
  const move = (direction: number) => {
    let next = active;
    for (let i = 0; i < options.length; i++) {
      next = (next + direction + options.length) % options.length;
      if (!options[next].disabled) {
        setActive(next);
        break;
      }
    }
  };
  return (
    <>
      <button
        type="button"
        ref={trigger}
        id={props.id}
        className={`select-trigger ${props.className ?? ''}`}
        disabled={props.disabled}
        aria-label={props['aria-label']}
        aria-labelledby={props['aria-labelledby']}
        title={props.title}
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={!!position}
        aria-controls={position ? id : undefined}
        aria-activedescendant={position ? `${id}-${active}` : undefined}
        onClick={() => {
          if (position) {
            close();
            return;
          }
          const rect = trigger.current!.getBoundingClientRect();
          const below = window.innerHeight - rect.bottom - 12;
          const above = rect.top - 12;
          const opensBelow = below >= Math.min(300, options.length * 36 + 12) || below >= above;
          const width = Math.min(Math.max(rect.width, 180), window.innerWidth - 24);
          setPosition({
            left: Math.max(12, Math.min(rect.left, window.innerWidth - width - 12)),
            ...(opensBelow
              ? { top: rect.bottom + 6 }
              : { bottom: window.innerHeight - rect.top + 6 }),
            width,
            maxHeight: Math.max(36, Math.min(300, (opensBelow ? below : above) - 6)),
          });
          setActive(Math.max(0, selected));
        }}
        onKeyDown={(e) => {
          if (!position) {
            if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
              e.preventDefault();
              e.currentTarget.click();
            }
            return;
          }
          if (
            ['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', ' ', 'Escape', 'Tab'].includes(e.key)
          ) {
            if (e.key !== 'Tab') e.preventDefault();
            if (e.key === 'ArrowDown') move(1);
            else if (e.key === 'ArrowUp') move(-1);
            else if (e.key === 'Home') setActive(options.findIndex((o) => !o.disabled));
            else if (e.key === 'End')
              setActive(options.length - 1 - [...options].reverse().findIndex((o) => !o.disabled));
            else if (e.key === 'Escape' || e.key === 'Tab') close();
            else choose(active);
          } else if (e.key.length === 1) {
            const index = options.findIndex(
              (o) => !o.disabled && String(o.label).toLowerCase().startsWith(e.key.toLowerCase()),
            );
            if (index >= 0) setActive(index);
          }
        }}
      >
        <span>{options[selected]?.label ?? options[0]?.label}</span>
        <ChevronDown size={13} aria-hidden="true" />
      </button>
      <select
        {...props}
        id={undefined}
        aria-label={undefined}
        aria-hidden="true"
        tabIndex={-1}
        hidden
        ref={native}
        onChange={onChange}
      >
        {children}
      </select>
      {position &&
        createPortal(
          <>
            <button
              className="select-scrim"
              tabIndex={-1}
              aria-label="Close choices"
              onClick={close}
            />
            <div
              id={id}
              className="select-menu"
              role="listbox"
              aria-label={props['aria-label']}
              style={position}
            >
              {options.map((o, i) => (
                <button
                  key={o.value}
                  type="button"
                  role="option"
                  id={`${id}-${i}`}
                  aria-selected={i === selected}
                  disabled={o.disabled}
                  className={i === active ? 'highlighted' : ''}
                  title={o.title}
                  tabIndex={-1}
                  onMouseEnter={() => setActive(i)}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => choose(i)}
                  ref={(el) => {
                    if (el && i === active) el.scrollIntoView({ block: 'nearest' });
                  }}
                >
                  <span>{o.label}</span>
                  {i === selected && <Check size={14} aria-hidden="true" />}
                </button>
              ))}
            </div>
          </>,
          document.body,
        )}
    </>
  );
}
