# Quota e cambio account senza duplicare lavoro — 0.8

Protocollo del 12 novembre 2026, baseline `086469a`. Riprodurre il cambio automatico account dopo errore di quota con effetti già osservati o risposta parziale. Criterio: una sola scrittura nel progetto temporaneo, testo parziale conservato e nessun secondo account invocato dopo lavoro osservato. Prima del lavoro osservato resta possibile un solo cambio account con identità effettiva corretta. Cooldown, permessi e cancellazione devono restare rispettati.

Prove con CLI offline e IPC/SQLite reali, senza inferenza. Budget live invariato a 13 slot. Il guard riguarda attività osservata: eventi mancanti o effetti non dichiarati dalle CLI non costituiscono una garanzia exactly-once.

## Implementazione

Il guard introdotto per la sessione mancante è condiviso con il chiamante che gestisce la quota, senza dedurre il risultato dalle attività persistite o dal solo testo. Rimane monotono anche se la sessione mancante causa prima un recupero con contesto completo: se quel recupero esegue uno strumento e poi raggiunge la quota, non può avviare un terzo tentativo su un altro account. Cooldown e messaggio di errore restano registrati; il testo parziale non viene cancellato. Nessun nuovo reinvio manuale o conferma è aggiunto all'interfaccia.

## Evidenza e limiti

Il test nativo sulla baseline rileva due scritture invece di una per `QUOTA_GUARD_TOOL`. La candidata controlla quattro casi: strumento seguito da quota, output parziale seguito da quota, ripresa mancante seguita da strumento/quota, e quota prima del lavoro osservato con passaggio singolo da Claude a GPT. I contatori delle invocazioni sono strumentazione locale e le scritture sono limitate al workspace temporaneo della fixture.

La suite Rust Windows passa 170 test, con 14 verifiche opt-in ignorate. Il test del guard esistente è esteso al criterio condiviso con il fallback di quota, includendo eventi sconosciuti, testo parziale e cancellazione. Passano Clippy con warning negati e build debug.

Le prove non certificano tutte le forme di quota di ogni provider, né effetti non segnalati dalle CLI. Il classificatore delle stringhe di errore e le politiche di routing restano quelli precedenti. Le chiamate reali ai modelli, l'autenticazione e il budget live non vengono usati o cambiati da queste fixture. [Piano di chiusura in cinque tranche](roadmap-0.8.md) e prerequisiti residui restano distinti dai risultati della correzione.

[Roadmap](roadmap-0.8.md) · [Documentazione](README.md)
