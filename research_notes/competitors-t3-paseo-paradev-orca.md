# T3 Code, Paseo, Paradev e Orca

Verifica documentale del 20 novembre 2026 sulle fonti ufficiali. Le piattaforme e le funzioni riportate sono quelle dichiarate dai progetti. Qualità, latenza, memoria e costi di esecuzione tra le quattro applicazioni restano da misurare con un confronto comune.

## T3 Code

- Il repository di pingdotgg è MIT. Il desktop è distribuito per macOS, Windows e Linux. Sono disponibili client web, iOS e Android, server da CLI e accesso via LAN, Tailscale o T3 Connect. [Repository e installazione](https://github.com/pingdotgg/t3code#install) · [Release ufficiali](https://github.com/pingdotgg/t3code/releases) · [Accesso remoto](https://github.com/pingdotgg/t3code/blob/main/docs/user/remote-access.md)
- I thread possono usare worktree dedicati. Il flusso documentato copre diff, commit, push, PR, review e stato dei check. Il server espone anche MCP per agenti esterni, con autorizzazione e livelli di accesso, e offre comandi CLI per avviare e seguire un thread. [Source control](https://github.com/pingdotgg/t3code/blob/main/docs/user/source-control.md) · [Outside agents](https://github.com/pingdotgg/t3code/blob/main/docs/user/outside-agents.md)

## Paseo

- Il repository principale è **Apache-2.0**. Il daemon serve desktop, web, CLI e app native iOS/Android. Il relay è opt-in sulle nuove installazioni e il canale fra daemon e client è cifrato end-to-end. Il relay self-hosted documenta un listener HTTP/WebSocket locale di default. L'E2EE del payload e il TLS del trasporto sono proprietà distinte. [Licenza](https://github.com/getpaseo/paseo/blob/main/LICENSE) · [Client](https://github.com/getpaseo/paseo) · [Sicurezza](https://paseo.sh/docs/security) · [Relay ufficiale](https://github.com/getpaseo/paseo-relay)
- Gli agenti possono già delegare via MCP o CLI, scegliere provider e modelli configurati, creare worktree e seguire task, anche fra host raggiungibili. I plugin sono codice fidato e senza sandbox. Lato server ereditano l'accesso dell'utente del daemon, lato client girano nell'app. [Orchestrazione](https://paseo.sh/docs/orchestration) · [Provider supportati](https://paseo.sh/docs/supported-providers) · [Plugin](https://paseo.sh/docs/plugins#install-and-try-it)

## Paradev

- Ex Superset, repository desktop MIT. L'annuncio ufficiale del cambio nome riporta un seed da 11,5 milioni di USD. La guida d'installazione indica macOS, AppImage Linux x64 sperimentale e Windows ancora indisponibile. [Repository](https://github.com/paradevai/paradev) · [Annuncio](https://paradev.com/blog/superset-is-now-paradev) · [Installazione](https://docs.paradev.com/install)
- I task possono usare worktree isolati con agenti CLI. **Changes** copre file modificati, staged e non tracciati, diff per file, staging, commit e push. La vista PR include review e check GitHub. Paradev documenta anche il controllo da parte di un agente. [Changes](https://docs.paradev.com/diff-viewer) · [Pull requests](https://docs.paradev.com/pull-requests) · [Controllo da agente](https://docs.paradev.com/recipes/agent-driven-superset)
- Il piano Free comprende desktop, CLI e workspace locali. **Pro** parte da 20 USD per utente al mese, oppure 16 USD al mese con fatturazione annuale, e comprende accesso agli host remoti, automazioni e app mobile iPhone/iPad. Il workspace cloud gestito parte da **Team**. [Prezzi ufficiali](https://paradev.com/pricing) · [Accesso remoto](https://docs.paradev.com/remote-access) · [App mobile](https://docs.paradev.com/mobile)

## Orca

- È un prodotto di **Stably AI**, nome commerciale di **Lovecast Inc.** Il repository è MIT e il desktop Electron è disponibile per macOS, Windows e Linux. Offre app companion iOS e Android, worktree isolati, host remoti via SSH e integrazioni GitHub e Linear. I servizi online hanno flussi di account e autenticazione documentati. [Stably](https://www.stably.ai/) · [Privacy](https://orca.dev/privacy) · [Repository](https://github.com/stablyai/orca) · [Installazione](https://orca.dev/docs/install) · [Integrazioni](https://orca.dev/)
- La CLI consente agli agenti di pilotare worktree, terminali, browser e operazioni del workspace. Orca documenta anche agenti coordinatori e worker. [CLI](https://orca.dev/docs/cli/overview) · [Orchestrazione](https://orca.dev/)

## Lumen oggi

Il branch include Auto, Compete, worktree, checkpoint, diff, check con evidenza, audit e budget API persistente. Nel corpus offline di sei task, Compete ha risolto 5/6 casi e Single 3/6. È una misura interna dei due flussi di Lumen. [Validazione](../docs/validation.md) · [Autonomia](../docs/autonomy.md)

F1–F5 hanno aggiunto il sandbox e il dispatch condiviso, l'opt-in Ollama per modello con fallback dopo due errori di sintassi consecutivi, il contesto e le sottoquery via router, il bridge MCP del turno per Claude Code, Codex e OpenCode, la riga Code nel transcript e le metriche di esecuzione. La copertura dipende dal tipo di adapter e dalle impostazioni. Code Mode è opt-in. [Configurazione e limiti](../docs/api-access.md)

Su Qwen3-Coder in locale, il confronto F2 ha dato 0/5 → 5/5 su A e 5/5 → 0/5 su B, dai tool classici a Code Mode. La descrizione esplicita di `content[].text` ha lasciato B a 0/5 in Code Mode. In E il braccio con RLM abilitato ha ottenuto 5/20, come il classico. Il modello non ha usato `llm()` nelle 20 prove di quel braccio. Uno smoke separato ha verificato la sottochiamata locale. [F2](../reports/bench/2026-11-18/f2-qwen3/summary.md) · [Rerun B](../reports/bench/2026-11-18/f2-mcp-text/summary.md) · [Suite E](../reports/bench/2026-11-18/f3-suite-e/summary.md)

## Prime tre priorità proposte

1. **Git/PR e pannello Changes.** Riutilizzare `gitops.rs`, `review.rs` e la proiezione dei diff. Prima lista per worktree con file staged, unstaged e untracked, diff per file e check relativi alla revisione. Poi staging selettivo e commit locale, con controllo dello stato mostrato prima di scrivere. Infine PR draft su GitHub via `gh` già autenticato, con anteprima di remote, base e head e conferme esplicite per push e apertura. Prova di uscita con repository temporaneo, due worktree, modifica concorrente, remote locale e fixture `gh` per esiti incerti e retry senza duplicati.
2. **Remoto e mobile con TLS obbligatorio.** Prima la PWA React in LAN/VPN prevista dalla 0.13. Esporre dal daemon un elenco ristretto di azioni, con HTTPS/WSS verificati, pairing monouso confermato sul desktop, identità per dispositivo, revoca ed eventi riprendibili. Verificare su telefono con GUI chiusa, perdita di rete, certificato rifiutato, replay e revoca. Dopo la 1.0, riusare lo stesso contratto per il relay self-hosted con connessioni in uscita, TLS obbligatorio su entrambe le tratte ed E2EE fra daemon e client. [Perimetro 1.0](../docs/roadmap-1.0.md#013--controllo-remoto-essenziale)
3. **Lumen pilotabile dagli agenti.** Estendere il bridge F4 `--lumen-code-mcp` e il dispatch comune. Prima offrire tool tipizzati per consultare stato del worktree, diff ed esiti dei check, contando su Moli già esposto nel turno. Poi permettere l'esecuzione di check configurati e, dopo il primo passo Git, la creazione di una worktree isolata attraverso le regole di autonomia esistenti. Opt-in, capability legata a turno e workspace, audit e budget esistenti. Prova di uscita con fixture MCP per Claude Code, Codex e OpenCode, inclusi scope errato, turno scaduto, Read only, rifiuto di approvazione e cancellazione.

Le tre proposte attendono approvazione. Backlog successivo da valutare: worker remoti via SSH, catalogo ACP più ampio, flusso issue → task → PR verificata, terminale integrato.

## Risposta breve per Reddit

Lumen condivide worktree e orchestrazione con altri strumenti. Il progetto punta su routing Auto, continuità tra agenti, permessi e verifica contro i check del progetto. Per il flusso Git/PR, il controllo da telefono e la possibilità di pilotare il workspace da un agente, i repo e le guide qui sopra mostrano già esempi concreti. I benchmark di Lumen misurano i suoi flussi interni.
