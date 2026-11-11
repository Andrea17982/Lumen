# Evidenze di verifica attraverso Compact — 0.8

## Protocollo prima dell'implementazione

Baseline `1b8a941`. Compact conserva nello stato di lavoro soltanto etichetta e stato dei check; il contesto provider legge invece dettagli e diagnostica dalle attività della chat corrente. Verificare se il passaggio a una nuova chat perde queste evidenze.

Criteri: riproduzione prima della modifica; etichetta/stato più recente, dettagli, diagnostica e sorgente originali conservati attraverso Compact ripetuto e riavvio SQLite. Nuovi check sostituiscono i precedenti con la stessa etichetta; nuova attività e cambio workspace non ereditano verifiche estranee; Rewind ripristina il seed coerente. I permessi correnti prevalgono sempre sullo stato trasferito. Vecchi seed JSON rimangono leggibili senza inventare dettagli assenti. Le evidenze restano dati storici, non istruzioni né prova della conclusione del compito.

Controlli backend isolati, quindi Compact effettivo via IPC Windows e CLI offline. Nessuna inferenza; budget live invariato a 13 slot. Nessuna misura di risparmio token o qualità generale del modello.

## Implementazione e risultati

Il test `compact_evidence_preserves_source_diagnostics_and_latest_status_across_restart` fallisce sulla baseline prima delle modifiche: il contesto provider non contiene più il check. La correzione aggiunge `checkEvidence` al JSON dello stato di lavoro, con valore di default vuoto per i vecchi dati. Non richiede una migrazione SQLite. Il seed contiene uno snapshot per valore dell'ultima evidenza di ogni etichetta, con ID del messaggio sorgente, report e diagnostica; non consulta successivamente la chat originale. La proiezione provider conserva il limite esistente di otto etichette, sei diagnostiche per check e testo troncato.

Passano 169 test Rust Windows (14 verifiche opt-in ignorate), Clippy con warning negati e build debug. Tre nuove regressioni backend controllano Compact ripetuto e riavvio, diagnostica/sorgente/stato più recente, isolamento per progetto, rimozione della chat originale, aggiornamento e Rewind, nuovo incarico, permessi e vecchi seed senza dettagli. Gli ID originali rimangono provenienza storica; se la chat originale è rimossa non diventano collegamenti a messaggi ancora consultabili.

Il collaudo `node tests/native-quality.mjs` aggiunge tre scenari Windows debug/WebView2: Compact verso un altro account Claude; secondo Compact con riepilogo modificato e passaggio a GPT, con evidenza inviata una sola volta; nuovo incarico seguito da Rewind e passaggio a un altro account con ripristino del seed. IPC e SQLite sono reali, CLI e account isolati sono offline. Report locale: `.impeccable/review/native-quality-results.json`.

Tutti i 12 scenari passano anche sul binario finale; passano formattazione e controllo dei riferimenti documentali.

I check trasferiti sono osservazioni storiche: Compact non li riesegue e non prova che i file attuali passino. I seed creati dalle versioni precedenti conservano i propri riepiloghi ma non recuperano dettagli già persi. Nessuna chiamata ai modelli né certificazione live o risparmio token.

[Roadmap](roadmap-0.8.md) · [Documentazione](README.md)
