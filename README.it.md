# Lumen

Un’app desktop locale per gestire conversazioni Claude Code e ChatGPT / Codex in un’interfaccia essenziale ispirata ad Apple Liquid Glass. Tauri 2, core Rust, React e TypeScript. L’interfaccia è in inglese, come scelto dal creatore.

**Versione 0.2 alpha.** [Scarica da Releases](https://github.com/Andrea17982/Lumen/releases) oppure compila il progetto. Progetto indipendente, senza affiliazione con Apple, Anthropic o OpenAI.

![Interfaccia Lumen](docs/images/desktop.png)

## Completamente AI Written

Il codice dell’applicazione, l’interfaccia, i test e la documentazione sono **completamente AI Written**, generati da AI sotto la direzione di Andrea ([Andrea17982](https://github.com/Andrea17982)). La visione e le decisioni di prodotto sono umane. Questa dichiarazione non equivale a un audit di sicurezza indipendente.

> Ringrazio tutti quelli che avranno voglia di provarla e farmi sapere come si trovano. Feedback, segnalazioni e idee sono benvenuti! — Andrea

Lumen è disponibile con licenza MIT: tutti possono scaricarla, utilizzarla e modificarla.

## Novità 0.2

Interfaccia ripulita dalle scritte promozionali, workspace nominati e cartella specifica per conversazione nel composer, rinomina account dalle impostazioni o dal selettore, modelli e reasoning letti dai CLI ufficiali. La nuova modalità **Autopilot** abilita strumenti e comandi senza sandbox o conferme del CLI, entro i permessi del sistema operativo. Il controllo grafico del PC richiede strumenti di automazione disponibili/configurati. [Guida completa](docs/workspace-models-permissions.md).

## Avvio rapido

Installa separatamente i CLI ufficiali di [Claude Code](https://code.claude.com/docs/en/setup) e/o [Codex](https://developers.openai.com/codex/cli). In Lumen apri Settings, aggiungi un account e premi Sign in: l’autenticazione avviene nel terminale e nel browser ufficiali del provider. Verifica la connessione con il pulsante a scudo, scegli una cartella di lavoro fidata e avvia la conversazione.

Per compilare servono Node.js, Rust stable, C++ Build Tools, Windows SDK e WebView2:

```powershell
git clone https://github.com/Andrea17982/Lumen.git
cd Lumen
npm ci
npm run tauri dev
```

`npm run tauri build` genera l’installer. `npm run dev` apre soltanto un’anteprima UI nel browser, senza inferenza AI o login.

## Funzioni e limiti

Sessioni SQLite persistenti, ricerca, pin, rinomina, stati, attività collassabili, account con directory isolate, tema chiaro/scuro/sistema e selettore Auto / Claude / GPT. Auto usa euristiche locali per scegliere provider, modello, effort e, quando sono disponibili entrambi i provider, review indipendente o prospettive parallele con sintesi.

Nessun limite artificiale al numero di sessioni salvate; massimo quattro lavori attivi e otto processi di inferenza. Le scritture nella stessa cartella vengono serializzate. Accesso in sola lettura predefinito. Il contesto viene ripassato al CLI a ogni turno fino a 100.000 caratteri recenti. Codex aggiorna la chat a messaggi completati; Claude trasmette anche testo parziale.

L’inferenza reale, l’isolamento OAuth con più account e le modifiche ai file richiedono test di accettazione con account dell’utente. La compatibilità dipende dalle versioni CLI e dai piani dei provider. Non sono implementati proxy, aggiramenti dei limiti o login OAuth proprietari. Le conversazioni e i file credenziali gestiti dai CLI non vengono cifrati da Lumen. Le prestazioni non sono state oggetto di benchmark.

Leggi il [README completo](README.md), la [documentazione](docs/getting-started.md), l’[architettura](docs/architecture.md), le [regole account](docs/accounts.md) e la [roadmap MVP → v1 → v2](docs/roadmap.md).

## Donazioni facoltative

Se vuoi sostenere il progetto: [Dona con PayPal](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen), destinatario **andreanastrobusiness1999@gmail.com**. Nessuna funzione a pagamento e nessun obbligo di donare. Per il feedback usa le [Issues](https://github.com/Andrea17982/Lumen/issues).
