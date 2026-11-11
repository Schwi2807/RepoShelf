import type { Switches } from '../switches';
import { lumen } from './lumen';
import type { RepoDef } from './model';
import { dotgo, emptyRepo, freshClone, hostile, missingRepo, quill } from './others';
import { parsekit } from './parsekit';

export { genericRepo } from './others';
export type { RepoDef } from './model';

/** The mock shelf: one repo per kind (imported Rust + README, cloned TS + releases, empty, missing, security) plus a
 * few more for a realistic dashboard. `?empty` → none. */
export function buildFixtures(sw: Switches): RepoDef[] {
  if (sw.empty) return [];
  return [parsekit(sw.huge), lumen(), quill(), dotgo(), emptyRepo(), missingRepo(), hostile(), freshClone()];
}
