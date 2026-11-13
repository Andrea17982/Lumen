# Accettazione e pubblicazione 0.8

Stato del 13 novembre 2026: **0.8.0 pubblicata; roadmap chiusa nel perimetro approvato**. [Release e download](https://github.com/Andrea17982/Lumen/releases/tag/v0.8.0). Il proprietario ha risposto esplicitamente «Pubblica 0.8 con limiti espliciti». La compatibilità universale non è dichiarata soddisfatta: connettori e runtime non coperti restano nella [roadmap successiva](roadmap-next.md). Il budget live resta invariato.

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

I gate richiesti sono passati sul commit candidato: CI, installer e portable Windows, DMG universale macOS, DEB/AppImage Linux, download e manifesto SHA-256. Le prove dei pacchetti restano distinte da OAuth e GUI native macOS/Linux. La chiusura riguarda il perimetro 0.8 approvato, con i requisiti non consegnati registrati nella roadmap successiva. Le evidenze di distribuzione sono riportate sotto.

## Verifiche locali del candidato

203 test Rust Windows passati dopo l'ultimo gate delle configurazioni condivise (14 opt-in ignorati); 72 frontend e 56 scenari in otto suite native sono passati. La CI ripete il gate Rust sul commit finale. Il probe con CLI Codex installata e server locale ritardato 2 s passa in 3.003 ms, senza login/inferenza: è interoperabilità, non prestazione di un account reale.

Il gate dei permessi omette i namespace MCP condivisi non filtrati dalle configurazioni native in Read only/File edits, compreso il percorso Google; lascia il gateway del proprietario in modalità filtrata. Ollama mantiene il proprio filtro per tool. Un'annotazione distruttiva esplicita prevale sul read-only hint. Configurazioni di intestazioni o abilitazioni plugin malformate sono rifiutate, non reinterpretate eliminandone i vincoli.

Il primo batch nativo ha incontrato un timeout all'avvio del tour. L'harness ora attende l'uscita dei propri processi prima di avviare la suite seguente e conserva diagnostica di avvio; la ripetizione delle otto suite passa. Il timeout iniziale non è presentato come una regressione dimostrata del prodotto.

## Commit e preflight dei pacchetti

Il tag `v0.8.0` identifica il commit [`47ed088dccb806afe61ed6a5b8a5d90124783604`](https://github.com/Andrea17982/Lumen/commit/47ed088dccb806afe61ed6a5b8a5d90124783604), passato nella [CI finale](https://github.com/Andrea17982/Lumen/actions/runs/43402643640). Il [preflight completo](https://github.com/Andrea17982/Lumen/actions/runs/43402643011) sullo stesso commit passa: 203 test Rust Windows, 195 macOS e 195 Linux, con 14 opt-in ignorati per piattaforma.

Installer e portable Windows superano i probe degli helper dopo installazione/estrazione. Il DMG montato contiene entrambe le architetture `arm64`/`x86_64`; DEB/AppImage superano gli stessi probe dopo estrazione. Ogni formato verifica discovery MCP grafo/browser, Moli incluso e SHA-256 degli 80 file di licenza delle nuove dipendenze. Il primo preflight aveva fallito soltanto l'ordine degli argomenti di `lipo`; il comando corretto e la ripetizione completa passano. Queste prove non equivalgono ad accettazione GUI/OAuth reale su macOS o Linux.

Il [workflow del tag](https://github.com/Andrea17982/Lumen/actions/runs/43405778712) passa su tutte le piattaforme e assembla la release. I cinque pacchetti scaricati corrispondono al [manifesto SHA-256 pubblico](https://github.com/Andrea17982/Lumen/releases/download/v0.8.0/SHA256SUMS.txt), alle dimensioni e ai digest GitHub. Il portable scaricato supera anche il probe locale degli helper e riporta versione file/prodotto 0.8.0. Nessuna apertura del profilo GUI reale è necessaria per queste prove.

La release è stata pubblicata il **13 novembre 2026 alle 10:13:56 UTC**, con changelog e limiti espliciti, e verificata come latest tramite API pubblica senza autenticazione. Il manifesto è stato riscaricato senza autenticazione e confrontato con quello verificato. [Registro strutturato: commit, workflow, pacchetti e hash](data/release-08-validation.json). I pacchetti restano non firmati. Il tag identifica il sorgente dei binari; i successivi aggiornamenti di documentazione su `main` registrano la pubblicazione senza spostarlo.
