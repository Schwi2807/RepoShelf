import { describe, expect, it } from 'vitest';
import { ensureLanguage, highlightLines, runHighlight } from './engine';
import { highlight } from './highlighter';
import { fromShebang, LANG_IDS, languageFor, languageForFence } from './languages';
import { LOADERS } from './loaders';
import type { FromWorker } from './protocol';
import { escapeHtml, splitLines } from './splitLines';

describe('splitLines', () => {
  it('closes and reopens nested spans at line breaks', () => {
    const html = '<span class="a">x<span class="b">y\nz</span>\nw</span>v';
    expect(splitLines(html)).toEqual([
      '<span class="a">x<span class="b">y</span></span>',
      '<span class="a"><span class="b">z</span></span>',
      '<span class="a">w</span>v',
    ]);
  });
  it('keeps entities and empty lines', () => {
    expect(splitLines('a &lt; b\n\n<span class="c">"&amp;"</span>')).toEqual(['a &lt; b', '', '<span class="c">"&amp;"</span>']);
  });
  it('every line is balanced', () => {
    const lines = splitLines('<span class="hljs-comment">/* one\ntwo\nthree */</span>\n<span class="k">fn</span>');
    for (const l of lines) {
      expect((l.match(/<span/g) ?? []).length).toBe((l.match(/<\/span>/g) ?? []).length);
    }
    expect(lines).toHaveLength(4);
  });
  it('escapeHtml', () => {
    expect(escapeHtml('<a href="x">&</a>')).toBe('&lt;a href="x"&gt;&amp;&lt;/a&gt;');
  });
});

describe('languages', () => {
  it.each([
    ['src/main.rs', 'rust'],
    ['a/b/App.tsx', 'typescript'],
    ['index.mjs', 'javascript'],
    ['Cargo.toml', 'ini'],
    ['Makefile', 'makefile'],
    ['docker/Dockerfile', 'dockerfile'],
    ['Dockerfile.dev', 'dockerfile'],
    ['CMakeLists.txt', 'cmake'],
    ['README.md', 'markdown'],
    ['styles/site.SCSS', 'scss'],
    ['logo.svg', 'xml'],
    ['main.go', 'go'],
    ['x.yml', 'yaml'],
    ['setup.ps1', 'powershell'],
  ])('%s → %s', (path, id) => {
    expect(languageFor(path)).toBe(id);
  });
  it('unknown → null', () => {
    expect(languageFor('LICENSE')).toBeNull();
    expect(languageFor('data.bin')).toBeNull();
  });
  it('about 45 languages, each with a loader', () => {
    expect(LANG_IDS.length).toBeGreaterThanOrEqual(45);
    for (const id of LANG_IDS) expect(typeof LOADERS[id]).toBe('function');
  });
  it('fence aliases', () => {
    expect(languageForFence('ts')).toBe('typescript');
    expect(languageForFence('toml')).toBe('ini');
    expect(languageForFence('rust')).toBe('rust');
    expect(languageForFence('brainfuck')).toBeNull();
  });
});

describe('shebang', () => {
  it.each([
    ['#!/bin/sh', 'bash'],
    ['#!/usr/bin/env bash', 'bash'],
    ['#!/usr/bin/env python3', 'python'],
    ['#!/usr/bin/python3.12 -u', 'python'],
    ['#!/usr/bin/env -S node --no-warnings', 'javascript'],
    ['#!/usr/bin/env pwsh', 'powershell'],
    ['#!/usr/bin/perl -w', 'perl'],
  ])('%s → %s', (line, id) => {
    expect(fromShebang(line)).toBe(id);
  });
  it('no shebang / unknown interpreter → null; extension wins over shebang', () => {
    expect(fromShebang('echo hi')).toBeNull();
    expect(fromShebang('#!/usr/bin/env frobnicate')).toBeNull();
    expect(languageFor('scripts/run', '#!/usr/bin/env bash')).toBe('bash');
    expect(languageFor('x.py', '#!/bin/sh')).toBe('python');
  });
});

describe('engine (worker body)', () => {
  it('produces class-based, escaped, per-line HTML', async () => {
    const out: FromWorker[] = [];
    await runHighlight(1, 'rust', 'fn main() {\n    let s = "<b>";\n}\n', (m) => out.push(m), () => false);
    expect(out[out.length - 1]).toEqual({ type: 'done', id: 1 });
    const chunk = out.find((m) => m.type === 'chunk');
    expect(chunk?.type === 'chunk' && chunk.lines).toHaveLength(3);
    const all = chunk?.type === 'chunk' ? chunk.lines.join('\n') : '';
    expect(all).toContain('hljs-keyword');
    expect(all).toContain('&lt;b&gt;');
    expect(all).not.toContain('style=');
  });

  it('posts 2,000-line chunks, first one provisional', async () => {
    const text = Array.from({ length: 4500 }, (_, i) => `let x${i} = ${i};`).join('\n');
    const out: FromWorker[] = [];
    await runHighlight(2, 'javascript', text, (m) => out.push(m), () => false);
    const chunks = out.filter((m) => m.type === 'chunk');
    expect(chunks.map((c) => (c.type === 'chunk' ? [c.start, c.lines.length] : []))).toEqual([
      [0, 2000],
      [0, 2000],
      [2000, 2000],
      [4000, 500],
    ]);
  });

  it('too large → plain; cancelled → stops', async () => {
    const out: FromWorker[] = [];
    await runHighlight(3, 'rust', 'x\n'.repeat(20_001), (m) => out.push(m), () => false);
    expect(out).toEqual([{ type: 'plain', id: 3, reason: 'tooLarge' }]);
    const out2: FromWorker[] = [];
    await runHighlight(4, 'rust', 'fn a() {}', (m) => out2.push(m), () => true);
    expect(out2).toEqual([]);
  });

  it('a multi-line comment keeps its class on every line', async () => {
    await ensureLanguage('c');
    const lines = highlightLines('/* a\nb\nc */\nint x;', 'c');
    expect(lines.slice(0, 3).every((l) => l.includes('hljs-comment'))).toBe(true);
  });

  it('client falls back to plain without Worker support', async () => {
    const job = highlight('fn a() {}', 'rust', () => undefined);
    expect(['plain', 'done']).toContain(await job.done);
  });
});
