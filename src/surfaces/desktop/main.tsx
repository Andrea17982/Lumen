import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '../../design/tokens.css';
import '../../design/reset.css';
import { Sync } from '../../sync/sync';
import { SyncProvider } from '../../sync/react';
import { selectTransport, unavailable } from '../../transport/select';
import { App } from './App';

const sync = new Sync((await selectTransport()) ?? unavailable);

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <SyncProvider sync={sync}>
      <App />
    </SyncProvider>
  </StrictMode>,
);
