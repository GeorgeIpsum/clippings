// The todo-tree settings import (spec 7.3): the activation offer and the
// Import Settings from Todo Tree command.

import * as vscode from 'vscode';
import type { SettingWriter } from '../config/writes';
import type { Prompts } from '../ui/prompts';
import { applyWrites, importPlan, overwriteCount, shouldOffer, type Inspected } from './plan';

export const IMPORT_OFFER_KEY = 'importOffer';

function inspect(section: string, key: string): Inspected | undefined {
  return vscode.workspace.getConfiguration(section).inspect(key);
}

export class TodoTreeImporter {
  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly prompts: Prompts,
    private readonly log: vscode.LogOutputChannel,
    private readonly writer: SettingWriter,
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

  /**
   * Copies each carried todo-tree value to the same scope under `clippings.*`.
   * On demand, this can overwrite explicit Clippings values; when it would,
   * it asks first (the activation offer never reaches this with a conflict,
   * since it only fires when no Clippings key is set).
   */
  async run(): Promise<number> {
    const plan = importPlan((key) => inspect('todo-tree', key));
    const conflicts = overwriteCount(plan.writes, (key) => inspect('clippings', key));
    if (conflicts > 0) {
      const noun = conflicts === 1 ? 'setting' : 'settings';
      const choice = await this.prompts.message(
        'warning',
        `Overwrite ${conflicts} Clippings ${noun} with your Todo Tree settings?`,
        'Overwrite',
        'Cancel',
      );
      if (choice !== 'Overwrite') return 0;
    }
    for (const line of plan.skipped) this.log.info(`Import: ${line}`);
    const written = await applyWrites(plan.writes, (key, value, scope) => this.writer.write(key, value, scope));
    const noun = written === 1 ? 'setting' : 'settings';
    void this.prompts.message('info', `Clippings: imported ${written} ${noun} from Todo Tree.`);
    return written;
  }
}
