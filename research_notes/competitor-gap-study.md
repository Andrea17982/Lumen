# Studio competitivo: T3 Code, Paseo, Orca, Paradev

Studio del **21 novembre 2026** su codice e documentazione clonati in `M:\Personal Projects\Lumen-competitors\` (snapshot shallow), più le issue aperte più votate di ciascun repository come segnale dei punti deboli. Integra la [verifica documentale del 20 novembre](competitors-t3-paseo-paradev-orca.md) e lo [studio T3 precedente](Lumen%20e%20T3%20Code/t3-product.md). Nessuna delle quattro app è stata installata o eseguita: qualità e prestazioni reali restano da misurare.

| Progetto | Commit | Licenza | Stack | Piattaforme |
| --- | --- | --- | --- | --- |
| T3 Code | `9168ca73` (20/11) | MIT | TS/Effect, server + web/Electron/Expo | Win/macOS/Linux, web, iOS, Android |
| Paseo | `c9159df` (21/11) | Apache-2.0 | TS, daemon + Expo/Electron | Win/macOS/Linux, web, iOS, Android, CLI |
| Orca | `753a25b9` (20/11) | MIT | Electron, PTY | Win/macOS/Linux, iOS, Android |
| Paradev (ex Superset) | `08db2f0e` (21/11) | MIT (+ cloud Pro) | Electron/Bun, PTY daemon | macOS, Linux sperimentale, **niente Windows** |

## Cosa hanno in comune (e Lumen deve dare per scontato)

Worktree per task, più agenti in parallelo, diff in app, commit/push/PR con un pulsante, browser integrato con anteprima del dev server, daemon che sopravvive alla chiusura della finestra, controllo dal telefono, CLI/MCP con cui gli agenti pilotano l'app, tracciamento delle quote per account. Sono la soglia minima del mercato, non differenziatori.

## Il loro modello: "tu scegli, l'app esegue"

Tutti e quattro lasciano all'utente la scelta di agente, modello, account, worktree, profilo e momento della review. Anche le funzioni "intelligenti" sono **manuali**:

- **Account multipli.** Orca: *hot swap* Codex con un clic, Antigravity a login condiviso da salvare a mano. T3: un `CLAUDE_CONFIG_DIR` per account, **Usage → Limits** somma le quote per provider in un'unica barra e offre **Use reset** sui crediti accumulati. Paradev: meter per account ogni 5 minuti. Nessuno sceglie da solo l'account.
- **Second opinion.** Paseo offre skill `paseo-advisor` e `paseo-committee` che l'utente deve invocare; profili agente salvati (UI work/Planning/Review) da scegliere alla creazione.
- **Orchestrazione.** Orca e Paradev: skill/CLI che l'agente coordinatore usa se glielo chiedi (Orca la tiene dietro *Experimental*). T3: delega Claude→Codex su richiesta.
- **Setup del worktree.** Paseo `paseo.json`, Orca `orca.yaml`, entrambi `.worktreeinclude`; Paradev script setup/teardown: tutti file da scrivere a mano.

Lumen fa già scegliere ad Auto modello, account, forma del team (Single/Review/Parallel/Collaborative/Compete) e risponde da solo alle domande degli agenti. **Questo è il vantaggio da allargare**: ogni funzione presa dai concorrenti deve arrivare in Lumen già decisa da Auto.

## Punti di forza da mutuare

Stato in Lumen verificato su `src/` e `src-tauri/crates/` (branch `codex/code-mode-rlm`). ✅ presente · 🟡 parziale/pianificato · ❌ assente.

| Funzione | Chi | Lumen oggi | Versione automatica per Lumen |
| --- | --- | --- | --- |
| Commit/push/PR, check CI, merge | tutti | 🟡 pannello changes e merge manuale; `gh pr create` classificato *External*; proposta Git/PR in attesa | Lumen crea il branch, committa a verifica superata con messaggio generato e prepara la PR draft. All'utente resta **un solo pulsante "Pubblica"** (azione esterna). |
| Check falliti / commenti PR → fix | Orca, Paradev, T3 | ❌ | Dopo la pubblicazione Lumen osserva CI e review e lancia da solo la correzione nello stesso worktree, entro budget e permessi. |
| Setup worktree (install, `.env`, servizi) | Paseo, Orca, Paradev | ❌ | Rilevare package manager/lockfile e lanciare l'install; copiare automaticamente gli ignorati necessari (`.env*`, config locali) senza file da scrivere. File di progetto solo come override. |
| Porte e anteprima per worktree | Paseo (porte dinamiche, proxy), Paradev | 🟡 anteprima web locale nell'inspector, nessun rilevamento porte | Rilevare le porte aperte dai processi del worktree e aprire da sola l'anteprima giusta; Moli la usa per verificare la UI prima di dire "fatto". |
| Pool quote multi-account | T3, Orca, Paradev | 🟡 Usage per account e balancing | Una sola barra "carburante" per provider; Auto distribuisce il lavoro fra gli account in base ai reset. Dettaglio per account solo in Avanzate. |
| Cambio account a quota esaurita | Orca (manuale) | 🟡 chiede conferma **a ogni tentativo** | Vedi [decisione D1](#decisioni-richieste). |
| Import progetti dalla cronologia CLI | T3 Welcome | ❌ (onboarding pianificato) | Primo avvio: Lumen trova CLI, account e progetti da cronologia Claude/Codex/OpenCode e li propone già pronti. |
| Installazione e login CLI gestiti | T3 (Codex, Antigravity) | ❌ | Installare la CLI mancante e aprire il login ufficiale dal Welcome, senza terminale. |
| Notifiche telefono / Live Activity | T3, Orca, Paradev | 🟡 0.13 remoto essenziale | Pairing con **QR code** (Paseo: "nessuna VPN da capire"), notifica solo quando serve l'utente. |
| Issue → task (GitHub/Linear) | Orca, Paradev, T3 | ❌ | Lumen legge le issue del repo e propone "posso risolvere queste 3": un tap avvia il task fino alla PR verificata. |
| Trigger ed eventi (PR aperta, CI fallita, Slack) | Paseo Hub, Orca Bots, Paradev Pro | 🟡 `/every` e routine nella stessa chat | Estendere le routine a eventi Git locali/`gh` senza cloud Lumen; bot Slack dopo la 1.0. |
| Heartbeat sullo stesso agente | Paseo | ✅ `/every` nella stessa chat | — |
| Advisor/committee, best-of-N | Paseo, Orca, Paradev | ✅ Review, Compete, Collaborative automatici | Già superiore: resta automatico. |
| Handoff fra provider | T3, Paseo | ✅ handoff verificabile con corpus | — |
| Memoria di progetto | Orca `SOUL.md`/`MEMORY.md` | ✅ `LUMEN.md` automatico con fonti | — |
| Attribuzione righe AI/umano | Orca | ❌ | Lumen conosce già le scritture per turno (checkpoint): marcare le righe AI nel diff e far controllare per prime quelle a review. |
| Note sul diff inviate in blocco | Orca, Paradev | ❌ | Commento su una riga → Lumen raccoglie le note e le manda come un unico turno di correzione. |
| Design mode (clic su elemento → agente) | Orca, Paradev | 🟡 Moli + layout canvas, nessun picker | Picker in anteprima: l'elemento selezionato (HTML, stile, selettore, screenshot) entra nel composer. |
| Risposte HTML (grafici, mockup) | T3 | ❌ | Render sandboxato nel transcript per report, tabelle, confronti di Compete. |
| Dettatura | T3, Paseo | ❌ (backlog 1.x) | Trascrizione locale via Ollama/Whisper, nessun cloud. |
| Emulatore iOS/Android, computer use | T3 Devices, Orca, Paradev | 🟡 cattura schermo/finestra e Godot, nessun controllo | Dopo la 1.0: controllo app desktop/emulatore per verifica visiva, nei permessi Autopilot. |
| Progetti multi-repo | T3 | ❌ | Richiesta n. 1 di Orca (66 👍) e Paradev. Un progetto Lumen con più cartelle e un diff unico. |
| Sandbox Docker per agenti | — (richiesta in Orca 30 👍, Paradev) | ❌ (sandbox solo per Code Mode) | Opzione "esegui isolato" scelta da Auto per task rischiosi o repo sconosciuti. |
| Ibernazione agenti inattivi | Orca | n/a | Lumen non tiene PTY vivi; serve solo misurare RAM su sessioni lunghe. |
| Guest link / Pages commentabili | T3, Paradev | ❌ | Dopo la 1.0, sopra il relay E2EE. |
| Autodiagnosi ("triage") | T3, Paseo `paseo-help` | ❌ | Pulsante unico **Ripara Lumen**: un agente legge log e stato locale, propone e applica la correzione o prepara il bug report. |

## Punti deboli da sfruttare

Dalle issue aperte più votate il 21 novembre:

- **Tempesta di permessi.** T3: Auto di OpenCode ignora i pattern dell'utente e chiede per ogni bash/MCP (24 👍); Codex Full access chiede comunque per `rm` (19 👍); Claude Auto esegue comandi che dovevano chiedere (7 👍). Paseo: `dangerously-skip-permissions` sempre attivo. → Lumen deve **provare** in fixture che ogni profilo rispetta le regole dell'utente nei due sensi: niente domande superflue, nessuna esecuzione che doveva chiedere.
- **Checkpoint lenti su repo grandi.** T3: timeout Git di 30 s, non converge e lascia oggetti temporanei (9 👍, 28 commenti). → Misurare lo snapshot di Rewind su un repo grande prima che diventi un problema.
- **Windows trascurato.** Paradev non supporta Windows; Paseo ha bug di avvio (porta riservata `EACCES`) e path `"/D:/..."`. → Windows è il target primario di Lumen: dirlo nel sito della 1.0.
- **Account e login obbligatori.** Paradev chiede il sign-in anche in locale (11 👍 per toglierlo), mobile e automazioni a pagamento. T3 Connect richiede account per le notifiche. → Lumen resta senza account, offline e con remoto gratuito.
- **Complessità di configurazione.** File YAML/JSON per worktree, profili, trigger e plugin. Orchestrazione Orca sotto *Experimental*. → In Lumen nulla di questo deve essere necessario.
- **Mancanze strutturali.** Multi-repo (Orca 66 👍, Paradev), finestre multiple (Orca 65 👍), LSP nell'editor (Orca 46+44 👍), nome di branch/workspace alla creazione (Paradev 16+11+10 👍). → Lumen genera da solo nomi di branch leggibili; multi-repo in roadmap.
- **Affidabilità del remoto.** Paseo: comandi in coda dal mobile eseguiti solo riaprendo l'app; T3: pairing LAN solo `http://` rifiutato. → Il gate remoto della 0.13 copre già replay e riconnessione: aggiungere questi due casi alle fixture.

## Decisioni richieste

- **D1 — Cambio account automatico.** La visione "Lumen gestisce da solo le subscription" contrasta con due regole attuali: in [PRODUCT.md](../PRODUCT.md) ogni fallback chiede prima dell'invio, e la [roadmap](../docs/roadmap-1.0.md#regole-di-esecuzione-e-accettazione) vieta la "rotazione di account per aggirare limiti". Proposta: un consenso unico all'onboarding ("Lumen può usare tutti gli account collegati") e passaggio silenzioso, prima di lavoro osservato, a un account o provider con modello **pari o superiore**. Con soli modelli più deboli, Lumen programma la ripresa al reset invece di degradare. **Deciso il 21 novembre:** il passaggio include anche più account dello **stesso provider**, come in T3 e Orca. Il rispetto dei termini di ciascun provider resta responsabilità dell'utente, avvisato al momento del consenso; Lumen continua a usare solo le CLI ufficiali, senza proxy né estrazione di token.
- **D2 — Pubblicazione automatica.** Push e PR sono azioni esterne. Proposta: un solo pulsante "Pubblica" per task, mai automatico; dopo la pubblicazione il ciclo CI/review → fix è automatico.
