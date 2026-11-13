# Accettazione e pubblicazione 0.8

Stato del 13 novembre 2026: **implementazione 0.8.0 completa nel perimetro approvato; verifiche e pubblicazione in corso**. Il proprietario ha risposto esplicitamente «Pubblica 0.8 con limiti espliciti». La compatibilità universale non è dichiarata soddisfatta: connettori e runtime non coperti restano nella [roadmap successiva](roadmap-next.md). Pubblicazione e changelog sono autorizzati; nessun aumento del budget live.

## Perimetro verificabile

| Area | Evidenza / criterio |
| --- | --- |
| Composer e osservabilità | Bozze, invio, riconciliazione e progressi reali già integrati. Non dedurre uno stallo dal solo silenzio. Gate nativi composer/execution e regressioni UI. |
| Continuità / Auto / Orchestra | Rewind e Compact ritirano o conservano correttamente sessioni/evidenze; quote e resume non duplicano effetti osservati. Dipendenze e limiti persistenti verificati nelle fixture. Nessuna nuova parità live fra modelli. |
| Contesto e grafo | Corpus mirato 23/23; duplicazione e budget verificati nei checkpoint. Semantica completa, allocazione globale e prestazioni assolute restano limiti dichiarati, non implementazioni certificate. |
| Interfaccia e accessibilità | Logo/sidebar, menu permessi e tour implementati. Tastiera/focus/temi/movimento ridotto verificati; conferma uditiva delle etichette ricevuta dal proprietario nella tranche 16. Non certificare WCAG o desktop non Windows da questi dati. |
| Integrazioni | Catalogo dalle sorgenti originali, plugin utente/progetto/locali con ambito, consenso privato, gateway Codex tool/skill/resource. Ollama stdio/HTTP, risorse/prompts e aggiornamento tool. Matrici remote e runtime proprietari rimangono distinti. |
| Candidati prestazionali | [Decisioni della tranche 17](final-acceptance-0.8.md): integrati solo i candidati che superano i gate; virtualizzazione esclusa, altri esperimenti rinviati per evidenza insufficiente. Nessuna percentuale storica riscritta come misura del nuovo pacchetto. |

## Correzioni aggiuntive del candidato

- Client Streamable HTTP dell'SDK MCP Rust ufficiale, con TLS, intestazioni originali, sessioni e risposte SSE. Consenso nell'host e ritiro della richiesta restano di proprietà del turno. Recupero automatico delle sessioni scadute disattivato per evitare replay delle scritture; timeout/errore ritirano il servizio.
- API di risorse, template e prompt nel loop locale; i messaggi dei prompt sono restituiti come dati del tool, senza cambiare ruoli o permessi. Namespace e filtri Read only/File edits sono mantenuti anche dopo `tools/list_changed`. I due percorsi Ollama aggiornano le definizioni senza accodarne copie alla cronologia.
- Lettura di risorse MCP via RPC del proprietario Codex; la scelta del server è limitata al catalogo dell'account/thread. Nessuna nuova inferenza GPT per effettuare la lettura.
- Componenti MCP/skill dei plugin Claude `project`/`local` visibili soltanto nella cartella dichiarata nell'indice d'installazione; rispettati override di disabilitazione del progetto. Hook/mods non diventano portabili per questo motivo.

## Vincoli esterni e perimetro approvato

La documentazione Claude espone in Agent SDK metadati MCP e una lettura di risorse UI limitata; non è stata trovata una chiamata diretta agli strumenti cloud equivalente a `mcpServer/tool/call` di Codex. I connettori claude.ai richiedono il login d'abbonamento corretto, e alcuni non consentono OAuth locale. Copiare credenziali o dichiarare installati i connettori del client web non crea un adattatore verificato. Componenti proprietari senza contratto pubblico di esecuzione, API/mods specifiche di Antigravity e trasporti/formati non implementati non sono dichiarati universalmente condivisi.

Il requisito universale originale rimane aperto. Il proprietario ha accettato esplicitamente una 0.8 con queste limitazioni e il mantenimento del requisito nella roadmap successiva. Le voci restanti non sono quindi funzionalità completate né escluse per sempre. [Matrice completa](shared-capabilities-0.8.md).

Fonti verificate: [MCP Claude e connettori claude.ai](https://code.claude.com/docs/en/mcp), [Agent SDK TypeScript](https://code.claude.com/docs/en/agent-sdk/typescript), [Codex app-server](https://learn.chatgpt.com/docs/app-server), [SDK MCP Rust](https://github.com/modelcontextprotocol/rust-sdk).

## Gate della release

Il changelog candidato è in [0.8.0](release-notes/0.8.0.md). Versioni di npm/lockfile/Tauri/Cargo devono coincidere; documentazione e collegamenti devono passare. Verificare suite Rust, Clippy/rustfmt, build/unitari frontend, Prettier, Playwright e harness nativi applicabili. Gli harness Windows e Playwright si eseguono in serie; nessuna inferenza o login aggiuntivo. Il registro live rimane **13 slot storici, incremento 0**.

La CI deve passare sul commit candidato, poi il workflow Release deve produrre installer e portable Windows, DMG universale macOS, DEB/AppImage Linux. Prima di rendere pubblica la release verificare pacchetti, manifesto SHA-256 e smoke dell'installer/portable; distinguere ciò dalle prove di OAuth e GUI native macOS/Linux. La chiusura riguarda il perimetro 0.8 approvato, con i requisiti non consegnati registrati nella roadmap successiva. La pubblicazione finale attende soltanto il superamento dei gate tecnici e la verifica dei pacchetti.
