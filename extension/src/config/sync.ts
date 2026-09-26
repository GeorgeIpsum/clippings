// Keeps the server's configuration in step with settings and view state:
// each push reads the configuration afresh and sends it.

import type { Settings } from '../protocol';
import { effectiveView } from './configuration';

export interface Push {
  before: Settings;
  after: Settings;
}

/**
 * Whether the server rebuilds the whole tree for this change and reports it
 * as a root refresh: a view mode, grouping or scan mode change (spec 5.12).
 */
export function replacesTree({ before, after }: Push): boolean {
  const a = effectiveView(before.tree, before.viewState);
  const b = effectiveView(after.tree, after.viewState);
  return (
    (Object.keys(a) as (keyof typeof a)[]).some((k) => a[k] !== b[k]) || before.tree.scanMode !== after.tree.scanMode
  );
}

export class ConfigurationSync {
  private last: Settings;
  private readonly listeners: ((push: Push) => void)[] = [];

  constructor(
    private readonly read: () => Settings,
    private readonly send: (settings: Settings) => void,
  ) {
    this.last = read();
  }

  /** The configuration most recently read. */
  get current(): Settings {
    return this.last;
  }

  /** Reads and sends the configuration, and tells listeners what changed. */
  push(): Push {
    const push = { before: this.last, after: this.read() };
    this.last = push.after;
    this.send(push.after);
    for (const l of this.listeners) l(push);
    return push;
  }

  onPush(listener: (push: Push) => void): void {
    this.listeners.push(listener);
  }
}
