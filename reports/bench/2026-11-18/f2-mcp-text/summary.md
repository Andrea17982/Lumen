# F2 · Suite B con il formato testuale MCP esplicito

La descrizione di `lumen_code` indica di selezionare i blocchi con `type === 'text'`, leggere `content[].text` e verificare il formato del testo prima di estrarre i dati. Un blocco può contenere più righe. La descrizione include un esempio di estrazione del testo generico.

Qwen3-Coder `30b-a3b-q4_K_M` (manifest `06c1097efce0`), Ollama 0.40.16 su loopback, effort di default, seed 0–4 e un confronto appaiato per seed. Stessi prompt, fixture, ordine dei bracci entro la suite e limiti della precedente [misura F2](../f2-qwen3/summary.md). Costo di inferenza 0 USD.

| Braccio | Successi F2 → nuova descrizione | Token mediani F2 → nuova descrizione | Round-trip mediani F2 → nuova descrizione |
| --- | ---: | ---: | ---: |
| Classico | 5/5 → 5/5 | 2.782 → 2.782 | 2 → 2 |
| Code Mode | 0/5 → 0/5 | 1.232 → 3.568 | 2 → 4 |

Le cinque prove Code Mode hanno prodotto tre errori a runtime ciascuna. Nel seed 0 il modello estrae `content[].text`, ma invoca il nome inesistente `lumen_scan_log` e non legge alcuno shard. Nei seed 1–4 invoca `bench_scan_log` correttamente, poi chiama `.filter()` direttamente sull'oggetto MCP. Il tentativo di recupero usa `console.error`, che genera `ReferenceError: console is not defined`. Il primo e il terzo script di ogni prova producono `TypeError: not a function`. Nessun errore è di sintassi e si osservano zero fallback.

Le tracce `filterTrace` del `results.jsonl` locale contengono gli script e le risposte osservate. Per ripetere il test, imposta `LUMEN_BENCH_SUITE=B` e usa la stessa [procedura T1](../summary.md#riproduzione).
