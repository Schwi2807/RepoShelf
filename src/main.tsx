import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import './styles/tokens.css';
import './styles/base.css';
import './styles/layout.css';
import './styles/components.css';
import './styles/code.css';
import './styles/diff.css';
import './styles/markdown.css';
import './styles/screens.css';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import { startQueryInvalidation } from './ipc/query';
import { startRouter } from './router/router';
import { startJobs } from './state/jobs';
import { startLibrary } from './state/library';
import { loadSettings } from './state/settings';
import { startTheme } from './state/theme';

const root = document.getElementById('root');
if (!root) throw new Error('#root missing');

// Theme first (data-theme on <html> before the first paint; no inline script, CSP), then the stores.
startTheme();
startRouter();
startQueryInvalidation();
void loadSettings();
void startLibrary();
void startJobs();

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
