// Managed (bare) clone of a TypeScript repo with releases.

import type { Rendered } from '../../types';
import { DAY, HOUR, NOW, PEOPLE } from './gen';
import type { RepoDef } from './model';

const INDEX_TS = `export { Button } from './components/Button';
export { Dialog } from './components/Dialog';
export { useTheme, ThemeProvider } from './theme';
export type { Theme, Tone } from './theme';
`;

const BUTTON_TSX = `import type { ButtonHTMLAttributes } from 'react';
import { cx } from '../util/cx';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  tone?: 'neutral' | 'primary' | 'danger';
  size?: 'sm' | 'md';
}

/** A button that follows the active theme. */
export function Button({ tone = 'neutral', size = 'md', className, ...rest }: ButtonProps) {
  return <button className={cx('lm-btn', \`lm-btn--\${tone}\`, \`lm-btn--\${size}\`, className)} {...rest} />;
}
`;

const DIALOG_TSX = `import { useEffect, useRef, type ReactNode } from 'react';

export function Dialog({ open, onClose, children }: { open: boolean; onClose: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);
  return (
    <dialog ref={ref} onClose={onClose} className="lm-dialog">
      {children}
    </dialog>
  );
}
`;

const THEME_TS = `import { createContext, useContext } from 'react';

export type Theme = 'light' | 'dark';
export type Tone = 'neutral' | 'primary' | 'danger';

const Ctx = createContext<Theme>('light');
export const ThemeProvider = Ctx.Provider;
export const useTheme = (): Theme => useContext(Ctx);
`;

// v0.2.0 (F2/F3): a typical centred GitHub README head, in the shape the rs-core sanitiser emits (D-96): centred
// logo + h1 + tagline, a badge row (links around remote images), a centred table of feature cards with images, a
// right-floated screenshot, and one image from an unreachable host (chip fallback).
const README_HTML = `<div align="center">
<img src="https://raw.githubusercontent.com/lumen-ui/lumen/main/docs/logo.png" width="96" height="96" alt="Lumen logo">
<h1 align="center">Lumen</h1>
<p align="center">Accessible React components with a tiny footprint. Themeable with CSS custom properties.</p>
<p align="center">
<a href="https://github.com/lumen-ui/lumen/actions" rel="noopener noreferrer"><img src="https://img.shields.io/badge/build-passing-green" alt="build"></a>
<a href="https://www.npmjs.com/package/lumen-ui" rel="noopener noreferrer"><img src="https://img.shields.io/badge/npm-v2.0.0--beta.1-blue" alt="npm"></a>
<img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="license">
</p>
</div>
<table align="center">
<tr>
<td align="center"><img src="https://user-images.example.org/lumen/button.png" width="72" alt="Button"><br><b>Buttons</b><br>Three tones, two sizes</td>
<td align="center"><img src="https://user-images.example.org/lumen/dialog.png" width="72" alt="Dialog"><br><b>Dialogs</b><br>Focus trapped and returned</td>
<td align="center"><img src="https://user-images.example.org/lumen/theme.png" width="72" alt="Theme"><br><b>Theming</b><br>CSS custom properties</td>
</tr>
</table>
<img align="right" width="180" src="https://user-images.example.org/lumen/screenshot.png" alt="Screenshot of the demo page">
<p>Lumen ships unstyled-by-default primitives with a small theme layer on top. Every component is keyboard friendly,
follows the WAI-ARIA authoring practices and renders without a runtime CSS-in-JS library. The demo page on the right
shows the default light theme.</p>
<p>Diagram (its server is offline in this mock): <img src="https://fail.example.com/lumen/architecture.png" alt="architecture diagram"></p>
<h2><a href="#user-content-quick-start" id="user-content-quick-start" rel="noopener noreferrer"></a>Quick start</h2>
<pre><code class="language-bash">npm install lumen-ui
</code></pre>
<pre><code class="language-tsx">import { Button } from 'lumen-ui';

export const Save = () =&gt; &lt;Button tone="primary"&gt;Save&lt;/Button&gt;;
</code></pre>
<h2><a href="#user-content-components" id="user-content-components" rel="noopener noreferrer"></a>Components</h2>
<ul>
<li><a href="rsrepo:/src/components/Button.tsx" rel="noopener noreferrer">Button</a></li>
<li><a href="rsrepo:/src/components/Dialog.tsx" rel="noopener noreferrer">Dialog</a></li>
</ul>
<blockquote>
<p>Lumen follows <a href="https://semver.org/" rel="noopener noreferrer">semantic versioning</a>.
Questions? <a href="mailto:maintainers@example.com" rel="noopener noreferrer">maintainers@example.com</a></p>
</blockquote>
`;

const README: Rendered = {
  html: README_HTML,
  headings: [
    { level: 2, text: 'Quick start', anchor: 'user-content-quick-start' },
    { level: 2, text: 'Components', anchor: 'user-content-components' },
  ],
  sourcePath: 'README.md',
};

export function lumen(): RepoDef {
  return {
    summary: {
      id: 2,
      owner: 'lumen-ui',
      name: 'lumen',
      source: 'cloned',
      path: 'C:\\Users\\demo\\Documents\\RepoShelf\\github.com\\lumen-ui\\lumen.git',
      description: 'Accessible React components with a tiny footprint.',
      topics: ['react', 'typescript', 'components', 'a11y'],
      homepage: 'https://lumen.example.dev',
      originUrl: 'https://github.com/lumen-ui/lumen.git',
      originHost: 'github.com',
      defaultBranch: 'main',
      primaryLanguage: 'TypeScript',
      lastCommitAt: NOW - 2 * DAY,
      addedAt: NOW - 12 * DAY,
      clonedAt: NOW - 12 * DAY,
      lastOpenedAt: NOW - 1 * DAY,
      lastFetchedAt: NOW - 20 * HOUR,
      status: { type: 'ok' },
    },
    isBare: true,
    head: { type: 'branch', name: 'main' },
    license: 'Apache-2.0',
    fetchable: true,
    canFastForward: true,
    dirty: null,
    files: {
      '.gitignore': 'node_modules/\ndist/\n',
      'package.json':
        '{\n  "name": "lumen-ui",\n  "version": "2.0.0-beta.1",\n  "type": "module",\n  "main": "dist/index.js",\n  "scripts": { "build": "tsc -p .", "test": "vitest run" }\n}\n',
      'tsconfig.json': '{\n  "compilerOptions": { "strict": true, "jsx": "react-jsx", "target": "ES2022" }\n}\n',
      'README.md': '# Lumen\n\nAccessible React components with a tiny footprint.\n',
      LICENSE: 'Apache License\nVersion 2.0, January 2004\n',
      'CHANGELOG.md': '# Changelog\n\n## 2.0.0-beta.1\n\n- New Dialog component\n',
      'src/index.ts': INDEX_TS,
      'src/theme.ts': THEME_TS,
      'src/components/Button.tsx': BUTTON_TSX,
      'src/components/Dialog.tsx': DIALOG_TSX,
      'src/components/button.css': '.lm-btn {\n  border-radius: 6px;\n  padding: 4px 12px;\n}\n.lm-btn--primary {\n  background: var(--lm-accent);\n}\n',
      'src/util/cx.ts':
        "export const cx = (...xs: (string | false | undefined)[]): string => xs.filter(Boolean).join(' ');\n",
      'test/button.test.tsx':
        "import { describe, it, expect } from 'vitest';\n\ndescribe('Button', () => {\n  it('renders', () => {\n    expect(true).toBe(true);\n  });\n});\n",
      'package-lock.json': '{\n  "name": "lumen-ui",\n  "lockfileVersion": 3\n}\n',
    },
    history: {
      seed: 'lumen',
      count: 412,
      newest: NOW - 2 * DAY,
      summaries: [
        'Dialog: return focus on close',
        'Release v2.0.0-beta.1',
        'Button: add size prop',
        'Docs: theming guide',
        'Release v1.1.0',
      ],
    },
    branches: [{ name: 'main', at: 0 }],
    remoteBranches: [
      { name: 'origin/main', at: 0, ahead: ['Dialog: trap focus inside', 'Bump deps'] },
      { name: 'origin/next', at: 3, ahead: ['Next: CSS layers'] },
      { name: 'origin/gh-pages', at: 40 },
    ],
    tags: [
      {
        name: 'v2.0.0-beta.1',
        at: 1,
        annotated: true,
        tagger: PEOPLE[4],
        message: 'Lumen 2.0 beta 1',
        notesHtml: '<p>First beta of <strong>Lumen 2</strong>.</p>\n<ul>\n<li>New <code>Dialog</code></li>\n<li>Buttons get a <code>size</code> prop</li>\n</ul>\n',
      },
      {
        name: 'v1.1.0',
        at: 4,
        annotated: true,
        tagger: PEOPLE[4],
        message: 'Lumen 1.1.0',
        notesHtml: '<p>Theming guide and small fixes.</p>\n',
      },
      { name: 'v1.0.0', at: 50, annotated: true, tagger: PEOPLE[2], message: 'Lumen 1.0.0', notesHtml: '<p>First stable release.</p>\n' },
      { name: 'v0.9.0', at: 120 },
    ],
    readmes: { '': README },
    markdown: { 'README.md': README },
    languages: [
      { name: 'TypeScript', bytes: 61_200, percent: 78.1 },
      { name: 'CSS', bytes: 11_400, percent: 14.6 },
      { name: 'JavaScript', bytes: 3_900, percent: 5.0 },
      { name: 'Markdown', bytes: 1_800, percent: 2.3 },
    ],
    // M7.8: a merge commit on the first commits page
    merges: [{ at: 28, branch: 'next-dialog', commits: ['Dialog: initial focus option', 'Dialog: docs', 'Dialog: fix Escape in nested menus'] }],
    ghNotes: {
      'v1.1.0': {
        title: 'Lumen 1.1 — theming',
        // v0.2.0 (F5): contributor avatars arrive as mention links (rs-core `render_release_notes`)
        html:
          '<p>Release notes from GitHub: <strong>theming guide</strong>, focus fixes.</p>\n' +
          '<p>Thanks to <a href="https://github.com/mira-dev" rel="noopener noreferrer" class="user-mention">@mira-dev</a>.</p>\n',
        assets: [
          { name: 'lumen-1.1.0-x86_64-pc-windows-msvc.zip', size: 2_412_800, downloadCount: 318, sha256: 'a'.repeat(64), forThisPc: true },
          { name: 'lumen-1.1.0-x86_64-unknown-linux-gnu.tar.gz', size: 2_301_440, downloadCount: 97 },
          { name: 'lumen-1.1.0-aarch64-apple-darwin.tar.gz', size: 2_280_100, downloadCount: 54 },
          { name: 'SHA256SUMS', size: 312 },
        ],
      },
    },
  };
}
