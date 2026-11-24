// Deterministic long transcripts (from the M0 bake-off generator) and a way to place them in a
// stopped daemon's store, for the performance and accessibility checks of the conversation view.
import { DatabaseSync } from 'node:sqlite';
import path from 'node:path';

function rng(seed) {
  return () => {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const WORDS =
  'the daemon session worker review checkpoint routing quota account provider model effort branch commit test check build output stream event snapshot sequence approval question resume handoff context memory project folder module function type error result value state store cache render message activity tool command file diff line change patch plan task graph lane agent token budget limit window reset retry receipt claim reply transport frame pipe socket handshake protocol version digest'.split(
    ' ',
  );
const IDENTS = ['send_message', 'RunOptions', 'EventHub', 'gatekeeper_decide', 'message_snapshots', 'Client::call', 'AccessMode', 'useSyncExternalStore'];

function make(seed) {
  const r = rng(seed);
  const pick = (a) => a[Math.floor(r() * a.length)];
  const int = (a, b) => a + Math.floor(r() * (b - a + 1));
  const sentence = () => {
    const words = Array.from({ length: int(6, 22) }, () => pick(WORDS));
    if (r() < 0.35) words[int(0, words.length - 1)] = '`' + pick(IDENTS) + '`';
    if (r() < 0.2) words[int(0, words.length - 1)] = '**' + pick(WORDS) + '**';
    const s = words.join(' ');
    return s[0].toUpperCase() + s.slice(1) + '.';
  };
  const paragraph = () => Array.from({ length: int(1, 5) }, sentence).join(' ');
  const codeLine = () => pick([`let ${pick(WORDS)} = ${pick(WORDS)}.${pick(WORDS)}(${int(0, 99)})?;`, `fn ${pick(WORDS)}_${pick(WORDS)}(id: &str) -> Result<(), String> {`, `}`, `// ${sentence()}`]);
  const code = () => '```' + pick(['rust', 'ts', 'sh']) + '\n' + Array.from({ length: int(6, 48) }, codeLine).join('\n') + '\n```';
  const list = () => Array.from({ length: int(3, 7) }, (_, i) => (r() < 0.5 ? `${i + 1}. ` : '- ') + sentence()).join('\n');
  const table = () => {
    const cols = int(2, 4);
    const row = () => '| ' + Array.from({ length: cols }, () => pick(WORDS)).join(' | ') + ' |';
    return [row(), '| ' + Array(cols).fill('---').join(' | ') + ' |', ...Array.from({ length: int(2, 6) }, row)].join('\n');
  };
  const block = () => {
    const x = r();
    if (x < 0.42) return paragraph();
    if (x < 0.62) return list();
    if (x < 0.82) return code();
    if (x < 0.9) return '## ' + sentence().slice(0, 50);
    if (x < 0.95) return table();
    return '> ' + sentence();
  };
  const activity = () => {
    const kind = pick(['tool', 'tool', 'command', 'file', 'thinking', 'model']);
    if (kind === 'thinking') return { kind, text: paragraph() };
    if (kind === 'model') return { kind, text: `claude · ${pick(WORDS)} · low` };
    if (kind === 'tool') return { kind, text: `Read\n${JSON.stringify({ file_path: `src/${pick(WORDS)}.rs` })}` };
    if (kind === 'file') return { kind, text: JSON.stringify([{ path: `src/${pick(WORDS)}.rs`, kind: 'update' }]) };
    return { kind, text: `cargo test -p lumen-core\n` + Array.from({ length: int(1, 12) }, codeLine).join('\n') };
  };
  return { r, int, block, activity, paragraph, code };
}

/** `count` alternating messages; markers in the first, middle and last make find checks exact. */
export function transcript(count, seed) {
  const g = make(seed);
  return Array.from({ length: count }, (_, i) => {
    const user = i % 2 === 0;
    const blocks = user ? [g.paragraph(), ...(g.r() < 0.15 ? [g.code()] : [])] : Array.from({ length: g.int(2, 8) }, g.block);
    if (i === 0) blocks.unshift('ZEBRAFIRST marker in the first message.');
    if (i === Math.floor(count / 2)) blocks.push('ZEBRAMIDDLE marker in the middle message.');
    if (i === count - 1) blocks.push('ZEBRALAST marker in the last message.');
    return { id: `m-${String(i).padStart(5, '0')}`, role: user ? 'user' : 'assistant', text: blocks.join('\n\n'), activity: user ? [] : Array.from({ length: g.int(0, 30) }, g.activity) };
  });
}

/** Writes messages into a session of a data directory whose daemon is stopped. */
export function seed(data, sessionId, messages) {
  const db = new DatabaseSync(path.join(data, 'lumen.db'));
  try {
    const insert = db.prepare('INSERT INTO messages(id,session_id,role,text,activity,created_at) VALUES(?,?,?,?,?,?)');
    db.exec('BEGIN');
    messages.forEach((m, i) => insert.run(m.id, sessionId, m.role, m.text, JSON.stringify(m.activity), 1_700_000_000_000 + i));
    db.exec('COMMIT');
  } finally {
    db.close();
  }
}
