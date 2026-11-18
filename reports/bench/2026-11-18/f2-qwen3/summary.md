# F2 · Qwen3-Coder e suite B

Qwen3-Coder `30b-a3b-q4_K_M` (manifest `06c1097efce0`), Ollama 0.40.16 in loopback su Windows, costo di inferenza 0 USD. Cinque seed appaiati per suite e 30 prove complessive. Il runner sintetico conserva prompt, limiti e gioco di fixture del [primo gate](../summary.md), usa il criterio di fallback F2 e salva in `filterTrace` gli script e le risposte della suite B. Il commit verifica i due loop Ollama del prodotto con fixture Axum.

| Suite | Successi TC → CM | Token mediani TC → CM | Round-trip mediani TC → CM |
| --- | ---: | ---: | ---: |
| A · Modifica di 20 file | 0/5 → 5/5 | 11.269 → 3.495 | 12 → 3 |
| B · Filtro di log | 5/5 → 0/5 | 2.782 → 1.232 | 2 → 2 |
| C · Diagnosi dei check | 0/5 → 0/5 | 858 → 1.089 | 2 → 2 |

In B entrambi i bracci leggono tutti e quattro gli shard in ogni prova. La risposta di ogni chiamata contiene un blocco MCP `{type: "text", text: "..."}` con più righe del log. Il seed 0 di Code Mode cerca `units: N` mentre il log usa `units=N`. I seed 1–4 trattano ciascun blocco come una riga con campi `status` e `units`. Le due implementazioni restituiscono sempre `0` e il modello lo copia nella risposta. Il replay T0 filtra le righe `state=PAID units=N` e restituisce `28`, lo stesso valore raggiunto da TC nei cinque seed. Gli script B sono validi per JavaScript. I fallback osservati sono 0.

In A le cinque esecuzioni TC raggiungono `benchmark step cap reached`. In C il modello tenta i tre nomi classici direttamente durante il braccio CM, che espone solo `lumen_code`. I guadagni nelle modifiche ripetute e la perdita semantica nel filtro riproducono l'esito del primo gate. Code Mode rimane opt-in per ID modello esatto.
