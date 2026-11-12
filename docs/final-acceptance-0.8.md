# Tranche 17 — prestazioni e accettazione finale

Protocollo del 12 novembre 2026, baseline `01bf144`, prima delle modifiche. Budget live invariato a 13 slot storici: nessuna inferenza, login o prova multiaccount live aggiuntiva. Versione pubblica 0.7.1 invariata; integrazione, versione e pacchetti 0.8 appartengono alla tranche 18.

Consolidare le misure storiche senza riscriverle come prove della build attuale. Congelare due release Windows con identità/profilo benchmark dedicati, prima e dopo chiarezza permessi/tutorial richiesti dal proprietario. Eseguire A/B/A/B, 20 campioni per blocco, sul carico misto di 5.000 messaggi. Conservare primo campione e outlier. Criterio di regressione mirato: mediana apertura e tempo task di digitazione non peggiori di oltre 10% in entrambi i confronti adiacenti; risultati discordanti restano inconcludenti. Streaming, scrolling, heap e memoria privata restano secondari e non dimostrano prestazioni generali. Nessuna compilazione o suite concorrente durante le misure.

Sulla release finale verificare ricerca, selezione, copia, link, immagini, disclosure, streaming, callback della chat e Both, mantenendo il DOM storico completo. Separare questa accettazione dalla build normale distribuibile e dalle prove debug/browser. Verificare permessi spiegati e selezionati esplicitamente, tutorial alla prima apertura con uscita/ripresa volontaria, tastiera, italiano/inglese, temi, finestra stretta e movimento ridotto. Il tutorial non deve inviare richieste, cambiare permessi o attivare login.

Classificare esplicitamente virtualizzazione, Markdown/ANSI Rust, pool CLI, metadati compatti e parser del grafo: integrato, escluso per gate fallito o rinviato per evidenza/prerequisiti insufficienti. Non trasformare assenza di prove in prova di inutilità. Riportare limiti su TTFT live, baseline assolute e desktop non Windows.

[Roadmap](roadmap-0.8.md) · [Prestazioni storiche](performance-0.8.md) · [Apertura](initial-open-0.8.md) · [Metadati Windows](metadata-shutdown-0.8.md)

## Decisioni sui candidati

La prima serie nello stesso processo è stata fermata prima del confronto candidato: **12 campioni della medesima baseline**, apertura 1.227,7–27.845,6 ms. Sono conservati in `excluded-warm-baseline.json` con il motivo; non sono rimossi come outlier né presentati come regressione causata dalla nuova UI. La causa della deriva non è dimostrata. **L'accettazione non certifica aperture ripetute a regime nello stesso processo.**

Prima della nuova serie l'unità di misura è stata cambiata a un processo nuovo per campione, con identica preparazione su chat corta, profilo/viewport/DPI/media controllati e gli stessi due binari congelati. Restano A/B/A/B, 20 campioni per blocco e soglia del 10%; 80 avvii sono conservati singolarmente, inclusi i primi. Le misure sono aperture con processo fresco e OS/profilo caldi, non cold-start del sistema. Nessuna compilazione o altra suite accompagna i campioni.

| Candidato | Decisione per 0.8 | Evidenza e confine |
| --- | --- | --- |
| Isolamento composer/transcript | Integrato | Riduzione storica del tempo task digitazione sul corpus primario: 63,1% rispetto alla prima baseline, 82,9% al controllo ripetuto. Sono confronti di quel checkpoint, non percentuali della build corrente. |
| Paragrafi letterali senza parser Markdown completo | Integrato | Conferma storica ordinaria: apertura 1.405,9→1.046,2 ms (−25,6%) con contenuto identico e digitazione +1,0%. Markdown vero conserva il percorso originale. |
| Chiusura metadati Windows supervisionata | Integrata | Chiamata completa storica Claude −37,2%, Codex −57,4%; il costo aggiuntivo di avvio/supervisione (~42 ms) è incluso nel netto. Cancellazione, figli e abort hanno gate dedicati. Non è TTFT del modello. |
| Finestra virtualizzata che smonta lo storico | Esclusa | Il prototipo fallisce ricerca/selezione/accesso storico. Nessuna virtualizzazione completa viene dichiarata consegnata. Una soluzione futura deve superare gli stessi gate. |
| Memoizzazione più ampia di MessageView | Esclusa | Il primo prototipo aveva risultati secondari di scroll non risolti. È integrata soltanto la variante ristretta verificata. |
| Markdown/ANSI spostati in Rust | Rinviati | Nessun confronto post-correzioni dimostra un beneficio sufficiente sul costo totale UI/IPC. L'assenza di un prototipo accettato non prova che Rust sia inutile; il renderer corrente resta. |
| Pool CLI caldo | Rinviato | I dati distinguono spawn, inizializzazione e cleanup ma non validano un pool. Mancano confronto utile e prototipo che preservi account, cartella, permessi, MCP, cancellazione e pulizia. Le sessioni native continuano ad avviare un processo per turno. |
| Metadati posizionali/tuple | Rinviati | Il precedente ~32% sintetico su dati posseduti non prova risparmio della conversazione o qualità/comprensione. Si integrano invece le riduzioni di duplicazione misurate nella tranche 15, senza riscrivere CLI/MCP. |
| Sostituzione parser del grafo | Rinviata | 23/23 aspettative del corpus mirato attuale, cache/mutazioni e handoff verificati; non è semantica completa. Alias arbitrari, stringhe/commenti complessi e UID Godot restano limiti. Nessuna migrazione per il solo cambio di tecnologia. |
| Allocazione globale contesto/riserva output | Rinviata | Non dimostrata sui percorsi nativi con storia/tool/provider differenti. I 48 KiB degli allegati sono un limite di contenuto, non un tetto dell'intera richiesta. |
| Baseline assoluta/cold-start/TTFT live | Non certificati | Deriva storica del medesimo binario e prerequisiti/budget live mancanti impediscono promesse generali. Le nuove misure sono regressioni mirate Windows, non un benchmark hardware universale. |
| Aperture ripetute a regime nello stesso processo | Non certificate | La nuova serie iniziale riproduce deriva della baseline. Diagnosi e controllo stabile di questo carico restano aperti; il confronto con processi freschi non la risolve e non viene esteso a questo caso. |

Fonti: [digitazione](performance-0.8.md), [apertura e prove escluse](initial-open-0.8.md), [attesa CLI](turn-latency-0.8.md), [shutdown](metadata-shutdown-0.8.md), [contesto/grafo](context-graph-budget-0.8.md). I record storici rimangono immutati; il consolidamento ne registra gli hash separatamente dai nuovi campioni.

## Risultati della build finale

Il [record aggregato](data/final-acceptance-0.8-results.json) mantiene hash e dati delle prove correnti distinti dai cinque record storici. Baseline congelata `e8256f8489334033f2007da72c838ef650f3ca8e31fe98e1893ab1aeb2fd906e`; candidato `42b026a8b3c276ac515c07a03f0582ee8f5e908e9f5c37b53077e3508f424289`. Sono build ottimizzate con frontend incorporato e identità/profilo dedicati, non pacchetti della release pubblica.

| Confronto, 20 campioni per ramo | Apertura mediana baseline → candidato | Task digitazione baseline → candidato |
| --- | --- | --- |
| A | 1.159,7 → 1.151,8 ms (−0,7%) | 136,6 → 140,8 ms (+3,1%) |
| B | 1.230,7 → 1.105,7 ms (−10,2%) | 157,1 → 130,1 ms (−17,2%) |

Entrambi rispettano la soglia di regressione prespecificata del 10%. Questo verifica il carico controllato con processi freschi; **non dimostra che il tutorial renda l'app più veloce**. La p95 di apertura nel primo confronto cresce 1.279,3→1.520,1 ms, mentre nel secondo scende 1.511,8→1.159,3: il peggioramento non si riproduce, ma non si promette stabilità delle code. P95 dei frame di scroll 29,2/20,9 e 20,9/20,8 ms; heap e memoria privata restano secondari, senza certificazione di fluidità universale, INP o riduzione della memoria.

Tutti gli 80 campioni conservano 5.000 articoli montati. Ogni avvio riporta WebView document visibile/con focus, 827×547 CSS px, DPR 1,5 e movimento ridotto. Il binario candidato supera otto gate reali su 1.000 messaggi misti: ricerca storica, selezione completa/codice, copia e disclosure/DOM conservati durante digitazione, link/tastiera e immagini/focus, streaming dell'ultimo messaggio, Compact senza generazione, callback della chat corrente e corsie Both. Nessuna finestra virtualizzata sperimentale è presente nel bundle normale. La selezione/AX completa di 5.000 messaggi non viene dedotta dai gate a 1.000.

Controlli sorgente/UI: 180 Rust (14 probe opt-in ignorati), Clippy senza warning, 72 unitari frontend, 40 browser, build e formattazione/documentazione. Tre scenari Windows nativi per il nuovo tour/menu, sette composer e otto execution passano con dati temporanei e provider offline. I test del menu confermano che l'apertura non modifica i permessi backend e la scelta esplicita sopravvive al riavvio. Il tour conserva la bozza e non crea account, chat o richieste. La verifica visuale riguarda temi chiaro/scuro, italiano/inglese, 390/720/827/1240 CSS px e movimento ridotto. Nessuna nuova conferma uditiva o certificazione WCAG viene dichiarata.

Il primo problema del composer era nel percorso di navigazione dell'harness, non una regressione della bozza; il ritorno del focus dal tour aperto nelle Impostazioni e la disposizione di Invio sono stati corretti e verificati. Non restano errori automatici aperti in questo perimetro. La deriva delle aperture ripetute nello stesso processo resta esplicitamente non risolta, con tutti i dati conservati. Budget live fermo a 13; tranche 17 conclusa nel perimetro dichiarato. Integrazione, versione 0.8, pacchetti e pubblicazione restano tranche 18.

## Permessi e tutorial

Il pulsante non cambia più modalità a rotazione: mostra il nome corrente e apre quattro opzioni con spiegazione. L'apertura non concede accessi; scelta e persistenza restano nei percorsi reali già esistenti. Le frecce/Home/End navigano il menu, Escape lo chiude con ritorno del focus. La barra riserva spazio al nome della modalità; su finestre strette provider e azioni occupano due righe coerenti, senza isolare Invio.

Alla prima apertura di un profilo senza chat/account compare un invito saltabile al tutorial. Cinque tappe identificano logo/progetti, account nelle Impostazioni, provider/Auto/Orchestra, permessi e composer/avanzamento. Il pannello non modale segue gli elementi reali con un highlight tenue e un breve ingresso; movimento ridotto elimina le animazioni. Skip, chiusura, indietro e avanti sono sempre disponibili nel percorso. Bozza e modalità restano invariate. Conclusione/skip vengono ricordati; i profili già usati non ricevono l'invito quando filtrano o svuotano i dati. La ripetizione è in Impostazioni → Generali e nel menu comandi.

Il tutorial non invia prompt, non effettua login e non modifica permessi. Se si abbandonano le Impostazioni per il tour e il precedente pulsante scompare, il focus ritorna al composer. Il testo italiano/inglese mantiene i nomi effettivi e dichiara la dipendenza dal supporto delle approvazioni del provider. Non si deduce comprensione degli utenti da screenshot o test automatici.

## Riproduzione

Usare Node 24 e gli strumenti Tauri/Rust/WebView2 installati. L'identità benchmark è separata dall'app reale e non è distribuibile. Prima delle modifiche:

```powershell
$env:LUMEN_PERF_ROOT='.impeccable/final-acceptance'
node tests/performance-prepare.mjs
npx tauri build --no-bundle --config .impeccable/final-acceptance/config.json
# Congelare il binario in baseline/lumen.exe; ripetere dopo le modifiche in candidate/lumen.exe.
# Terminare compilazioni e altre suite prima della serie.
node tests/final-performance.mjs
$env:LUMEN_PERF_EXE=(Resolve-Path .impeccable/final-acceptance/candidate/lumen.exe).Path
node tests/performance-acceptance.mjs
node tests/final-acceptance-report.mjs
```

Il driver esegue quattro blocchi seriali da 20 processi nuovi, conservando i risultati individuali. Riapre la sidebar tramite il logo fuori dai tempi di apertura: entrambi i rami seguono la stessa navigazione attuale. Il vecchio harness composer viene aggiornato per lo stesso motivo; il primo tentativo su una riga nascosta è un errore di harness conservato separatamente, non un campione prestazionale o un difetto dell'app candidato. Un build intermedio è stato sostituito prima delle misure per includere il marcatore dei profili già usati; soltanto il binario finale congelato alimenta il confronto.

Le regressioni UI sono `npx playwright test`, `node tests/native-onboarding.mjs`, `node tests/native-composer.mjs` e `node tests/native-execution.mjs`. I comandi nativi WebView2 vanno eseguiti in serie. I controlli funzionali non certificano lettori di schermo o desktop non Windows; restano distinti dalla CI multipiattaforma e dalla packaging acceptance della tranche 18.
