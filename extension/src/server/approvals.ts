// Allow or Deny for a `server.path` set in workspace settings (spec 7.4),
// remembered per resolved path in global storage (spec 7.8).

import type { Memento } from '../state/viewState';

export type Decision = 'allow' | 'deny';
export const DECISIONS_KEY = 'serverPathDecisions';

export class ServerPathApprovals {
  constructor(
    private readonly globalState: Memento,
    /** Asks the user; `undefined` when they dismiss the prompt. */
    private readonly ask: (path: string) => PromiseLike<Decision | undefined>,
  ) {}

  private decisions(): Record<string, Decision> {
    const value = this.globalState.get<unknown>(DECISIONS_KEY);
    return value && typeof value === 'object' ? { ...(value as Record<string, Decision>) } : {};
  }

  async approve(path: string): Promise<boolean> {
    const known = this.decisions()[path];
    if (known) return known === 'allow';
    const decision = await this.ask(path);
    // A dismissed prompt denies this time and asks again next time.
    if (decision) await this.globalState.update(DECISIONS_KEY, { ...this.decisions(), [path]: decision });
    return decision === 'allow';
  }
}
