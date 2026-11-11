// CSP vs. build output (D-86): the real app's CSP (`tauri.conf.json`) allows fonts and styles only from 'self', so
// the build must never inline assets as data: URLs. Found by the M8 e2e: a small @fontsource woff2 subset was inlined
// into the CSS and blocked (`font-src`).
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// vitest runs from the repo root.
const conf = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')) as { app: { security: { csp: string } } };
const csp = conf.app.security.csp;

describe('CSP and the build', () => {
  it('keeps fonts and styles on self', () => {
    expect(csp).toContain("font-src 'self';");
    expect(csp).toContain("style-src 'self';");
  });

  it('vite never inlines assets', () => {
    expect(readFileSync('vite.config.ts', 'utf8')).toMatch(/assetsInlineLimit:\s*0\b/);
  });

  it('a built dist has no data: URLs in CSS (skipped before the first build)', () => {
    if (!existsSync('dist/assets')) return;
    const css = readdirSync('dist/assets').filter((f) => f.endsWith('.css'));
    for (const f of css) {
      expect(readFileSync(`dist/assets/${f}`, 'utf8'), f).not.toMatch(/url\(\s*["']?data:/);
    }
  });
});
