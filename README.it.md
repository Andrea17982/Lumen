<div align="center">
  <img src="public/lumen.png" width="76" height="76" alt="Lumen">
  <h1>Lumen</h1>
  <p><strong>Gli abbonamenti AI per programmare che paghi già, in una finestra tranquilla.</strong></p>
  <p>
    <a href="https://github.com/Andrea17982/Lumen/releases/latest">Download</a> ·
    <a href="docs/README.md">Documentazione (EN)</a> ·
    <a href="README.md">English</a>
  </p>
</div>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/conversation-dark.png">
  <img src="docs/images/conversation.png" alt="Lumen con un task concluso: la richiesta, i passaggi raccolti in una riga e la risposta con cosa è cambiato e cosa è stato verificato">
</picture>

Pago Claude, ChatGPT e un paio di altre cose, e mi ero stancato di decidere quale aprire, copiare il contesto dall'uno all'altro e ricontrollare il loro lavoro a mano. Lumen è quello che volevo al posto di tutto questo: accedi una volta con la CLI ufficiale di ciascuno (Claude Code, Codex, Antigravity, OpenCode, Cursor, Ollama in locale o una chiave API), scrivi cosa ti serve e decide lui chi fa il lavoro.

È gratuito, con licenza MIT, ed è ancora in alpha.

## Cosa fa

- **Sceglie l'agente al posto tuo.** Il migliore disponibile in quel momento; se una quota finisce, ne continua un altro senza rifare quello che era già finito.
- **Ti dice cosa è successo.** Una sola risposta alla fine: cosa è cambiato, cosa è stato verificato, cosa è fallito, cosa resta aperto.
- **Si ferma quando deve.** Le azioni rischiose o irreversibili aspettano te. Il resto no.
- **I task grandi hanno un branch.** Una barra sopra la conversazione ti fa pubblicarlo come PR, applicarlo alla cartella o buttarlo via.
- **Si ricorda del progetto.** Le decisioni finiscono in `LUMEN.md`, che ogni agente legge.
- **Continua a lavorare.** Chiudi la finestra e il lavoro va avanti in background.

Due cose sono semplici interruttori nelle Impostazioni, ognuno con una riga di spiegazione: **Orchestra**, che divide un task grande tra i tuoi agenti dopo che hai approvato il piano, e **Code Mode**, che trasforma molte chiamate ai tool in un solo script per risparmiare token.

## Download

Windows x64: il setup è nell'[ultima release](https://github.com/Andrea17982/Lumen/releases/latest). Non è ancora firmato, quindi Windows ti avviserà; verificalo con `SHA256SUMS.txt`. Per ora macOS e Linux si compilano da sorgente: vedi il [README inglese](README.md#build-from-source).

## Da sapere

Autopilota dà agli agenti un accesso ampio al computer e non è una sandbox. Lumen non tocca le credenziali dei provider: ogni CLI fa il login da sé, e le chiavi API finiscono nel portachiavi di sistema. [Validazione](docs/validation.md) · [Sicurezza](SECURITY.md)

## Come è fatto

Codice, test e documentazione li scrive Claude Code sotto la mia direzione; le decisioni di prodotto sono mie. È una dichiarazione, non un audit di sicurezza.

Grazie a **Theo / pingdotgg, T3 Tools Inc. e ai contributor di T3 Code**: Lumen adatta parti del design di T3 Code. [Fonti e modifiche](src-tauri/resources/legal/t3code/ADAPTATIONS.md) · [Licenze](THIRD_PARTY_NOTICES.md). Ciò non implica alcun endorsement.

Bug e idee nelle [Issues](https://github.com/Andrea17982/Lumen/issues). Se vuoi sostenerlo c'è [PayPal](https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=andreanastrobusiness1999%40gmail.com&currency_code=EUR&item_name=Support%20Lumen); non sblocca nulla. — Andrea

MIT © 2026 Andrea17982. Non affiliato con Anthropic, OpenAI, Google o con gli altri provider citati.
