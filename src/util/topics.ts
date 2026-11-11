// Client-side mirror of rs-core `validate::topics` / `description` limits (FR-20). The backend validates again and
// its message wins; this only gives instant feedback in the Edit About dialog.

export const MAX_TOPICS = 20;
export const MAX_TOPIC_LEN = 50;
export const MAX_DESCRIPTION = 350;

export type TopicCheck = { ok: true; topic: string } | { ok: false; message: string };

/** Same rule and wording as rs-core: trimmed, lower-cased, `[a-z0-9-]`, starts with a letter or digit, ≤ 50. */
export function checkTopic(raw: string): TopicCheck {
  const t = raw.trim().toLowerCase();
  const ok = t.length > 0 && t.length <= MAX_TOPIC_LEN && /^[a-z0-9]/.test(t) && /^[a-z0-9-]+$/.test(t);
  if (ok) return { ok: true, topic: t };
  return {
    ok: false,
    message: `Topic ${JSON.stringify(raw.slice(0, 60))}: use lowercase letters, digits and '-', start with a letter or digit, at most 50 characters.`,
  };
}

/** Split typed text on commas / whitespace into candidate topics. */
export function splitTopics(text: string): string[] {
  return text
    .split(/[\s,]+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/** Add candidates to a list (dedup, limit). Returns the new list or the first error. */
export function addTopics(list: string[], text: string): { list: string[]; error: string | null } {
  const out = [...list];
  for (const raw of splitTopics(text)) {
    const c = checkTopic(raw);
    if (!c.ok) return { list: out, error: c.message };
    if (out.includes(c.topic)) continue;
    if (out.length >= MAX_TOPICS) return { list: out, error: `${out.length + 1} topics; the limit is ${MAX_TOPICS}.` };
    out.push(c.topic);
  }
  return { list: out, error: null };
}
