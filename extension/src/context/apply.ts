// Sets the context keys that changed.

import * as vscode from 'vscode';
import { changedValues, type ContextValues } from './keys';

export class ContextKeys {
  private applied: ContextValues = {};

  /** The values last set, for the tests. */
  get values(): Readonly<ContextValues> {
    return this.applied;
  }

  async apply(next: ContextValues): Promise<void> {
    const changed = changedValues(this.applied, next);
    this.applied = { ...next };
    await Promise.all(Object.entries(changed).map(([k, v]) => vscode.commands.executeCommand('setContext', k, v)));
  }
}
