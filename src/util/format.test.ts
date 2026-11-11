import { describe, expect, it } from 'vitest';
import { dayGroups, dayKey, formatBytes, formatCount, formatDay, initials, plural, relativeTime, shortOid } from './format';

describe('formatBytes', () => {
  it.each([
    [0, '0 Bytes'],
    [1, '1 Byte'],
    [1023, '1023 Bytes'],
    [1024, '1 KB'],
    [12_595, '12.3 KB'],
    [5 * 1024 * 1024, '5 MB'],
    [148_326_400, '141 MB'],
    [3 * 1024 ** 3, '3 GB'],
  ])('%d → %s', (n, s) => {
    expect(formatBytes(n)).toBe(s);
  });
});

describe('counts', () => {
  it('thousands separators and plural', () => {
    expect(formatCount(10_000)).toBe('10,000');
    expect(formatCount(999)).toBe('999');
    expect(plural(1, 'commit')).toBe('1 commit');
    expect(plural(1234, 'commit')).toBe('1,234 commits');
    expect(plural(2, 'branch', 'branches')).toBe('2 branches');
  });
});

describe('relativeTime', () => {
  const now = new Date(2026, 10, 11, 15, 0, 0).getTime() / 1000;
  it.each([
    [10, 'just now'],
    [60, '1 minute ago'],
    [5 * 60, '5 minutes ago'],
    [60 * 60, '1 hour ago'],
    [3 * 3600, '3 hours ago'],
    [26 * 3600, 'yesterday'],
    [4 * 86400, '4 days ago'],
  ])('%d s ago → %s', (ago, s) => {
    expect(relativeTime(now - ago, now)).toBe(s);
  });
  it('older dates', () => {
    expect(relativeTime(new Date(2026, 2, 3).getTime() / 1000, now)).toBe('on Mar 3');
    expect(relativeTime(new Date(2024, 0, 9).getTime() / 1000, now)).toBe('on Jan 9, 2024');
  });
});

describe('day groups (local dates)', () => {
  it('groups by local day and splits at local midnight', () => {
    const t = (d: number, h: number, m = 0) => new Date(2026, 10, d, h, m).getTime() / 1000;
    const items = [t(11, 0, 30), t(10, 23, 59), t(10, 8), t(9, 12)];
    const g = dayGroups(items, (x) => x);
    expect(g.map((x) => [x.label, x.items.length])).toEqual([
      ['Nov 11, 2026', 1],
      ['Nov 10, 2026', 2],
      ['Nov 9, 2026', 1],
    ]);
    expect(dayKey(t(10, 23, 59))).toBe('2026-11-10');
    expect(formatDay(t(1, 1))).toBe('Nov 1, 2026');
  });
});

describe('misc', () => {
  it('shortOid and initials', () => {
    expect(shortOid('0123456789abcdef')).toBe('0123456');
    expect(initials('Avery Lindqvist')).toBe('AL');
    expect(initials('jun')).toBe('J');
    expect(initials('  ')).toBe('?');
    expect(initials('mary-jane watson')).toBe('MW');
  });
});
