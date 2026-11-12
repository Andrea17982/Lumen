# Logo e sidebar — 0.8

Implementazione del 12 novembre 2026, baseline `8a15521`. Direzione visiva già approvata: il logo stesso apre e chiude la sidebar di vetro, senza un secondo pulsante menu. Nessun nuovo account, modello o slot live.

## Comportamento

Un solo pulsante-logo resta in alto a sinistra nelle due composizioni. La sidebar desktop può essere chiusa per lasciare spazio alla conversazione e al composer; resta aperta inizialmente sul desktop, conservando la composizione con progetti a portata di mano. Nelle finestre fino a 800px parte chiusa e si apre sopra la chat con scrim. Lo stato resta locale alla finestra e non è una preferenza persistita. La selezione di una chat mantiene il pannello sul desktop e lo chiude nella finestra stretta.

Il pulsante espone nome tradotto, `aria-expanded` e `aria-controls`. La sidebar chiusa è `inert` e nascosta all'albero accessibile; i contenuti restano montati. Escape dal pannello o chiusura dello scrim restituiscono il focus al logo; `/` apre la ricerca e la focalizza. Escape in un menu di conversazione chiude prima il menu e restituisce il focus alla sua azione. Enter e Spazio azionano il logo. Il focus del logo ha un contorno visibile.

Un breve riflesso attraversa il logo al clic; il pannello usa una transizione interrompibile con i token di movimento esistenti. Movimento ridotto elimina transizione e riflesso. Chiaro/scuro e vetro usano i materiali esistenti; testo, chat, avanzamento e composer non ricevono un nuovo stile.

## Verifica e limiti

`tests/sidebar.spec.ts` controlla desktop, finestra stretta, bozza, apertura/chiusura con tastiera, ricerca, focus, etichette italiane e movimento ridotto nei due temi. La suite esistente continua a controllare menu, contesto, progetti e inspector.

`node tests/native-sidebar.mjs` usa Windows debug/WebView2, IPC e dati temporanei isolati, senza account o inferenza. Controlla composizioni desktop, bozza, focus, ricerca, tema scuro, movimento ridotto e viewport stretta emulata. Catture e report locali: `.impeccable/sidebar/`. La viewport stretta non è un collaudo di una finestra OS fisicamente ridimensionata; nessuna certificazione screen reader o di apprezzamento delle animazioni.

Passano build TypeScript/Vite, 72 test unitari, 33 scenari browser (quattro nuovi sul logo/sidebar) e cinque scenari nativi Windows, oltre a formattazione e riferimenti documentali. Il primo giro delle regressioni ha trovato la chiusura indesiderata del pannello desktop dopo selezione e la propagazione di Escape dal menu: corretti prima del giro finale completo. Due timeout aggiuntivi nel primo lancio a 16 worker non si riproducono nella suite finale a due worker.

È il primo intervento sulla composizione approvata, non un redesign generale. La composizione essenziale è disponibile dal logo; avvio desktop sempre essenziale e persistenza della scelta non vengono introdotti qui. La release pubblicata resta 0.7.1 e il budget live resta a 13 slot.

[Roadmap](roadmap-0.8.md) · [Design](../DESIGN.md) · [Documentazione](README.md)
