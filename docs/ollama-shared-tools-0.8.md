# Ollama e strumenti condivisi — tranche 0.8

## Risultato e perimetro

Ollama usa ora gli strumenti MCP **stdio** del catalogo condiviso nella stessa chat: configurazioni rilevate nei profili originali, installazioni condivise e gateway dell'account ChatGPT/Codex. Il modello resta locale; il gateway Codex esegue RPC degli strumenti senza inviare `turn/start`. Sono coperti sia il tool calling nativo sia il percorso di azioni JSON per modelli che non dichiarano strumenti nativi. Il risultato effettivo torna al modello, compreso `structuredContent`, per proseguire il lavoro.

Le skill attive del progetto e degli account diventano leggibili con `lumen_list_skills` e `lumen_read_skill`. Le risorse testuali restano nelle cartelle originali; la lettura canonica respinge file non elencati, attraversamenti e symlink verso l'esterno. Limite di 1 MB per risorsa. Una skill non concede nuovi permessi.

Questa tranche completa il percorso stdio locale; **non dichiara compatibilità universale**. MCP HTTP/SSE/WebSocket in Ollama, risorse/prompts MCP come API autonome, aggiornamenti dinamici del catalogo e componenti proprietari restano lavoro obbligatorio nel [checkpoint generale](shared-capabilities-0.8.md). Un servizio che richiede un adattatore produce un evento esplicito e non viene convertito scartandone policy o autenticazione. Non sono stati provati l'OAuth o gli account del segnalante.

## Esecuzione, permessi e ciclo di vita

- Namespace stabile per origine e nome dello strumento: servizi omonimi non si sovrascrivono. La paginazione del catalogo è coperta; limiti di 64 servizi, 20 pagine e 2.000 strumenti per servizio.
- Read only e File edits espongono solo strumenti con `readOnlyHint: true`, coerentemente con il gateway degli account. È una dichiarazione del server, non una sandbox del suo codice. Ask richiede una decisione privata per ogni chiamata; Autopilot conserva l'autorizzazione del compito, ma non accetta automaticamente i consensi del connettore.
- Le richieste MCP `elicitation/create` usano il modulo privato già presente nella chat. Accetta/Rifiuta/Annulla non diventano decisioni trasferite al modello o ad altri account. La revoca del server chiude la richiesta; una vecchia risposta non può autorizzare un'altra chiamata.
- Sampling e altre capacità client non implementate ricevono un errore di protocollo. Non avviene alcuna inferenza aggiuntiva implicita. Risposte malformate, disconnessioni e timeout ritirano il servizio senza rieseguire la chiamata.
- Ogni turno possiede connessioni e processi. La chiusura invia EOF e attende la pulizia anche dopo errore o Stop; Job Object Windows e process group Unix restano i confini dei processi figli. I servizi stdio portabili sono avviati direttamente dalla dichiarazione originale, senza un relay intermedio. Su Unix la chiusura concede fino a 2 s all'helper per chiudere il runtime proprietario e ripulisce il gruppo salvato anche se il padre è già uscito; una regressione Unix controlla il discendente che ignora SIGTERM. Il buffer di lettura sopravvive alla competizione tra risposta GUI e frame parziale del server, con limite di 2 MB prima dell'allocazione ulteriore.
- Avvio e disponibilità dei servizi producono eventi `connector-status`. Non viene introdotto un processo persistente o un nuovo controllo nell'interfaccia.

## Evidenza

**Budget live invariato: 13 slot storici, incremento 0.** Nessun login, download di modelli o inferenza reale. Il server Ollama delle prove è una API locale simulata; i servizi MCP sono subprocessi Node reali, con dati temporanei. Il gateway usa la CLI di protocollo simulata che rifiuta esplicitamente `turn/start`.

- Suite Rust Windows completa: **198 passati, 14 opt-in ignorati**. Sette nuove regressioni coprono i due cicli Ollama, argomenti tipizzati e contenuto strutturato, paginazione, collisioni, filtri dei permessi, skill/risorse, rifiuto prima dell'esecuzione, consenso privato, revoca, cancellazione/processo e protocollo malformato senza replay. La regressione aggiuntiva del gruppo di processi è eseguita soltanto su Unix dalla CI, non certificata su Windows.
- `node tests/native-ollama-shared.mjs`: quattro verifiche Windows/WebView2/IPC, con turni reali dell'app: modello locale con MCP del profilo Claude e consenso nell'host; azioni JSON che consumano il risultato originale; gateway dell'account ChatGPT chiamato senza turno GPT; uscita dei processi originali a fine turno.
- Build Rust e TypeScript/Vite, **72 test frontend**, Clippy con warning negati, rustfmt, Prettier e riferimenti documentali. La CI del checkpoint precedente `7ca7c38` è passata su frontend, Windows, macOS e Linux; la CI di questa tranche resta un'evidenza separata.

Un primo test nativo ha raggiunto correttamente il consenso ma usava un selettore troppo restrittivo per l'etichetta booleana; corretto e ripetuto. La prima suite Rust concorrente ha rilevato una corsa tra Drop del processo e rimozione della directory temporanea: la chiusura ora attende esplicitamente il processo. Non si deduce da queste fixture qualità del modello, parità dei servizi remoti o miglioramento prestazionale.

Riproduzione: `cargo test --manifest-path src-tauri/Cargo.toml --locked`, poi build debug e `node tests/native-ollama-shared.mjs`. Eseguire gli harness nativi e Playwright in serie perché condividono la porta 1420. Le prove non richiedono installazioni MCP né modificano i profili originali.

Riferimenti di protocollo: [trasporto MCP](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports), [elicitation MCP](https://modelcontextprotocol.io/specification/2025-06-18/client/elicitation), [tool calling Ollama](https://docs.ollama.com/capabilities/tool-calling).
