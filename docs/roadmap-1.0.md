# Roadmap 1.0 — All your agents. One seamless workflow.

Revisione del **16 novembre 2026**, basata sulle decisioni del proprietario raccolte in questa sessione e sulla [chat strategica condivisa](https://chatgpt.com/share/e/6af6d485-2b1c-8335-9a07-2fe523f8a7a4), letta dopo il suo login. Sostituisce la precedente revisione del 13 novembre e le roadmap 0.8/successiva alla 0.8. Questa è l'unica roadmap attiva; la storia pubblicata resta nel [tag v0.8.0](https://github.com/Andrea17982/Lumen/tree/v0.8.0/docs) e nel commit `05f4066`.

Il blueprint condiviso è una proposta, non un elenco di funzionalità già disponibili. Le decisioni esplicite qui sotto prevalgono sui suoi esempi: Gemini CLI resta ritirata, non si introducono benchmark live né promesse di calendario. Lumen è sviluppato principalmente dal proprietario con assistenza AI; ogni milestone si chiude sui criteri di uscita, non su una data.

## Visione e priorità

Lumen deve essere il miglior orchestratore di agenti AI per developer, accessibile anche ai principianti. L'utente apre un progetto, descrive l'obiettivo e vede un solo lavoro comprensibile; scelta degli agenti, contesto, verifiche e recupero restano dietro le quinte, ispezionabili quando servono.

La priorità della 1.0 è **affidabilità e collaborazione fra agenti**. Non il numero di provider o pannelli. Un passaggio Claude → Codex → Antigravity/OpenCode deve conservare obiettivo, vincoli e lavoro svolto; una richiesta di build deve produrre un artefatto verificato o un errore esplicito, non soltanto una risposta che dichiara il task completato.

Il principio guida resta **“it just works”**: UI minimale in stile Apple, poche opzioni ragionate, strumenti contestuali e nessuna configurazione manuale di DAG, worktree o handoff per i flussi ordinari. Compatibilità basata su capacità reali, senza promettere equivalenza totale dove l'interfaccia ufficiale non la consente.

## Decisioni confermate

| Area | Perimetro della 1.0 |
| --- | --- |
| Pubblico | Developer, con onboarding comprensibile anche ai principianti. Nessuna trasformazione in app generalista. |
| Agenti fondamentali | **Claude Code, Codex, Antigravity e OpenCode**: flussi di sessione, strumenti, permessi, ripresa e handoff verificati. Ripresa nativa soltanto dove ufficialmente disponibile; le alternative e i limiti devono essere espliciti e non duplicare lavoro. |
| Altri adapter | Cursor CLI e Ollama restano supportati con capacità dichiarate; non è richiesta la stessa garanzia completa del gruppo fondamentale. Un adapter iniziale non equivale alla parità nativa. |
| API ufficiali | **Anthropic, OpenAI e Google obbligatorie prima della 1.0**. Chiavi dell'utente nel keychain OS; isolamento dalle CLI in abbonamento; uso/costo per turno trasparente. |
| API in Auto | Soltanto dopo **abilitazione esplicita e budget obbligatorio**. Registrare una chiave non autorizza automaticamente Auto a consumare credito. |
| Collaborazione | Auto può avviare review, parallelismo e collaborazione **automaticamente entro limiti impostati**. Il provider/modello fissato dall'utente resta un vincolo; Orchestra conserva il controllo esplicito sul lavoro coordinato. |
| Memoria | Memoria **di progetto automatica**; memoria **globale soltanto con conferma**. Entrambe ispezionabili, correggibili e cancellabili, con fonti e isolamento fra progetti. |
| Autonomia | Elevata entro permessi e limiti, con audit e recupero. Operazioni irreversibili o esterne non ereditano autorizzazione dal solo fatto che Auto possa delegare. |
| Remoto | **Controllo essenziale**: chat, nuovi task, Stop/Resume e risposte ad approvazioni/domande. Accesso a conversazioni e stato del lavoro necessario a queste azioni. |
| Connessione | **LAN o VPN dell'utente**, per esempio Tailscale. Nessuna esposizione di rete di default. Relay E2EE dopo la 1.0. |
| Mobile | Frontend web/PWA responsive per il controllo essenziale. Workspace mobile quasi completo e app native dopo la 1.0. |
| Privacy | Core locale, servizi cloud facoltativi, nessun cloud Lumen obbligatorio. Database locale non cifrato da Lumen; protezione affidata all'OS e alla cifratura del disco. |
| Distribuzione | Profili preconfigurati e installazione personalizzabile; aggiornamento integrato con firme degli aggiornamenti. Code signing OS escluso. |
| Sicurezza | Hardening mirato in ogni milestone e **gate formale sulla superficie remota**. Una VPN non sostituisce autenticazione, pairing e revoca. |
| Validazione | Fixture, harness nativi e uso quotidiano. Niente benchmark live fra modelli o contro concorrenti; piccoli smoke reali soltanto con autorizzazione e budget per la release. |
| Monetizzazione | Core libero e open source. Perk software in abbonamento registrati per il futuro, non implementati nella 1.0; nessuna funzione fondamentale dietro un pagamento. |

Il ridimensionamento del remoto è una scelta esplicita del 16 novembre: il workspace mobile quasi completo e il relay self-hosted non sono più gate della 1.0. Le tre API ufficiali, invece, restano nel perimetro obbligatorio.

## Regole di esecuzione e accettazione

- **Interfacce ufficiali.** Nessun proxy di inferenza, estrazione di token o rotazione di account per aggirare limiti. Le autorizzazioni dei connettori restano con il profilo proprietario.
- **Capacità dichiarate.** Ripresa, strumenti, immagini, sub-agenti, approvazioni, streaming, annullamento, cambio modello, MCP, skill e hook hanno stato e limiti per adapter. “Non disponibile” e “non implementato” non sono sinonimi di “completato”.
- **Continuità senza replay del lavoro osservato.** Quota, Stop, crash e passaggi di agente conservano output parziale ed effetti noti. Se lo stato è incerto, si ferma o si propone un recupero esplicito; non si promette exactly-once su strumenti esterni opachi.
- **Qualità conservata.** “Prosegui.” eredita difficoltà, reasoning e vincoli. Un modello più debole richiede consenso; una working tree già modificata non giustifica da sola il rifiuto di lavorare.
- **Completamento reale.** Modifiche, test e build richiesti sono collegati a esiti degli strumenti e artefatti. Se una verifica non è eseguita o fallisce, il risultato lo dice e il lavoro mancante rimane visibile.
- **Quote e spesa.** Distinguere uso riportato, stime e valori non disponibili. I costi API calcolati sono etichettati come stime, non fatture. Mancanza di dati affidabili per applicare il budget impedisce invii API autonomi.
- **Più agenti quando servono.** Regole deterministiche e segnali misurabili prima di un ulteriore LLM-router; nessuna delega per il solo effetto dimostrativo. Review e worker non possono superare i permessi del task.
- **Un grande cambiamento alla volta.** Adapter → core/daemon → contesto/collaborazione → autonomia → remoto essenziale → beta. Riutilizzare i componenti esistenti e misurare prima di ottimizzare.

## Milestone e criteri di uscita

### 0.9 — Compatibilità e accessi ufficiali

**Completata il 16 novembre: gate funzionali e pacchetti Windows 0.9.0 verificati.** [Changelog e limiti](release-notes/0.9.0.md) · [Prove](data/compatibility-0.9.json) · [Matrice delle integrazioni](integrations.md).

- Contratto versionato per nove adapter: capacità, eventi normalizzati, permessi, errori, uso/costi e fallback, con motivazioni visibili per ciò che manca.
- Corpus offline comune passato per Claude Code, Codex, Antigravity e OpenCode: strumenti, permessi, errori/quota, cancellazione e continuità nei rispettivi harness. OpenCode usa ACP ufficiale e policy di sessione verificate prima del prompt; test separati sul runtime installato confermano restrizioni, approvazioni reali, MCP e ripresa senza effetti duplicati.
- API Anthropic/OpenAI/Google implementate con keychain, strumenti comuni, quattro profili, Stop, uso/stime di costo, opt-in Auto e prenotazioni persistenti/concorrenziali del budget prima di ogni richiesta.
- Audit di connettori, hook/comandi/mods, trasporti legacy, OAuth/helper e filtri nativi aggiornato. Le capacità proprietarie non portabili restano limiti dichiarati, senza copia di credenziali o falsa equivalenza. Cursor mantiene Autopilot esplicito; le API sono bufferizzate e non worker Orchestra. Questi limiti non sono presentati come parità nativa completata.

**Uscita verificata:** 225 test Rust, 73 frontend, 44 browser e dodici suite native; controllo separato OpenCode 2.2.7 con modello locale deterministico. Le prove API usano keychain/IPC/SQLite reali e fixture HTTP: non certificano chiavi, credito o qualità di account reali. Nessuna inferenza a pagamento o nuovo benchmark live. Packaging e limiti di distribuzione sono registrati nella [validazione](validation.md).

### 0.10 — Core e daemon locale

**Completata il 16 novembre:** core separato, daemon locale autenticato, recupero e pacchetti verificati. [Changelog](release-notes/0.10.0.md) - [Prove](data/local-service-0.10.json) - [Avvio, arresto e aggiornamento](local-service.md).

Separare `lumen-core` dalla GUI: storage, account, esecuzione, routing, permessi e Moli. `lumen-daemon` possiede processi, sessioni e coda; Tauri diventa un client locale autenticato. Il daemon sostituisce la continuità affidata alla tray, si avvia con l'utente e prepara il remoto senza esporre ancora la rete.

Recupero dopo crash con identità dei processi verificata, stato persistito e Resume esplicito quando non è sicuro continuare. Le attese di quota non consumano uno slot di inferenza; la politica di ripresa resta visibile e cancellabile.

**Uscita:** chiudere/riaprire la GUI durante turno, Orchestra o domanda nativa non perde eventi e non duplica lavoro; crash e aggiornamento hanno una procedura verificata; migrazione dei dati 0.8 transazionale; suite precedenti senza regressioni.

### 0.11 — Contesto e collaborazione affidabile

**In corso (17 novembre):** primo tassello della memoria di progetto automatica: ogni agente salva fatti durevoli con `<lumen-remember>`, datati in `LUMEN.md` sotto `## Lumen memory`, ispezionabili e cancellabili, mai globali. Fonti per sessione, memoria globale con conferma ed esportazione restano da fare. [Dettagli](task-quality.md#project-memory). Aggiunte anche le attività ricorrenti `/every`, eseguite dal daemon ([dettagli](task-quality.md#recurring-tasks)) e le ricette salvate come skill condivise ([dettagli](task-quality.md#recipes)).

- Context Engine strutturato a tre livelli: sessione, progetto, globale. Decisioni di progetto salvate automaticamente con fonti; passaggio alla memoria globale solo dopo conferma. Ispezione, correzione, cancellazione ed esportazione leggibile, per esempio `LUMEN.md`.
- Handoff verificabile: obiettivo, vincoli, decisioni, file rilevanti, stato Git, lavoro svolto/mancante e incertezze. Non fingere di trasferire la memoria privata dell'agente precedente.
- Auto sceglie internamente Single, Review, Parallel o Collaborative con regole deterministiche, entro limiti di tempo, concorrenza, turni/consumo e spesa API. L'abilitazione API e il suo budget restano separati dal semplice accesso alle CLI.
- Una sola attività visibile e un risultato unificato: implementazione, review indipendente, verifiche, artefatti e rischi residui; dettagli degli agenti su richiesta. Gestione di dipendenze e conflitti senza worktree manuali nel flusso ordinario.
- Ricerca residua sul grafo e allocazione globale del contesto soltanto con misure prima/dopo sul corpus disponibile.

**Uscita:** un corpus di handoff fra i quattro agenti fondamentali e API conserva decisioni e vincoli; la memoria non attraversa progetti estranei; regole di Auto e limiti hanno test deterministici; task multi-agente e task singoli riportano correttamente ciò che è stato verificato e ciò che manca.

### 0.12 — Autonomia recuperabile

Classificare azioni reversibili, rischiose/irreversibili ed esterne; audit log per task e checkpoint/rollback comprendenti file non tracciati. Dichiarare i limiti su database, servizi remoti e file esterni al progetto. Tempo, turni, risorse e spesa API sono limitabili; arresto e ripresa non aggirano i limiti.

Hardening su comandi distruttivi, prompt injection da pagine Moli/output strumenti, confini IPC/MCP e dipendenze. Il salvataggio automatico della memoria di progetto non trasforma contenuti esterni non attendibili in istruzioni approvate.

**Uscita:** fixture di injection, comandi distruttivi, crash e budget esaurito verificano conferme, log, arresto e recupero; il rollback dichiarato riporta il progetto allo stato del checkpoint nei limiti supportati. Nessuna dichiarazione di build riuscita senza artefatto e controlli richiesti.

### 0.13 — Controllo remoto essenziale

Frontend web/PWA responsive servito dal daemon, riusando React: elenco conversazioni/progetti necessario alla navigazione, chat, nuovi task, stato del lavoro, Stop/Resume, domande e approvazioni. Il PC resta l'host di esecuzione. Niente workspace mobile completo, gestione avanzata di file/diff, inspector, canvas o Godot nel gate remoto della 1.0.

Connessione in LAN o tramite VPN dell'utente; pairing con conferma sul desktop. Nessun relay nella 1.0 e nessuna app mobile nativa. Le notifiche nello stato attivo del client sono parte del controllo; Web Push/background e i relativi limiti iOS appartengono al successivo ampliamento mobile.

**Gate di sicurezza remoto obbligatorio:** threat model, autenticazione del canale, identità dei dispositivi, pairing robusto, scadenza/revoca delle sessioni, rate limiting e log. Nessuna porta di rete aperta di default. Il client remoto non può superare il profilo deciso sul desktop; Autopilot remoto richiede abilitazione esplicita. Test di pairing debole, replay, furto del dispositivo e revoca; nessun finding high/critical aperto.

**Uscita:** con GUI desktop chiusa, un telefono in LAN e via VPN può avviare un task, seguire la chat, rispondere a una domanda, fermare/riprendere il lavoro. Eventi e comandi non vengono duplicati dopo una riconnessione; il gate remoto passa.

### 1.0 beta → 1.0 — Esperienza e qualità di rilascio

- Onboarding: rilevamento delle CLI/account, autenticazione ufficiale, profili preconfigurati (React/TypeScript, Godot, solo locale con Ollama), installazione personalizzabile e primo task senza manuale tecnico.
- UI minimale e accessibile, stato del lavoro comprensibile, diagnostica progressiva; brand satin-glass approvato. Conservare disclosure AI, ringraziamenti ai tester, MIT/open source e supporto economico facoltativo.
- Auto-update con firme degli aggiornamenti, rollback/migrazione documentati, packaging e risorse/licenze verificati. Code signing OS resta escluso.
- Windows x64 target principale; matrice di installazione, flussi fondamentali e limiti di macOS/Linux esplicita. Nessuna compatibilità dedotta dalla sola compilazione.
- Beta su uso quotidiano e corpus riproducibile, accessibilità e procedure di recupero; nessun benchmark live competitivo o fra modelli.

**Uscita:** nessun blocco noto nei flussi fondamentali; requisiti dei quattro agenti, delle tre API, memoria, collaborazione e remoto essenziale soddisfatti e documentati. Pacchetti, aggiornamento, migrazione e recupero verificati sui sistemi dichiarati. Restano separati implementazione, evidenza mancante ed esclusioni concordate.

## Metriche di riuscita

Misurare su corpus e harness riproducibili: task completati correttamente senza interventi manuali superflui; decisioni/vincoli conservati nei passaggi di agente; recupero da crash, quota e riconnessione; correttezza di verifiche e artefatti; passaggi manuali dell'onboarding; latenza/risorse nelle conversazioni lunghe sullo stesso hardware.

Le misure storiche della 0.8 sono baseline dei loro carichi, non promesse nuove. Non si fissano percentuali di miglioramento o limiti economici universali prima dei dati e delle impostazioni dell'utente.

## Dopo la 1.0 — backlog 1.x

- Workspace mobile quasi completo, navigazione/modifica avanzata di file e diff da telefono, inspector e notifiche Web Push/background; valutare app native iOS/Android sui limiti effettivi della PWA.
- Relay E2EE self-hosted, poi eventuale relay gestito da Lumen: entrambi successivi alla 1.0.
- Parità più ampia per Cursor/Ollama, ACP generico e altri agenti; portabilità aggiuntiva di runtime/estensioni oltre i flussi garantiti.
- Dettatura vocale, condivisione delle sessioni e gestione multi-macchina.
- Profili Office/Creator/Research, marketplace aperto di skill ed editor visuale bidirezionale avanzato.
- Perk in abbonamento, senza bloccare funzioni fondamentali. Nessuna automazione autonoma senza supervisione implicita nella 1.0.

## Decisioni tecniche da prendere durante l'implementazione

L'intervista di perimetro è chiusa; questi punti richiedono prove del codice, non altre scelte strategiche preventive: trasporto IPC locale autenticato e recupero eventi; confini del core/daemon; applicazione dei budget con uso/costi mancanti; protocollo di handoff e gestione dei conflitti; threat model e pairing LAN/VPN. Documentare le alternative e scegliere il percorso minimo che passa i criteri di uscita.

[Validazione attuale](validation.md) · [Integrazioni](integrations.md) · [Prodotto](../PRODUCT.md) · [Documentazione](README.md)
