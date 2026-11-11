import { afterEach, describe, expect, it } from 'vitest';
import { MARK_DASHBOARD_ROWS, markOnce, resetMarks } from './perfMark';

describe('markOnce', () => {
  afterEach(() => {
    resetMarks();
    performance.clearMarks();
  });

  it('records a mark only the first time', () => {
    markOnce(MARK_DASHBOARD_ROWS);
    markOnce(MARK_DASHBOARD_ROWS);
    expect(performance.getEntriesByName(MARK_DASHBOARD_ROWS, 'mark')).toHaveLength(1);
  });

  it('keeps names apart', () => {
    markOnce('a');
    markOnce('b');
    expect(performance.getEntriesByType('mark').map((m) => m.name).sort()).toEqual(['a', 'b']);
  });
});
