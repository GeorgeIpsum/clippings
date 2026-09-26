import * as assert from 'node:assert/strict';
import { CRASH_WINDOW_MS, CrashHistory } from '../../server/crashHistory';

describe('crash history', () => {
  it('restarts four times and gives up on the fifth crash within three minutes', () => {
    const history = new CrashHistory();
    const results = [0, 1000, 2000, 3000, 4000].map((t) => history.record(t));
    assert.deepEqual(results, ['restart', 'restart', 'restart', 'restart', 'give up']);
    assert.equal(history.record(5000), 'restart', 'giving up starts a new count');
  });

  it('forgets crashes older than the window', () => {
    const history = new CrashHistory();
    for (const t of [0, 1, 2, 3]) history.record(t);
    assert.equal(history.record(CRASH_WINDOW_MS + 10), 'restart');
  });

  it('starts over when cleared', () => {
    const history = new CrashHistory();
    for (const t of [0, 1, 2, 3]) history.record(t);
    history.clear();
    assert.equal(history.record(4), 'restart');
  });
});
