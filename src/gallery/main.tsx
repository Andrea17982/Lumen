import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '../design/tokens.css';
import '../design/reset.css';
import { Gallery } from './Gallery';

// Development gallery of the brand and the design system. Not part of the desktop build.
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Gallery />
  </StrictMode>,
);
