// Deterministic generators for mock data (same output on every load; no Math.random).

import type { CommitSummary, Signature } from '../../types';

/** 32-bit FNV-1a. */
export function hash32(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h >>> 0;
}

/** Fake 40-hex oid from any seed parts. */
export function fakeOid(...parts: (string | number)[]): string {
  const base = parts.join('\u0000');
  let out = '';
  for (let i = 0; out.length < 40; i++) out += hash32(base + '#' + i).toString(16).padStart(8, '0');
  return out.slice(0, 40);
}

/** Mulberry32 PRNG. */
export function prng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export interface Person {
  name: string;
  email: string;
  offsetMin: number;
}

/** Invented people (no real names). */
export const PEOPLE: Person[] = [
  { name: 'Avery Lindqvist', email: 'avery@example.org', offsetMin: 60 },
  { name: 'Rowan Okafor', email: 'rowan.okafor@example.com', offsetMin: 0 },
  { name: 'Mika Hollander', email: 'mika@example.net', offsetMin: 120 },
  { name: 'Sam Teixeira', email: 'sam.t@example.org', offsetMin: -180 },
  { name: 'Jun Park', email: 'jun@example.dev', offsetMin: 540 },
  { name: 'Noor Haddad', email: 'noor@example.com', offsetMin: 180 },
  { name: 'Priya Raman', email: 'priya.r@example.org', offsetMin: 330 },
  { name: 'Elliot Brandt', email: 'elliot@example.net', offsetMin: -300 },
];

/** "Now" for all fixtures: fixed relative to the page load so relative dates look natural. */
export const NOW = Math.floor(Date.now() / 1000);
export const HOUR = 3600;
export const DAY = 24 * HOUR;

export function sig(p: Person, secs: number): Signature {
  return { name: p.name, email: p.email, time: { secs, offsetMin: p.offsetMin } };
}

const GENERIC_SUMMARIES = [
  'Fix off-by-one in range handling',
  'Refactor error types',
  'Update dependencies',
  'Add tests for edge cases',
  'Improve docs for the public API',
  'Tidy up imports',
  'Speed up the hot path',
  'Handle empty input gracefully',
  'Rename internal helpers',
  'Bump version',
  'Clarify README wording',
  'Remove dead code',
  'Make CI faster',
  'Add benchmark',
  'Merge branch for review fixes',
];

export interface CommitGen {
  seed: string;
  count: number;
  /** Newest commit time (epoch seconds). */
  newest: number;
  summaries?: string[];
  people?: Person[];
}

export interface GenCommit {
  summary: CommitSummary;
  message: string;
}

/** Linear history, newest first; parent of i is i+1. */
export function genHistory(g: CommitGen): GenCommit[] {
  const rnd = prng(hash32(g.seed));
  const people = g.people ?? PEOPLE;
  const summaries = g.summaries ?? GENERIC_SUMMARIES;
  const oids: string[] = [];
  for (let i = 0; i < g.count; i++) oids.push(fakeOid(g.seed, 'c', i));
  const out: GenCommit[] = [];
  let t = g.newest;
  for (let i = 0; i < g.count; i++) {
    const p = people[Math.floor(rnd() * people.length)];
    const s = i < summaries.length ? summaries[i] : GENERIC_SUMMARIES[Math.floor(rnd() * GENERIC_SUMMARIES.length)];
    const body = rnd() < 0.35 ? `\n\nThe previous behaviour surprised users when the input was empty.\nThis keeps the old API and adds a test.` : '';
    out.push({
      summary: {
        oid: oids[i],
        summary: s,
        author: sig(p, t),
        committer: sig(p, t + Math.floor(rnd() * 600)),
        parents: i + 1 < g.count ? [oids[i + 1]] : [],
      },
      message: s + body,
    });
    t -= Math.floor(HOUR * (1 + rnd() * 40));
  }
  return out;
}
