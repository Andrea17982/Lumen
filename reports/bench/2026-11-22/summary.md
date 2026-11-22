# Code Mode · 0.12.1, ottimizzazioni e nuove suite

22 novembre 2026. Tutte le esecuzioni sono locali (T0 scriptato, T1 Ollama): costo di inferenza **0 USD**. I JSONL grezzi restano locali accanto a questo file. Confronto con il [primo gate](../2026-11-18/summary.md).

**Esito.** Nessuna configurazione T1 supera il gate §0. gpt-oss 20B (effort low) supera per la prima volta la soglia di **qualità** (TC 44/50 → CM 46/50, IC 95% [0, +12] pp) ma non quella di **efficienza**: sui task da una o due call Code Mode costa più token. Qwen3-Coder 30B peggiora sui task di codice (F) e resta a 0/5 sul filtro di log (B). Code Mode resta opt-in per tutti i modelli locali. Sulle CLI (T3, Codex e Claude Code) i tool MCP ora passano solo da `lumen_code`: con risultati grandi Claude risparmia il 90% dei token non in cache e sbaglia meno; con risultati piccoli Code Mode costa qualche richiesta in più. Resta opt-in. T2 (API) non è stato misurato: non ci sono chiavi API.

## Cosa è cambiato rispetto al 18 novembre

- **Errori azionabili:** riga dello script e suggerimento negli errori; nome di tool sconosciuto → "did you mean"; argomento non oggetto spiegato; `console.*` come alias di `log`; `r.text` unisce i blocchi di testo MCP.
- **Esempi onesti:** l'esempio nella descrizione nomina solo un tool offerto nel turno (prima mostrava sempre `lumen_read_file`, da cui Qwen3 inventava `lumen_scan_log`).
- **Dichiarazioni compatte (§2.2):** firme di una riga con la prima frase della descrizione; oltre 40 tool solo i nomi più `searchTools`/`describeTool` nel sandbox.
- **Chiamate dirette:** se il modello chiama un tool del catalogo invece di `lumen_code`, Lumen lo esegue come script di una riga che chiama quel tool, con lo stesso dispatch e gli stessi permessi. Era la causa principale degli 0/5 di gpt-oss su B, C ed F.
- **Contesto Ollama:** Lumen non impostava `num_ctx`; Ollama usava 4.096 token e troncava l'inizio del prompt, dove stanno i tool. Ora la finestra parte da 8K, segue la richiesta ed è limitata dal modello. Il benchmark usava già 8.192, quindi l'effetto si vede nell'app, non in queste tabelle.
- **`context.grep`** accetta anche un `RegExp`.
- **Misurato e scartato:** runtime pre-avviato (avvio a freddo mediana 0,75 ms, p90 1,3 ms contro round-trip di secondi); cache read-only nel turno (letture locali in millisecondi, nessun effetto su token o round-trip, rischio di dati stantii dopo un comando); campione automatico del primo output di ogni tool (+136 token, Qwen3 resta a 0/5 su B).

## T0 · scriptato

| Suite | Successi TC → CM | Token TC → CM | Round-trip TC → CM |
| --- | ---: | ---: | ---: |
| A | 1/1 → 1/1 | 8943 → 994 | 5 → 2 |
| B | 1/1 → 1/1 | 2175 → 886 | 2 → 2 |
| C | 1/1 → 1/1 | 605 → 874 | 2 → 2 |
| D | 1/1 → 1/1 | 14072 → 1714 | 3 → 2 |
| F | 6/6 → 6/6 | 7065 → 6174 | 18 → 12 |

D annuncia 88 tool sintetici con nomi e schemi dei server MCP ufficiali (GitHub, filesystem, Playwright, sqlite, memory, fetch, git): Code Mode con catalogo compatto riduce i token dell'88%. F sono i sei task del corpus Compete, giudicati da `node check.js`. Token stimati con tiktoken o200k.

## T1 · Ollama, 5 seed appaiati

**Qwen3-Coder 30B** (`30b-a3b-q4_K_M`, effort di default)

| Suite | Successi TC → CM | Token mediani | Round-trip mediani | Errori script CM |
| --- | ---: | ---: | ---: | ---: |
| A | 0/5 → 4/5 | 11269 → 5468 | 12 → 4 | 11 |
| B | 5/5 → 0/5 | 2782 → 1316 | 2 → 2 | 0 |
| C | 5/5 → 5/5 | 856 → 1317 | 2 → 2 | 0 |
| D | 5/5 → 0/5 | 43581 → 47744 | 6 → 8 | 10 |
| F | 30/30 → 10/30 | 5283 → 24028.5 | 6 → 12 | 111 |

**gpt-oss 20B** (effort low)

| Suite | Successi TC → CM | Token mediani | Round-trip mediani | Errori script CM |
| --- | ---: | ---: | ---: | ---: |
| A | 0/5 → 5/5 | 6694 → 5491 | 12 → 5 | 15 |
| B | 5/5 → 5/5 | 5163 → 6491 | 5 → 5 | 0 |
| C | 5/5 → 5/5 | 1067 → 1998 | 4 → 4 | 0 |
| D | 4/5 → 1/5 | 15872 → 2582 | 5 → 3 | 0 |
| F | 30/30 → 30/30 | 1681 → 2418 | 4 → 4 | 0 |

Rispetto al 18 novembre, gpt-oss low passa in Code Mode da 2/5 a 5/5 su A, da 0/5 a 5/5 su B e su C. Qwen3 scende su A da 5/5 a 4/5; F è nuova (10/30, contro 15/30 in una prima esecuzione di oggi senza instradamento delle chiamate dirette).

## Gate T1

| Modello | Coppie | Successi TC → CM | Δ qualità (IC 95%) | Risparmio token | Risparmio round-trip | McNemar p | Esito |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Qwen3-Coder 30B / default | 50 | 45 → 19 | -52 pp [-56, -50] | -334.9% | -83.3% | 0.00000616 | Qualità sotto soglia |
| gpt-oss 20B / low | 50 | 44 → 46 | +4 pp [0, 12] | -42.1% | 0% | 0.754 | Efficienza sotto soglia |

Il risparmio è sulle mediane appaiate di tutte le suite; F pesa 30 coppie su 50. Su A (molte call) gpt-oss risparmia il 58% dei round-trip e il 18% dei token; su D l'84% dei token. È il segnale per la scelta automatica CM/TC per task (§2.9): Code Mode solo quando il task richiede molte call.

## Perché B fallisce con Qwen3

Le tracce mostrano due errori indipendenti. Il modello scrive `split('\\\\n')` con il backslash raddoppiato nel JSON dell'argomento, quindi divide sulla sequenza letterale `\\n` e vede una sola riga; lo stesso accadeva con `/units: (\\\\d+)/` nelle tracce del 18 novembre. E indovina il formato (`line.split(' ')[2]` legge `state=PAID`, non `units=4`) senza aver visto i dati, che in TC legge nel contesto. Il campione automatico del primo output non lo corregge: il modello si fida del proprio parser. gpt-oss, con lo stesso runtime, risolve B 5/5.

## E · contesto lungo e `llm()`

| Braccio | Successi (5 casi × 5 seed) | Chiamate `llm()` |
| --- | ---: | ---: |
| Classico | 5/25 | – |
| Code Mode | 0/25 | – |
| Code Mode con RLM | 5/25 | 0 |

Nuovo caso **classificazione** simile a OOLONG: 36 ticket in linguaggio naturale (~130k token), la categoria va capita, non cercata con una regex. Baseline con la descrizione 0.12.0: 0/15, nessuna chiamata `llm()`. Con la nuova descrizione (limiti del turno, pattern seleziona → batch → `Promise.all` → combina, esempio di classificazione): 0/15, ancora **0 chiamate**. Le tracce mostrano Qwen3 che legge i ticket con `context.grep` e poi classifica "a mano" o con parole chiave. Su Qwen3 il problema non è l'affordance; resta da misurare su modelli più capaci in T2.

## App pacchettizzata (WebView2)

Build release con identità isolata (dati in una cartella dedicata, nessun dato reale). Con Qwen3 il modello non vedeva `lumen_code` per la finestra Ollama da 4K: corretto (vedi sopra); dopo la correzione Qwen3 chiama `lumen_code` ma come testo XML che il parser di Ollama non converte. Con gpt-oss gli script girano davvero: righe **Code** e figlie `code-call`, chiamate a Moli (`browser_navigate`, `browser_evaluate`), errori restituiti allo script e corretti dal modello. In Read only `browser_evaluate` è stato negato dal profilo, come per una call classica. Nessun errore in console; righe Code verificate a 1440, 768 e 390 px in chiaro e scuro. L'esito del compito dipende dal modello (in Autopilot gpt-oss si è confuso con una skill importata).

## Rewind su un repository grande

Sonda `rewind_scale_probe`: 50.000 file tracciati, 2.500 modifiche (1% dei file modificati e 2.000 nuovi), macchina carica dal benchmark.

| | Checkpoint | Anteprima | Restore | Esatto |
| --- | ---: | ---: | ---: | --- |
| Prima | 9,1 s | 10,5 s | 66,1 s | sì |
| Dopo (solo i file cambiati) | 7,0 s | 10,8 s | 11,3 s | sì |

## T3 · CLI con abbonamento (Codex e Claude Code)

CLI ufficiali negli slot account esistenti, Autopilot, 3 seed, prompt identici nei due bracci, quota dell'abbonamento (nessuna chiave API). Modelli: `gpt-6.2-sol` (Codex) e Claude Haiku. I token vengono dai log locali delle CLI (`token_count` di Codex, `usage` dei transcript di Claude): "non in cache" = input non in cache + output, la parte che consuma la quota.

**Prima prova: `lumen_code` accanto ai tool diretti.** In 24 sessioni Codex e 24 Claude nessuno dei due modelli ha scelto `lumen_code` se non richiesto esplicitamente (una sessione di controllo lo chiama e risponde correttamente): con Moli e MCP diretti disponibili, Code Mode non cambiava nulla.

**Seconda prova: solo tramite codice**, come la Code Mode di Anthropic. Con Code Mode attivo Lumen non dà più alla CLI Moli e gli MCP condivisi o di progetto come tool diretti: li espone solo dentro `lumen_code` (stessi permessi). Shell e file nativi della CLI restano. A, F = file e test (la CLI usa la sua shell); M = Moli; D = issue e file su un server MCP di progetto con 64 tool; G = 12 letture piccole; H = 12 letture da ~30 KB di cui serve un solo dato.

**Codex**

| Task | Successi TC → CM | Token non in cache (mediana) | Richieste al modello | CM ha usato `lumen_code` |
| --- | ---: | ---: | ---: | ---: |
| A | 3/3 → 3/3 | 22340 → 22219 | 3 → 3 | 0/3 |
| F | 3/3 → 3/3 | 10013 → 21485 | 4 → 4 | 0/3 |
| M | 3/3 → 3/3 | 10664 → 10998 | 3 → 3 | 3/3 |
| D | 3/3 → 3/3 | 24620 → 21939 | 6 → 8 | 3/3 |
| G | 3/3 → 3/3 | 10199 → 17331 | 4 → 6 | 3/3 |
| H | 3/3 → 3/3 | 12450 → 14151 | 3 → 7 | 3/3 |

**Claude Code (Haiku)**

| Task | Successi TC → CM | Token non in cache (mediana) | Richieste al modello | CM ha usato `lumen_code` |
| --- | ---: | ---: | ---: | ---: |
| A | 3/3 → 3/3 | 16876 → 16410 | 3 → 3 | 0/3 |
| F | 3/3 → 3/3 | 16113 → 16109 | 3 → 3 | 0/3 |
| M | 3/3 → 3/3 | 14763 → 16243 | 2 → 3 | 3/3 |
| D | 3/3 → 3/3 | 18098 → 28982 | 4 → 8 | 3/3 |
| G | 3/3 → 2/3 | 20709 → 22342 | 5 → 7 | 3/3 |
| H | 1/3 → 2/3 | 356751 → 36537 | 7 → 10 | 1/3 |

- **Dove aiuta:** risultati grandi (H). Claude in modalità classica riversa ~30 KB per file nel contesto: ~357k token non in cache e 1/3 corretti. Con Code Mode lo script tiene i file fuori dal contesto: ~37k (−90%) e 2/3.
- **Dove costa:** task con risultati piccoli (D, G). Script e ricerca del tool aggiungono 2–4 richieste. Codex fa già chiamate parallele in un turno e tronca da solo i risultati MCP lunghi, quindi non risparmia neanche su H.
- **Qualità:** Codex 18/18 in entrambi i bracci; Claude 16/18 → 17/18. Due correzioni sono venute da queste tracce: il nome leggibile del server (`acme`) nella descrizione dei tool condivisi e il divieto dei server che Claude Code caricava da solo dal `.mcp.json`.
- **Limiti:** 3 seed e un solo modello per CLI; Haiku è il più piccolo dei Claude. Il server fixture era inizialmente dentro il workspace e Claude lo leggeva come file: D, G e H sono stati rifatti con il fixture fuori dal workspace. In un caso Claude ha avviato il fixture da shell invece di usare MCP.

## Metodo e limiti

- T1: Ollama 0.40.16 in loopback, temperatura 0, finestra 8.192, 768 token per richiesta, fino a 12 round-trip; bracci alternati entro le coppie. Il gate usa coppie per suite, seed e caso; IC bootstrap a blocchi per seed (2.000 campioni); McNemar esatto. Con 5 seed le stime restano esplorative.
- Durante il T1 finale la macchina compilava e misurava il Rewind: i tempi non sono confrontabili, i token e i round-trip sì.
- Qwen2.5-Coder 7B e Llama 3.2 3B non sono stati rimisurati: nel primo gate non risolvevano nulla in nessun braccio.
- Il caso E di classificazione è sintetico; i risultati non sono punteggi OOLONG.
- T3: un modello per CLI, 3 seed. Il primo pilota Claude usava uno slot con un abbonamento non più attivo; quello slot è stato disconnesso.

## Riproduzione

```powershell
$env:LUMEN_BENCH_URL = 'http://127.0.0.1:11434'
$env:LUMEN_BENCH_OUTPUT = "$PWD\reports\bench\<data>\results.jsonl"
cargo test --manifest-path src-tauri/Cargo.toml -p lumen-core --locked code_mode_bench::bench_t0 -- --nocapture --exact
$env:LUMEN_BENCH_MODELS = 'qwen3-coder:30b-a3b-q4_K_M'
cargo test --manifest-path src-tauri/Cargo.toml -p lumen-core --locked code_mode_bench::bench_t1 -- --ignored --nocapture --exact
$env:LUMEN_BENCH_MODELS = 'gpt-oss:20b'; $env:LUMEN_BENCH_EFFORT = 'low'
cargo test --manifest-path src-tauri/Cargo.toml -p lumen-core --locked code_mode_bench::bench_t1 -- --ignored --nocapture --exact
cargo test --manifest-path src-tauri/Cargo.toml -p lumen-core --locked code_mode_bench::bench_e -- --ignored --nocapture --exact
cargo test --manifest-path src-tauri/Cargo.toml -p lumen-core --locked gitops::tests::rewind_scale_probe -- --ignored --nocapture --exact
# T3, quota dell'abbonamento: richiede LUMEN_LIVE_CODE_MODE=1
$env:LUMEN_LIVE_CODE_MODE = '1'; $env:LUMEN_LIVE_PROVIDERS = 'gpt'
node tests/live-code-mode.mjs
```

## Ambiente

Windows 11 x64, Intel Core i9-13980HX, 31,8 GiB RAM, NVIDIA RTX 2000 Ada Laptop 8 GiB. Qwen3-Coder gira al 67% su CPU. Digest: qwen3-coder `06c1097efce0`, gpt-oss `f38aa0c53da5` (gli stessi del 18 novembre).
