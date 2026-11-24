import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '../../design/tokens.css';
import '../../design/reset.css';
import { Gallery } from '../../gallery/Gallery';

// M1: the desktop surface shows the brand gallery until the real shell lands in M2.
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Gallery />
  </StrictMode>,
);
