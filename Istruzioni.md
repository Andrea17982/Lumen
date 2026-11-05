Voglio che tu progetti e costruisca da zero un’applicazione desktop chiamata **Lumen**.

### Visione del prodotto
Lumen è un client AI minimale, estremamente elegante e “it just works”, in stile Apple Liquid Glass.
L’obiettivo è permettere all’utente di avere **infinite sessioni** di Claude Code e di ChatGPT/Codex contemporaneamente, gestite in modo intelligente e quasi invisibile.

Filosofia:
- Pochissime opzioni visibili
- Tanta intelligenza sotto il cofano
- Interfaccia che sembra un prodotto Apple del 2027
- Velocità e leggerezza come priorità assoluta
- L’utente non deve mai pensare a “quale modello usare”

### Funzionalità core (devono esserci tutte)

1. **Sidebar sinistra**
   - Lista di tutte le sessioni (infinite)
   - Ogni sessione ha un nome automatico intelligente + possibilità di rinominare
   - Indicatori di stato (in esecuzione, in attesa, completata, errore)
   - Possibilità di pinnare sessioni
   - Ricerca sessioni

2. **Area principale**
   - Chat pulita e minimalissima
   - Streaming delle risposte
   - Visualizzazione di thinking, tool calls e file edits in modo elegante e collassabile

3. **Barra inferiore (la parte più importante)**
   Un selettore estremamente semplice con queste opzioni:
   - Claude
   - GPT
   - Auto (default e consigliato)

   Quando l’utente sceglie Claude o GPT può (opzionalmente) espandere per scegliere:
   - Quale account (supporto nativo a multipli account Claude e multipli account ChatGPT)
   - Quale modello
   - Quale effort/reasoning level

   La modalità **Auto** deve essere intelligente:
   - Analizza la complessità della task
   - Sceglie automaticamente il provider migliore (Claude o GPT)
   - Sceglie modello e effort ottimali
   - Decide da sola se usare sotto-agenti
   - Decide se far fare a un modello il ruolo di writer e all’altro di reviewer
   - Decide se lanciare una modalità pair/review automatica
   - Tutto questo senza chiedere nulla all’utente

4. **Gestione account**
   - Supporto nativo a **infiniti account Claude** (usando CLAUDE_CONFIG_DIR isolati + login OAuth ufficiale)
   - Supporto a multipli account ChatGPT/Codex
   - Nessun proxy, nessun trick rischioso: solo autenticazione ufficiale OAuth
   - L’utente può aggiungere/rimuovere account facilmente

5. **Comportamento “Auto”**
   Deve essere il fiore all’occhiello. Quando è attivo, Lumen deve comportarsi come un direttore d’orchestra invisibile:
   - Task semplici → modello leggero e veloce
   - Task complesse di coding → Claude Opus o GPT ad alto effort + eventuale reviewer
   - Task ambigue → può farli ragionare in parallelo e poi sintetizzare
   - Deve poter usare sotto-agenti in autonomia

### Design
- Stile **Apple Liquid Glass** (traslucenze, blur, riflessi, angoli morbidi, animazioni fluide)
- Tema chiaro e scuro nativi
- Tipografia di sistema o SF Pro-like
- Zero clutter. Ogni pixel deve avere uno scopo.
- Animazioni sottili e di altissima qualità
- Deve sembrare un’app nativa di altissimo livello

### Stack tecnico obbligatorio
- **Tauri 2** (Rust) come shell desktop
- **Rust** per tutto il core (gestione processi, orchestrazione, multi-account, sessioni)
- **React** o **SolidJS** + TypeScript per il frontend
- Tailwind CSS + Framer Motion per l’UI
- Architettura pulita e modulare

### Requisiti non funzionali
- Avvio istantaneo
- Consumo di memoria bassissimo
- Deve girare su Windows (Linux nice-to-have)
- Codice pulito, ben strutturato e commentato dove serve
- Facilmente estendibile

### Cosa voglio da te adesso
1. Proposta di architettura completa (cartelle, moduli Rust, componenti React)
2. Design system e principi UI (come realizzare il look Liquid Glass)
3. Come gestire i multipli account Claude in modo pulito e sicuro
4. Come implementare la modalità Auto in modo intelligente
5. Piano di implementazione a step (MVP → v1 → v2)
6. Inizia a generare il codice partendo dalla struttura del progetto e dal core più importante
7. Puoi andare ad ispirarti al progetto M:\Personal Projects\duo per mutuare eventuali semplici algoritmi di autenticazione o ragionamento ed evitarti cosi' scrittura di codice inutile e consumo di token

Parti dalla visione e non scendere a compromessi sulla qualità e sulla minimalità.
Lumen deve far sembrare tutti gli altri client AI obsoleti e rumorosi.