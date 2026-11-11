// Language colours (FR-19): our own 12-slot palette (`--lang-0..11` + `--lang-other` in tokens.css, both themes),
// not GitHub/linguist colours (R-11). Common languages have a fixed slot so they look the same in every repo;
// anything else gets a slot from a hash of its name.

import { hash32 } from './hash';

export const LANG_SLOTS = 12;

const FIXED: Record<string, number> = {
  rust: 0,
  typescript: 1,
  javascript: 2,
  python: 3,
  go: 4,
  c: 5,
  'c++': 6,
  'c#': 7,
  java: 8,
  kotlin: 9,
  shell: 10,
  html: 11,
  css: 7,
  scss: 7,
  markdown: 9,
  toml: 4,
  json: 10,
  yaml: 3,
  makefile: 5,
  ruby: 11,
  php: 8,
  swift: 2,
  lua: 6,
  powershell: 1,
  sql: 0,
  vue: 4,
  svelte: 11,
};

/** Palette slot 0..11 for a language name; null for "Other". */
export function langSlot(name: string): number | null {
  const k = name.trim().toLowerCase();
  if (k === 'other') return null;
  const fixed = FIXED[k];
  if (fixed !== undefined) return fixed;
  return hash32(k) % LANG_SLOTS;
}

/** Class setting `--lang` for `.lang-dot` / `.langbar__seg` (no inline colours). */
export function langClass(name: string): string {
  const s = langSlot(name);
  return s === null ? 'lang--other' : `lang--${s}`;
}

/** CSS colour value (a token, so it follows the theme). */
export function langColor(name: string): string {
  const s = langSlot(name);
  return s === null ? 'var(--lang-other)' : `var(--lang-${s})`;
}
