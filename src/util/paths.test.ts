/// <reference types="node" />
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { LANG_SLOTS, langColor, langSlot } from './lang-colors';
import { baseName, extension, isMarkdownPath, joinPath, normalizePath, parentDir, pathSegments } from './paths';

describe('paths', () => {
  it('breadcrumb segments are cumulative', () => {
    expect(pathSegments('crates/core/src')).toEqual([
      { name: 'crates', path: 'crates' },
      { name: 'core', path: 'crates/core' },
      { name: 'src', path: 'crates/core/src' },
    ]);
    expect(pathSegments('')).toEqual([]);
    expect(pathSegments('/a//b/')).toEqual([
      { name: 'a', path: 'a' },
      { name: 'b', path: 'a/b' },
    ]);
  });

  it('parent dir and base name', () => {
    expect(parentDir('src/stream/mod.rs')).toBe('src/stream');
    expect(parentDir('README.md')).toBe('');
    expect(parentDir('')).toBe('');
    expect(baseName('src/stream/mod.rs')).toBe('mod.rs');
    expect(baseName('')).toBe('');
  });

  it('joins git paths with one slash', () => {
    expect(joinPath('', 'src')).toBe('src');
    expect(joinPath('src/', '/lib.rs')).toBe('src/lib.rs');
    expect(joinPath('a/b', '')).toBe('a/b');
    expect(normalizePath('//a///b//')).toBe('a/b');
    // names with Windows-invalid characters are plain git names here (D-39)
    expect(joinPath('docs', 'a:b.txt')).toBe('docs/a:b.txt');
  });

  it('extensions and markdown detection', () => {
    expect(extension('src/lib.RS')).toBe('rs');
    expect(extension('.gitignore')).toBe('');
    expect(extension('Makefile')).toBe('');
    expect(isMarkdownPath('docs/Guide.md')).toBe(true);
    expect(isMarkdownPath('README.markdown')).toBe(true);
    expect(isMarkdownPath('README.txt')).toBe(false);
  });
});

describe('lang-colors', () => {
  it('fixed slots for common languages, hash slot otherwise, null for Other', () => {
    expect(langSlot('Rust')).toBe(0);
    expect(langSlot('rust')).toBe(0);
    expect(langSlot('TypeScript')).toBe(1);
    expect(langSlot('Other')).toBeNull();
    const s = langSlot('Zig');
    expect(s).not.toBeNull();
    expect(s!).toBeGreaterThanOrEqual(0);
    expect(s!).toBeLessThan(LANG_SLOTS);
    expect(langSlot('Zig')).toBe(s);
    expect(langColor('Rust')).toBe('var(--lang-0)');
    expect(langColor('Other')).toBe('var(--lang-other)');
  });

  it('every slot is a token in both themes', () => {
    const css = readFileSync('src/styles/tokens.css', 'utf8');
    for (const theme of ['light', 'dark']) {
      const m = new RegExp(`:root\\[data-theme='${theme}'\\]\\s*\\{([^}]*)\\}`).exec(css);
      expect(m, theme).not.toBeNull();
      for (let i = 0; i < LANG_SLOTS; i++) expect(m![1]).toMatch(new RegExp(`--lang-${i}:\\s*#[0-9a-f]{6};`));
      expect(m![1]).toMatch(/--lang-other:\s*#[0-9a-f]{6};/);
    }
  });
});
