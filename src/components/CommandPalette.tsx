import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { filterCommands, type Command } from '../lib/commands';

/** Searchable list of every registered command; focus returns to the opener on close. */
export default function CommandPalette({
  commands,
  onClose,
}: {
  commands: Command[];
  onClose: () => void;
}) {
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const opener = useRef<Element | null>(document.activeElement);
  const base = useId();
  const results = useMemo(() => filterCommands(commands, query), [commands, query]);
  const current = Math.min(index, Math.max(0, results.length - 1));

  const ran = useRef(false);

  useEffect(() => {
    input.current?.focus();
    const previous = opener.current;
    return () => {
      // After a command, the view it opens owns the focus.
      if (!ran.current && previous instanceof HTMLElement) previous.focus();
    };
  }, []);

  function run(command: Command | undefined) {
    if (!command) return;
    ran.current = true;
    onClose();
    command.run();
  }

  return (
    <>
      <button
        className="usage-scrim palette-scrim"
        aria-label="Close command palette"
        onClick={onClose}
      />
      <section className="palette" role="dialog" aria-label="Command palette">
        <input
          ref={input}
          role="combobox"
          aria-expanded="true"
          aria-controls={`${base}-list`}
          aria-activedescendant={results[current] ? `${base}-${results[current].id}` : undefined}
          aria-label="Search commands"
          placeholder="Type a command"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setIndex(0);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Escape') {
              e.preventDefault();
              e.stopPropagation();
              onClose();
            } else if (e.key === 'ArrowDown') {
              e.preventDefault();
              setIndex((current + 1) % Math.max(1, results.length));
            } else if (e.key === 'ArrowUp') {
              e.preventDefault();
              setIndex((current - 1 + results.length) % Math.max(1, results.length));
            } else if (e.key === 'Enter') {
              e.preventDefault();
              run(results[current]);
            } else if (e.key === 'Tab') {
              e.preventDefault();
            }
          }}
        />
        <ul id={`${base}-list`} role="listbox" aria-label="Commands">
          {results.map((command, i) => (
            <li
              key={command.id}
              id={`${base}-${command.id}`}
              role="option"
              aria-selected={i === current}
              className={i === current ? 'active' : ''}
              onMouseEnter={() => setIndex(i)}
              onClick={() => run(command)}
            >
              <span>{command.label}</span>
              <small>{command.group}</small>
              {command.shortcut && <kbd>{command.shortcut}</kbd>}
            </li>
          ))}
        </ul>
        <small role="status" className="palette-status">
          {results.length === 0 ? 'No matching commands' : `${results.length} commands`}
        </small>
      </section>
    </>
  );
}
