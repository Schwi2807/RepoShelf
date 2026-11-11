// FR-33 / R-11: every text token on every background it is used on has WCAG contrast ≥ 4.5:1, in both themes.

/// <reference types="node" />
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// Read from disk: vitest's CSS handling turns `?raw` CSS imports into an empty string.
// (vitest runs from the repo root; under happy-dom import.meta.url is not a file: URL.)
const tokensCss = readFileSync('src/styles/tokens.css', 'utf8');

function block(theme: 'light' | 'dark'): Map<string, string> {
  const re = new RegExp(`:root\\[data-theme='${theme}'\\]\\s*\\{([^}]*)\\}`);
  const m = re.exec(tokensCss);
  if (!m) throw new Error(`no ${theme} block`);
  const out = new Map<string, string>();
  for (const decl of m[1].matchAll(/--([a-z0-9-]+):\s*(#[0-9a-fA-F]{6})\s*;/g)) out.set(decl[1], decl[2].toLowerCase());
  return out;
}

function luminance(hex: string): number {
  const ch = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const lin = ch.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
}

export function contrast(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

const SURFACES = ['bg-canvas', 'bg-subtle', 'bg-overlay', 'bg-hover', 'bg-inset', 'bg-selected'];
const TEXT = ['fg-default', 'fg-muted', 'fg-subtle', 'fg-link', 'accent-fg', 'success', 'danger', 'attention', 'done'];
const CODE_BG = ['bg-canvas', 'bg-subtle'];
const HL = [
  'hl-keyword',
  'hl-string',
  'hl-number',
  'hl-comment',
  'hl-function',
  'hl-title',
  'hl-type',
  'hl-attr',
  'hl-variable',
  'hl-meta',
  'hl-deletion',
  'hl-addition',
];

function pairs(): [string, string][] {
  const p: [string, string][] = [];
  for (const fg of TEXT) for (const bg of SURFACES) p.push([fg, bg]);
  for (const fg of HL) for (const bg of CODE_BG) p.push([fg, bg]);
  p.push(['fg-on-accent', 'accent'], ['fg-on-accent', 'accent-hover']);
  p.push(['accent-fg', 'accent-subtle'], ['success', 'success-subtle'], ['danger', 'danger-subtle']);
  p.push(['attention', 'attention-subtle'], ['done', 'done-subtle'], ['indigo', 'indigo-subtle']);
  p.push(['fg-default', 'accent-subtle'], ['fg-default', 'indigo-subtle']);
  p.push(['header-fg', 'header-bg'], ['header-fg-muted', 'header-bg'], ['header-fg', 'header-input-bg']);
  p.push(['header-fg-muted', 'header-input-bg']);
  p.push(['diff-hunk-fg', 'diff-hunk-bg']);
  for (const bg of ['diff-add-bg', 'diff-del-bg', 'diff-add-num-bg', 'diff-del-num-bg']) p.push(['fg-default', bg]);
  for (const bg of ['diff-add-num-bg', 'diff-del-num-bg']) p.push(['fg-muted', bg]);
  return p;
}

describe.each(['light', 'dark'] as const)('%s theme', (theme) => {
  const vars = block(theme);
  it('defines every token group', () => {
    for (const k of [...SURFACES, ...TEXT, ...HL, 'fg-on-accent', 'accent', 'border-default', 'focus-ring', 'header-bg']) {
      expect(vars.has(k), k).toBe(true);
    }
  });
  it.each(pairs())('%s on %s ≥ 4.5:1', (fg, bg) => {
    const a = vars.get(fg);
    const b = vars.get(bg);
    expect(a, fg).toBeDefined();
    expect(b, bg).toBeDefined();
    const c = contrast(a!, b!);
    expect(c, `${fg} ${a} on ${bg} ${b} = ${c.toFixed(2)}`).toBeGreaterThanOrEqual(4.5);
  });
  it('focus ring is visible on the canvas (≥ 3:1, WCAG non-text)', () => {
    expect(contrast(vars.get('focus-ring')!, vars.get('bg-canvas')!)).toBeGreaterThanOrEqual(3);
  });
});
