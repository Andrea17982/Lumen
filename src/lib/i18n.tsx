import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';

export type Language = 'en' | 'it';

const it: Record<string, string> = {
  Appearance: 'Aspetto',
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
  'Browser preview': 'Anteprima browser',
  'Project context': 'Contesto del progetto',
  'Project context: shared notes and files': 'Contesto del progetto: note e file condivisi',
  'Personal workspace': 'Spazio di lavoro personale',
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
  'Auto usage balancing': 'Bilanciamento dell’utilizzo di Auto',
  'Balance remaining quota against time until reset. Fresh snapshots last 15 minutes. Unknown usage never counts as free capacity.':
    'Bilancia la quota residua con il tempo al ripristino. I dati aggiornati restano validi per 15 minuti. L’utilizzo sconosciuto non viene considerato quota disponibile.',
  Account: 'Account',
  'Usage account': 'Account per l’utilizzo',
  'Choose account': 'Scegli un account',
  'Refresh official quota': 'Aggiorna la quota ufficiale',
  Stale: 'Dati non aggiornati',
  'Codex supports automatic quota queries. For Claude, copy the percentages and reset times from /usage. Leave a window blank if it is unavailable.':
    'Codex supporta la lettura automatica della quota. Per Claude, copia le percentuali e gli orari di ripristino da /usage. Lascia vuoto il periodo se non è disponibile.',
  '5-hour': '5 ore',
  Weekly: 'Settimanale',
  'used %': 'utilizzo %',
  'Reset (local time)': 'Ripristino (ora locale)',
  'Enter at least one quota percentage and reset time.':
    'Inserisci almeno una percentuale di quota e un orario di ripristino.',
  'Save quota snapshot': 'Salva dati quota',
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
  'Local API': 'API locale',
  'Let other apps on this PC use your subscriptions through an OpenAI-compatible endpoint. Read-only, local only.':
    'Consenti alle altre app di questo PC di usare i tuoi abbonamenti tramite un endpoint compatibile con OpenAI. Solo lettura e solo in locale.',
  'Serve on localhost': 'Rendi disponibile su localhost',
  'Models: lumen-auto, lumen-claude, lumen-chatgpt.':
    'Modelli: lumen-auto, lumen-claude, lumen-chatgpt.',
  Error: 'Errore',
  'No requests yet.': 'Ancora nessuna richiesta.',
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
