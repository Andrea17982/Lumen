# Code Mode Automatico · T1

23 novembre 2026. Solo locale (Ollama, gpt-oss 20B, effort low, 5 seed appaiati, stesse suite del [22 novembre](../2026-11-22/summary.md)): costo di inferenza **0 USD**. I JSONL grezzi restano locali accanto a questo file.

**Esito.** Automatico supera il gate: **45/50 → 50/50** successi, nessuna coppia in cui il classico vince (5 discordanti a favore di Automatico, McNemar esatto p = 0,0625), token e round-trip mai peggiori sulle mediane di suite. Code Mode sempre attivo, sullo stesso modello, aveva 44 → 46 ma costava il 42% di token in più. Automatico diventa il default.

## Cosa fa Automatico

Il turno parte con le chiamate classiche, quindi i task brevi non pagano nulla. Dopo cinque round di tool, o quando un risultato supera 24 KiB (trattenuto come documento di contesto), il resto del turno usa `lumen_code`. Il modello riceve un avviso con un esempio che usa l'ultimo tool chiamato e le regole del sandbox (niente `fs`, `require`, `process`).

## Risultati

| Suite | Successi TC → Auto | Token mediani | Round-trip mediani | Escalation |
| --- | ---: | ---: | ---: | ---: |
| A | 0/5 → 5/5 | 6694 → 3659 | 12 → 7 | 5/5 |
| B | 5/5 → 5/5 | 5161 → 5163 | 5 → 5 | 0/5 |
| C | 5/5 → 5/5 | 1067 → 1067 | 4 → 4 | 0/5 |
| D | 5/5 → 5/5 | 15819 → 15819 | 5 → 5 | 0/5 |
| F | 30/30 → 30/30 | 1680.5 → 1676 | 4 → 4 | 0/30 |

Su A (molte modifiche ripetute) Automatico risparmia il 45% dei token e il 42% dei round-trip. Su B, C, D ed F non scatta mai ed è identico al classico.

## Iterazioni scartate

- **Escalation silenziosa** (cambia solo la lista dei tool): su A 0/5. Il modello continuava una chiamata per round; token −28%, ma nessun successo.
- **Avviso senza esempio**: su A 0/5. Il modello scriveva codice Node (`require('fs')`, `return` fuori da una funzione), due errori, poi si arrendeva (parziale 0,1 contro 0,3 del classico).

## Limiti

- Solo gpt-oss; le famiglie non misurate (Qwen3-Coder perdeva qualità in Code Mode) restano classiche anche in Automatico.
- Claude Code, Codex e OpenCode restano sulle chiamate dirette in Automatico: il beneficio misurato (T3, risultati grandi) richiede che Lumen faccia da proxy ai server MCP del progetto. Va misurato su T3 prima di cambiarlo.
- API a pagamento (T2) non misurate: niente chiavi API.
- Il benchmark limita i turni a 12 round; nell'app il limite è 32 (API) o 64 (Ollama).
