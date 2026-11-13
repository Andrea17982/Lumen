# Tranche 14 — Auto/Orchestra e continuità

Baseline `6628c27`, protocollo offline del 12 novembre 2026. Prima delle modifiche riprodurre tre difetti dello scheduler: dipendenze fallite considerate completate, budget tentativi superabile dopo ripresa e dipendenze inesistenti lasciate in attesa. Verificare che ripresa non rimborsi ammissioni già consumate.

Per i worker, distinguere errori prima del lavoro da errori dopo attività osservata: non ritentare automaticamente lavoro già avviato, conservare output parziale e cooldown. Completamenti e attività indipendenti devono restare utilizzabili. I limiti di concorrenza, quota e tentativi restano quelli esistenti.

Auto viene verificato sui preset e sui confini già approvati: sessioni dirette predefinite, Orchestra esplicita, aiuti nativi/cross-audit circoscritti dove supportati. Nessuna nuova harness universale di delega o parità tra provider è dedotta dai test. Budget live fermo a 13 slot; non ritentare autenticazione o inferenza.

[Roadmap](roadmap-0.8.md) · [Policy](auto.md)

## Risultati e correzioni

I tre casi dello scheduler fallivano sulla baseline e passano dopo la correzione. Solo `done` soddisfa una dipendenza: errori, cancellazioni e riferimenti inesistenti bloccano anche i dipendenti transitivi, lasciando eseguibili le attività indipendenti. La ripresa conserva il conteggio delle ammissioni già consumate e non avvia un quarto tentativo dopo i tre consentiti. Un risultato completato non viene rilanciato.

I worker condividono ora la protezione monotona dal replay dei turni ordinari: osservazione prima dei filtri, metadati neutri consentiti, attività sconosciute trattate prudentemente come lavoro. Dopo lavoro osservato, un errore è terminale per quel worker; la quota mantiene il cooldown. L'output parziale e la motivazione sono conservati entro 16.000 caratteri Unicode, riservando spazio alla spiegazione. Un errore prima del lavoro resta soggetto ai limiti di retry esistenti. Non si promette conservazione dei file parziali di un worker fallito: le lane seguono la pulizia esistente.

La prova nativa di Stop ha riprodotto un quarto difetto: l'abort del future saltava la terminazione del provider e lasciava vivi due processi della fixture. La cancellazione attende ora l'uscita dei worker tramite il segnale condiviso e la terminazione dei loro processi prima di rimuovere le worktree; non avvia nuove attività o retry. Lo Stop esplicito non diventa una ripresa automatica.

## Verifiche riproducibili

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo build --manifest-path src-tauri/Cargo.toml --locked
node tests/native-orchestra-continuity.mjs
node tests/native-quality.mjs
```

I cinque nuovi test Rust coprono dipendenze, ammissioni/tentativi, completamenti conservati, errori terminali e troncamento Unicode. La suite comprende anche concorrenza, quote, probe, cooldown, cancellazione e arresto di processi orfani.

Risultato locale finale: **175 test Rust passati, 14 probe opt-in ignorati; Clippy senza warning, rustfmt e build debug passati; build frontend, 72 unitari, Prettier e controllo documentale passati**. Tutti e quattro gli scenari Orchestra e le 12 regressioni native qualità passano. [Record sintetico](data/auto-orchestra-continuity-0.8-results.json).

La matrice Windows WebView2 usa IPC reale, database/account temporanei, un repository Git dedicato e CLI PowerShell offline. Verifica quattro scenari: quota dopo scrittura/output parziale senza retry o avvio del dipendente; limite salvato a tre senza lancio CLI; completamento conservato e solo worker interrotto rilanciato con contesto approvato; Stop con verifica dei PID propri e rimozione delle lane. La carta Resume già consumata viene rifiutata. La scrittura SQL prepara soltanto carte interrotte nel database temporaneo: nessun comando di test è aggiunto al prodotto.

Le 12 regressioni native della qualità restano separate: due account Claude e un GPT simulati, isolamento delle chat, Rewind, Compact, bilanciamento quota, audit esplicito anche in Budget e indisponibilità dell'audit prima del lavoro. Il corpus deterministico Auto resta 14/14; non è una nuova misura live né un confronto della qualità dei modelli.

## Confini di accettazione

Auto conserva Budget/Default/Quality e Balance quota usage; Orchestra resta opt-in. Gli aiuti nativi dipendono dal protocollo del provider e il cross-audit rimane quello circoscritto già implementato. Non si introduce un nuovo coordinatore universale, né configurazione manuale aggiuntiva in Auto. La ripresa simultanea della stessa carta da client concorrenti e le interruzioni durante integrazione/finalizzazione non ricevono qui una nuova certificazione.

Nessuna inferenza o autenticazione live, nessuno slot aggiuntivo: budget storico fermo a 13. Qualità multiaccount reale, parità tra provider e comportamento desktop macOS/Linux restano non verificati; la CI multipiattaforma è distinta dalle prove native Windows. Versione e release pubblica restano 0.7.1. Restano le tranche 15, 17 e 18.
