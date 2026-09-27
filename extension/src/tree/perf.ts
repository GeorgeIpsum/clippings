// Extension host time spent applying tree updates (spec 13): the provider's
// own synchronous work per refresh, children response and tree item.

export class TreePerf {
  /** Milliseconds spent in the provider's synchronous code since the last reset. */
  busyMs = 0;
  /** Tree items built since the last reset. */
  items = 0;
  /** When the last `clippings/treeChanged` arrived, as `performance.now()`. */
  lastChangeAt = 0;
  /** When the last tree item was built. */
  lastItemAt = 0;
  private waiters: { count: number; resolve: () => void }[] = [];

  reset(): void {
    this.busyMs = 0;
    this.items = 0;
  }

  /** Runs `work`, adding its duration to `busyMs`. */
  time<T>(work: () => T): T {
    const start = performance.now();
    try {
      return work();
    } finally {
      this.busyMs += performance.now() - start;
    }
  }

  itemBuilt(): void {
    this.items++;
    this.lastItemAt = performance.now();
    if (this.waiters.length === 0) return;
    const ready = this.waiters.filter((w) => this.items >= w.count);
    this.waiters = this.waiters.filter((w) => this.items < w.count);
    for (const w of ready) w.resolve();
  }

  /** Resolves once `count` items have been built since the last reset. */
  whenItems(count: number): Promise<void> {
    if (this.items >= count) return Promise.resolve();
    return new Promise((resolve) => this.waiters.push({ count, resolve }));
  }
}
