<div align="center">
  <img src="public/lumen.png" width="76" height="76" alt="Lumen">
  <h1>Lumen</h1>
  <p><strong>Collega i tuoi account AI una volta. Scrivi cosa ti serve. Al resto pensa Auto.</strong></p>
  <p>
    <a href="https://github.com/Andrea17982/Lumen/releases/latest">Scarica</a> ·
    <a href="docs/getting-started.md">Per iniziare</a> ·
    <a href="docs/README.md">Documentazione</a> ·
    <a href="README.md">English</a>
  </p>
</div>

Questo checkout contiene il daemon, il backend e gli strumenti di sviluppo. Le release desktop citate sotto appartengono all’interfaccia precedente.

Lumen è un'app desktop per programmare con l'AI. Accedi agli abbonamenti che paghi già (Claude Code, Codex, Antigravity, OpenCode, Cursor, Ollama in locale o una chiave API) e Lumen li fa lavorare insieme, come una squadra, sul tuo progetto.

È gratuita, open source (MIT) e ancora in alpha.

## Auto

Auto è il motivo per cui esiste Lumen. Non scegli il modello, non dividi il lavoro e non controlli gli agenti uno per uno. Scrivi la richiesta e Auto decide:

- **chi ci lavora** — l'agente migliore che hai disponibile in quel momento, e un altro quando finisce una quota, senza rifare quello che era già fatto;
- **come** — un agente solo, un agente più un revisore indipendente di un altro provider, oppure una squadra che si divide il lavoro in worktree separati e poi lo riunisce;
- **se fare una gara** — nel profilo Qualità, per i compiti importanti due provider diversi scrivono due soluzioni indipendenti; Lumen esegue i controlli del progetto su entrambe e tiene quella che li passa (0.11);
- **cosa rispondere quando un agente fa una domanda** — Auto risponde in base al codice e alla memoria del progetto, e alla fine elenca le ipotesi fatte. Si ferma comunque per tutto ciò che è rischioso, irreversibile o esterno (0.11).

Alla fine ricevi una sola risposta: cosa è stato fatto, cosa è stato verificato, cosa è fallito e cosa resta aperto. Quella parte la scrive Lumen, in base a ciò che ha visto, non a ciò che dichiarano gli agenti.

I profili Budget e Default non fanno mai gare, quindi un'installazione nuova non ti brucia la quota.

## Perché l'ho fatto (e cosa fanno meglio gli altri)

Uso più abbonamenti AI ed ero stanco di scegliere quale aprire, copiare il contesto da uno all'altro e controllare il loro lavoro a mano.

**Hermes Agent** — Mi piace molto come impara skill e conserva la memoria, e che puoi parlargli da Telegram o Slack. È migliore come agente personale che vive su un server. Lumen è uno spazio di lavoro per programmare sul tuo desktop, che usa i tuoi abbonamenti tramite le CLI ufficiali e verifica il risultato con i controlli del tuo progetto.

**Conductor, Claude Squad e simili** — Ottimi per far girare tanti agenti in parallelo nei worktree.

**Cursor, l'app Codex, Claude Code** — Curati, ma ognuno ti tiene dentro un solo fornitore. Lumen esiste per farli lavorare insieme.

Dove Lumen oggi è indietro: niente app mobile per ora, pacchetti non firmati, e Windows è l'unica piattaforma provata a fondo sul desktop.

**Misure di Code Mode.** Con Code Mode attivo, Moli e i tool MCP sono raggiungibili solo da uno script. Su Claude Code, leggere dodici file grandi è costato circa 357k token non in cache con i tool diretti e 37k con uno script (−90%), con più risposte corrette (da 1/3 a 2/3). Con risultati piccoli lo script aggiunge qualche richiesta, e Codex non ha risparmiato nulla. Ora Code Mode è **Automatico** di default: il turno parte con le chiamate classiche e passa a uno script solo dopo cinque round di tool o un risultato oltre 24 KiB. Su gpt-oss 20B è passato da 45/50 a 50/50 senza perdere alcun task, con il 45% di token in meno sulle modifiche ripetute; altrove è identico alle chiamate classiche. Claude Code, Codex e OpenCode usano gli script solo con Code Mode su Sempre. Solo quota degli abbonamenti e modelli locali, nessuna spesa API. [Risultati](reports/bench/2026-11-22/summary.md) · [Automatico](reports/bench/2026-11-23/summary.md)

## C'è anche

- **Memoria** — le decisioni di progetto finiscono in `LUMEN.md`, che ogni agente legge; le preferenze valide per tutti i progetti si salvano solo dopo la tua conferma.
- **Annulla** — checkpoint prima delle modifiche rischiose, registro delle azioni per ogni task e rollback che si rifiuta di partire quando non sarebbe sicuro.
- **Moli** — un browser integrato condiviso da tutti gli agenti per ricerche e compiti web.
- **Attività ricorrenti e ricette** — "ogni lunedì alle 9 riassumi i TODO"; salva una buona risposta come ricetta riutilizzabile.
- **Il lavoro continua a finestra chiusa** — un servizio locale in background porta avanti i task anche dopo che chiudi Lumen.

Il resto è nella [guida alle funzioni](docs/features.md).

## Scarica

Prendi l'ultima versione da **[Releases](https://github.com/Andrea17982/Lumen/releases/latest)**.

| Piattaforma | File |
| --- | --- |
| Windows | `…_x64-setup.exe`, oppure lo ZIP portabile `…_x64-portable.zip` (estrai tutto lo ZIP) |
| macOS | `.dmg` universale |
| Linux | `.deb` o `.AppImage` |

I pacchetti non sono firmati; controllali con `SHA256SUMS.txt` della release.

1. Installa la CLI ufficiale di almeno un provider (oppure avvia Ollama).
2. In **Impostazioni → Account** aggiungi i tuoi account e accedi nel modo ufficiale.
3. Apri la cartella di un progetto, lascia selezionato **Auto** e scrivi cosa ti serve.

## Compilare dai sorgenti

```sh
git clone https://github.com/Andrea17982/Lumen.git
cd Lumen
npm ci
npm run tauri dev
```

Test e build di release sono in [development](docs/development.md).

## Da sapere

Autopilot dà agli agenti accesso ampio al computer; non è una sandbox del sistema operativo. Lumen non copia mai le credenziali dei provider: ogni CLI fa il login per conto suo. Cosa è stato provato, e cosa no, è in [validazione](docs/validation.md). I dettagli di sicurezza sono in [SECURITY.md](SECURITY.md).

## Come è fatto

Codice, test e documentazione di Lumen sono scritti dall'AI (Claude Code e Codex) sotto la mia direzione. Le decisioni di prodotto sono mie. È una dichiarazione, non un audit di sicurezza.

Grazie a **Theo / pingdotgg, T3 Tools Inc. e ai contributori di T3 Code**: Lumen adatta parti del design di T3 Code. [Fonti e modifiche esatte](src-tauri/resources/legal/t3code/ADAPTATIONS.md) · [licenze](THIRD_PARTY_NOTICES.md). Nessuna approvazione da parte loro è implicita.

Feedback e segnalazioni nelle [Issues](https://github.com/Andrea17982/Lumen/issues) aiutano tantissimo. Se vuoi sostenere il progetto c'è [PayPal](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen); non sblocca nulla in più. — Andrea

MIT © 2026 Andrea17982. Nessuna affiliazione con Anthropic, OpenAI, Google o gli altri provider citati.
