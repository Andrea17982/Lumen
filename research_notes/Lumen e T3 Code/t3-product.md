# T3 Code prodotto e maturità

## Quali capacità esistono oggi e dove sono verificabili?

### Takeaway
T3 Code al HEAD esaminato è già una control plane multipiattaforma e multiagente con remoto, mobile nativo, Git/PR, servizi in background e handoff; sarebbe scorretto presentare daemon, supporto Claude/Codex/Antigravity/OpenCode o delega come differenze esclusive di Lumen. Le prove sono lettura di documentazione e codice, non esecuzione o benchmark comparativo.

### Cited Findings
- Snapshot verificato via clone shallow: HEAD `375343aaf35133f659435554cd01634bab333a7d`, commit 16 novembre 2026 11:33:58 -08:00 (19:33:58 UTC), fix crash mobile per espansione tool-call. [Commit](https://github.com/pingdotgg/t3code/commit/375343aaf35133f659435554cd01634bab333a7d).
- Il README elenca Claude Code, Codex, Cursor, OpenCode, Grok Build, Antigravity, Kiro, Pi, Muse Code, fx e generici agenti ACP; desktop Windows/macOS/Linux, client web, app iOS e Android. Elenca cambio modello nel thread e delega Claude→Codex. [README fissato](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/README.md).
- Il contratto architetturale tiene processi provider, terminali, Git e filesystem sul server; client web/desktop/mobile via RPC autenticato. Il runtime condiviso evita implementazioni divergenti di riconnessione. Adattatori normalizzano gli eventi. [Architettura](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/internals/overview.md).
- Worktree separato per thread e invio dello stesso prompt a più modelli avviano thread/worktree distinti; il pannello Agents segue deleghe, Stop del parent ferma i child. [Thread](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/thread-sidebar.md).
- Source control integra GitHub/GitLab/Forgejo/Gitea/Bitbucket/Azure/GitCafe/Cursor Origin: clone, commit/push/PR, review, commenti, merge e guide di review beta. Diff del turno separa lavoro agente da cambiamenti portati da pull/rebase. [Source control](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/source-control.md).
- Terminale server con scrollback fino a 5.000 righe/8 MiB, persistito e ripreso alla riconnessione; comandi/navigation/copy e touch mobile. [Terminale](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/terminal.md).
- MCP consente anche ad agenti esterni di leggere/lanciare/messaggiare/fermare thread, con OAuth e scope scelto. Alcuni tool (subagent/worktree handoff/browser) sono solo per agenti avviati in un thread T3. [Outside agents](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/outside-agents.md).
- Antigravity usa il runtime ACP ufficiale, installazione e login gestiti; supporta Gemini, eventuali Claude/GPT secondo account e livelli thinking nativi. Non può riavvolgere la conversazione, quindi T3 non propone rewind/edit-resubmit per questo provider. [Antigravity](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/providers-antigravity.md).
- ACP generico tratta modelli/configurazioni/usage/compaction dinamici, registry o comandi locali literal-args; preferisce ACP v2 preview, negozia v1 quando serve. [ACP](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/providers-acp.md).
- OpenCode supporta 2.0.18+, server locale o esterno, recupera connessioni locali e sessione; grants “Allow for workspace” sono dichiarati più ampi di un thread; .env richiede approvazione nelle modalità ristrette. [OpenCode](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/providers-opencode.md).

### Inferences
- T3 appare oggi più completo come esperienza quotidiana cross-device e flusso Git/PR integrato. È una valutazione della superficie implementata/documentata, non una prova di affidabilità o ergonomia superiore in ogni scenario.
- Il README “any ACP” non significa parità di capacità per tutti gli agenti: le guide descrivono differenze native e limiti.
- Non usare “multiagent” da solo per differenziare Lumen: distinguere Orchestra/controllo deterministico/validazione specifici da delega e side thread già disponibili in T3.

### Gaps
- Non installato/eseguito T3; nessuna prova reale iOS/Android, tempi/frame/RAM, onboarding o modello/provider di qualità.
- Non dimostrata l'assenza di routing automatico equivalente a Lumen Auto: nelle guide controllate “Auto” indica soprattutto la revisione delle approvazioni; non confondere le due funzioni.

## Recupero, remoto e sicurezza: cosa si può imparare?

### Takeaway
T3 documenta già un percorso completo server/servizio, multi-dispositivo e recovery. Le idee più utili sono trasferibili come vincoli e test, senza importare tutta l'architettura TypeScript/Effect/Electron nel core Rust/Tauri di Lumen.

### Cited Findings
- EventSink committa eventi, proiezioni, command receipts e outbox effects nella stessa transazione; effect worker esegue I/O dopo il commit. ACK indica intento accettato, non completamento del provider; gli effetti di processi persi non si ripetono alla cieca. Eventi conservati circa un giorno, command receipts mantenute per dedup; cursor troppo vecchi richiedono snapshot. [Architettura](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/internals/overview.md).
- Checkpoint usa hidden Git refs senza commit sul branch utente. Revert coordina filesystem e conversazione provider; rifiuta prima di toccare file se il provider non può rollback. [CheckpointStore](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/checkpointing/CheckpointStore.ts), [Restore safety](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/orchestration-v2/CheckpointRestoreSafety.ts).
- “Edit from here”: rewind conversazione solo, o restore file (solo worktree). Restore rifiutato con altri thread/sessioni nello stesso checkout o percorsi sovrapposti; include anche edit dell'utente e non annulla effetti esterni. [Composer](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/composer.md).
- Handoff trasferisce cronologia attribuita, originale/recenti/activity utili e parziali; non ragionamento privato/stato tool/allegati. Ha budget iniziale default16.000, fallback e link di recupero storia omessa; non accorcia il nuovo input dell'utente. [Portable handoffs](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/portable-handoffs.md).
- Modalità Supervised/Auto-accept edits/Auto/Full access, con differenze per provider e limiti espliciti; default iniziale Full access. Auto sfrutta reviewer nativo ove disponibile, altrove chiede. [Permission modes](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/permission-modes.md).
- Servizio utente Linux systemd+linger, macOS LaunchAgent al login, Windows TaskScheduler per utente con restart entro un minuto in caso di exit. CLI install/status/update/repair/uninstall conserva dati; nuova installazione loopback-only, LAN esplicita. [Background service](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/background-service.md).
- Update app/provider/server ha controlli di compatibilità DB/protocollo e rollback in caso di fallimento. “Continue threads after restarts” di default on riavvia thread supportati con resume state, ma non mentre attendono domanda/approvazione/segreto; dopo 3 crash consecutivi serve input utente. [Updating](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/updating.md).
- Remoto LAN/VPN pairing con link monouso, QR, connessioni future per dispositivo; T3 Connect evita port-forward, sign-in account, approvazione nuovi dispositivi opzionale, richieste scadono10min, revoca. Limite gratuito3host, Pro10/Premier100 (telefoni/browser non contano). [Remote access](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/remote-access.md).
- Dati prodotto inviati a ingest.t3.codes/PostHog comprendono account hash, provider/modello/token/durata ma non prompt/risposte/file/tokenautenticazione. Opt-out server via env; trace client web/mobile separati non sono spenti da quello e non hanno proprio toggle relay. Mobile crash-report toggle distinto. [Telemetry](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/docs/user/telemetry.md).

### Inferences
- Per la0.11 di Lumen: mutuare budget di handoff e recuperabilità della cronologia omessa prima di costruire un riassuntore LLM. Conservare originale+vincoli+parziali; aggiungere test di overflow e cambio modello.
- Per la0.12: mutuare soprattutto i safety gate dei checkpoint, non una promessa generale di “undo”. I percorsi sovrapposti e capability native sono edge case concreti.
- Per la0.13: pairing monouso, scadenza/revoca e autorizzazione dispositivi sono base utile; non introdurre T3Connect/cloudflared/account-cloud dato che proprietario ha scelto LAN/VPN e relay post1.0.
- Una posizione Lumen più semplice e senza telemetria potrebbe essere distintiva, se confermata dal ricercatore locale. Non significa che T3 invii contenuti dei progetti nelle normali metriche: la fonte esplicitamente lo nega.
- Non copiare default Fullaccess o autoresumecrash senza valutare profili e semantica conservativa di Lumen.

### Gaps
- Nessuna revisione di sicurezza indipendente, prova di pairing/revoca, race/replay o efficacia contenzioneeffetti in T3 eseguita qui.
- Non provati E2EE, garanzie exactly-once sugli effetti esterni, né “T3 meno sicuro”: non sostenerlo.

## Quanto è maturo e cosa si può affermare sul riuso?

### Takeaway
È un progetto già distribuito con molte integrazioni e contributori, ma versione0.1.x non è da sola una misura di maturità. Separare sempre HEAD studiato da release pubblicata, e fonte di codice da benchmark eseguito.

### Cited Findings
- API GitHub latest: release stabile `v0.1.9`, pubblicata11 novembre2026 11:33:49Z; note dichiarano198modifiche da31contributori, includono sign-inMCP, screenreader, OpenCodegoals, GitLabrevert e fix/performance. [Release](https://github.com/pingdotgg/t3code/releases/tag/v0.1.9).
- README elenca canali downloaddesktop, web, storesmobile, winget/scoop/brew/AUR/Nix/deb e comunitàDiscord, ma segnala capacitàreviewlimitata e richiede discussioneapprovata prima di nuove feature. [README](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/README.md), [Contributing](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/CONTRIBUTING.md).
- LicenzaMIT del repo: copyright2026 T3 ToolsInc, permette copia/modifica/distribuzione con conservazione copyright e permissionnotice nelle copie/parti sostanziali. [LICENSE](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/LICENSE).

### Inferences
- Per codice copiato/adattato: registrare repo+SHA+percorso+origine nei commenti mirati/THIRD_PARTY_NOTICES, distribuire testoMIT, verificarelicenze dei singoli file/deps/asset. Citazione in chat o README da sola non sostituisce notice distribuito. Idee reimplementate da specifiche non equivalgono a copia del codice; distinguere con precisione.
- Non importare interi servizi TS/Effect nel coreRust né migrare Tauri→Electron solo per riusare T3; prendere algoritmi/gate/casi di test e piccoli componenti isolati compatibili.
- Riferimenti HEAD possono descrivere novità successive alla0.1.9: report deve indicarlo esplicitamente.

### Gaps
- Non effettuata analisi completa dipendenze/licenzeasset o audit prestazionale, né confronto A/B T3/Lumen. Nessun codice applicativo copiato o modificato.

### Riscontro codice mirato aggiuntivo
- Letto `CheckpointRestoreSafety.ts`: verifica isolamento sia admission sia rollback; canonicalizza realpath, controlla containership parent/child e thread archiviati/sessioni provider anche errored non stopped; ignora sidechat idle solo se nessun tool/change registrato. Questo dà una proposta concreta per Lumen: portare i gate e fixture di percorsi sovrapposti, non tutta la dependencyEffect. [Fonte](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/orchestration-v2/CheckpointRestoreSafety.ts).
- Letto EventSink contratti+commitThenPublish: serializza commit/pubblicazione, withTransaction, gestione cancellazione e ownershipsessione ricontrollata dentro transazione. [Fonte](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/orchestration-v2/EventSink.ts).
- Anchor codice per ulteriori verifiche senza promesse aggiuntive: [Terminal Manager](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/terminal/Manager.ts), [Pairing grants](https://github.com/pingdotgg/t3code/blob/375343aaf35133f659435554cd01634bab333a7d/apps/server/src/auth/PairingGrantStore.ts). Non eseguiti né revisionati integralmente.
