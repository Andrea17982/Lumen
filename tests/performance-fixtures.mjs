import { DatabaseSync } from 'node:sqlite';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

export async function seedPerformance(h) {
  for (const old of await h.invoke('list_sessions', { search: 'Performance ', offset: 0 })) {
    if (old.workspace === h.workspace) await h.invoke('delete_session', { id: old.id });
  }
  const sessions = [];
  for (const size of [100, 1000, 5000])
    for (const kind of ['short', 'mixed']) {
      const session = await h.invoke('create_session', { workspace: h.workspace });
      const title = `Performance ${size} ${kind}`;
      await h.invoke('update_session', { id: session.id, title, pinned: false });
      const messages = Array.from({ length: size }, (_, i) => ({
        id: `${session.id}-${i}`,
        role: i % 2 ? 'assistant' : 'user',
        text:
          `Fixture message ${i}: PERF_SEARCH_${i}. ` +
          (kind === 'short'
            ? 'A small coding requirement and a concrete result.'
            : `A retained decision ${i}: preserve React, Godot and account scope. `.repeat(
                i % 7 === 0 ? 30 : 5,
              ) +
              (i % 2
                ? '\n\n```typescript\nexport const limit = 3;\n```\n\n| Check | State |\n| --- | --- |\n| Compile | passed |\n\n[Local source](counter.ts:1)\n'
                : '')),
        activity:
          kind === 'mixed' && i % 10 === 1
            ? [
                { kind: 'tool', text: 'Controlled check returned; no external command.' },
                {
                  kind: 'child',
                  text: JSON.stringify({
                    id: `child-${i}`,
                    kind: 'snapshot',
                    title: 'Controlled reviewer',
                    status: 'completed',
                    text: 'A bounded independent observation.',
                  }),
                },
              ]
            : [],
      }));
      const file = path.join(h.artifacts, `fixture-${size}-${kind}.json`);
      await writeFile(file, JSON.stringify(messages));
      const db = new DatabaseSync(path.join(h.data, 'lumen.db'));
      try {
        db.exec('BEGIN');
        const insert = db.prepare(
          'INSERT INTO messages(id,session_id,role,text,activity,created_at) VALUES(?,?,?,?,?,?)',
        );
        messages.forEach((m, i) =>
          insert.run(m.id, session.id, m.role, m.text, JSON.stringify(m.activity), i),
        );
        db.exec('COMMIT');
      } finally {
        db.close();
      }
      sessions.push({ ...session, title, size, kind, lastId: messages.at(-1).id });
    }
  return sessions;
}
