// The todo-tree settings import (spec 7.3): the activation offer and the
// Import Settings from Todo Tree command.

import * as vscode from 'vscode';
import type { Prompts } from '../ui/prompts';
import { importPlan, shouldOffer, type Inspected } from './plan';

export const IMPORT_OFFER_KEY = 'importOffer';

function inspect(section: string, key: string): Inspected | undefined {
  return vscode.workspace.getConfiguration(section).inspect(key);
}

export class TodoTreeImporter {
  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly prompts: Prompts,
    private readonly log: vscode.LogOutputChannel,
  ) {}

  /** On activation: offers the import once the conditions of spec 7.3 hold. */
  async offer(): Promise<void> {
    if (!shouldOffer(inspect, this.context.globalState.get(IMPORT_OFFER_KEY))) return;
    const choice = await this.prompts.message(
      'info',
      'Clippings found Todo Tree settings. Import them into Clippings?',
      'Import',
      'Not Now',
      'Never',
    );
    if (choice === 'Import') await this.run();
    if (choice === 'Never') await this.context.globalState.update(IMPORT_OFFER_KEY, 'never');
  }

  /** Copies each carried todo-tree value to the same scope under `clippings.*`. */
  async run(): Promise<number> {
    const plan = importPlan((key) => inspect('todo-tree', key));
    for (const line of plan.skipped) this.log.info(`Import: ${line}`);
    let written = 0;
    for (const w of plan.writes) {
      const target = w.scope === 'global' ? vscode.ConfigurationTarget.Global : vscode.ConfigurationTarget.Workspace;
      try {
        await vscode.workspace.getConfiguration('clippings').update(w.key, w.value, target);
        written++;
      } catch (err) {
        this.log.warn(`Import: could not write clippings.${w.key}: ${String(err)}`);
      }
    }
    const noun = written === 1 ? 'setting' : 'settings';
    void this.prompts.message('info', `Clippings: imported ${written} ${noun} from Todo Tree.`);
    return written;
  }
}
