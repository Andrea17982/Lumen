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
- API a pagamento (T2) non misurate: niente chiavi API.
- Il benchmark limita i turni a 12 round; nell'app il limite è 32 (API) o 64 (Ollama).

## T3 · CLI in Automatico

Pomeriggio del 23 novembre, quota degli abbonamenti, Autopilot, 3 seed per suite, bracci alternati. Le suite sono le stesse T3 del [22 novembre](../2026-11-22/summary.md#t3--cli-con-abbonamento-codex-e-claude-code). In **Automatico** Lumen fa da proxy a Moli e ai server MCP condivisi e di progetto. La CLI li vede come tool diretti accanto a `lumen_code`. Un risultato oltre 8 KiB resta a Lumen come documento di contesto: la CLI riceve i primi 2 KiB e un'espressione `lumen_code` pronta per leggere solo ciò che serve. "Non in cache" significa input non in cache più output, cioè la parte che consuma la quota.

**Claude Code (Haiku)**, con i tool in nomi leggibili (`acme_filesystem_read_file`):

| Suite | Successi TC → Auto | Non in cache (mediana) | Richieste | Risultati trattenuti |
| --- | ---: | ---: | ---: | ---: |
| A | 3/3 → 3/3 | 17832 → 17869 | 3 → 3 | 0/3 |
| F | 3/3 → 3/3 | 17193 → 17441 | 3 → 4 | 0/3 |
| M | 3/3 → 3/3 | 15869 → 16578 | 3 → 4 | 0/3 |
| D | 3/3 → 3/3 | 19269 → 17440 | 4 → 4 | 0/3 |
| G | 1/3 → 2/3 | 24135 → 20867 | 18 → 15 | 0/3 |
| H | 1/3 → 2/3 | 196730 → 38075 | 19 → 18 | 2/3 |

In totale si passa da 14/18 a 16/18 successi, nessuna suite peggiora. I token non in cache scendono del 56% sulla somma delle mediane e dell'81% su H. A, F e M restano entro il 4%, dentro il rumore con 3 seed.

Una prima esecuzione con i nomi interni (`shared_<hash>`) dava già 13/18 → 16/18 e −81% su H. Peggiorava però D e G: Claude non riconosceva i tool e partiva esplorando con Glob, Bash e ToolSearch (G 7 → 19 richieste). Da lì vengono i nomi leggibili. Righe grezze: `t3-auto-claude-v1-hashed-names.jsonl`.

**Codex (`gpt-6.2-sol`)**: da 18/18 a 14/18, con più token. Su G e H Codex sceglie `lumen_code` per ogni lettura (H 24168 → 35066 non in cache). In classico tronca già da solo i risultati MCP lunghi, quindi H gli costa 24k contro i 197k di Claude e non c'è margine da recuperare. I quattro fallimenti sono sessioni ferme: Codex chiede un'approvazione per ogni tool del server `lumen_code` che non è segnato come approvato, e lo script non la concedeva. **Codex e OpenCode (non misurato) restano quindi sulle chiamate dirette in Automatico; Sempre resta disponibile.**

**Misura dei token.** Claude Code 2.1.327 non scrive più il transcript della sessione. Lumen registra ora i totali dell'evento `result` di Claude Code (attività `native-usage`, nascosta nell'interfaccia), e lo script di benchmark legge quelli.

**Limiti.** 3 seed, un solo modello per CLI (Haiku è il Claude più piccolo). Le fixture sono sintetiche. Non sono stati misurati Sonnet od Opus, né i server MCP reali.

```powershell
$env:LUMEN_LIVE_CODE_MODE = '1'; $env:LUMEN_BENCH_ARMS = 'TC,AUTO'; $env:LUMEN_LIVE_PROVIDERS = 'claude'
node tests/manual/live-code-mode.mjs
```
