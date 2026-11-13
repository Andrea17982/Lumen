# Continuità dopo Rewind e cambio incarico — 0.8

## Contratto fissato prima della correzione

Baseline: `26ab3b9`, Windows, test backend isolati senza inferenza. Due regressioni devono essere riprodotte prima delle modifiche:

- Rewind rimuove turni dalla chat ma non dalla conversazione remota della CLI. Nessun identificatore nativo del ramo precedente può essere riutilizzato: il prossimo turno deve ricostruire il contesto dalla cronologia conservata. Attività e conversazioni estranee devono restare intatte; un target inesistente non modifica dati.
- Un incarico esplicitamente nuovo (`New task:`, `Nuovo obiettivo:`, `Nuova attività:`, `/task `) ritira le verifiche dell'incarico precedente sia dallo stato di lavoro sia dal contesto inviato ai provider. Rewind del nuovo incarico rende nuovamente disponibili le verifiche conservate.

Criterio primario: entrambi i test falliscono sulla baseline e passano dopo la correzione; nessun rilancio, chiamata modello, modifica dei permessi o cancellazione di sessioni remote. La cronologia remota precedente resta nel provider ma non viene ripresa da Lumen. Il budget live resta a 13 slot.

## Verifica

Comando mirato: `cargo test --manifest-path src-tauri/Cargo.toml continuity_ -- --nocapture`.

I test controllano SQLite e la selezione effettiva della sessione nativa, non il comportamento di un modello live. Le prove di Windows desktop e i provider reali restano distinte da questi controlli backend. I prefissi sono convenzioni esplicite già esistenti: non viene introdotto un classificatore semantico per dedurre un nuovo incarico.

## Risultati

I due test riproducono entrambi i difetti sulla baseline prima della modifica (un altro test di continuità già esistente passa). Dopo la correzione passano 166 test Rust Windows, con 14 verifiche opt-in ignorate, e Clippy con warning negati. Un terzo test nuovo controlla la sostituzione esplicita delle evidenze durante una continuazione con note `LUMEN.md` ancora presenti e permessi read-only.

`node tests/native-quality.mjs` passa nove scenari Windows debug/WebView2, con IPC e SQLite reali, account isolati e CLI offline. Le due nuove prove, una Claude e una GPT, verificano ritiro dei vecchi check, Rewind, assenza dei vecchi marcatori nativi, conservazione delle validazioni e successivo turno con il contesto ricostruito. Le altre sette verificano continuità Claude/Claude/GPT, isolamento chat, quota e audit/fallback. Report locale: `.impeccable/review/native-quality-results.json`; nessuna inferenza.

La pulizia dei marcatori e il taglio cronologico condividono una transazione. Un target mancante non modifica il database. Il cambio non chiude né cancella conversazioni sul provider: riprendere direttamente quelle conversazioni fuori da Lumen può ancora mostrare i turni scartati. Ricostruire una sessione dopo Rewind può richiedere più contesto di una continuazione ordinaria; è il costo della coerenza, non un risparmio token dichiarato. Compact e le altre voci della matrice generale restano da estendere.

[Roadmap](roadmap-0.8.md) · [Documentazione](README.md)
