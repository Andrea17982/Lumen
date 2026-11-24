<div align="center">
  <img src="public/lumen.png" width="76" height="76" alt="Lumen">
  <h1>Lumen</h1>
  <p><strong>Collega i tuoi account AI una volta. Scrivi cosa ti serve. Auto fa il resto.</strong></p>
  <p>
    <a href="https://github.com/Andrea17982/Lumen/releases/latest">Download</a> ·
    <a href="docs/README.md">Documentazione (EN)</a> ·
    <a href="README.md">English</a>
  </p>
</div>

Lumen è un'app desktop per programmare con l'AI. Accedi con gli abbonamenti che paghi già (Claude Code, Codex, Antigravity, OpenCode, Cursor, Ollama in locale o una chiave API) e Lumen li fa lavorare insieme, come una squadra, sul tuo progetto. È gratuito, open source (MIT) e in alpha.

> **Stato:** l'interfaccia desktop è in riscrittura da zero. Questo branch contiene il daemon e il backend; le release pubblicate usano ancora l'interfaccia precedente.

## Auto

Non scegli il modello, non dividi il lavoro e non controlli gli agenti a mano. Auto decide:

- **chi lavora**: il miglior agente disponibile in quel momento, e un altro quando una quota finisce, senza ripetere il lavoro già fatto;
- **come**: un agente, un agente più un revisore di un altro provider, oppure un team in worktree isolati;
- **se far competere**: nel profilo Qualità due provider risolvono lo stesso compito importante in modo indipendente, e vince la soluzione che passa i tuoi check;
- **cosa rispondere**: alle domande di chiarimento risponde a partire dal codice e dalla memoria del progetto, ed elenca le ipotesi alla fine. Per le azioni rischiose, irreversibili o esterne si ferma e chiede a te.

Ricevi una sola risposta, scritta da Lumen in base a ciò che ha osservato: cosa è stato fatto, cosa è stato verificato, cosa è fallito e cosa resta aperto. [Come funziona](docs/product.md)

## Dentro c'è anche

- **Dal task alla PR unita.** Un task grande lavora nel suo worktree. Lumen fa il commit quando i check passano; **Pubblica** apre la PR, poi Lumen corregge CI e review e fa il merge.
- **Memoria.** Le decisioni del progetto finiscono in `LUMEN.md`, che ogni agente legge. Le preferenze valide per tutti i progetti si salvano solo dopo la tua conferma.
- **Annulla.** Checkpoint prima di ogni scrittura, un audit log e un rollback che si rifiuta di partire quando non sarebbe sicuro.
- **Moli.** Un browser incluso, condiviso da tutti gli agenti.
- **Code Mode.** Molte chiamate ai tool diventano un solo script, e con risultati grandi si risparmiano token. [Misure](docs/validation.md#code-mode-measurements)
- **Servizio in background.** Il lavoro continua anche dopo aver chiuso la finestra.

## Download e build

Scarica l'ultima release da [Releases](https://github.com/Andrea17982/Lumen/releases/latest). I pacchetti non sono firmati: verificali con `SHA256SUMS.txt`. Solo Windows ha test desktop completi. Per compilare da sorgente vedi il [README inglese](README.md#build-from-source).

## Da sapere

Autopilot dà agli agenti un accesso ampio al computer e non è una sandbox del sistema operativo. Lumen non copia mai le credenziali dei provider: ogni CLI fa il login da sé. [Validazione](docs/validation.md) · [Sicurezza](SECURITY.md)

## Come è fatto

Codice, test e documentazione di Lumen sono scritti dall'AI (Claude Code) sotto la mia direzione. Le decisioni di prodotto sono mie. È una dichiarazione, non un audit di sicurezza.

Grazie a **Theo / pingdotgg, T3 Tools Inc. e ai contributor di T3 Code**: Lumen adatta parti del design di T3 Code. [Fonti e modifiche](src-tauri/resources/legal/t3code/ADAPTATIONS.md) · [Licenze](THIRD_PARTY_NOTICES.md). Ciò non implica alcun endorsement.

Feedback e segnalazioni nelle [Issues](https://github.com/Andrea17982/Lumen/issues) aiutano molto. Per sostenere il progetto c'è [PayPal](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen); non sblocca nulla in più. — Andrea

MIT © 2026 Andrea17982. Non affiliato con Anthropic, OpenAI, Google o con gli altri provider citati.
