# Tranche 15 — contesto, grafo e budget dei prompt

Protocollo offline del 12 novembre 2026, baseline `250e6ec`. Budget live invariato: 13 slot storici, nessuna nuova inferenza o autenticazione. Release pubblica ferma a 0.7.1.

Prima delle modifiche, misurare il payload prodotto da `build_prompt` su fixture React/TypeScript e Godot e verificare: requisiti/decisioni identici ripetuti nella proiezione dello stato; allegati duplicati risolti allo stesso file; anteprime che superano il budget di 48 KiB del contenuto allegato; perdita dei requisiti centrali di note di progetto aggiornate durante la ripresa; contratti del grafo omessi senza segnalazione del limite. Confrontare byte e token locali o200k, senza equipararli al conteggio o alla qualità del provider.

Conservare lo stato completo con fonti, snapshot, requisiti, correzioni e controlli; rimuovere soltanto duplicati esatti della stessa fonte già rappresentati integralmente. Allegati esclusi dall'inline restano riferimenti leggibili dal provider. Le note aggiornate non devono perdere requisiti silenziosamente. Nessuna modifica ai formati nativi CLI/MCP. Il grafo rimane approssimato: verificare il corpus TS/Godot esistente e dichiarare ogni proiezione incompleta.

Misurare anche i prompt effettivamente consegnati alla CLI offline nell'app Windows: avvio, continuazione invariata, modifica delle note, passaggio tra account e cartella. Riutilizzare le regressioni native Rewind/Compact e Orchestra. Un candidato senza prova end-to-end di comprensione/produzione del modello rimane non dimostrato: JSON posizionale, parser nuovo, budget globale e riserva d'output non vengono dedotti da un risparmio sintetico.

[Roadmap](roadmap-0.8.md) · [Continuità Auto/Orchestra](auto-orchestra-continuity-0.8.md)

## Risultati e correzioni

I cinque test mirati falliscono sulla baseline e passano dopo le correzioni. Il report conserva separatamente il corpus di costruzione e i payload intercettati alla CLI: [risultati sintetici](data/context-graph-budget-0.8-results.json). Le identità/cartelle temporanee differiscono tra le esecuzioni: i conteggi locali o200k possono oscillare leggermente. Non sono token fatturati, misure di latenza o prove di qualità del codice.

| Payload del corpus fisso | Byte prima → dopo | Token locali prima → dopo |
| --- | --- | --- |
| React/TypeScript, richiesta e allegato ripetuti | 7.305 → 4.719 (−35,4%) | 1.198 → 862 |
| Godot, stessa classe di requisiti con cronologia lunga | 106.119 → 103.533 (−2,4%) | 17.033 → 16.698 |
| Anteprime di 40 allegati Unicode, solo contenuto | 81.920 → 49.152 (−40%) | Payload completo: 44.881 → 28.321 |

Il requisito nel prompt completo passa da quattro a due copie: una nello stato storico e una nella conversazione. Lo stato renderizzato elimina solo coppie fonte/testo identiche già mostrate **integralmente**; copie di fonti differenti, correzioni e proiezioni già troncate non sono soppresse. Il goal conserva l'ID di origine. Il database e lo snapshot completo non perdono fatti e i budget per categoria restano quelli esistenti. Anche il contesto destinato ai worker beneficia della stessa proiezione; manifest, scope, dipendenze e regole di esecuzione restano presenti.

Gli allegati vengono deduplicati per percorso canonico, compresi riferimenti relativi/assoluti allo stesso file. Due file distinti con lo stesso contenuto restano due allegati. Inline e anteprime consumano entrambi il budget di 48 KiB di **contenuto UTF-8**; le anteprime rispettano i confini Unicode e gli altri file restano riferimenti con percorso. Il limite non include envelope/percorsi: nel corpus il payload completo passa da 94.030 a 61.262 byte. Oltre 50 selezioni viene segnalata l'omissione già prevista dal limite esistente.

Le note aggiornate durante una sessione nativa non vengono più tagliate a 6.000 caratteri con un riferimento inesatto a un messaggio: l'intero `LUMEN.md` corrente viene consegnato una volta quando cambia la revisione. La versione della proiezione entra nella revisione, così le note riconosciute da una versione precedente con testo tagliato vengono reinviate al primo turno dopo l'aggiornamento. Nei turni successivi invariati non vengono ripetute. Non si riduce il requisito per ottenere una misura più piccola.

Nel grafo, omettere contratti oltre i 16 visualizzati segnala ora la proiezione limitata; anche il merge che conserva al massimo 24 contratti marca il troncamento. L'avviso invita a interrogare target più circoscritti. Nessun parser, risolutore o formato nativo MCP viene sostituito.

## Payload nativi e continuità

Sei catture per versione nell'app Windows con CLI offline: avvio, continuazione invariata, note modificate, secondo turno invariato, cambio account e cambio cartella. I dati grezzi sono soltanto nelle cartelle temporanee dei due account Claude simulati; il record versionato contiene conteggi e verifiche, senza prompt o percorsi privati.

| Input effettivamente consegnato | Byte prima → dopo | Esito |
| --- | --- | --- |
| Avvio | 29.489 → 27.247 | Requisiti/Unicode e note presenti, allegato una volta |
| Continuazione invariata, anche dopo aggiornamento | 758 → 758 | 148 token locali; note/storia non ripetute |
| Note aggiornate | 6.989 → 23.402 | Il requisito centrale omesso nella baseline è presente |
| Cambio account | 30.861 → 28.793 | Requisito conservato, note correnti |
| Cambio cartella esplicito | 8.879 → 6.811 | Invocazione nella nuova cartella, nuove note, nessuna vecchia nota |

La crescita quando cambiano note lunghe è intenzionale: include il contenuto che la baseline perdeva. Non si sommano questi casi in un risparmio percentuale generale. Le 12 regressioni qualità native passano su due account Claude e GPT simulati: isolamento chat, Rewind, Compact ripetuto, evidenze e audit/quota. Passano i quattro scenari Orchestra e i cinque casi del grafo MCP reale (dichiarazioni/linee, caller/test, modifiche/rinomine/cancellazioni e ricarica cache). Il corpus Rust degli import TS/Godot conserva le 23 aspettative già definite; non si deduce copertura semantica generale.

## Riproduzione e accettazione

```powershell
# Baseline: applicare i cinque test context_budget a 250e6ec, senza le correzioni.
cargo test --manifest-path src-tauri/Cargo.toml --locked context_budget -- --nocapture
# Candidato finale:
cargo test --manifest-path src-tauri/Cargo.toml --locked -- --nocapture
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo build --manifest-path src-tauri/Cargo.toml --locked
node tests/native-context-budget.mjs
node tests/native-quality.mjs
node tests/native-orchestra-continuity.mjs
node tests/native-graph.mjs
```

Le prove WebView2 vanno eseguite in serie. `native-context-budget.mjs --baseline` richiede il binario debug baseline e la fixture di cattura; il comando senza opzioni richiede il candidato. `context-budget-report.mjs` aggrega i file baseline/candidato mantenuti sotto `.impeccable/context-budget`, i risultati delle regressioni native e il log Rust completo. Nessun percorso speciale di test viene aggiunto al backend del prodotto.

Accettazione locale: **180 test Rust passati, 14 probe opt-in ignorati**, Clippy senza warning, build debug e rustfmt; build frontend, 72 unitari, Prettier e controllo link/versioni. Sono prove offline Windows, separate dalla CI macOS/Linux e dalla qualità degli agenti live.

## Budget e candidati non dimostrati

Il budget di contenuto allegato è un limite effettivo; i budget esistenti di cronologia (100.000 caratteri) e categorie dello stato sono proiezioni distinte. Note, skill, layout, diff, envelope e strumenti possono aggiungere contenuto. Il context meter rimane una stima parziale e non misura tutto l'input nativo. Non viene introdotto un tetto globale per modello o una riserva garantita di output: il provider gestisce inoltre storia, tool e compattazione nativi. I fatti omessi restano nello snapshot completo con fonti; gli estratti della cronologia/log mantengono i limiti già documentati.

Non si adotta JSON posizionale: il risparmio sintetico precedente non dimostra migliore comprensione o produzione del modello. Non si introduce tree-sitter: questo corpus non dimostra un beneficio rispetto al parser attuale; alias/configurazioni arbitrarie, commenti inline/blocco, stringhe multilinea, UID Godot e semantica completa restano limiti. Rilevanza automatica, una nuova allocazione globale del contesto e risparmio di token/qualità end-to-end restano non dimostrati nel budget live disponibile. I formati ufficiali CLI/MCP rimangono intatti. Tranche 15 conclusa in questo perimetro; restano 17 e 18.
