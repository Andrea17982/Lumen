# Tranche 16 — interfaccia e accessibilità Windows

Baseline `804f6f1`, verifica del 12 novembre 2026. Eseguita anticipatamente su richiesta del proprietario: le tranche 14 e 15 non sono implicitamente completate. Nessuna inferenza, budget live invariato a 13 slot.

## Correzioni

Impostazioni aveva una trappola di focus ma si dichiarava non modale e lasciava accessibile lo sfondo. Ora espone `aria-modal=true`; workspace, sidebar e logo sono `inert` e nascosti all'albero accessibile mentre è aperta. Apertura chiude navigazione e popover; Escape/chiusura ripristinano uno strumento ancora utilizzabile, con fallback al logo se il comando proveniva da una palette rimossa. Le scorciatoie globali non aprono pannelli dietro il dialogo. La sidebar resta chiusa al ritorno, riapribile dal logo.

Il focus di tastiera in impostazioni, inspector e domande usa riempimento e sottolineatura nei colori del tema; la politica di assenza di anelli globali resta rispettata. Le domande distinguono controllo iniziale, errore temporaneo di lettura e richiesta scaduta. Nessuna opzione è selezionata automaticamente. Invio fallito conserva la risposta e torna al pulsante di invio; invio riuscito sposta il focus sullo stato dopo il commit React, senza rubarlo a un'altra superficie. Il provider non viene richiamato dalle verifiche di disponibilità.

## Matrice verificata

| Flusso | Prova | Perimetro |
| --- | --- | --- |
| Impostazioni | Dialogo modale, background escluso, Tab/Shift+Tab, Escape, scorciatoie, ripristino opener e bozza | Browser e WebView2 nativo, chiaro/scuro; viewport stretta e movimento ridotto nei test browser |
| Domande | Stato iniziale e lettura fallita controllati, ripresa del polling, scelta vuota iniziale, risposta da tastiera, focus dopo errore/successo | IPC e SQLite reali, CLI Claude offline; gli errori di lettura/consegna sono iniettati soltanto nel modulo servito al test |
| Inspector | Apertura con focus sul comando di chiusura, Escape e ritorno all'opener | WebView2 nativo; strumenti e canvas restano coperti anche dalla suite browser |
| Chat lunga e immagini | 1.000 messaggi montati, ricerca storica, primo/ultimo messaggio nell'AX CDP, immagine locale ingrandita e focus ripristinato | Dati temporanei; nessuna deduzione di fluidità generale o lettura vocale di tutti i 1.000 messaggi |
| Editor | Backend apre il file di prova alla riga 13; VS Code mostra `Ln 13, Col 1` nell'albero Windows | VS Code installato, profilo/estensioni temporanei, file con spazi; nessun login o modifica delle impostazioni di sicurezza |
| Lettore di schermo | Assistente vocale realmente avviato, conversazione/comandi/impostazioni attraversati; dialogo esposto senza controlli dello sfondo, Tab ed Escape osservati | Conferma uditiva del proprietario: etichette comprensibili; non è certificazione WCAG né copertura di tutti i lettori |

Assistente vocale non era attivo prima della prova ed è stato arrestato al termine tramite il [comando ufficiale Narrator+Esc](https://support.microsoft.com/en-us/accessibility/windows/narrator/appendix-b-narrator-keyboard-commands-and-touch-gestures). La finestra VS Code temporanea è stata chiusa. Nessuna autenticazione, estensione o nuovo lettore installato. Cursor, Zed, Sublime e NVDA non risultano disponibili nel PATH controllato; non sono dichiarati collaudati. La schermata iniziale di VS Code è rimasta senza login; la posizione era verificabile dallo status editor sottostante.

## Riproduzione e limiti

Passano build frontend, 72 unitari e 35 scenari browser. `npx playwright test --workers=2` include due nuove regressioni del dialogo. `node tests/native-accessibility.mjs` verifica i flussi con dati/app Windows isolati; gli errori sono iniettati usando una route Playwright, senza hook nel prodotto. Passano inoltre gli otto scenari `native-execution` e i cinque `native-sidebar`, aggiornando quest'ultimo per riaprire esplicitamente il pannello dopo le impostazioni. Non eseguire in parallelo suite che condividono il server Vite: chiudere il server posseduto da un harness può invalidare l'altra suite.

Per il controllo interattivo dell'editor/lettore, `LUMEN_A11Y_HOLD=1` lascia aperta la sola fixture e abilita l'apertura nell'editor installato con profilo temporaneo. Per rilasciarla creare `.impeccable/accessibility/release-fixture-<pid>` usando il PID stampato; il percorso dipende dal singolo avvio. Chiudere prima l'editor e ripristinare l'eventuale lettore avviato per la prova. Il normale test automatico non apre finestre editor aggiuntive.

Catture chiaro/scuro e risultato grezzo: `.impeccable/accessibility/`. La prima selezione di riga e la simulazione IPC sono state corrette nel verificatore prima dell'esito valido; un giro browser contemporaneo a un harness che chiudeva Vite è escluso. La conferma vocale è umana e circoscritta alle etichette provate. NVDA, zoom OS al 200%, braille e desktop macOS/Linux restano non collaudati; la CI non li sostituisce. Questo checkpoint chiude la tranche 16 nel perimetro dichiarato, senza chiudere le altre tranche della release.

[Roadmap](roadmap-0.8.md) · [Validazione](validation.md) · [Design](../DESIGN.md)
