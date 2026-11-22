# F3 · Suite E su Ollama

Il 18 novembre 2026, Qwen3-Coder `30b-a3b-q4_K_M` (manifest `06c1097efce0`) su Ollama 0.40.16 ha eseguito 60 prove locali, per un costo d'inferenza di 0 USD. Ogni caso usa gli stessi cinque seed per tre bracci. Il criterio richiede la risposta esatta. I round-trip contano anche i tentativi senza una risposta completa. I token includono l'uso riportato da Ollama per il modello principale e le eventuali sottochiamate.

| Caso | Token corpus stimati | Classico | Code Mode | Code Mode con RLM |
| --- | ---: | ---: | ---: | ---: |
| Aggregazione | 209.080 | 0/5 | 3/5 | 5/5 |
| Multi-hop | 209.026 | 0/5 | 0/5 | 0/5 |
| Repository | 230.553 | 0/5 | 0/5 | 0/5 |
| QA documentale | 418.022 | 5/5 | 1/5 | 0/5 |

| Braccio | Successi | Token mediani | Round-trip mediani | Tempo mediano |
| --- | ---: | ---: | ---: | ---: |
| Classico | 5/20 | 1.327 | 2,5 | 6,85 s |
| Code Mode | 4/20 | 3.670,5 | 3 | 25,08 s |
| Code Mode con RLM | 5/20 | 1.667 | 2 | 10,47 s |

Qwen3 non ha invocato `llm()` nelle 20 prove che lo offrivano. Il 5/5 sull'aggregazione viene dagli script del modello principale. In uno smoke separato, uno script prefissato ha selezionato quattro righe con `context.grep()` e ha ottenuto `17` con una sottochiamata Ollama da 52 token in ingresso e 3 in uscita. La suite misura i bracci esposti al modello. Lo smoke verifica il percorso di esecuzione. Tracce della suite · Traccia dello smoke

Le quattro fixture locali coprono aggregazione, ricerca multi-hop, lettura di sorgenti Lumen e QA su documenti. Il corpus comprende testo ripetuto e le fixture sono sintetiche, con l'eccezione dei sorgenti Lumen inclusi nel caso repository. Le misure non sono punteggi sui dataset originali. Classico accede ai documenti tramite tool di ricerca e lettura. Code Mode usa `context` a effort `low`. Il terzo braccio espone anche `llm()` a effort `medium`. Il modello principale usa 8.192 token di finestra, 320 token di generazione per richiesta, un massimo di quattro round-trip e 45 secondi per prova. Il massimo ingresso osservato in una richiesta principale è 5.837 token. [OOLONG](https://arxiv.org/abs/2511.02817) · [RULER](https://arxiv.org/abs/2404.06654) · [LongBench v2](https://arxiv.org/abs/2412.15204)

La suite registra nove `Benchmark 45-second trial limit reached` e un `Benchmark step cap reached`. Il punteggio esatto penalizza anche risposte con il valore corretto ma formato diverso, come `2 + 7 + 3 + 5 = 17<tool_call>` nel braccio classico. Il pilot precedente, escluso dalle 60 prove, ha registrato `error decoding response body` dopo un tentativo di 120 secondi. La matrice precede la validazione più stretta degli argomenti numerici di `context` e il ramo JSON per la ricorsione a effort `max`. Lo smoke locale e i test Rust hanno usato il codice finale.

Per riprodurre la prova, imposta `LUMEN_BENCH_URL` a un endpoint Ollama loopback, `LUMEN_BENCH_MODELS` al tag indicato e `LUMEN_BENCH_OUTPUT` a un nuovo file JSONL. Il runner usa cinque seed di default.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p lumen-core --locked code_mode_bench::bench_e -- --ignored --nocapture --exact
```
