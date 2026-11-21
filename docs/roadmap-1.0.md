# Roadmap 1.0 — All your agents. One seamless workflow.

Revisione del **21 novembre 2026**, basata sulle decisioni del proprietario, sulla [chat strategica condivisa](https://chatgpt.com/share/e/6af6d485-2b1c-8335-9a07-2fe523f8a7a4) e sullo [studio competitivo](../research_notes/competitor-gap-study.md). È l'unica roadmap attiva.

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
- **Un grande cambiamento alla volta.** Adapter → core/daemon → contesto/collaborazione → autonomia → flusso Git automatico → remoto essenziale → beta. Riutilizzare i componenti esistenti e misurare prima di ottimizzare.

## Milestone e criteri di uscita

### Completate

- **0.9 — Compatibilità e accessi ufficiali.** Contratto versionato per nove adapter; corpus offline comune per Claude Code, Codex, Antigravity e OpenCode (ACP ufficiale); API Anthropic/OpenAI/Google con keychain, strumenti comuni, quattro profili e budget prenotati. Limiti dichiarati nella [matrice delle integrazioni](integrations.md).
- **0.10 — Core e daemon locale.** `lumen-core` separato dalla GUI; `lumen-daemon` autenticato possiede processi, sessioni e coda; recupero dopo crash senza reinvio silenzioso; ricevute dei comandi. Nella stessa versione sono arrivati i primi tasselli della 0.11 (sotto). [Changelog](https://github.com/Andrea17982/Lumen/releases/tag/v0.10.0) · [Servizio locale](local-service.md).
- **0.11 — Contesto e collaborazione affidabile.** Memoria a tre livelli con fonti, handoff verificabile e corpus offline fra i quattro agenti e le API; Auto sceglie Single, Review, Parallel, Collaborative o Compete (solo profilo Qualità) e risponde da solo alle domande degli agenti; inbox delle approvazioni dei worker; plugin con permessi dichiarati. La ricerca sul grafo resta aperta e va fatta con misure prima/dopo. Nella stessa versione sono arrivati i primi tasselli della 0.12. [Changelog](release-notes/0.11.0.md).
- **0.12 — Code Mode sperimentale.** Uscita fuori sequenza rispetto alla milestone qui sotto: `lumen_code` opt-in per API ufficiali, Ollama (per modello) e Claude Code/Codex/OpenCode via MCP, contesto locale al turno e sottoquery `llm()` con limiti per effort, righe **Code** e contatori nell'esecuzione. Resta disattivato di default: nessun modello misurato supera il gate di qualità; mancano misure su API a pagamento, CLI e app pacchettizzata. [Changelog](release-notes/0.12.0.md).

### Autonomia recuperabile (milestone 0.12, ancora aperta)

**Stato:** consegnati nella 0.11 classificazione delle azioni, audit log per conversazione, controlli di sicurezza su Rewind con snapshot prima del ripristino, snapshot byte-exact e memoria trattenuta dopo contenuti non attendibili ([limiti](autonomy.md)). Restano limiti di turni/risorse oltre a tempo e budget, fixture complete di crash e budget esaurito, e hardening dei confini IPC/MCP.

Classificare azioni reversibili, rischiose/irreversibili ed esterne; audit log per task e checkpoint/rollback comprendenti file non tracciati. Dichiarare i limiti su database, servizi remoti e file esterni al progetto. Tempo, turni, risorse e spesa API sono limitabili; arresto e ripresa non aggirano i limiti.

Hardening su comandi distruttivi, prompt injection da pagine Moli/output strumenti, confini IPC/MCP e dipendenze. Il salvataggio automatico della memoria di progetto non trasforma contenuti esterni non attendibili in istruzioni approvate.

**Dallo [studio competitivo](../research_notes/competitor-gap-study.md#punti-deboli-da-sfruttare):** i bug più votati di T3 sono permessi ignorati in entrambi i sensi (domande per ogni comando, oppure esecuzione senza la domanda dovuta) e checkpoint che superano il timeout Git sui repository grandi. Fixture per ogni profilo e adapter: nessuna domanda superflua sulle azioni già autorizzate, nessuna esecuzione che doveva chiedere. Misurare snapshot e Rewind su un repository grande.

**Uscita:** fixture di injection, comandi distruttivi, crash e budget esaurito verificano conferme, log, arresto e recupero; il rollback dichiarato riporta il progetto allo stato del checkpoint nei limiti supportati. Nessuna dichiarazione di build riuscita senza artefatto e controlli richiesti.

### 0.12.5 — Dal task alla PR verificata, da solo

Funzioni che T3 Code, Paseo, Orca e Paradev offrono tutte, ma che lì l'utente configura e aziona a mano. In Lumen Auto le decide; l'utente vede il risultato e un solo pulsante.

- **Worktree pronto da solo.** Rilevare package manager e lockfile, eseguire l'install, copiare gli ignorati necessari (`.env*`, config locali) nel worktree e generare un nome di branch leggibile. Nessun file di configurazione richiesto; un file di progetto serve solo come override.
- **Anteprima per worktree.** Rilevare le porte aperte dai processi del worktree e aprire l'anteprima giusta. Moli la usa per verificare la UI prima che il lavoro risulti completato.
- **Commit e PR.** Riutilizzare `gitops.rs`, `review.rs` e la proiezione dei diff. Commit locale automatico a verifica superata, messaggio generato, PR draft preparata via `gh` già autenticato. Push e apertura restano azioni esterne: **un solo pulsante "Pubblica"** con anteprima di remote, base e head.
- **Ciclo dopo la pubblicazione.** Check CI falliti e commenti di review rientrano come turno di correzione nello stesso worktree, entro budget, permessi e limiti di tentativi.
- **Revisione più rapida.** Righe scritte dall'AI marcate nel diff a partire dai checkpoint per turno; note su singole righe inviate all'agente come un unico turno.

**Uscita:** repository temporaneo con due worktree, modifica concorrente, remote locale e fixture `gh` per esiti incerti e retry senza duplicati. Un task "aggiungi una funzione e i test" arriva da prompt a PR draft pronta senza passaggi manuali oltre a "Pubblica"; un check fallito simulato produce un fix e un nuovo push solo dopo conferma.

### 0.13 — Controllo remoto essenziale

Frontend web/PWA responsive servito dal daemon, riusando React: elenco conversazioni/progetti necessario alla navigazione, chat, nuovi task, stato del lavoro, Stop/Resume, domande e approvazioni. Il PC resta l'host di esecuzione. Niente workspace mobile completo, gestione avanzata di file/diff, inspector, canvas o Godot nel gate remoto della 1.0.

Connessione in LAN o tramite VPN dell'utente; pairing con conferma sul desktop. Nessun relay nella 1.0 e nessuna app mobile nativa. Le notifiche nello stato attivo del client sono parte del controllo; Web Push/background e i relativi limiti iOS appartengono al successivo ampliamento mobile.

**Gate di sicurezza remoto obbligatorio:** threat model, autenticazione del canale, identità dei dispositivi, pairing robusto, scadenza/revoca delle sessioni, rate limiting e log. Nessuna porta di rete aperta di default. Il client remoto non può superare il profilo deciso sul desktop; Autopilot remoto richiede abilitazione esplicita. Test di pairing debole, replay, furto del dispositivo e revoca; nessun finding high/critical aperto.

Pairing con **QR code** mostrato dal desktop, come in Paseo: l'utente non deve capire VPN o indirizzi. Fixture aggiuntive tratte dai bug dei concorrenti: comandi accodati dal telefono eseguiti anche con l'app in background, pairing LAN con TLS e non soltanto `http://`.

**Uscita:** con GUI desktop chiusa, un telefono in LAN e via VPN può avviare un task, seguire la chat, rispondere a una domanda, fermare/riprendere il lavoro. Eventi e comandi non vengono duplicati dopo una riconnessione; il gate remoto passa.

### 1.0 beta → 1.0 — Esperienza e qualità di rilascio

- Onboarding: rilevamento delle CLI/account, autenticazione ufficiale, profili preconfigurati (React/TypeScript, Godot, solo locale con Ollama), installazione personalizzabile e primo task senza manuale tecnico. Lumen installa le CLI mancanti e apre il login ufficiale senza terminale, e propone già pronti i progetti trovati nella cronologia di Claude Code, Codex e OpenCode (come il Welcome di T3).
- Quote semplici: una sola barra per provider che somma gli account collegati, con il prossimo reset. Il dettaglio per account resta in Avanzate. Auto distribuisce il lavoro secondo la [decisione D1](../research_notes/competitor-gap-study.md#decisioni-richieste) sul cambio account.
- **Ripara Lumen:** un pulsante che fa leggere a un agente log e stato locale, propone e applica la correzione o prepara una segnalazione senza dati personali.
- UI minimale e accessibile, stato del lavoro comprensibile, diagnostica progressiva; brand satin-glass approvato. Conservare disclosure AI, ringraziamenti ai tester, MIT/open source e supporto economico facoltativo.
- Auto-update con firme degli aggiornamenti, rollback/migrazione documentati, packaging e risorse/licenze verificati. Code signing OS resta escluso.
- Windows x64 target principale; matrice di installazione, flussi fondamentali e limiti di macOS/Linux esplicita. Nessuna compatibilità dedotta dalla sola compilazione.
- Piccolo sito di presentazione per il lancio della 1.0 (cosa fa Auto, download, confronto onesto con le alternative).
- Beta su uso quotidiano e corpus riproducibile, accessibilità e procedure di recupero; nessun benchmark live competitivo o fra modelli.

**Uscita:** nessun blocco noto nei flussi fondamentali; requisiti dei quattro agenti, delle tre API, memoria, collaborazione e remoto essenziale soddisfatti e documentati. Pacchetti, aggiornamento, migrazione e recupero verificati sui sistemi dichiarati. Restano separati implementazione, evidenza mancante ed esclusioni concordate.

## Metriche di riuscita

Misurare su corpus e harness riproducibili: task completati correttamente senza interventi manuali superflui; decisioni/vincoli conservati nei passaggi di agente; recupero da crash, quota e riconnessione; correttezza di verifiche e artefatti; passaggi manuali dell'onboarding; latenza/risorse nelle conversazioni lunghe sullo stesso hardware.

Non si fissano percentuali di miglioramento o limiti economici universali prima dei dati e delle impostazioni dell'utente.

## Dopo la 1.0 — backlog 1.x

- Workspace mobile quasi completo, navigazione/modifica avanzata di file e diff da telefono, inspector e notifiche Web Push/background; valutare app native iOS/Android sui limiti effettivi della PWA.
- Relay E2EE self-hosted, poi eventuale relay gestito da Lumen: entrambi successivi alla 1.0.
- Parità più ampia per Cursor/Ollama, ACP generico e altri agenti; portabilità aggiuntiva di runtime/estensioni oltre i flussi garantiti.
- Dettatura vocale, condivisione delle sessioni e gestione multi-macchina.
- Profili Office/Creator/Research, marketplace aperto di skill ed editor visuale bidirezionale avanzato.
- Perk in abbonamento, senza bloccare funzioni fondamentali. Nessuna automazione autonoma senza supervisione implicita nella 1.0.
- Dallo [studio competitivo](../research_notes/competitor-gap-study.md#punti-di-forza-da-mutuare), in ordine di valore stimato:
  - issue GitHub/Linear → task: Lumen propone quali risolvere e le porta fino alla PR verificata;
  - routine attivate da eventi (PR aperta, check fallito) sopra `/every`, senza cloud Lumen; bot Slack/Discord dopo;
  - progetti multi-repo con un diff unico (richiesta più votata in Orca e Paradev);
  - esecuzione isolata in Docker, scelta da Auto per task rischiosi o repository sconosciuti;
  - design mode in Moli: clic su un elemento dell'anteprima e contesto pronto nel composer;
  - risposte HTML sandboxate nel transcript per report, grafici e confronti di Compete;
  - controllo di app desktop ed emulatori iOS/Android per la verifica visiva, entro Autopilot;
  - guest link e pagine commentabili sopra il relay E2EE.

## Decisioni tecniche da prendere durante l'implementazione

Lo [studio competitivo del 21 novembre](../research_notes/competitor-gap-study.md) assegna il flusso Git/PR alla 0.12.5 e il pairing QR alla 0.13. Restano da approvare la PWA con TLS obbligatorio, i tool per pilotare Lumen attraverso il bridge MCP di Code Mode ([proposta](../research_notes/competitors-t3-paseo-paradev-orca.md#prime-tre-priorità-proposte)) e le decisioni D1 (cambio account automatico) e D2 (pubblicazione).

L'intervista di perimetro è chiusa; questi punti richiedono prove del codice, non altre scelte strategiche preventive: trasporto IPC locale autenticato e recupero eventi; confini del core/daemon; applicazione dei budget con uso/costi mancanti; protocollo di handoff e gestione dei conflitti; threat model e pairing LAN/VPN. Documentare le alternative e scegliere il percorso minimo che passa i criteri di uscita.

[Validazione attuale](validation.md) · [Integrazioni](integrations.md) · [Prodotto](../PRODUCT.md) · [Documentazione](README.md)
