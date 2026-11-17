<div align="center">
  <img src="public/lumen.png" width="76" height="76" alt="Lumen">
  <h1>Lumen</h1>
  <p><strong>Il tuo spazio di lavoro AI. Sessioni native. Un browser per gli agenti.</strong></p>
  <p>Claude Code · Codex · Antigravity · OpenCode · Cursor · Ollama · API ufficiali</p>
  <p><a href="https://github.com/Andrea17982/Lumen/releases/latest">Scarica la 0.10.0</a> · <a href="docs/README.md">Documentazione</a> · <a href="README.md">English</a></p>
</div>

![Lumen su Windows](docs/images/desktop.png)

*Schermata dell'app Windows con dati dimostrativi. Le immagini rappresentano l'interfaccia implementata, non un benchmark dei modelli.*

Lumen riunisce le conversazioni AI sul tuo computer. Scegli provider e modello, continua la sessione nativa e lavora nella cartella corretta. Attiva **Orchestra** quando serve una squadra coordinata. **Moli** gestisce navigazione e ricerca web nelle integrazioni supportate.

Tauri 2, core Rust, React e TypeScript. Interfaccia in italiano e inglese, temi chiaro/scuro, conversazioni locali e autenticazione ufficiale dei provider. La 0.10 resta una versione alpha di un progetto indipendente, con limiti espliciti sulle integrazioni e sulle piattaforme verificate.

## 0.10 — Servizio locale, memoria e attività ricorrenti

Il lavoro ora gira in un servizio locale in background: chiudere la finestra non ferma più un turno, una domanda o un'esecuzione Orchestra. Gli assistenti salvano le decisioni di progetto in `LUMEN.md`, propongono preferenze valide per tutti i progetti che confermi tu, ripetono attività su richiesta a parole o con `/every` e salvano le risposte riuscite come ricette. Gli handoff riportano obiettivo, vincoli, lavoro fatto e da fare, controlli e incertezze; Orchestra termina con un risultato scritto da Lumen in base a ciò che ha osservato. [Novità e limiti della 0.10](docs/release-notes/0.10.0.md).

## Verso la 1.0

La [roadmap 1.0](docs/roadmap-1.0.md) privilegia collaborazione affidabile fra Claude Code, Codex, Antigravity e OpenCode, con API ufficiali Anthropic/OpenAI/Google. Auto può usare credito API solo dopo abilitazione esplicita e con budget obbligatorio. Seguono autonomia recuperabile con audit e rollback, poi il controllo remoto essenziale (chat, nuovi task, Stop/Resume e approvazioni in LAN/VPN). Workspace mobile completo e relay E2EE arrivano dopo la 1.0.

## Funzioni dell'applicazione

| Lavorare con continuità | Strumenti vicini al progetto |
| --- | --- |
| Sessioni dirette predefinite, con provider e modello scelti dall'utente. | Moli incluso e configurato automaticamente, anche nei worker Orchestra. |
| Domande native con opzioni, attività e output degli agenti figli. | Inspector, zoom/griglia immagini, JSON/SQLite e canvas dei layout. |
| Autopilot senza ripetere le approvazioni dei piani già autorizzati. | Merge manuale di Both e confronto delle varianti statiche nei worktree. |
| Resume nativo Claude/Codex, memoria del lavoro e cambio esplicito della cartella. | Diagnostica, controlli sui progetti interessati e integrazione Godot opzionali. |
| Il lavoro continua nel servizio locale anche a finestra chiusa. | Pacchetti Windows, macOS universale e Linux. |
| Memoria di progetto, memoria globale confermata, ricette e attività ricorrenti. | Handoff verificabili e risultato del team scritto da Lumen per Orchestra. |

[Changelog 0.10](docs/release-notes/0.10.0.md) · [Guida alle funzioni](docs/features.md) · [Validazione](docs/validation.md).

## Scegli come lavorare

**Sessione diretta.** Seleziona Claude, GPT/Codex, Antigravity, OpenCode, Cursor, Ollama oppure una delle API ufficiali. Account, modello e ragionamento provengono dai cataloghi disponibili. Claude usa lo stream nativo bidirezionale e Codex app-server; i provider Google mantengono le CLI ufficiali. Ollama usa l'API ufficiale sul computer.

**Auto.** Affida a Lumen la selezione da catalogo con preferenze Budget, Default o Quality ed eventuale revisione indipendente. I passaggi aggiuntivi consumano quota.

**Orchestra.** Attiva esplicitamente l'orchestratore per attività più grandi: dipendenze, assegnazioni, worker in worktree Git separati, limiti, integrazione e recupero dei lavori interrotti. Autopilot e l'esecuzione già approvata non producono un altro ciclo di autorizzazione del piano.

**Both.** Confronta Claude e Codex affiancati in sola lettura. I risultati conservati nei worktree possono essere ispezionati e uniti con scelte per file e modifiche manuali dove applicabile.

![Piano e assegnazioni Orchestra](docs/images/orchestra-plan.png)

## Navigazione con Moli

Moli condiviso raggiunge anche OpenCode tramite ACP e le tre API ufficiali. Cursor conserva il proprio runtime nativo senza questa integrazione. [Matrice di supporto](docs/integrations.md).

Moli si avvia quando viene chiamato uno strumento del browser. Offre lettura Markdown, ricerca, navigazione persistente, interazioni DOM e screenshot. I contesti sono separati per conversazione e worker.

Con Ollama, Lumen riceve le chiamate agli strumenti dall'API locale, esegue Moli e restituisce gli esiti allo stesso modello. È disponibile un formato JSON alternativo per modelli conversazionali senza function calling nativo. Nessuna chiave e nessuna inferenza Ollama remota.

Per evitare un blocco osservato nella CLI Antigravity con le immagini MCP inline, questa riceve il percorso del PNG originale per il proprio visualizzatore nativo; Lumen mostra l'immagine completa.

![Screenshot Moli nella conversazione](docs/images/moli-browser.png)

Moli non rende un modello automaticamente più intelligente e non garantisce accelerazioni esponenziali. CAPTCHA, compatibilità dei siti e limiti di rendering restano reali. [Architettura, misure e limiti](docs/moli.md).

## Dal contesto alla verifica

![Inspector e canvas](docs/images/inspector-canvas.png)

L'inspector affianca alla chat controlli di progetto, modifiche, anteprime locali, immagini e canvas dei layout. I controlli Godot sono opzionali e verificano il progetto connesso; gli addon acquistati non sono distribuiti con Lumen.

Il registro persistente del lavoro conserva requisiti e decisioni attraverso troncamento del contesto, Compact e Rewind. Le sessioni native inviano la nuova richiesta senza ripetere inutilmente tutta la conversazione. Le stime locali dei token restano distinte dai dati riportati dal provider.

## Scarica e avvia

I pacchetti sono in **[Releases](https://github.com/Andrea17982/Lumen/releases/latest)**:

- **Windows x64:** `Lumen_0.10.0_x64-setup.exe`, oppure `Lumen_0.10.0_x64-portable.zip`. Estrai **tutto** lo ZIP e conserva `resources` e `lumen-daemon.exe` accanto a `Lumen.exe`.
- **macOS:** DMG universale per Intel e Apple silicon.
- **Linux x64:** DEB e AppImage, compilati su Ubuntu 24.04.

I pacchetti non sono firmati. Il rilascio include `SHA256SUMS.txt`. Installa separatamente le CLI ufficiali dei provider oppure Ollama con un modello locale; Moli è già incluso.

1. Apri **Settings → Accounts**, aggiungi un account e completa il login ufficiale. Ollama non richiede login.
2. Scegli la cartella del progetto e il provider diretto. Espandi i controlli per modello e ragionamento.
3. Seleziona i permessi e invia la richiesta. **Read only** è il permesso iniziale; **Autopilot** autorizza gli strumenti disponibili con il tuo account del sistema operativo.
4. Attiva Orchestra separatamente quando serve una squadra.

[Avvio e risoluzione dei problemi](docs/getting-started.md) · [Account](docs/accounts.md) · [Cartelle e permessi](docs/workspace-models-permissions.md)

## Compilazione

Servono Node.js 22.12+ o una versione supportata di Node 24, Rust stable e i [prerequisiti Tauri](https://v2.tauri.app/start/prerequisites/).

```sh
git clone https://github.com/Andrea17982/Lumen.git
cd Lumen
npm ci
npm run tauri dev
```

La build prepara automaticamente Moli con versione e checksum fissati. `npm run dev` apre una sola anteprima UI, senza login o inferenza. Su Windows `npm run build:release` genera installer e ZIP in `builds/latest/`. [Guida per sviluppatori](docs/development.md).

## Verifiche e confini

La 0.10.0 ha superato **239 test Rust, 76 frontend e 44 browser**, Clippy, formattazione e controlli di versioni/documentazione, più suite native Windows che usano la GUI e il servizio locale reali con fixture offline dei provider. Un controllo live con Claude Sonnet ha riconosciuto 6/6 richieste di pianificazione/memoria e 3/3 casi di memoria di progetto o globale. La CI copre Windows, macOS e Linux; solo Windows ha un collaudo desktop interattivo. [Validazione e limiti noti](docs/validation.md) · [Supporto integrazioni](docs/integrations.md) · [Roadmap 1.0](docs/roadmap-1.0.md).

Autopilot concede accesso ampio. Lumen non aggiunge una gabbia OS comune e non cifra internamente il database; usa permessi del sistema e cifratura del disco. Non sostituisce le politiche dei provider. [Sicurezza](SECURITY.md).

## Completamente AI Written

Codice, interfaccia, test e documentazione sono **completamente AI Written**, generati da AI sotto la direzione di Andrea. Visione e decisioni sono umane. È una dichiarazione di provenienza, non un audit indipendente.

> Grazie a chi vorrà provare Lumen e raccontare come si trova. Feedback, segnalazioni e idee sono benvenuti! — Andrea

Licenza MIT. Per il feedback usa le [Issues](https://github.com/Andrea17982/Lumen/issues). Il [sostegno PayPal facoltativo](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen) va a **andreanastrobusiness1999@gmail.com** e non sblocca funzioni.

[Documentazione](docs/README.md) · [Roadmap 1.0](docs/roadmap-1.0.md) · [Architettura](docs/architecture.md) · [Contribuire](CONTRIBUTING.md) · [Licenze terze parti](THIRD_PARTY_NOTICES.md)
