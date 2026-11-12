import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';

export type Language = 'en' | 'it';

const it: Record<string, string> = {
  'Account tools': 'Strumenti dell’account',
  'Native hooks': 'Hook nativi',
  Skill: 'Skill',
  Tool: 'Strumento',
  'Shared tools are available to Claude, GPT, Gemini and Antigravity. Provider authentication and permission controls still apply.':
    'Gli strumenti condivisi sono disponibili a Claude, GPT, Gemini e Antigravity. Restano validi autenticazione e permessi dei provider.',
  'Detected integrations': 'Integrazioni rilevate',
  'Portable tools and skills are discovered automatically. Cloud connectors and native hooks may require their original account or runtime.':
    'Strumenti portabili e skill vengono rilevati automaticamente. Connettori cloud e hook nativi possono richiedere l’account o il runtime di origine.',
  Configured: 'Configurato',
  'Requires a native adapter': 'Richiede un adattatore nativo',
  'Share in Lumen': 'Condividi in Lumen',
  'Request from a connector': 'Richiesta di un connettore',
  'Connector consent is separate from agent permissions. Your answers stay in this request and are not added to shared context.':
    'Il consenso al connettore è separato dai permessi dell’agente. Le risposte restano in questa richiesta e non entrano nel contesto condiviso.',
  'Open sign-in page': 'Apri la pagina di accesso',
  'Complete sign-in in your browser, then choose Accept. The connector will verify the result.':
    'Completa l’accesso nel browser, poi scegli Accetta. Il connettore verificherà il risultato.',
  'Review this connector request. Choose Accept, Decline or Cancel; no choice is automatic.':
    'Verifica la richiesta del connettore. Scegli Accetta, Rifiuta o Annulla: nessuna scelta è automatica.',
  Accept: 'Accetta',
  Optional: 'Facoltativo',
  Decline: 'Rifiuta',
  'Read only': 'Sola lettura',
  'File edits': 'Modifica file',
  'Choose agent permissions': 'Scegli i permessi dell’agente',
  'Choose what the agent is allowed to do': 'Scegli cosa può fare l’agente',
  'Inspect and answer without changing files.': 'Esamina e risponde senza modificare i file.',
  'Allow project file changes within the provider’s controls.':
    'Consenti modifiche ai file del progetto entro i controlli del provider.',
  'Review tool actions before they run. Requires a provider that supports approvals.':
    'Approva le azioni degli strumenti prima che vengano eseguite. Richiede un provider che supporti le approvazioni.',
  'Allow available tools without approval prompts, with your OS account’s access.':
    'Consenti gli strumenti disponibili senza richieste di conferma, con gli accessi del tuo account di sistema.',
  'Your subscriptions. One place to work.': 'I tuoi abbonamenti. Un solo spazio di lavoro.',
  'Take a quick tour': 'Scopri Lumen in pochi passi',
  'Learn your way around Lumen': 'Come usare Lumen',
  'Skip tour': 'Salta il tutorial',
  'Close tutorial': 'Chiudi il tutorial',
  'Tutorial progress': 'Avanzamento del tutorial',
  'Previous step': 'Passo precedente',
  Next: 'Avanti',
  'Start exploring': 'Comincia a lavorare',
  'Your projects, close at hand': 'I tuoi progetti, a portata di mano',
  'Click the Lumen logo to show or hide projects and conversations. Choose a project folder to give the agent the right workspace.':
    'Il logo di Lumen mostra o nasconde progetti e conversazioni. Scegli la cartella del progetto per dare all’agente lo spazio di lavoro corretto.',
  'Bring your subscriptions together': 'Riunisci i tuoi abbonamenti',
  'Connect your accounts in Settings → Accounts using each provider’s official sign-in. You can connect more than one account.':
    'Collega i tuoi account in Impostazioni → Account, con l’accesso ufficiale di ciascun provider. Puoi collegare più di un account.',
  'Choose who works with you': 'Scegli chi lavora con te',
  'Pick a provider directly, or use Auto to choose for the task. Auto offers Budget, Default and Quality. Enable Orchestra in Settings for coordinated work across agents.':
    'Scegli direttamente un provider oppure usa Auto per assegnare il compito. Auto offre Budget, Default e Quality. Attiva Orchestra nelle Impostazioni per coordinare più agenti.',
  'Decide what the agent can do': 'Decidi cosa può fare l’agente',
  'Permissions controls actions, not the model. Start in Read only, allow file edits, approve individual actions, or choose Autopilot. Opening this menu does not grant access.':
    'I permessi regolano le azioni, non il modello. Parti da Sola lettura, consenti la modifica dei file, approva ogni azione o scegli Autopilot. Aprire questo menu non concede accessi.',
  'Ask, then follow the work': 'Chiedi e segui il lavoro',
  'Describe the result you want and attach useful files. Enter sends; Shift+Enter starts a new line. During a task, expand the progress summary to see tools, agents and checks. You can stop a response at any time.':
    'Descrivi il risultato che vuoi e allega i file utili. Invio invia il messaggio; Maiusc+Invio va a capo. Durante un compito, espandi il riepilogo per vedere strumenti, agenti e verifiche. Puoi fermare una risposta in qualsiasi momento.',
  'Connect and check actions': 'Collega e verifica le azioni',
  'Controls are off until enabled for this project. Every action verifies that the connected editor belongs to this folder. No input is automated.':
    'I controlli restano disattivati finché non li abiliti per questo progetto. Ogni azione verifica che l’editor collegato appartenga a questa cartella. Nessun comando di input viene automatizzato.',
  'Declare a project Godot MCP server to use Play and Reload. Window capture remains available in Visual review.':
    'Configura un server MCP Godot nel progetto per avviare e ricaricare il gioco. Puoi comunque catturare la finestra da Revisione visiva.',
  'Enable visual controls for this project': 'Abilita i controlli visivi per questo progetto',
  'Godot MCP server': 'Server MCP Godot',
  'Godot visual controls': 'Controlli visivi Godot',
  'Inspect pixels and grid': 'Esamina pixel e griglia',
  Role: 'Ruolo',
  'Scene path (empty uses main scene)': 'Percorso della scena (vuoto per la scena principale)',
  'Waiting for Godot…': 'In attesa di Godot…',
  'Worktree visual comparison': 'Confronto visivo dei worktree',
  'Compare versions': 'Confronta versioni',
  'Choose a version': 'Scegli una versione',
  'Web output folder': 'Cartella della build web',
  'Compare rendered versions': 'Confronta le versioni renderizzate',
  'First rendered version': 'Prima versione renderizzata',
  'Second rendered version': 'Seconda versione renderizzata',
  'Build each version in its own worktree, then choose its web output folder. Lumen serves those files locally; it does not run project commands.':
    'Compila ogni versione nel suo worktree, poi scegli la cartella della build web. Lumen mostra questi file tramite un server locale senza eseguire comandi del progetto.',
  'Task graph': 'Grafo dei compiti',
  'Drag a task onto an agent to assign its implementation worktree. Keyboard users can use the agent selectors below.':
    'Trascina un compito su un agente per assegnare il suo worktree. Con la tastiera puoi usare i selettori qui sotto.',
  'Implementation worktree': 'Worktree di implementazione',
  'Read-only task': 'Compito di sola lettura',
  'Assign to': 'Assegna a',
  'Manual merge': 'Fusione manuale',
  'Merge both versions manually': 'Unisci manualmente le due versioni',
  'Choose a result for each file': 'Scegli un risultato per ogni file',
  'Close merge view': 'Chiudi la vista di fusione',
  'Binary or large file: choose a complete version.':
    'File binario o grande: scegli una versione completa.',
  'First version': 'Prima versione',
  'Second version': 'Seconda versione',
  'Original project version': 'Versione originale del progetto',
  'File absent': 'File assente',
  'Result for': 'Risultato per',
  'Choose result': 'Scegli il risultato',
  'Edit merged text': 'Modifica il testo unito',
  'Merged text': 'Testo unito',
  'Apply reviewed merge': 'Applica la fusione revisionata',
  'Applying merge…': 'Applicazione della fusione…',
  'Loading versions…': 'Caricamento delle versioni…',
  'Layout canvas': 'Canvas del layout',
  'Add block': 'Aggiungi blocco',
  'Save layout': 'Salva layout',
  'Block label': 'Etichetta del blocco',
  Constraints: 'Vincoli',
  'Remove block': 'Rimuovi blocco',
  'Layout saved. New interface tasks will receive these constraints.':
    'Layout salvato. I prossimi compiti sull’interfaccia riceveranno questi vincoli.',
  'Move blocks with the pointer or arrow keys. Shift moves ten pixels. Save to constrain the agent’s next interface change.':
    'Sposta i blocchi con il puntatore o le frecce. Maiusc sposta di dieci pixel. Salva per vincolare la prossima modifica dell’interfaccia.',
  'Verify changes after native turns': 'Verifica le modifiche dopo i turni nativi',
  'Run additional Lumen project checks after the CLI finishes. Native sessions otherwise use the CLI’s own tools and checks.':
    'Esegui controlli aggiuntivi di Lumen al termine della CLI. Le sessioni native usano già gli strumenti e le verifiche della CLI.',
  'When enabled, split complex tasks across agents. Turn off for direct native sessions.':
    'Quando attiva, suddivide i compiti complessi fra più agenti. Disattivala per usare sessioni native dirette.',
  'Project inspector': 'Inspector del progetto',
  'Close inspector': 'Chiudi inspector',
  'Inspector tools': 'Strumenti dell’inspector',
  'Web preview': 'Anteprima web',
  Data: 'Dati',
  Changes: 'Modifiche',
  'Preview address': 'Indirizzo di anteprima',
  'Load preview': 'Carica anteprima',
  'Project web preview': 'Anteprima web del progetto',
  'Start the project server, then load its address.':
    'Avvia il server del progetto, poi carica il suo indirizzo.',
  'Enter a localhost web address.': 'Inserisci un indirizzo web localhost.',
  'Project image path': 'Percorso dell’immagine nel progetto',
  'Open image': 'Apri immagine',
  'Image zoom': 'Zoom dell’immagine',
  'Pixel grid': 'Griglia dei pixel',
  'JSON or SQLite project path': 'Percorso JSON o SQLite nel progetto',
  'Read-only SQL': 'SQL di sola lettura',
  'Inspect data': 'Esamina dati',
  rows: 'righe',
  'Preview truncated': 'Anteprima troncata',
  'View changes': 'Mostra modifiche',
  'Changes in': 'Modifiche in',
  'No text changes.': 'Nessuna modifica al testo.',
  'Preview truncated. Open the file to review all changes.':
    'Anteprima troncata. Apri il file per esaminare tutte le modifiche.',
  'Loading changes…': 'Caricamento modifiche…',
  'Agent sessions': 'Sessioni degli agenti',
  Agent: 'Agente',
  'Waiting for agent output…': 'In attesa della risposta dell’agente…',
  'Questions from the agent': 'Domande dell’agente',
  'Write a different answer': 'Scrivi una risposta diversa',
  'Your answer': 'La tua risposta',
  'Send answers': 'Invia risposte',
  'Answers sent': 'Risposte inviate',
  'This question is no longer waiting for an answer':
    'Questa domanda non è più in attesa di risposta',
  'Load image': 'Carica immagine',
  'Enlarge image': 'Ingrandisci immagine',
  'Image preview': 'Anteprima immagine',
  'Close image': 'Chiudi immagine',
  'The image could not be loaded.': "Impossibile caricare l'immagine.",
  'Available commands': 'Comandi disponibili',
  'Change the conversation folder': 'Cambia la cartella della conversazione',
  'Continue in a fresh session with a summary': 'Continua in una nuova sessione con un riepilogo',
  'Open a conversation with messages before compacting.':
    'Apri una conversazione con messaggi prima di compattarla.',
  Appearance: 'Aspetto',
  Interface: 'Interfaccia',
  Reliability: 'Affidabilità',
  Context: 'Contesto',
  Budget: 'Risparmio',
  Default: 'Predefinito',
  Quality: 'Qualità',
  'Auto policy': 'Criterio di Auto',
  'Prefer accounts with more quota left before their reset.':
    'Preferisci gli account con più quota residua prima del reset.',
  'About Lumen': 'Informazioni su Lumen',
  'Lumen is completely AI Written, created at the direction of Andrea. Thank you to everyone who tries it and shares how it feels.':
    "Lumen è scritto interamente dall'AI, sotto la direzione di Andrea. Grazie a chi lo prova e racconta come si trova.",
  'advanced option is on': 'opzione avanzata attiva',
  'advanced options are on': 'opzioni avanzate attive',
  'Orchestra limits': 'Limiti di Orchestra',
  'Simultaneous agents': 'Agenti simultanei',
  'Sub-agents running at once. Others wait for a free slot.':
    'Sotto-agenti in esecuzione contemporanea. Gli altri attendono uno slot libero.',
  'Subtasks per run': 'Sotto-attività per esecuzione',
  'Admitted in total. Finished subtasks still count; raise the limit to admit more.':
    'Ammesse in totale. Anche quelle completate contano; alza il limite per ammetterne altre.',
  'No limit': 'Nessun limite',
  'quota threshold': 'soglia di quota',
  'Stop giving new work at this usage. Missing or outdated quota data is checked with one job first.':
    'Smetti di assegnare lavoro oltre questo utilizzo. Se i dati di quota mancano o sono vecchi, prima si verifica con un solo lavoro.',
  Off: 'Disattivata',
  Theme: 'Tema',
  Language: 'Lingua',
  English: 'Inglese',
  Italian: 'Italiano',
  Light: 'Chiaro',
  Dark: 'Scuro',
  System: 'Sistema',
  'Larger text': 'Testo pi\u00f9 grande',
  'Increase interface text size while keeping the current layout style.':
    "Aumenta il testo dell'interfaccia mantenendo il layout attuale.",
  'Conversation folder': 'Cartella della conversazione',
  'Linked to this conversation. New conversations use your last selection.':
    "Collegata a questa conversazione. Le nuove conversazioni useranno l'ultima selezione.",
  'Choose a folder': 'Scegli una cartella',
  Window: 'Finestra',
  'Keep Lumen in the notification area when closed':
    "Mantieni Lumen nell'area di notifica alla chiusura",
  'Closing the window keeps running responses going. Quit from the tray icon.':
    "Chiudendo la finestra le risposte in corso continuano. Esci dall'icona nell'area di notifica.",
  'Open Lumen at login': "Apri Lumen all'accesso",
  'Starts quietly in the notification area.': "Si avvia in background nell'area di notifica.",
  'Opens the window when you sign in.': 'Apre la finestra quando accedi.',
  Settings: 'Impostazioni',
  General: 'Generali',
  Accounts: 'Account',
  Auto: 'Auto',
  Teamwork: 'Collaborazione',
  Tools: 'Strumenti',
  About: 'Informazioni',
  Simple: 'Semplice',
  Advanced: 'Avanzate',
  'Settings panel': 'Pannello impostazioni',
  'Close settings': 'Chiudi impostazioni',
  'Settings mode': 'Modalit\u00e0 impostazioni',
  'Settings sections': 'Sezioni delle impostazioni',
  'Dismiss settings error': 'Chiudi errore impostazioni',
  'Auto preferences': 'Preferenze Auto',
  'Choose how Auto weighs speed, quality and the remaining provider quota.':
    'Scegli come Auto bilancia velocit\u00e0, qualit\u00e0 e quota residua del provider.',
  'Balance usage': "Bilancia l'utilizzo",
  'Support Lumen on PayPal': 'Sostieni Lumen su PayPal',
  'Optional donations \u00b7 No features behind a paywall.':
    'Donazioni facoltative \u00b7 Nessuna funzione a pagamento.',
  'Share your feedback': 'Condividi un feedback',
  'Open source': 'Open source',
  'Restore defaults': 'Ripristino predefiniti',
  'Reset Lumen preferences in Simple and Advanced settings. Accounts and conversations are kept.':
    'Ripristina le preferenze di Lumen nelle impostazioni Semplici e Avanzate. Account e conversazioni restano invariati.',
  'Restore all settings to defaults': 'Ripristina tutte le impostazioni',
  'Confirm reset': 'Conferma ripristino',
  Cancel: 'Annulla',
  'Open sidebar': 'Apri barra laterale',
  'Checking question status…': 'Verifica dello stato della domanda…',
  'Question status unavailable. Retrying…': 'Stato della domanda non disponibile. Nuovo tentativo…',
  'Close navigation overlay': 'Chiudi pannello di navigazione',
  'Conversations and projects': 'Conversazioni e progetti',
  'Browser preview': 'Anteprima browser',
  'Project context': 'Contesto del progetto',
  'Project context: shared notes and files': 'Contesto del progetto: note e file condivisi',
  'Personal workspace': 'Spazio di lavoro personale',
  'Enter an absolute path after /workspace.': 'Scrivi il percorso assoluto dopo /workspace.',
  'Wait for the current response to finish before using this command.':
    'Attendi la fine della risposta in corso prima di usare questo comando.',
  'New conversation': 'Nuova conversazione',
  'New project': 'Nuovo progetto',
  'Project name': 'Nome del progetto',
  'Cancel new project': 'Annulla nuovo progetto',
  Create: 'Crea',
  Unsorted: 'Senza progetto',
  'A clear space': 'Uno spazio per',
  'for a brighter idea.': 'idee pi\u00f9 luminose.',
  'AI provider': 'Provider AI',
  Both: 'Entrambi',
  Ask: 'Chiedi',
  Edits: 'Modifiche',
  'Approve each action': 'Approva ogni azione',
  Autopilot: 'Autopilot',
  Permissions: 'Permessi',
  'Permissions: Ask': 'Permessi: Chiedi',
  'Permissions: Edits': 'Permessi: Modifiche',
  'Permissions: Approve each action': 'Permessi: approva ogni azione',
  'Permissions: Autopilot': 'Permessi: Autopilot',
  'click to change': 'clicca per cambiare',
  Project: 'Progetto',
  'Choose project': 'Scegli un progetto',
  'Change folder': 'Cambia cartella',
  'Advanced provider options': 'Opzioni avanzate del provider',
  'Attach files': 'Allega file',
  'Stop response': 'Interrompi risposta',
  'Send message': 'Invia messaggio',
  Message: 'Messaggio',
  "What's on your mind?": 'A cosa stai pensando?',
  'Enter to send': 'Invio per inviare',
  'Shift Enter for a new line': 'Maiusc+Invio per andare a capo',
  'All conversations': 'Tutte le conversazioni',
  'Search conversations': 'Cerca conversazioni',
  Usage: 'Utilizzo',
  'Dismiss error': 'Chiudi errore',
  'Your conversations': 'Le tue conversazioni',
  'No conversations found.': 'Nessuna conversazione trovata.',
  'A little room for your next big idea.': 'Spazio per la tua prossima grande idea.',
  'Load more conversations': 'Carica altre conversazioni',
  'Close sidebar': 'Chiudi barra laterale',
  'Orchestra is on: complex tasks are planned and split across agents. Click to turn off.':
    'Orchestra \u00e8 attiva: le attivit\u00e0 complesse vengono pianificate e suddivise tra agenti. Clicca per disattivarla.',
  'No features behind a paywall.': 'Nessuna funzione a pagamento.',
  'Connected accounts': 'Account collegati',
  'Account setup and provider requirements': 'Configurazione account e requisiti dei provider',
  'Sign in through the official CLI. Each account gets its own configuration folder. Antigravity, Gemini CLI and Ollama (local) are emergency fallbacks in Auto, used only after Claude and GPT are rate limited or out of quota.':
    'Accedi tramite la CLI ufficiale. Ogni account ha una propria cartella di configurazione. Antigravity, Gemini CLI e Ollama (locale) sono alternative di emergenza per Auto, usate solo quando Claude e GPT raggiungono il limite o esauriscono la quota.',
  found: 'rilevato',
  'not found': 'non rilevato',
  'Connection unchecked': 'Connessione non verificata',
  'Add account': 'Aggiungi account',
  'Connect an account to start your first conversation.':
    'Collega un account per iniziare la tua prima conversazione.',
  'Sign in': 'Accedi',
  'Install Ollama': 'Installa Ollama',
  Model: 'Modello',
  'Ollama model': 'Modello Ollama',
  'Automatic (largest)': 'Automatico (più grande)',
  'Remove this account and its local login files?':
    'Rimuovere questo account e i relativi file di accesso locali?',
  Remove: 'Rimuovi',
  Keep: 'Mantieni',
  'Account name': 'Nome account',
  'e.g. Personal': 'es. Personale',
  Provider: 'Provider',
  'How Claude, ChatGPT and the other connected assistants help each other.':
    'Come Claude, ChatGPT e gli altri assistenti collegati collaborano tra loro.',
  'For complex tasks, plan and split work across agents. Works with Auto.':
    'Per le attivit\u00e0 complesse, pianifica e suddividi il lavoro tra agenti. Funziona con Auto.',
  'Switch on rate limit': 'Passa ad un altro al raggiungimento del limite',
  'If one provider hits its limit, another continues the same message.':
    'Se un provider raggiunge il limite, un altro continua lo stesso messaggio.',
  'Code graph': 'Grafo del codice',
  "Claude and Codex get Lumen's code graph as tools: definitions with exact line ranges, callers, file outlines, related files and the impact of a change. They read only the lines they need instead of searching whole files.":
    'Claude e Codex ricevono il grafo del codice di Lumen come strumenti: definizioni con righe esatte, chiamanti, struttura dei file, file correlati e impatto di una modifica. Leggono solo le righe che servono invece di cercare in file interi.',
  'Self-healing': 'Correzione automatica',
  'After edits, run the project check and fix failures automatically (up to 3 times).':
    'Dopo le modifiche, esegui il controllo del progetto e correggi automaticamente gli errori (fino a 3 tentativi).',
  'Cross-audit rounds': 'Cicli di verifica incrociata',
  'How many review and revise cycles an audit may run.':
    'Numero di cicli di revisione e correzione che una verifica può eseguire.',
  Account: 'Account',
  'Codex supports automatic quota queries. For Claude, copy the percentages and reset times from /usage. Leave a window blank if it is unavailable.':
    'Codex supporta la lettura automatica della quota. Per Claude, copia le percentuali e gli orari di ripristino da /usage. Lascia vuoto il periodo se non è disponibile.',
  '5-hour': '5 ore',
  Weekly: 'Settimanale',
  'Tools (MCP)': 'Strumenti (MCP)',
  'Add MCP server': 'Aggiungi server MCP',
  'Turn a tool on once and both assistants can use it when edits are allowed.':
    'Attiva uno strumento una volta: entrambi gli assistenti potranno usarlo quando sono consentite le modifiche.',
  Name: 'Nome',
  'Command or URL': 'Comando o URL',
  'Add tool': 'Aggiungi strumento',
  'No tools yet.': 'Nessuno strumento.',
  Ready: 'Pronto',
  'Command not found': 'Comando non trovato',
  Enable: 'Attiva',
  Skills: 'Competenze',
  'none installed': 'nessuna installata',
  'Import skill folder containing SKILL.md': 'Importa una cartella di competenze con SKILL.md',
  'Import skill': 'Importa competenza',
  'Edit as JSON': 'Modifica come JSON',
  'Shared MCP configuration': 'Configurazione MCP condivisa',
  Save: 'Salva',
  Error: 'Errore',
  'Sending…': 'Invio in corso…',
  'Delivery not confirmed': 'Invio non confermato',
  'Draft restored in this conversation.': 'Bozza ripristinata in questa conversazione.',
  'Check message delivery': 'Verifica invio del messaggio',
  'Wait for message delivery before deleting this conversation.':
    'Attendi la conferma di invio prima di eliminare questa conversazione.',
  'No requests yet.': 'Ancora nessuna richiesta.',
  'Execution progress': 'Avanzamento dell’esecuzione',
  'Loading model catalog': 'Caricamento catalogo modelli',
  'Waiting for an execution slot': 'In attesa di uno slot di esecuzione',
  'Starting provider': 'Avvio del provider',
  'Waiting for provider': 'In attesa del provider',
  'Receiving response': 'Ricezione della risposta',
  'Reasoning received': 'Ragionamento ricevuto',
  'Tool operation open': 'Operazione strumento aperta',
  'Tool activity received': 'Attività strumento ricevuta',
  'Waiting for your answer': 'In attesa della tua risposta',
  'Waiting for permission': 'In attesa di un permesso',
  'Waiting for plan approval': 'In attesa di approvazione del piano',
  'Waiting for project access': 'In attesa di accesso al progetto',
  'Preparing context': 'Preparazione del contesto',
  'Running checks': 'Verifiche in corso',
  'Provider turn returned': 'Il provider ha concluso il turno',
  'Provider call cancelled': 'Chiamata al provider annullata',
  'Provider call failed': 'Chiamata al provider fallita',
  'Turn ended': 'Turno terminato',
  'Turn failed': 'Turno fallito',
  'Turn interrupted': 'Turno interrotto',
  'Waiting to start': 'In attesa di avvio',
  'Between execution stages': 'Passaggio tra fasi di esecuzione',
  'Execution status': 'Stato dell’esecuzione',
  'Reported active agents': 'Agenti attivi segnalati',
  Checks: 'Verifiche',
  'Last event': 'Ultimo evento',
  'Observed duration': 'Durata osservata',
  'Providers active': 'Provider attivi',
  'Preparing request': 'Preparazione della richiesta',
  'Awaiting backend acceptance. Provider execution has not been confirmed.':
    'In attesa dell’accettazione del backend. L’esecuzione del provider non è ancora confermata.',
  'No recent updates': 'Nessun aggiornamento recente',
  'Process start': 'Avvio processo',
  'First provider event': 'Primo evento dal provider',
  'First text': 'Primo testo',
  'Provider return': 'Ritorno del provider',
  'Timing ends at the last observation; the interruption time is unknown.':
    'I tempi si fermano all’ultima osservazione; l’istante dell’interruzione non è noto.',
  'No new recorded provider events for at least 30 seconds. This does not prove a stall. Silence does not trigger an automatic resend.':
    'Nessun nuovo evento registrato dal provider da almeno 30 secondi. Non è una prova di blocco. Il silenzio non attiva un reinvio automatico.',
  'The turn ended. This does not certify that the requested task or its checks are complete.':
    'Il turno è terminato. Questo non certifica che il compito richiesto o le sue verifiche siano conclusi.',
  'An input is pending. Use the question or approval controls in this conversation.':
    'È richiesto un intervento. Usa i controlli della domanda o del permesso in questa conversazione.',
  'Last observed activity': 'Ultima attività osservata',
  'No provider event yet': 'Nessun evento dal provider ricevuto',
  'Check results': 'Esiti delle verifiche',
  'Detailed timings are unavailable for this turn.':
    'I tempi dettagliati non sono disponibili per questo turno.',
  'Observed events': 'Eventi osservati',
  'Tool events': 'Eventi strumento',
  'Open tool operations': 'Operazioni strumento aperte',
  'Open questions': 'Domande aperte',
  'Refreshing status…': 'Aggiornamento stato…',
  'Refresh execution status': 'Aggiorna stato dell’esecuzione',
  text: 'testo',
  reasoning: 'ragionamento',
  tool: 'strumento',
  question: 'domanda',
  initialized: 'inizializzazione',
  'provider-event': 'evento del provider',
  error: 'errore',
};

interface I18nValue {
  language: Language;
  setLanguage: (language: Language) => void;
  t: (english: string) => string;
}
const I18nContext = createContext<I18nValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [language, setLanguage] = useState<Language>(() =>
    localStorage.getItem('lumen-language') === 'it' ? 'it' : 'en',
  );
  useEffect(() => {
    localStorage.setItem('lumen-language', language);
    document.documentElement.lang = language;
  }, [language]);
  const value = useMemo<I18nValue>(
    () => ({
      language,
      setLanguage,
      t: (english) => {
        if (language !== 'it') return english;
        const attachedFiles = english.match(/^(\d+) files attached$/);
        if (attachedFiles) return `${attachedFiles[1]} file allegati`;
        return it[english] ?? english;
      },
    }),
    [language],
  );
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const value = useContext(I18nContext);
  if (!value) throw new Error('useI18n must be used inside I18nProvider');
  return value;
}
