# Lumen

Un’app desktop locale per gestire conversazioni Claude Code e ChatGPT / Codex in un’interfaccia essenziale ispirata ad Apple Liquid Glass. Tauri 2, core Rust, React e TypeScript. L’interfaccia è in inglese, come scelto dal creatore.

**Versione 0.3 alpha.** [Scarica da Releases](https://github.com/Andrea17982/Lumen/releases) oppure compila il progetto. Progetto indipendente, senza affiliazione con Apple, Anthropic o OpenAI.

![Interfaccia Lumen](docs/images/desktop.png)

## Completamente AI Written

Il codice dell’applicazione, l’interfaccia, i test e la documentazione sono **completamente AI Written**, generati da AI sotto la direzione di Andrea ([Andrea17982](https://github.com/Andrea17982)). La visione e le decisioni di prodotto sono umane. Questa dichiarazione non equivale a un audit di sicurezza indipendente.

> Ringrazio tutti quelli che avranno voglia di provarla e farmi sapere come si trovano. Feedback, segnalazioni e idee sono benvenuti! — Andrea

Lumen è disponibile con licenza MIT: tutti possono scaricarla, utilizzarla e modificarla.

## Novità 0.3: Claude e ChatGPT lavorano in squadra

- **Both**: lo stesso prompt a entrambi, con le risposte affiancate. Con le modifiche abilitate ognuno lavora in un proprio `git worktree` e applichi la versione migliore con un clic.
- **Cross-Audit**: «Audit with…» su ogni risposta, oppure un interruttore sempre attivo. L'altro modello revisiona il diff del turno, per un massimo di 3 cicli.
- **Note di progetto condivise** (`LUMEN.md`) e **file allegati** con il loro peso in token, letti da entrambi a ogni turno.
- **Fallback sui rate limit**: se un provider raggiunge il limite, lo stesso messaggio prosegue sull'altro. Indicatori di stato e conto alla rovescia nell'header.
- **Rewind**: snapshot git invisibili prima di ogni turno che modifica file. Con un'anteprima, riporti file e conversazione a qualsiasi tuo messaggio.
- **Self-healing**: dopo le modifiche Lumen esegue il controllo del progetto e rimanda gli errori all'assistente perché li corregga (al massimo 3 volte).
- **Risparmio token**: stima live dei token, log lunghi compressi agli errori (log completo in `.lumen/spill`), «Compact and continue» per le conversazioni lunghe, lockfile e segreti bloccati.
- **Hub MCP** con interruttori ON/OFF per entrambe le CLI e **API locale compatibile OpenAI** su `localhost:8317/v1`.
- Build **macOS** oltre all'installer e alla versione portable per Windows.

Dettagli in [Teamwork features](docs/features.md).

## Avvio rapido

Installa separatamente i CLI ufficiali di [Claude Code](https://code.claude.com/docs/en/setup) e/o [Codex](https://developers.openai.com/codex/cli). In Lumen apri Settings, aggiungi un account e premi Sign in: l’autenticazione avviene nel terminale e nel browser ufficiali del provider. Verifica la connessione con il pulsante a scudo, scegli una cartella di lavoro fidata e avvia la conversazione.

Per compilare servono Node.js, Rust stable, C++ Build Tools, Windows SDK e WebView2. Dopo `npm run build:release`, l'installer più recente è in `builds/latest/`; l'output originale di Tauri resta in `src-tauri/target/release/bundle/nsis/`.

```powershell
git clone https://github.com/Andrea17982/Lumen.git
cd Lumen
npm ci
npm run tauri dev
```

`npm run build:release` genera l'installer e lo copia in `builds/latest/`. `npm run dev` apre soltanto un'anteprima UI nel browser, senza inferenza AI o login.

## Funzioni e limiti

Sessioni SQLite persistenti, ricerca, pin, rinomina, stati, attività collassabili, account con directory isolate, tema chiaro/scuro/sistema e selettore Auto / Claude / GPT. Auto usa euristiche locali per scegliere provider, modello, effort e, quando sono disponibili entrambi i provider, review indipendente o prospettive parallele con sintesi.

Nessun limite artificiale al numero di sessioni salvate; massimo quattro lavori attivi e otto processi di inferenza. Le scritture nella stessa cartella vengono serializzate. Accesso in sola lettura predefinito. Il contesto viene ripassato al CLI a ogni turno fino a 100.000 caratteri recenti. Codex aggiorna la chat a messaggi completati; Claude trasmette anche testo parziale.

L’inferenza reale, l’isolamento OAuth con più account e le modifiche ai file richiedono test di accettazione con account dell’utente. La compatibilità dipende dalle versioni CLI e dai piani dei provider. Non sono implementati proxy, aggiramenti dei limiti o login OAuth proprietari. Le conversazioni e i file credenziali gestiti dai CLI non vengono cifrati da Lumen. Le prestazioni non sono state oggetto di benchmark.

Leggi il [README completo](README.md), la [documentazione](docs/getting-started.md), l’[architettura](docs/architecture.md), le [regole account](docs/accounts.md) e la [roadmap (0.4 e oltre)](docs/roadmap.md).

## Donazioni facoltative

Se vuoi sostenere il progetto: [Dona con PayPal](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen), destinatario **andreanastrobusiness1999@gmail.com**. Nessuna funzione a pagamento e nessun obbligo di donare. Per il feedback usa le [Issues](https://github.com/Andrea17982/Lumen/issues).
