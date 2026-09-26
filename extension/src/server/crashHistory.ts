// The crash count behind automatic restarts (spec 7.4). Pure. A server that
// exits while starting counts as a crash, so one that always fails at startup
// stops after five attempts instead of looping.

export const MAX_CRASHES = 5;
export const CRASH_WINDOW_MS = 3 * 60 * 1000;

export const GIVE_UP_MESSAGE =
  'The Clippings server crashed 5 times in the last 3 minutes. The server will not be restarted. ' +
  'See the output for more information.';

export class CrashHistory {
  private times: number[] = [];

  /** Records a crash at `now`: `restart` while under the limit, else `give up`. */
  record(now: number): 'restart' | 'give up' {
    this.times = [...this.times.filter((t) => now - t <= CRASH_WINDOW_MS), now];
    if (this.times.length < MAX_CRASHES) return 'restart';
    this.times = [];
    return 'give up';
  }

  /** Forgets every crash, as when the user restarts the server. */
  clear(): void {
    this.times = [];
  }
}
