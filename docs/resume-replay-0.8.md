# Recupero sessioni senza duplicare effetti — 0.8

Protocollo del 12 novembre 2026, baseline `90f632e`. Il fallback per sessione nativa mancante verifica soltanto l'assenza di testo visibile: riprodurre il caso in cui strumenti hanno già prodotto effetti, prima dell'errore di sessione.

Criterio fissato prima della modifica: la fixture nativa effettua una scrittura nel proprio progetto temporaneo, emette un avvio strumento e termina con errore di sessione, senza risposta testuale. La scrittura deve avvenire una sola volta; nessun fallback automatico dopo attività di lavoro osservata. Se la sessione manca prima di qualsiasi attività di lavoro, conservare l'unico recupero con contesto completo. Testare Claude e GPT offline, cancellazione, output parziale, eventi sconosciuti e metadati di trasporto.

Nessuna inferenza; budget live invariato a 13 slot. Il test rileva duplicazioni controllate, non certifica l'assenza di effetti non dichiarati dai provider reali.

## Implementazione

Un indicatore monotono per chiamata osserva gli eventi prima dei filtri per il lifecycle degli strumenti e prima della compattazione dello storico. Solo PID, identificatore di sessione, segnali di latenza e metadati quota/uso/finestra non contano come lavoro osservato; tutti gli altri tipi, compresi quelli sconosciuti, impediscono il replay. Non memorizza contenuti. Il recupero rimane limitato a un solo passaggio al contesto completo, con lo stesso account, modello, permessi e segnale di cancellazione. Se è già stata osservata attività, l'errore invita a esaminare il lavoro registrato prima di riprovare, senza reinviare il compito.

## Verifica e limiti

`node tests/native-resume-replay.mjs` riproduce sulla baseline una scrittura duplicata nella fixture Claude: due effetti per una richiesta. Usa CLI offline, IPC e SQLite reali, progetti temporanei isolati. I due casi per provider controllano l'effetto dello strumento e il numero di invocazioni nel recupero senza attività di lavoro; il contatore nel secondo caso è strumentazione della fixture, non una modifica richiesta dal compito. Non autentica account e non usa progetti personali.

`cargo test --manifest-path src-tauri/Cargo.toml --locked` passa 170 test Windows, con 14 controlli opt-in ignorati. Il nuovo test del guard copre metadati, testo parziale, cancellazione, errori non pertinenti, strumenti, ragionamento, domande, sottoagenti ed eventi futuri. Il guard resta attivo anche dopo metadati successivi. Passano Clippy con warning negati e build debug.

Il riconoscimento degli errori di sessione resta quello precedente. Le politiche di fallback per quota e gli altri recuperi non vengono modificati da questa tranche. L'assenza di eventi non dimostra l'assenza di effetti non riportati da una CLI: questa verifica esclude la duplicazione riprodotta dopo attività osservata, non certifica tutti i provider live o una garanzia generale di exactly-once.

Passano tutti e quattro i nuovi casi nativi: dopo uno strumento rimane una sola scrittura per provider; prima del lavoro osservato restano esattamente due invocazioni (ripresa fallita e recupero completo riuscito). Passano anche i dodici scenari nativi offline di continuità Claude/Claude/GPT, Compact, quota e audit/fallback con il launcher aggiornato. Il primo giro aveva rilevato un errore di parsing nel launcher batch della fixture per il revisore; corretto prima del giro finale. Questo errore del verificatore non è contato come comportamento del provider reale né come uso live. Report locali: `.impeccable/review/native-resume-replay-results.json` e `native-quality-results.json`.

[Roadmap](roadmap-0.8.md) · [Documentazione](README.md)
