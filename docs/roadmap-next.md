# Roadmap successiva alla 0.8

Il 13 novembre 2026 il proprietario ha approvato la pubblicazione della 0.8 con i limiti espliciti delle integrazioni verificate. Questa roadmap conserva requisiti e limiti non consegnati; **non sono funzionalità dichiarate complete, né esclusioni definitive**. La versione della prossima release non è ancora fissata. [Perimetro 0.8](release-readiness-0.8.md).

## Compatibilità universale — requisito mantenuto

| Lavoro | Criterio per chiuderlo |
| --- | --- |
| Connettori cloud Claude e altri runtime proprietari | Adattatore verificato nel runtime proprietario, con identità/account, consensi, cancellazione, errori e costo visibili. Usare API realmente disponibili; non simulare portabilità copiando credenziali. Verificare GitHub/Gmail/Drive sugli account autorizzati. |
| Hook, mods, comandi e agenti nativi | Conservare eventi, semantica, risorse e policy del componente originale, oppure indicare precisamente quale parte non è supportata. Le sole skill/MCP estratte da un plugin non certificano il plugin intero. |
| Trasporti/formati/policy ancora non adattati | Aggiungere gli adattatori necessari per legacy SSE, WebSocket, OAuth/helper e configurazioni non portabili, senza scartarne i vincoli. Streamable HTTP con risposta SSE della 0.8 non è il trasporto SSE legacy. |
| Namespace condivisi in modalità ristrette native | Aggiungere un filtro/esecutore per-tool anche nei client nativi per abilitare servizi portabili in Read only/File edits. Nella 0.8 soltanto il gateway filtrato del proprietario è esposto in quei modi; Ollama applica già il proprio filtro. Non autorizzare un intero namespace sconosciuto come scorciatoia. |
| Estensioni MCP e aggiornamenti dei runtime | Task e notifiche proprietarie, prompt/API non esposti dal gateway dell'account e UI delle estensioni richiedono contratti e prove indipendenti. Non inferire la copertura da un tool fixture funzionante. |
| Matrice reale account/versioni/desktop | Prove autorizzate delle integrazioni su macOS/Linux/Windows con gli account effettivi, incluse autenticazione scaduta, isolamento, revoca e richieste concorrenti. CI/build e simulazioni restano evidenze diverse. |

## Qualità, prestazioni e ricerca ancora aperte

- Qualità live e continuità della topologia effettiva con due Claude e un ChatGPT; inferenza/TTFT e costo dei gateway sui carichi reali. Il budget storico rimane 13 slot: una nuova campagna richiederà un budget esplicito, non viene avviata automaticamente.
- Estendere e calibrare gli aiuti ordinari/cross-provider soltanto quando il beneficio su qualità, attesa e costo è dimostrato. Auto resta semplice e Orchestra esplicita; non è già consegnato un coordinatore universale di tutte le estensioni.
- Diagnosticare la deriva delle aperture ripetute nello stesso processo e ottenere baseline assolute/cold-start stabili. Non estendere le misure a processo fresco della 0.8 a questo carico non risolto.
- Rivalutare virtualizzazione compatibile con ricerca/selezione/accessibilità. Il prototipo che perdeva contenuti è escluso, non una soluzione da riattivare senza gli stessi gate.
- Eventuali prototipi Markdown/ANSI Rust, pool CLI, metadati posizionali e allocazione globale del contesto richiedono misure prima/dopo e preservazione di requisiti, account, permessi e cleanup. Nessuna migrazione per il solo cambio di tecnologia.
- Estendere la semantica del grafo oltre il corpus mirato: alias arbitrari, commenti/stringhe complessi, UID Godot e altri casi richiedono un oracolo indipendente e prove di refresh/handoff.

Il registro dei risultati storici resta nella [roadmap 0.8](roadmap-0.8.md), nei [candidati della tranche 17](final-acceptance-0.8.md) e nella [matrice delle integrazioni](shared-capabilities-0.8.md). I dati esclusi/inconcludenti non vengono eliminati o riscritti come successi.
