// Sample content for the brand gallery only; product strings live in src/i18n.
export const demo = {
  en: {
    project: 'lumen',
    path: '~/code/lumen',
    title: 'Fix the flaky gatekeeper test',
    sessionsToday: ['Fix the flaky gatekeeper test', 'Italian strings for settings', 'Speed up the routing classifier'],
    sessionsYesterday: ['Why did the quota reset early?', 'Release notes for 0.12.2'],
    user: 'The gatekeeper test fails about one run in ten on Windows. Find the cause and fix it without slowing the suite down.',
    answer1:
      'The failure comes from a race: the test reads the approval journal before the broker has flushed its write, so on a slow pipe it sees the previous state. I replaced the fixed sleep with a wait on the journal’s change notification.',
    answer2: 'The suite is now deterministic, and 4% faster because the 200 ms sleep is gone.',
    command: 'git push origin lumen/fix-gatekeeper-race',
    cwd: '~/code/lumen',
    reason: 'Publish the fix as a draft pull request.',
    streaming: 'While that runs, I am checking whether the same pattern appears in the Codex adapter tests. Two places use a fixed sleep',
    model: 'Claude',
  },
  it: {
    project: 'lumen',
    path: '~/code/lumen',
    title: 'Correggi il test instabile del gatekeeper',
    sessionsToday: ['Correggi il test instabile del gatekeeper', 'Testi italiani per le impostazioni', 'Velocizza il classificatore di routing'],
    sessionsYesterday: ['Perché la quota si è azzerata prima?', 'Note di rilascio della 0.12.2'],
    user: 'Il test del gatekeeper fallisce circa una volta su dieci su Windows. Trova la causa e correggila senza rallentare la suite.',
    answer1:
      'Il fallimento nasce da una corsa: il test legge il journal delle approvazioni prima che il broker abbia scritto, quindi su una pipe lenta vede lo stato precedente. Ho sostituito l’attesa fissa con l’attesa della notifica di modifica del journal.',
    answer2: 'Ora la suite è deterministica e più veloce del 4%, perché l’attesa da 200 ms non c’è più.',
    command: 'git push origin lumen/fix-gatekeeper-race',
    cwd: '~/code/lumen',
    reason: 'Pubblica la correzione come pull request in bozza.',
    streaming: 'Nel frattempo controllo se lo stesso schema compare nei test dell’adapter Codex. Due punti usano un’attesa fissa',
    model: 'Claude',
  },
};

export const code = `let changed = journal.subscribe();
broker.decide(decision)?;
// Wait for the write instead of sleeping.
tokio::time::timeout(Duration::from_secs(5), changed.recv()).await??;
assert_eq!(journal.snapshot()?.len(), 1);`;
