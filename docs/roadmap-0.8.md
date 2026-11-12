# Roadmap 0.8 — misure, qualità e continuità

Stato: **sette tranche pubblicate nella 0.7.1 — Performance Update; ottava sul grafo, nona su Rewind, decima su Compact, undicesima su logo/sidebar, dodicesima sul recupero sessioni e tredicesima sulla quota, quattordicesima su Auto/Orchestra e quindicesima sul contesto implementate nel ramo 0.8; tranche 16 verificata anticipatamente e tranche 17 completata nel perimetro dichiarato**, 12 novembre 2026. La release 0.7.1 resta invariata e questa roadmap resta aperta. Digitazione, apertura e chiusura metadati Windows superano i rispettivi criteri; il corpus mirato degli import passa da 9/23 a 23/23 aspettative. Virtualizzazione con perdita di contenuti esclusa; inferenza/TTFT live, tempi assoluti stabili e semantica completa del grafo non sono dichiarati risolti.

## Direzione approvata nell'intervista dell'11 novembre

- **Uso principale:** coding React/TypeScript/JavaScript e Godot, dalla creazione e architettura di un progetto al lavoro attivo su roadmap, ricerca e rifiniture. Un'unica applicazione aggrega gli abbonamenti; il caso prioritario usa due account Claude e uno ChatGPT.
- **Priorità di qualità:** completare l'esecuzione senza solleciti ingiustificati, migliorare ragionamento/architettura/codice, conservare requisiti e decisioni. La qualità inferiore alle app ufficiali è una segnalazione da diagnosticare, non una differenza già misurata a parità di modello e impostazioni.
- **Contesto:** condiviso tra account nella singola chat; grafo e repocontext forniscono conoscenza mirata del progetto. Nessuna condivisione indiscriminata delle conversazioni di uno stesso progetto.
- **Auto:** un responsabile mantiene il filo e integra. Le sole scelte ordinarie sono Balance quota usage e Budget/Default/Quality. L'algoritmo decide assegnazione, aiuti, delega e cross-audit quando il beneficio giustifica token e attesa, oppure quando richiesti esplicitamente. Non lanciare sempre più agenti sullo stesso problema.
- **Orchestra:** resta esplicita; autorizza orchestrator e distribuzione parallela con piano, dipendenze e worktree quando necessari. L'intervista chiarisce dubbi reali prima di lavorare; non deve ripetere decisioni già risolte. Auto senza Orchestra può comunque richiedere aiuti circoscritti. Questa è una specifica di destinazione: la calibrazione e l'estensione della delega ordinaria devono ancora essere verificate/implementate.
- **Both:** richiesta esplicita di soluzioni da confrontare, distinta dalla collaborazione automatica. Sessioni dirette restano predefinite e conservano le scelte dell'utente.
- **Autonomia e conferme:** lavoro tecnico richiesto → esecuzione e controlli, senza revisione obbligatoria del codice. Scelte visive → proposta o prima/dopo concreto, approvazione del risultato, poi integrazione nella destinazione richiesta e chiusura senza altra autorizzazione dello stesso perimetro. Pubblicazioni/deploy seguono l'autorizzazione del singolo compito.
- **Identità:** minimale Apple Liquid Glass; vetro e trasparenza cruciali, entrambi i temi, leggibilità e pochi controlli. Composizione approvata: vista essenziale con barra flottante; il logo in alto a sinistra è il pulsante che apre/chiude la sidebar di vetro con progetti. Transizione tra le due composizioni con breve scintillio del logo al clic, senza movimento continuo e rispettando movimento ridotto. Nessun redesign generale implicito.
- **Avanzamento:** riepilogo compatto con fase, ultima azione reale, agenti e verifiche; dettagli espandibili. Distinguere attesa provider, strumenti, domande pendenti, errori e silenzio del protocollo; una durata senza eventi non dimostra uno stallo. Recupero automatico solo quando non duplica effetti; mai rilanciare alla cieca operazioni di scrittura.
- **Preset:** nessuno rinuncia alla correttezza; Budget riduce duplicazione/collaborazioni superflue, Quality può spendere di più per verifiche e confronti mirati. Nessuna garanzia di soddisfazione delle animazioni senza prove sul prototipo.

## Ordine di esecuzione e accettazione

| Priorità | Attività e dipendenze | Criterio di accettazione | Stato |
| --- | --- | --- | --- |
| P0 | Bozze e allegati per conversazione; invio e riconciliazione con il backend | Cambiare chat non trasferisce testo/allegati; il messaggio appare subito in invio e il composer si libera; errore recuperabile senza perdita né duplicazione; risposte tardive e Compact non alterano un'altra chat o una nuova bozza | Integrata localmente; 7 scenari Windows debug/offline verificati |
| P0 | Osservabilità dell'esecuzione e analisi degli arresti segnalati | Stati basati su eventi reali e durata visibile; distinguere conclusione, domanda, errore e assenza di eventi; riproduzione controllata prima di modificare recupero/istruzioni | Riepilogo integrato e verificato Windows/offline; causa degli episodi live ancora da stabilire |
| P1 | Qualità, continuità tra account e Auto; dipende da compiti e verifiche indipendenti | Compiti React/TS e Godot con requisiti/check fissati prima; misurare completamento senza solleciti, errori, correzioni, estranei, costo totale e attesa; limiti dichiarati, nessuna parità universale dedotta | Correzioni e fixture Windows integrate; GPT: candidata React 2/2 e Godot 3/3, confronto mirato; Claude live fermo per autenticazione; qualità generale/delega ordinaria ancora aperte |
| P1 | Baseline release Windows e candidati prestazionali dei punti 0–4 | Misure ripetute e metrica/soglia fissate prima del prototipo; integrare solo benefici dimostrati | Digitazione CPU −63% sul primo carico primario; confronto successivo di 360 aperture: −26% sulla chat mista da 5.000 messaggi; finestra virtualizzata esclusa per perdita di contenuti raggiungibili; baseline assoluta stabile e altri candidati aperti |
| P2 | Composizione visiva approvata, logo/sidebar, rifiniture e movimento | Chiaro/scuro, tastiera/focus e movimento ridotto; controlli accessibili pur mantenendo pochi elementi; confronto visivo nativo Windows | Logo/sidebar implementati e verificati; rifiniture generali e screen reader restano aperti |
| P1 | Attesa tra turni e avvio CLI/provider | Separare preparazione, processo, handshake, invio e testo; mantenere distinti fixture e provider reali | 80 turni release/offline e 40 inizializzazioni CLI reali senza inferenza; telemetria monotona integrata; inferenza live e Auto/Orchestra ancora da attribuire |
| P2 | Regressioni mirate Windows e build/CI multipiattaforma | Copertura dei flussi modificati, con fixture/live/desktop distinti; account e hardware mancanti restano visibili | Da avviare |

La verifica iniziale ha confermato i difetti segnalati nel composer: stato unico per testo/allegati e svuotamento soltanto dopo `send_message`. La prima tranche sotto li corregge. La preparazione/classificazione/catalogo precede tuttora l'accettazione backend; il feedback immediato non misura né elimina quell'attesa. La descrizione obsoleta di link/immagini non interattivi in `DESIGN.md` è stata corretta sulla fonte `MessageBody`/`ResponseImage`, senza usare catture come prova live.

L'accettazione non richiede di integrare ogni candidato: scarti e risultati inconcludenti sono validi se documentati e rinviati esplicitamente. Il budget live resta quello già concordato. Nessun aumento versione o pubblicazione in questa preparazione.

### Prima tranche — evidenza dell'11 novembre

`tests/native-composer.mjs` verifica sette scenari nell'app Windows debug/WebView2 con dati e account isolati, IPC/SQLite reali e CLI offline: isolamento testo/allegati, invio immediato con accettazione ritardata e protezione dal doppio invio, rifiuto con testo più recente, accettazione con readback indisponibile, rifiuto con primo readback fallito, creazione chat e Compact ritardato. I messaggi usano un UUID client conservato dal backend, anche nelle continuazioni Orchestra; l'inserimento iniziale non può sovrascrivere un messaggio già esistente. La verifica di un esito incerto legge lo stato e non rilancia la richiesta.

Le bozze sono **locali alla finestra**: cambiare chat le conserva; reload/riavvio non sono ancora coperti da persistenza. Gli errori di invio sono associati alla chat sorgente e il ripristino conserva anche eventuale testo nuovo. Il feedback immediato non elimina il tempo reale di preparazione del provider, né rende cancellabile ogni sua fase.

Controlli superati: build debug Rust; TypeScript; 3 test storage e 13 Orchestra; 2 regressioni Playwright mirate (slash commands e filtro sidebar); Clippy con warning negati, formattazione e riferimenti documentali. La revisione indipendente ha trovato e poi giudicato risolti il recupero dopo rifiuto + readback fallito e il comportamento tardivo di Compact. Catture di invio leggibili in chiaro/scuro e azione di verifica esito controllate. Questi risultati riguardano il working tree basato su `657c679`, non una nuova release o un confronto live. Report e catture locali: `.impeccable/review/native-composer-results.json` e `native-composer*.png`.

La chiusura della prima tranche non implicava osservabilità generale, diagnosi dell'esecuzione/qualità, baseline release, confronti prestazionali o nuova composizione logo/sidebar. La tranche seguente aggiorna soltanto l'osservabilità e le riproduzioni controllate.

### Seconda tranche — osservabilità e diagnosi controllata

**Implementata localmente l'11 novembre, con collaudi Windows debug/offline.** Un solo riepilogo espandibile sopra il composer mostra fase osservata, durata, ultimo evento registrato, agenti con stato esplicito e verifiche strutturate disponibili. Non conta un flusso di testo di un sottoagente come prova di una chat autonoma attiva. Le verifiche assenti non sono passaggi impliciti; risultati ripetuti dello stesso check sostituiscono il precedente. Le fasi comprendono attesa di avvio/accesso al progetto, contesto, catalogo, capacità, avvio provider, risposta, ragionamento, strumento, domanda/permesso, verifica e fine turno.

`execution.rs` registra snapshot versione 1 nelle attività esistenti: tempi di avvio/ultimo evento/primo testo/ritorno, contatori e fasi, senza prompt, risposte, comandi o valori delle domande. Ogni snapshot conserva al massimo 32 transizioni; restano al massimo 64 snapshot di chiamate e 64 marcatori di fase per messaggio, con massimo 256 ID aperti per strumenti/domande. Gli aggiornamenti di fase e di input sono immediati; gli altri snapshot vengono pubblicati al massimo ogni 750 ms e alla chiusura. Sono osservazioni del flusso normalizzato, non una misura dell'inferenza interna né una baseline prestazionale.

Le coppie esplicite avvio/fine strumenti esposte da Claude e Codex sono distinte dal semplice risultato di uno strumento. Le capacità degli altri provider restano diverse. Le osservazioni delle chiamate interne di Orchestra sono conservate anche dopo salvataggio/reload; la UI sceglie una chiamata ancora attiva quando un'altra parallela è già terminata. Dopo un riavvio che marca il turno interrotto senza un evento finale, il tempo si ferma all'ultima osservazione e dichiara ignoto l'istante finale.

**Diagnosi dimostrata nei casi controllati:** il vecchio indicatore basato sull'ultimo tool poteva suggerire uno strumento ancora in esecuzione dopo il suo risultato; ora la distinzione è esplicita. Una risposta che promette lavoro e termina senza strumenti produce "Turno terminato" e zero eventi strumento, non attività fittizia né una certificazione del compito. Domande e permessi pendenti non vengono classificati come blocchi. Dopo 30 secondi senza aggiornamenti registrati compare un'indicazione di silenzio, non un errore o un rilancio. "Aggiorna stato" legge messaggi/sessioni/journal dei permessi senza invocare modelli; le letture tardive non sovrascrivono streaming o altre chat. Stop usa la cancellazione esistente.

**Evidenze:** `tests/native-execution.mjs` passa 8 scenari (domanda/risposta, promessa senza strumenti e reload, coppia strumenti, errore, 30 secondi reali di silenzio + refresh + Stop, refresh tardivo dopo cambio chat, intervista interna Orchestra persistita, italiano/scuro dopo reload). Passano di nuovo i 7 scenari composer. Le regressioni mirate includono 8 test TypeScript per stati/pressione/parallelismo/crash, 4 Rust execution per privacy/lifecycle/pressione/persistenza e i mapping di protocollo. Build debug, TypeScript, Clippy, formattazione e documentazione restano distinti dalla prova nativa. Report/catture locali: `.impeccable/review/native-execution-results.json` e `execution-*.png`. Revisione indipendente: ship a questo perimetro desktop, non audit universale.

**Limiti aperti:** non è stata stabilita la causa degli episodi storici sui veri account, né dimostrata maggiore qualità dei modelli. Prima dell'accettazione backend, la UI misura soltanto l'attesa osservata dell'invio; classificazione/catalogo preliminari non hanno ancora una scomposizione completa e Stop non cancella ogni loro fase. Nessun reinvio è aggiunto in risposta al silenzio; le politiche native/fallback preesistenti restano distinte. Tempi da clock locale, copertura provider/account/riavvio e accessibilità con lettore di schermo non sono certificazioni. Restano aperti qualità/continuità Auto, baseline release, esperimenti, UI logo/sidebar e copertura multipiattaforma; nessuna chiamata live o nuova release è stata eseguita.

### Terza tranche — qualità, continuità e calibrazione

**Integrata localmente l'11 novembre.** Corrette l'eredità di verdict su richieste brevi non correlate, la ricerca oltre un incarico manuale più recente, l'affinità account che annullava il bilanciamento quota e l'accettazione di audit qualificati come passaggi. Quality non avvia più audit per ogni lavoro ordinario di coding; Budget conserva richieste esplicite e valutazioni di rischio, con opt-out rispettato. La difficoltà singola non basta a giustificare il grafo parallelo Orchestra. Review può usare un secondo account dello stesso provider; Both conserva il contratto Claude + GPT. Quota/cooldown non possono eliminare silenziosamente un audit richiesto. Il modello del revisore è scelto dal catalogo dell'account indipendente effettivo anche dopo fallback.

La ripresa nativa riceve note/check posseduti da Lumen solo quando cambiano: latest per etichetta, sorgente, workspace/chat correnti, massimo otto etichette recenti e dettagli limitati. Il cambio account riceve gli stessi fatti nel passaggio completo senza condividere ID nativi; una nuova chat non eredita i check di un'altra. Nessun reinvio automatico del compito è aggiunto per silenzio o ritorno senza strumenti.

**Risultati:** corpus deterministico fissato prima del prototipo da 2/14 a 14/14; non è una misura dell'intelligenza del modello. Le fixture Windows coprono due Claude + GPT, isolamento, invio delle nuove evidenze una volta sola, bilanciamento e audit/fallback. I progetti minimi React/TS e Godot C#/GDScript compilano, falliscono sui difetti intenzionali e passano dopo la correzione di riferimento. Nessun file Rodent Heroes è usato.

**Live mirato GPT:** requisiti completi della candidata React 2/2 e Godot 3/3, baseline 0/2 e 1/3. Tutti i campioni validi compilano e preservano il perimetro; la differenza riguarda il comportamento UI rimasto da completare tramite le evidenze Lumen. Spesi 13 slot totali, compresi due React invalidati da un difetto del verificatore TEMP Windows e un errore OAuth Claude. Il verificatore è stato corretto e riprovato offline prima degli slot rimasti; nessuna coppia è stata sostituita e nessun limite di sei tentativi per task/provider è superato. Claude non è ritentato, né è fabbricato un secondo account live.

Tempi, input/output/cache e tentativi esclusi restano nel [protocollo con risultati](quality-experiments-0.8.md) e nel [report aggregato](data/quality-0.8-results.json). Costi monetari assenti e uso parziale del compito interrotto sono espliciti. Il campione prova il beneficio mirato del contesto di verifica nel percorso diretto; non certifica qualità generale, Auto/cross-audit live, confronto con app ufficiali, latenza release o copertura multipiattaforma. Le ultime rifiniture audit sono collaudate offline sulla build finale, separatamente dal binario congelato per A/B. Baseline prestazionali, delega ordinaria più ampia e composizione logo/sidebar rimangono aperte.

### Quarta tranche — baseline Windows e aggiornamenti delle chat lunghe

**Integrata localmente l'11 novembre.** La trascrizione completa è isolata dal composer tramite `Transcript`; proiezioni dei messaggi, stima dello storico e riepilogo di esecuzione evitano lavoro ripetuto quando cambia solo la bozza. Le azioni leggono chat/permessi correnti; un readback tardivo dopo merge non sovrascrive un'altra chat. MessageView, CSS e politiche di scorrimento restano quelli di ingresso; nessuna cronologia è troncata o resa inaccessibile.

Protocollo e metrica fissati prima dei prototipi: build release Windows con frontend incorporato, identità/profilo isolati, 100/1.000/5.000 messaggi brevi e misti, 20 ripetizioni per scenario. Sono conservati baseline, primo prototipo escluso, controllo della baseline e variante finale: 480 prove complete. Ulteriori 160 prove di scroll A/B/A/B verificano le differenze secondarie. Nessuna chiamata ai modelli; il registro live precedente resta a 13 slot.

**Risultato primario:** per dodici caratteri nella chat mista da 5.000 messaggi, mediana di lavoro del thread UI da 734 a 271 ms (−63,1%), contro soglia del 20%. Il beneficio resta contro la baseline ripetuta (1.582 ms). I tempi assoluti variano anche sul binario invariato: apertura da 1.386 a 2.921 ms, quindi nessuna promessa di velocità generale o baseline hardware stabile.

Il primo prototipo aggiungeva memoizzazione per riga ed è escluso dopo differenze di scroll non chiarite. La variante finale conserva soltanto l'isolamento della trascrizione dal composer. Nei due confronti mirati, con HTML, viewport/DPI e focus uguali, p95 frame 37,5/37,6 ms e 33,4/33,3 ms. Tutti i messaggi restano montati; ricerca, selezione, copia, immagini, disclosure, streaming, Compact, callback dopo cambio chat e lane Both passano 8 scenari release. Passano anche i 7 scenari composer e gli 8 execution debug/offline. Catture chiaro/scuro preservano la composizione esistente.

Il [protocollo e le decisioni](performance-0.8.md) e il [record aggregato](data/performance-0.8-results.json) distinguono CPU, tempo, frame, heap e memoria privata. L'apertura iniziale con DOM molto grande, virtualizzazione completa, provider/handshake, memoria a regime, screen reader e baseline assolute stabili restano aperti. Questa tranche non chiude automaticamente tutti i punti 0–4, non cambia versione e non pubblica pacchetti.

### Quinta tranche — apertura iniziale e prototipo di virtualizzazione

Il [confronto fissato prima dei prototipi](initial-open-0.8.md) usa un solo binario release sperimentale, tre varianti e 20 aperture per ciascuno dei sei carichi: **360 prove**. Sulla chat mista da 5.000 messaggi, il percorso conservativo per paragrafi letterali scende da 1.259,7 a 932,4 ms (−26,0%, mediana delle riduzioni appaiate −25,8%), oltre la soglia del 20%. Markdown e casi ambigui restano al renderer originale. HTML completo identico a 5.000 messaggi; ricerca, selezione e proiezioni AX mirate identiche a 1.000, senza certificazione screen reader.

Il prototipo con altezze misurate e overscan smonta lo storico e fallisce i vincoli di ricerca/selezione/esposizione accessibile: **non integrato**. La build ordinaria esclude il modulo sperimentale; nessuna opzione nuova. [Dati aggregati](data/initial-open-0.8-results.json), limiti dei due pilot diagnostici interrotti e segnali secondari restano visibili. Nessuna inferenza aggiuntiva; registro live invariato a 13 slot. Restano aperti una virtualizzazione compatibile, startup a freddo, latenza provider e verifiche screen reader/multipiattaforma.

Conferma sulla release ordinaria corretta: 20 coppie appaiate, apertura da 1.405,9 a 1.046,2 ms (−25,6%), digitazione +1,0%. Una prima conferma con impostazione asimmetrica dello scorrimento è esclusa con causa riprodotta e dati conservati; non altera le 360 prove originali. Passano i vincoli sul binario normale, 8 scenari release, 7 composer/8 execution debug, 72 unitari, 29 browser, build e controlli formali. La quinta tranche è conclusa nel perimetro valutato, senza dichiarare completata la virtualizzazione o l'intera 0.8.

### Sesta tranche — attribuzione dell'attesa e avvio CLI

Il [protocollo fissato prima delle misure](turn-latency-0.8.md) separa 80 turni release/offline (20 iniziali e 20 continuazioni per provider) da 20 inizializzazioni senza inferenza per ciascuna CLI installata. Telemetria monotona limitata e senza contenuti registra preparazione, catalogo/capacità, creazione processo, protocollo, enqueue/ack disponibili, primo testo normalizzato e ritorno. I confini di trasporto sono marcati all'origine, prima di eventuali code; i segnali confluiscono nei checkpoint già esistenti.

CLI reali: processo→ack di inizializzazione 531 ms Claude / 186 ms Codex; chiamata metadati comprensiva di chiusura 953/588 ms. La sola creazione processo è circa 4–5 ms; il percorso di chiusura aggiunge 405/387 ms, candidato concreto da profilare prima di un pool generale. Nei turni sintetici la preparazione a cache calda è 1 ms e l'accettazione IPC circa 9 ms; il primo uso costa 734/686 ms di accettazione. È effetto della cache già presente, non un nuovo miglioramento né latenza reale dei modelli.

Passano 152 test Rust, Clippy, 4 turni smoke, 8 scenari execution e 7 quality/account/audit; gli 80 turni release verificano anche persistenza dei confini e continuazione. [Dati aggregati](data/turn-latency-0.8-results.json). Nessuna nuova inferenza: registro live invariato a 13 slot. Pool, TTFT dei provider, classificazione Auto e scheduling worker Orchestra restano da validare; nessuna modifica a policy native, permessi o autenticazione.

### Settima tranche — chiusura dei metadati e 0.7.1

Confronto A/B/A/B delle CLI installate, 40 campioni per provider/variante: chiamata completa −37,2% Claude / −57,4% Codex, inclusa la supervisione. Conferma separata sul sorgente finale 0.7.1: −30,2%/−50,2% contro le mediane preservate. La chiusura usa un Job Object assegnato a processo sospeso, EOF con massimo 80 ms di grazia e arresto/reaping del gruppo residuo; cancellazione, abort, padre già uscito e gruppi indipendenti sono verificati. Modelli, altre piattaforme e policy native mantengono i percorsi precedenti.

Passano 158 test Rust Windows, 72 unitari frontend, 29 browser, 22 scenari nativi offline, build e controlli formali. [Misure, compromesso di avvio e limiti](metadata-shutdown-0.8.md). Il proprietario autorizza push/merge e release **0.7.1 — Performance Update** delle tranche concluse dopo CI e verifica dei pacchetti. Questa autorizzazione sostituisce il precedente rinvio della pubblicazione, senza chiudere l'intera roadmap 0.8. Budget live invariato a 13 slot; nessuna nuova inferenza.

### Ottava tranche — import affidabili e contesto di impatto

Il corpus dichiarato prima delle correzioni rileva 14 errori su 23 aspettative nella baseline 0.7.1. Risoluzione degli import typed `.mjs/.cjs`→`.mts/.cts`, priorità delle sorgenti TypeScript, GDScript con apici singoli/ereditarietà relativa e normalizzazione entro i confini portano il corpus a 23/23. Commenti su righe intere non creano più definizioni/import/riferimenti nei linguaggi mirati. Indice persistente 3→4, refresh dopo rinomina/cancellazione e conservazione dei chiamanti nell'handoff verificati.

Passano 163 test Rust Windows, sei controlli con TypeScript/Node/Godot installati e cinque scenari MCP nativi; Clippy, build debug e controlli formali. [Protocollo e limiti](graph-reliability-0.8.md), [dati](data/graph-reliability-0.8-results.json). Nessun parser nuovo o inferenza; registro live a 13 slot. È una correzione mirata del contesto, non prova di accuratezza globale/risparmio token o chiusura dell'intero punto 4.

### Nona tranche — Rewind e ritiro delle evidenze obsolete

Due regressioni riprodotte sulla baseline `26ab3b9`: Rewind conservava identificatori CLI che puntavano a una cronologia remota contenente anche i turni rimossi; un nuovo incarico esplicito ritirava i check dallo stato di lavoro ma non dal contesto provider. La rimozione dei marcatori nativi è ora atomica con il taglio SQLite; il prossimo turno ricostruisce il contesto conservato. Le altre attività e chat restano intatte. La stessa regola di nuovo incarico governa entrambi i contesti; le continuazioni dichiarano quando il nuovo contesto sostituisce evidenze precedenti, anche se restano note di progetto.

Passano 166 test Rust Windows e nove scenari nativi offline Claude/Claude/GPT, inclusi i due nuovi flussi Rewind/cambio incarico. [Protocollo, verifiche e limiti](continuity-rewind-0.8.md). Budget live invariato a 13 slot; nessuna nuova inferenza o pubblicazione. Non è un rollback della cronologia sul provider né una certificazione della continuità live generale.

### Decima tranche — evidenze attraverso Compact

Riprodotta sulla baseline `1b8a941` la perdita di dettagli, diagnostica e sorgenti dei check nel contesto provider dopo Compact. Il nuovo seed conserva l'ultima evidenza strutturata per etichetta; continuazioni e cambio account leggono la stessa proiezione, ancora limitata agli otto check più recenti. Compact ripetuto conserva le sorgenti originali senza seguire puntatori alla chat precedente. I nuovi risultati sostituiscono quelli vecchi; nuovo incarico, Rewind e cambio progetto rispettano i rispettivi confini. Vecchi seed restano leggibili senza inventare informazioni perdute.

[Protocollo e limiti](compact-evidence-0.8.md). Nessuna inferenza aggiuntiva: budget a 13 slot. Questa è continuità delle evidenze backend, non una misura della qualità generale del modello o chiusura della matrice completa.

### Undicesima tranche — logo e sidebar

Implementato il 12 novembre il logo come unico comando per aprire/chiudere la sidebar di vetro, anche sul desktop; menu mobile separato rimosso. Il logo resta ancorato, la chat occupa lo spazio disponibile, bozza e contenuti restano montati. La composizione con progetti resta quella iniziale sul desktop; la vista essenziale si richiama dal logo. Tastiera, focus, etichette inglesi/italiane, pannello chiuso `inert`, riflesso breve e movimento ridotto sono coperti dai controlli mirati. La selezione di una chat conserva il pannello desktop, mentre la finestra stretta lo chiude.

[Perimetro, prove e limiti](logo-sidebar-0.8.md). Nessuna nuova inferenza o release; budget invariato a 13 slot. È il primo intervento sulla composizione approvata: rifiniture generali, persistenza della composizione e screen reader effettivo restano aperti.

### Dodicesima tranche — recupero sessioni senza replay dopo attività

Riprodotta sulla baseline `90f632e` una duplicazione: un errore di sessione dopo l'avvio di uno strumento, senza testo visibile, faceva ripetere il compito e la scrittura nella fixture. Il fallback ora osserva l'attività prima che i segnali vengano scartati/compattati e si ferma dopo lavoro osservato, eventi sconosciuti, testo parziale o cancellazione. Il recupero singolo con contesto completo resta disponibile per sessione mancante prima del lavoro osservato; l'errore spiega quando Lumen evita il replay.

[Protocollo e limiti](resume-replay-0.8.md). Nessuna nuova inferenza, budget a 13 slot. La correzione riguarda questo fallback di ripresa nativa, non tutte le politiche di ritentativo o gli effetti non riportati dai provider.

### Tredicesima tranche — quota senza replay del lavoro parziale

Riprodotta sulla baseline `086469a` una seconda duplicazione: dopo lavoro osservato e quota esaurita, il cambio automatico account reinviava il compito e ripeteva la scrittura. Il guard monotono è ora condiviso con questo percorso: niente cambio automatico dopo attività, output parziale o cancellazione; testo ed evidenze restano conservati. Il cooldown viene registrato comunque. Prima del lavoro osservato resta disponibile un solo passaggio a un altro account con i permessi correnti.

[Protocollo e limiti](quota-replay-0.8.md). Budget live invariato a 13 slot; nessuna inferenza o pubblicazione.

### Tranche 16 — interfaccia e verifiche Windows/accessibilità

Eseguita anticipatamente su richiesta del proprietario il 12 novembre. Corrette le impostazioni non modali rispetto al focus, lo sfondo attivo/esposto, le scorciatoie dietro il dialogo e il ritorno all'opener. Focus da tastiera visibile con riempimento/sottolineatura, senza anelli globali. Le domande distinguono controllo iniziale, lettura fallita e scadenza, conservano la risposta dopo errore di invio e ripristinano il focus dopo errore/successo.

Passano 35 scenari browser e 72 unitari; matrice WebView2 con dati isolati per impostazioni chiaro/scuro, domanda da tastiera, inspector, 1.000 messaggi ricercabili/esposti in AX e immagine locale. Provato realmente Assistente vocale, con conferma uditiva del proprietario sulle etichette; VS Code mostra la riga 13 richiesta dal backend con profilo temporaneo. Lettore e finestre editor ripristinati al termine. [Matrice e limiti](windows-accessibility-0.8.md), [risultato aggregato](data/windows-accessibility-0.8-results.json). Nessuna certificazione generale di accessibilità, NVDA o desktop non Windows; budget live invariato a 13 slot.

### Tranche 14 — Auto/Orchestra e continuità

Tre difetti dello scheduler riprodotti e corretti: dipendenze fallite considerate completate, quarto tentativo dopo ripresa e dipendenze inesistenti sospese. Solo completamenti riusciti sbloccano i dipendenti; ammissioni e tentativi già consumati restano tali. Nei worker, errori dopo lavoro osservato non avviano retry; output parziale, spiegazione e cooldown quota sono conservati. La prova nativa di cancellazione ha riprodotto processi figli superstiti: Stop attende ora la pulizia del provider prima di rimuovere le lane.

Cinque regressioni Rust aggiunte, quattro scenari Orchestra Windows con IPC/worktree reali e CLI offline e 12 regressioni native account/contesto/routing/audit coprono questo perimetro. Le regole Auto e cross-audit restano quelle approvate; nessuna nuova harness universale o prova di qualità multiaccount live. [Protocollo, risultati e limiti](auto-orchestra-continuity-0.8.md). Budget fermo a 13 slot; release 0.7.1 invariata.

### Tranche 15 — contesto, grafo e budget dei prompt

Cinque regressioni riprodotte sulla baseline `250e6ec` e corrette: copie identiche dei fatti proiettati, allegati duplicati, anteprime fuori budget, requisiti centrali delle note persi alla ripresa e contratti omessi senza avviso di limite. I fatti completi con fonte rimangono persistiti; vengono rimossi solo duplicati già mostrati integralmente. Inline e preview rispettano 48 KiB di contenuto UTF-8, mantenendo i riferimenti ai file. Le note modificate vengono consegnate integralmente una volta, con revisione aggiornata anche per vecchie sessioni.

Payload React 7.305→4.719 byte, Godot con cronologia lunga 106.119→103.533; input CLI iniziale reale offline 29.489→27.247. I turni nativi invariati restano 758 byte/148 token locali; l'aggiornamento di note lunghe cresce intenzionalmente per conservare requisiti prima omessi. Passano 180 Rust, 72 unitari frontend, sei catture native per versione, 12 regressioni qualità/continuità, quattro Orchestra e cinque grafo MCP; corpus import 23/23 preservato. [Protocollo e record](context-graph-budget-0.8.md). Nessuna nuova inferenza o prova di risparmio fatturato/qualità generale. JSON posizionale, parser nuovo, budget globale per modello e riserva d'output restano non dimostrati, senza cambiamento dei formati nativi.

### Tranche 17 — prestazioni, accettazione e primo utilizzo

Consolidate le decisioni: isolamento del transcript, paragrafi letterali e shutdown Windows integrati; finestra virtualizzata e memoizzazione più ampia escluse per i rispettivi gate; Rust Markdown/ANSI, pool CLI, metadati posizionali, parser nuovo e budget globale rinviati per prove insufficienti. I record storici restano distinti dalle nuove misure. [Protocollo e classificazione](final-acceptance-0.8.md).

Il controllo originario nello stesso processo riproduce una deriva di apertura della baseline (12 campioni, circa 1,2–27,8 s), conservata ed esclusa prima del confronto candidato. La causa e la stabilità delle aperture ripetute restano non certificate. Ottanta campioni con processi freschi, A/B/A/B, superano il gate mirato di regressione del 10%: apertura −0,7%/−10,2%, task digitazione +3,1%/−17,2%. P95 e variazioni secondarie restano visibili, senza attribuire una nuova accelerazione alla UI. Otto gate del contenuto storico passano sul medesimo binario release isolato.

Il controllo permessi mostra ora modalità e spiegazioni, con selezione esplicita. Il tutorial iniziale è saltabile, ripetibile, bilingue e guidato sugli elementi reali, con highlight, focus e movimento ridotto; non invia richieste né cambia accessi. Passano 180 Rust (14 opt-in ignorati), 72 unitari frontend, 40 browser e 18 scenari nativi debug (tre onboarding, sette composer, otto execution). Nuova verifica uditiva del tutorial, desktop non Windows e qualità/TTFT live non sono dedotti da queste prove. Budget fermo a 13; release pubblica ancora 0.7.1.

## Piano di chiusura — una tranche residua, 14–17 completate nel perimetro dichiarato

Stima operativa aggiornata il 12 novembre, non un numero di difetti garantito. Dei cinque pacchetti previsti, le tranche 14–17 sono completate nel perimetro dichiarato; resta la 18. Risultati inconcludenti, candidati rinviati e prerequisiti rimangono espliciti nel protocollo 17, comprese le aperture ripetute nello stesso processo. Nuovi difetti bloccanti possono richiedere lavoro ulteriore: la roadmap non viene dichiarata conclusa in anticipo.

| Tranche prevista | Lavoro residuo | Criterio di uscita |
| --- | --- | --- |
| 14 — Auto e Orchestra — completata | Dipendenze, budget di ripresa, replay dei worker e pulizia su Stop corretti | Casi offline e confini di delega nel checkpoint sopra; qualità multiaccount live non certificata |
| 15 — Contesto e grafo — completata | Duplicati e budget delle preview corretti, note di ripresa complete, limiti dei contratti espliciti | Payload costruiti e consegnati misurati offline; corpus preservato; candidati senza prova end-to-end classificati nel protocollo |
| 16 — Interfaccia e Windows — completata | Impostazioni/focus/domande corretti; matrice nativa, editor alla riga e prova con Assistente vocale eseguiti | Perimetro e limiti nel checkpoint sopra; integrazione e release ancora aperte |
| 17 — Prestazioni e accettazione finale — completata | Misure e candidati classificati, regressioni release mirate e permessi/tutorial verificati | Gate e limitazioni espliciti nel protocollo; non si certificano aperture ripetute a regime, qualità live o parità desktop |
| 18 — Integrazione e release | Integrare i rami verificati, versione 0.8, CI, pacchetti, checksum e note di rilascio | Sorgente e pacchetti coerenti, controlli richiesti verdi e release verificata; nessuna eliminazione delle limitazioni irrisolte |

Il budget live resta invariato: i confronti Claude/Gemini, le prove multiaccount live e l'attribuzione dell'inferenza senza prerequisiti/budget non vengono rilanciati. Saranno dichiarati non verificati o rinviati nel perimetro di accettazione; non diventano risultati positivi. Le prove desktop macOS/Linux richiedono tester esterni e restano separate dalla CI. La sola tranche residua comprende integrazione e pubblicazione.

## Decisioni concordate

- Bilanciare fluidità, tempi di attesa e qualità del codice prodotto.
- Usare Lumen e un **minuscolo progetto Godot dedicato**, insieme a fixture sintetiche ripetibili. Non copiare né modificare Rodent Heroes.
- Includere lacune funzionali mirate: affidabilità del grafo, continuità delle sessioni e verifiche native Windows.
- Integrare le ottimizzazioni soltanto dopo un miglioramento misurabile e senza regressioni di correttezza o del comportamento nativo.
- Conservare sessioni dirette predefinite, Orchestra esplicita, provider/modello/cartella scelti, autenticazione ufficiale e Moli condiviso.
- Restano escluse firma, Telegram, cifratura interna, daemon separato e gabbia OS aggiuntiva comune, come già concordato.
- Windows resta il riferimento nativo. macOS/Linux ricevono build e controlli automatici; i collaudi desktop reali restano distinti e richiedono tester.

## Progetto minimo e budget concordati

1. **Godot con C# e GDScript:** una scena 2D minimale e piccoli script in entrambi i linguaggi. Nessun asset esterno, servizio o copia di Rodent Heroes. Registrare i prerequisiti Godot .NET e SDK .NET e separare il costo della prima compilazione dalle prove a caldo.
2. **Prove live mirate:** prima misure locali; poi tre coppie A/B per attività e provider disponibile, soltanto sui candidati promettenti. Ogni coppia confronta il compito completo sulla baseline e sul candidato: sei esecuzioni del compito, che possono contenere più turni o chiamate agli strumenti. Registrare consumo, tentativi falliti, cache e quota; non estendere automaticamente il budget.

Entrambe le scelte sono state confermate dal proprietario il 10 novembre 2026. L'11 novembre è stata autorizzata l'esecuzione dopo l'intervista; gli esperimenti non ancora eseguiti conservano lo stato indicato sotto.

## Regole per decidere

Ogni voce attraversa **baseline → ipotesi → prototipo → confronto → decisione → integrazione e regressioni**. Registrare commit, ambiente, comandi, dati grezzi, risultati e limiti. Gli stati sono: da misurare, in esperimento, accettata, scartata con evidenza, bloccata da prerequisito, integrata e verificata. Una voce valutata non è una voce implementata.

Prima del prototipo scegliere una metrica primaria, soglia e carico rappresentativo. Le seguenti soglie sono una proposta tecnica da calibrare sulla variabilità della baseline, non risultati promessi:

- Prestazioni: almeno **20% sul tempo rilevante** oppure **15% sulla memoria privata**, sul carico dichiarato, con effetto ripetibile oltre il rumore. Non scegliere la metrica vincente dopo aver visto i risultati.
- Riportare tempi assoluti, mediana e p95 dove il campione è sufficiente. Una sola misura o un massimo isolato non stabiliscono un guadagno generale. Come punto di partenza: 20 ripetizioni locali per scenario, estendendole se instabili.
- Nessuna regressione riproducibile di correttezza, isolamento, permessi, cancellazione o integrità delle sessioni. Indagare regressioni superiori al 5% sulle altre metriche principali prima dell'integrazione.
- Token: confrontare l'intero scambio effettivo, inclusi schema, istruzioni, riparazioni e richieste aggiuntive; non soltanto il payload più compatto.
- Qualità: stessi requisiti, modello, impostazioni, contesto iniziale e verifiche indipendenti. Contare requisiti soddisfatti, errori, correzioni necessarie, modifiche estranee, chiamate e durata. Non usare la sola autovalutazione del modello come prova.
- Confronti live piccoli sono indicativi: alternare A/B e B/A e registrare cache/quota. Se il risultato è ambiguo, dichiararlo inconcludente; ulteriori esecuzioni richiedono una nuova definizione del budget. Non dedurre superiorità universale da tre coppie.
- Un miglioramento di UX può essere valido senza aumentare la qualità del codice; dichiarare quale beneficio è stato effettivamente osservato.

## 0. Baseline e strumenti di misura

**Stato: prima baseline locale release misurata; tempi assoluti non stazionari, copertura da estendere.** Vedi [protocollo Windows](performance-0.8.md): 20 ripetizioni per scenario, controlli su binario congelato, DOM/frame/heap e memoria privata. Startup a freddo, tempi CLI/provider e maggiore controllo dell'ambiente non sono ancora certificati.

- Preparare una build release Windows di riferimento e dati applicativi isolati. Separare debug/release, avvio a freddo/caldo e tempo di Lumen/CLI/modello/strumenti.
- Misurare apertura chat, digitazione, scorrimento continuo e salti, streaming con immagini/codice/sottoagenti, Compact e cambio conversazione. Carichi iniziali: 100, 1.000 e 5.000 messaggi, brevi e lunghi.
- Registrare intervalli tra frame, blocchi del thread UI, DOM, memoria privata e numero di processi. La somma dei working set condivisi non è memoria privata.
- Separare avvio processo, handshake, richiesta inviata, primo evento, primo testo e completamento; distinguere percorsi diretti, continuazione e Orchestra.
- Conservare fixture e risultati versionati senza segreti, dati di account o conversazioni private; escludere dati pesanti e log grezzi dal repository.
- Il Godot minimo deve aprirsi ed essere verificabile offline, con una scena 2D, pochi nodi e piccoli script C# e GDScript, nessun addon acquistato, asset esterno, servizio o dipendenza da Rodent Heroes. Prevedere stato, pulsante, limite e reset con controlli indipendenti, includendo compilazione C#, controllo GDScript e interazione tra i due linguaggi.

**Nota sul punto di partenza:** la prova sintetica debug del 10 novembre alle 17:06 registra circa 563 ms di apertura e 782 ms di massimo intervallo tra frame per 1.000 messaggi. I precedenti 330–360 ms non esauriscono i risultati. Nessuno dei campioni costituisce ancora una baseline release ripetuta.

## 1. Chat lunghe e virtualizzazione

**Stato: isolamento dal composer e percorso per paragrafi letterali accettati sui rispettivi carichi; prototipo di virtualizzazione valutato ed escluso.** L'apertura migliora senza ridurre il DOM; la finestra che smonta lo storico non conserva ricerca, selezione e accessibilità. Una soluzione compatibile resta aperta. Le prove escluse e i controlli secondari restano nei report; non è una prova che ogni virtualizzazione sia inutile.

- Profilare il percorso attuale con memoizzazione e `content-visibility`.
- Confrontare un prototipo di virtualizzazione con la stessa baseline, includendo altezze variabili e streaming.
- Verificare posizione di scorrimento, ritorno alla chat, messaggi/immagini espansi, ricerca, selezione/copia del testo, collegamenti, tastiera e lettura accessibile. Non sostituire un rallentamento con perdita di contenuti raggiungibili.
- Integrare soltanto con beneficio ripetibile e regressioni superate; conservare una modalità di confronto/ripristino durante l'esperimento.

## 2. Attesa tra turni e processi riutilizzabili

**Stato: attribuzione locale misurata e chiusura metadati Windows ottimizzata.** Vedi [misure tra turni](turn-latency-0.8.md) e [confronto A/B/A/B della chiusura](metadata-shutdown-0.8.md): chiamata completa −37,2% Claude / −57,4% Codex, includendo la supervisione. Tempi dei modelli, classificazione Auto e scheduling Orchestra non sono ancora separati. Nessun pool introdotto; la pulizia dei turni modello e delle altre piattaforme non cambia.

- Misurare quanto dell'attesa deriva da Lumen, avvio CLI, handshake, provider e strumenti. Confrontare con un controllo tramite CLI ufficiale quando le condizioni sono comparabili.
- Prima eliminare lavoro duplicato dimostrato; sperimentare processi mantenuti pronti solo se avvio/inizializzazione pesano sul percorso reale.
- Verificare cambio account, cartella, modello e permessi, cancellazione, crash, sessioni simultanee, scadenza e consumo a riposo. Nessuna configurazione o risposta deve passare alla sessione sbagliata.
- Non confondere riuso del processo con riuso della conversazione nativa già esistente; non promettere riduzioni del tempo di inferenza.

## 3. Contesto, metadati e qualità del codice

**Stato: payload costruiti e consegnati misurati offline nella [tranche 15](context-graph-budget-0.8.md); duplicati, budget preview e requisiti delle note corretti. Qualità/token fatturati end-to-end, budget globale e riserva d'output non dimostrati.**

- Misurare duplicazione, rilevanza, troncamento, budget, riserve di output e continuità dei requisiti nei percorsi effettivi.
- Provare prima riduzione dei duplicati e proiezioni mirate dello stato; confrontare JSON con chiavi e formato compatto solo per metadati posseduti da Lumen.
- Preservare formati nativi CLI/MCP. Verificare Unicode, campi assenti, versioni, dipendenze, errori e riparazioni; misurare comprensione/produzione del modello, non soltanto round-trip del parser.
- Il precedente risparmio sintetico di circa 32% sui metadati è un'ipotesi utile, non un risparmio dell'intera conversazione né una prova di qualità.
- Usare piccoli compiti di modifica e correzione su Lumen e sul progetto Godot, con requisiti e controlli indipendenti fissati prima delle prove.

## 4. Affidabilità del grafo

**Stato: corpus import TS/JS e Godot, commenti su righe intere e refresh/handoff mirati verificati.** [Ottava tranche](graph-reliability-0.8.md): 23/23 aspettative contro 9/23, senza cambio parser. Alias/configurazioni arbitrarie, semantica completa dei linguaggi, altre forme di commenti/stringhe e accuratezza su progetti reali restano da estendere.

- Creare fixture per dichiarazioni, riferimenti, rinomina, cancellazione e dipendenze nei linguaggi effettivamente coinvolti; registrare omissioni e falsi positivi.
- Verificare che il contesto di impatto migliori le modifiche e i controlli selezionati, senza aggiungere rumore o eccessivo costo di indicizzazione.
- Considerare tree-sitter soltanto in presenza di errori concreti non risolti adeguatamente dal percorso attuale; confrontare accuratezza, tempi, memoria e manutenzione. Non introdurlo per il solo cambio di tecnologia.

## 5. Continuità e comportamento nativo

**Stato: Rewind e cambio esplicito incarico corretti nella nona tranche; matrice generale da ampliare.**

- Verificare requisiti e decisioni attraverso continuazione, Compact, Rewind, cambio modello/provider e cambio esplicito della cartella.
- Riprodurre domande/approvazioni pendenti, cancellazione, coda, quota, riavvio e Resume ripetuto senza duplicare effetti o chiedere di approvare nuovamente un piano già autorizzato.
- Coprire sessioni dirette e worker Orchestra, Moli incluso; verificare isolamento dei contesti e messaggi tardivi di una connessione chiusa.
- Distinguere output dei sottoagenti da chat figlie autonome e capacità native differenti dei provider. La gestione universale di chat figlie autonome non entra implicitamente in questa voce.
- Riutilizzare fixture per tutti i provider e prove live secondo disponibilità e budget concordato. Login/quota mancanti restano prerequisiti visibili, non falsi passaggi né esclusioni silenziose.

## 6. Verifiche native Windows mirate

**Stato: tranche 16 verificata nel perimetro Windows dichiarato; ulteriori lettori, zoom OS e altri desktop restano non collaudati.**

- Verificare tastiera, focus, domande a scelta, immagini, chat lunga, inspector, scorciatoie e ripristino, includendo una prova effettiva con lettore di schermo.
- Verificare apertura alla riga negli editor disponibili; dichiarare separatamente quelli non installati.
- Misurare e riprodurre eventuali differenze tra UI di Lumen e protocollo nativo, evitando di aggiungere turni di pianificazione o richieste di autorizzazione duplicati.

## 7. Markdown/ANSI in Rust

**Stato: rinviato per la 0.8, non implementazione promessa.** La [tranche 17](final-acceptance-0.8.md) non dispone di un confronto post-correzioni che giustifichi la migrazione. Un esperimento futuro richiede beneficio sul costo totale e gli stessi gate di contenuto/accessibilità; non si deduce inutilità del candidato dalla mancanza di prove.

- Procedere al prototipo soltanto se parsing/rendering rappresentano un costo significativo dopo le misure del punto 0.
- Confrontare il costo totale, compresi IPC e aggiornamento UI, mantenendo corretti immagini, link, streaming, selezione e accessibilità.
- Se non migliora il carico reale, registrare il confronto e conservare il rendering attuale. L'uso di Rust da solo non è un criterio di successo.

## Confini e lavoro successivo

La 0.8 non include automaticamente avvio affiancato di applicazioni arbitrarie dai worktree, gestione universale di chat figlie autonome, redesign generale o nuovi servizi remoti. Il confronto visuale statico esistente resta distinto da queste estensioni. Sono possibili voci future, non funzionalità considerate completate o scartate definitivamente.

Le verifiche live Claude/Gemini e le matrici account/versione/OS già aperte restano visibili in [Validazione](validation.md). Non dichiarare parità universale o qualità superiore se dimostrata soltanto su fixture o su un singolo provider.

## Correzione aggiuntiva prima della release — connettori

La segnalazione 0.7.1 su macOS non è ancora riprodotta. Corrette separatamente due falle dimostrate sulla sorgente: protocollo di consenso MCP GPT e approvazione dei tool MCP Claude in Ask. Scope, dati privati, URL, cancellazione e limiti sono nel [checkpoint connettori](connector-interactions-0.8.md). Nessuna migrazione di credenziali, inferenza aggiuntiva o modifica della release pubblicata; il lavoro confluisce nell'integrazione della tranche 18.

## Chiusura della 0.8

**Aggiornamento obbligatorio del 12 novembre:** il requisito di riuso di connettori/plugin/skill/MCP/mods tra tutti i provider amplia la precedente stima della tranche 18. Il [checkpoint delle capacità condivise](shared-capabilities-0.8.md) include catalogo, relay, gateway RPC dell'account Codex e consenso dell'host verificati offline. Restano necessari adattatori dei runtime proprietari, percorso MCP Ollama, formati/cicli di vita non coperti e accettazione degli account reali. La 0.8 non è quindi riducibile al solo packaging e queste capacità non sono dichiarate universalmente implementate.

- Per ogni candidato: decisione, numeri, carichi, limiti e stato dell'integrazione. Le prove inconcludenti restano aperte o vengono rinviate esplicitamente.
- Test mirati per ogni modifica accettata, regressioni applicabili, verifica Windows nativa e CI/build multipiattaforma.
- Documentazione coerente con i risultati effettivi; nessuna eliminazione della roadmap con voci irrisolte. Spostare nell'archivio soltanto dopo aver classificato ogni voce.
- Build, aumento versione e pubblicazione sono attività successive alla definizione e realizzazione della roadmap; non fanno parte della sola preparazione di questo documento.

[Documentazione](README.md) · [Validazione 0.7](validation.md) · [Qualità e contesto](task-quality.md)
