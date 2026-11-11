import { describe, expect, it } from 'vitest';
import { addTopics, checkTopic, MAX_TOPICS, splitTopics } from './topics';

describe('topics (mirror of validate::topics)', () => {
  it('normalises and accepts valid topics', () => {
    expect(checkTopic('  Rust ')).toEqual({ ok: true, topic: 'rust' });
    expect(checkTopic('web-ui')).toEqual({ ok: true, topic: 'web-ui' });
    expect(checkTopic('3d')).toEqual({ ok: true, topic: '3d' });
  });

  it('rejects bad topics with the backend wording', () => {
    for (const bad of ['-lead', 'under_score', 'dot.ted', 'ü', 'x'.repeat(51)]) {
      const c = checkTopic(bad);
      expect(c.ok, bad).toBe(false);
      if (!c.ok) expect(c.message).toContain("use lowercase letters, digits and '-'");
    }
  });

  it('splits on commas and spaces, dedups, enforces the limit', () => {
    expect(splitTopics('a, b  c,,d')).toEqual(['a', 'b', 'c', 'd']);
    expect(addTopics(['rust'], 'Rust, cli')).toEqual({ list: ['rust', 'cli'], error: null });
    const full = Array.from({ length: MAX_TOPICS }, (_, i) => `t${i}`);
    const r = addTopics(full, 'one-more');
    expect(r.list).toHaveLength(MAX_TOPICS);
    expect(r.error).toBe('21 topics; the limit is 20.');
    expect(addTopics([], 'ok bad_one').error).toContain('"bad_one"');
  });
});
