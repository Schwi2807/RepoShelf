import { describe, expect, it } from 'vitest';
import hljs from 'highlight.js/lib/core';
import rust from 'highlight.js/lib/languages/rust';

describe('highlight.js core smoke', () => {
  it('registers one language and emits class-based tokens', () => {
    hljs.registerLanguage('rust', rust);
    const out = hljs.highlight('fn main() { let x = 1; }', { language: 'rust' }).value;
    expect(out).toContain('hljs-keyword');
    expect(out).not.toContain('style=');
  });
});
