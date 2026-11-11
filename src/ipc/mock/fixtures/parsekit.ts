// Imported (non-bare) Rust repo with a rich README: table, code, relative image, headings, remote badge.

import type { Rendered } from '../../types';
import { DAY, HOUR, NOW, PEOPLE } from './gen';
import type { FileSpec, RepoDef } from './model';

const LIB_RS = `//! parsekit: a small, fast streaming parser toolkit.
//!
//! See the README for an overview.

pub mod error;
pub mod lexer;
pub mod parser;
pub mod stream;

pub use error::{Error, Result};
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::Parser;

/// Parse a complete input into tokens.
///
/// \`\`\`
/// let tokens = parsekit::tokenize("let x = 42;").unwrap();
/// assert_eq!(tokens.len(), 5);
/// \`\`\`
pub fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut lexer = Lexer::new(input);
    let mut out = Vec::with_capacity(input.len() / 4);
    while let Some(tok) = lexer.next_token()? {
        out.push(tok);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_has_no_tokens() {
        assert!(tokenize("").unwrap().is_empty());
    }

    #[test]
    fn counts_simple_statement() {
        let toks = tokenize("let x = 42;").unwrap();
        assert_eq!(toks.len(), 5);
        assert_eq!(toks[3].kind, TokenKind::Number);
    }
}
`;

const LEXER_RS = `use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Ident,
    Number,
    Str,
    Punct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer { src: src.as_bytes(), pos: 0 }
    }

    pub fn next_token(&mut self) -> Result<Option<Token>> {
        self.skip_ws();
        let start = self.pos;
        let Some(&b) = self.src.get(self.pos) else { return Ok(None) };
        let kind = match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                self.eat_while(|c| c.is_ascii_alphanumeric() || c == b'_');
                TokenKind::Ident
            }
            b'0'..=b'9' => {
                self.eat_while(|c| c.is_ascii_digit());
                TokenKind::Number
            }
            b'"' => {
                self.pos += 1;
                self.eat_while(|c| c != b'"');
                if self.src.get(self.pos) != Some(&b'"') {
                    return Err(Error::Unterminated { at: start });
                }
                self.pos += 1;
                TokenKind::Str
            }
            _ => {
                self.pos += 1;
                TokenKind::Punct
            }
        };
        Ok(Some(Token { kind, start, end: self.pos }))
    }

    fn skip_ws(&mut self) {
        self.eat_while(|c| c.is_ascii_whitespace());
    }

    fn eat_while(&mut self, f: impl Fn(u8) -> bool) {
        while self.src.get(self.pos).is_some_and(|&c| f(c)) {
            self.pos += 1;
        }
    }
}
`;

const ERROR_RS = `use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unterminated { at: usize },
    Unexpected { at: usize, found: char },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unterminated { at } => write!(f, "unterminated string at byte {at}"),
            Error::Unexpected { at, found } => write!(f, "unexpected {found:?} at byte {at}"),
        }
    }
}

impl std::error::Error for Error {}
`;

const PARSER_RS = `use crate::lexer::{Lexer, Token};
use crate::Result;

/// Pull parser over a token stream.
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    peeked: Option<Token>,
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a str) -> Self {
        Parser { lexer: Lexer::new(src), peeked: None }
    }

    pub fn peek(&mut self) -> Result<Option<&Token>> {
        if self.peeked.is_none() {
            self.peeked = self.lexer.next_token()?;
        }
        Ok(self.peeked.as_ref())
    }
}
`;

const STREAM_MOD = `//! Incremental input for parsers that read from sockets or files.

mod buffer;

pub use buffer::RingBuffer;
`;

const STREAM_BUFFER = `/// Fixed-size ring buffer used by the streaming lexer.
pub struct RingBuffer {
    data: Box<[u8]>,
    head: usize,
    len: usize,
}

impl RingBuffer {
    pub fn with_capacity(cap: usize) -> Self {
        RingBuffer { data: vec![0; cap].into_boxed_slice(), head: 0, len: 0 }
    }

    pub fn push(&mut self, byte: u8) -> bool {
        if self.len == self.data.len() {
            return false;
        }
        let i = (self.head + self.len) % self.data.len();
        self.data[i] = byte;
        self.len += 1;
        true
    }
}
`;

const CARGO_TOML = `[package]
name = "parsekit"
version = "0.3.0"
edition = "2024"
description = "A small, fast streaming parser toolkit"
license = "MIT OR Apache-2.0"

[dependencies]

[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "throughput"
harness = false
`;

const README_MD = `# parsekit

A small, fast **streaming parser toolkit** for Rust.

![parsekit logo](docs/logo.svg)

## Features

| Feature | Status | Notes |
|---|:---:|---|
| Lexer | done | zero-copy tokens |
| Pull parser | done | one-token lookahead |
| Streaming input | beta | ring buffer, see [docs](docs/architecture.md) |
| Error recovery | planned | |

## Install

\`\`\`toml
[dependencies]
parsekit = "0.3"
\`\`\`

## Usage

\`\`\`rust
let tokens = parsekit::tokenize("let x = 42;")?;
assert_eq!(tokens.len(), 5);
\`\`\`

## License

MIT or Apache-2.0, at your option.
`;

const LOGO_SVG = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 480 120" width="480" height="120">
  <rect width="480" height="120" rx="16" fill="#123047"/>
  <g fill="#3cc3b0"><rect x="28" y="28" width="16" height="64" rx="4"/><rect x="52" y="44" width="16" height="48" rx="4"/><rect x="76" y="20" width="16" height="72" rx="4"/></g>
  <text x="116" y="74" font-family="Segoe UI, sans-serif" font-size="40" font-weight="700" fill="#e9f3f7">parsekit</text>
</svg>
`;

const ARCH_MD = `# Architecture

The lexer works on byte slices; the streaming front end feeds it from a ring buffer.

See [the README](../README.md#user-content-usage) for usage.
`;

/** A 1x1 PNG. */
const TINY_PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

const README_HTML = `<h1><a href="#user-content-parsekit" id="user-content-parsekit" rel="noopener noreferrer"></a>parsekit</h1>
<p>A small, fast <strong>streaming parser toolkit</strong> for Rust.
<a href="https://ci.example.com/parsekit" rel="noopener noreferrer"><img src="https://ci.example.com/parsekit/badge.svg" alt="build status"></a></p>
<p><img src="rsrepo:/docs/logo.svg" alt="parsekit logo"></p>
<h2><a href="#user-content-features" id="user-content-features" rel="noopener noreferrer"></a>Features</h2>
<table>
<thead>
<tr>
<th>Feature</th>
<th align="center">Status</th>
<th>Notes</th>
</tr>
</thead>
<tbody>
<tr>
<td>Lexer</td>
<td align="center">done</td>
<td>zero-copy tokens</td>
</tr>
<tr>
<td>Pull parser</td>
<td align="center">done</td>
<td>one-token lookahead</td>
</tr>
<tr>
<td>Streaming input</td>
<td align="center">beta</td>
<td>ring buffer, see <a href="rsrepo:/docs/architecture.md" rel="noopener noreferrer">docs</a></td>
</tr>
<tr>
<td>Error recovery</td>
<td align="center">planned</td>
<td></td>
</tr>
</tbody>
</table>
<h2><a href="#user-content-install" id="user-content-install" rel="noopener noreferrer"></a>Install</h2>
<pre><code class="language-toml">[dependencies]
parsekit = "0.3"
</code></pre>
<h2><a href="#user-content-usage" id="user-content-usage" rel="noopener noreferrer"></a>Usage</h2>
<pre><code class="language-rust">let tokens = parsekit::tokenize("let x = 42;")?;
assert_eq!(tokens.len(), 5);
</code></pre>
<p>Jump back to <a href="#user-content-features" rel="noopener noreferrer">features</a>, read the
<a href="https://docs.example.org/parsekit" rel="noopener noreferrer">API docs</a> or open
<a href="rsrepo:/src/lib.rs" rel="noopener noreferrer">src/lib.rs</a>.</p>
<ul>
<li><input type="checkbox" checked="" disabled=""> streaming lexer</li>
<li><input type="checkbox" disabled=""> error recovery</li>
</ul>
<h2><a href="#user-content-license" id="user-content-license" rel="noopener noreferrer"></a>License</h2>
<p>MIT or Apache-2.0, at your option.</p>
`;

const README: Rendered = {
  html: README_HTML,
  headings: [
    { level: 1, text: 'parsekit', anchor: 'user-content-parsekit' },
    { level: 2, text: 'Features', anchor: 'user-content-features' },
    { level: 2, text: 'Install', anchor: 'user-content-install' },
    { level: 2, text: 'Usage', anchor: 'user-content-usage' },
    { level: 2, text: 'License', anchor: 'user-content-license' },
  ],
  sourcePath: 'README.md',
};

const ARCH: Rendered = {
  html: `<h1><a href="#user-content-architecture" id="user-content-architecture" rel="noopener noreferrer"></a>Architecture</h1>
<p>The lexer works on byte slices; the streaming front end feeds it from a ring buffer.</p>
<p>See <a href="rsrepo:/README.md#user-content-usage" rel="noopener noreferrer">the README</a> for usage.</p>
`,
  headings: [{ level: 1, text: 'Architecture', anchor: 'user-content-architecture' }],
  sourcePath: 'docs/architecture.md',
};

const SUMMARIES = [
  'Streaming lexer: handle split UTF-8 sequences',
  'Add ring buffer benchmarks',
  'Merge pull request #41 from fix/utf8',
  'Document the pull parser',
  'Release v0.3.0',
  'Parser: one-token lookahead',
  'Lexer: report unterminated strings with position',
  'Add roundtrip tests',
  'CI: run clippy with -D warnings',
  'Release v0.2.0',
];

export function parsekit(huge: boolean): RepoDef {
  const files: Record<string, FileSpec> = {
    '.gitignore': '/target\n**/*.rs.bk\n',
    'Cargo.toml': CARGO_TOML,
    'Cargo.lock': '# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = "parsekit"\nversion = "0.3.0"\n',
    LICENSE: 'MIT License\n\nCopyright (c) the parsekit authors\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software...\n',
    'README.md': README_MD,
    'rustfmt.toml': 'max_width = 120\n',
    'src/lib.rs': LIB_RS,
    'src/lexer.rs': LEXER_RS,
    'src/parser.rs': PARSER_RS,
    'src/error.rs': ERROR_RS,
    'src/stream/mod.rs': STREAM_MOD,
    'src/stream/buffer.rs': STREAM_BUFFER,
    'benches/throughput.rs':
      'use criterion::{criterion_group, criterion_main, Criterion};\n\nfn bench(c: &mut Criterion) {\n    let input = "let x = 42;".repeat(10_000);\n    c.bench_function("tokenize 110 KB", |b| b.iter(|| parsekit::tokenize(&input)));\n}\n\ncriterion_group!(benches, bench);\ncriterion_main!(benches);\n',
    'examples/json_tokens.rs':
      'fn main() {\n    let src = r#"{"name": "parsekit", "stars": 128}"#;\n    for t in parsekit::tokenize(src).unwrap() {\n        println!("{:?} {}", t.kind, &src[t.start..t.end]);\n    }\n}\n',
    'tests/roundtrip.rs':
      'use parsekit::tokenize;\n\n#[test]\nfn roundtrip_spans_cover_input() {\n    let src = "a = b + 1";\n    let toks = tokenize(src).unwrap();\n    assert_eq!(toks.first().unwrap().start, 0);\n}\n',
    'docs/logo.svg': { svg: LOGO_SVG },
    'docs/architecture.md': ARCH_MD,
    'docs/screenshot.png': { pngBase64: TINY_PNG },
    'scripts/release.sh': { exec: '#!/usr/bin/env bash\nset -euo pipefail\ncargo test\ncargo publish --dry-run\n' },
    'assets/sample.bin': { binary: 18_432 },
    'data/corpus.json': { lfs: 52_428_800 },
    'vendor/ucd': { submodule: 'https://git.example.org/unicode/ucd-tables.git' },
    latest: { symlink: 'docs/architecture.md' },
    'src/generated_tables.rs': { generatedText: { lines: 1800, seed: 'tables' } },
    // M6.8: every file-view placeholder / notice kind
    'data/readings.csv': { bigText: { lines: 26_000, seed: 'readings', bytes: 1_310_720 } },
    'data/export.log': { hugeText: 6_710_886 },
    'docs/legacy-notes.txt': { lossy: 'Caf� notes (Latin-1)\nR�sum�: the old parser read ISO-8859-1 input.\n' },
    'odd/a:b.txt': 'A colon is legal in git but not in a Windows file name.\n',
    'odd/CON.txt': 'CON is a reserved device name on Windows.\n',
    'odd/caf�.txt': 'The name of this file is Latin-1 encoded.\n',
  };
  if (huge) {
    for (let i = 0; i < 1000; i++) {
      const n = String(i).padStart(4, '0');
      files[`generated/case_${n}.rs`] = `// generated case ${n}\npub const CASE_${n}: u32 = ${i};\n`;
    }
  }
  return {
    summary: {
      id: 1,
      owner: 'northwind',
      name: 'parsekit',
      source: 'imported',
      path: 'C:\\Code\\parsekit',
      description: 'A small, fast streaming parser toolkit for Rust: zero-copy lexer, pull parser and ring-buffer input.',
      topics: ['rust', 'parser', 'lexer', 'streaming'],
      homepage: 'https://docs.example.org/parsekit',
      originUrl: 'https://github.com/northwind/parsekit.git',
      originHost: 'github.com',
      defaultBranch: 'main',
      primaryLanguage: 'Rust',
      lastCommitAt: NOW - 3 * HOUR,
      addedAt: NOW - 40 * DAY,
      clonedAt: null,
      lastOpenedAt: NOW - 2 * HOUR,
      lastFetchedAt: NOW - 5 * DAY,
      status: { type: 'ok' },
    },
    isBare: false,
    head: { type: 'branch', name: 'main' },
    license: 'MIT',
    fetchable: true,
    canFastForward: null,
    // M7.6: shown only with Settings → "Note uncommitted changes" on (D-16)
    dirty: true,
    files,
    history: { seed: 'parsekit', count: huge ? 10_000 : 148, newest: NOW - 3 * HOUR, summaries: SUMMARIES },
    branches: [
      { name: 'main', at: 0 },
      { name: 'feature/streaming', at: 6, ahead: ['Streaming: refill from Read', 'Streaming: back-pressure', 'WIP: async adapter'] },
      { name: 'fix/utf8', at: 2 },
      // `?huge`: 6,000 behind → the mock's ahead/behind cap (1,000) shows the capped marker
      { name: 'docs/tutorial', at: huge ? 6_000 : 30, ahead: ['Tutorial: first draft'] },
    ],
    remoteBranches: [
      { name: 'origin/main', at: 0 },
      { name: 'origin/feature/streaming', at: 6, ahead: ['Streaming: refill from Read'] },
    ],
    tags: [
      { name: 'v0.3.0', at: 4, annotated: true, tagger: PEOPLE[0], message: 'parsekit 0.3.0\n\nStreaming input (beta).' },
      { name: 'v0.2.0', at: 9, annotated: true, tagger: PEOPLE[1], message: 'parsekit 0.2.0' },
      { name: 'v0.1.0', at: 60 },
      { name: 'experiment-1', at: 80 },
    ],
    readmes: { '': README },
    markdown: { 'README.md': README, 'docs/architecture.md': ARCH },
    languages: [
      { name: 'Rust', bytes: 48_210, percent: 86.4 },
      { name: 'TOML', bytes: 3_120, percent: 5.6 },
      { name: 'Markdown', bytes: 2_890, percent: 5.2 },
      { name: 'Shell', bytes: 1_560, percent: 2.8 },
    ],
    hugeCommitAt: huge ? 0 : undefined,
    // M7.8: a merge on page 2 of the commits list, and one commit with a rename, a deletion and a no-newline change
    merges: [{ at: 40, branch: 'feature/lexer-tables', commits: ['Lexer: table-driven keyword lookup', 'Lexer: tests for the keyword table'] }],
    special: { 3: { rename: { from: 'docs/design.md', to: 'docs/architecture.md' }, deleted: 'docs/old-notes.md', noNewline: 'rustfmt.toml' } },
    unstorable: ['odd/a:b.txt', 'odd/CON.txt'],
    rawNames: { 'odd/caf�.txt': [...new TextEncoder().encode('odd/caf'), 0xe9, ...new TextEncoder().encode('.txt')] },
  };
}
