// Setting writes that report failure as a warning instead of throwing
// (spec 10.2), at the targets in targets.ts.

import * as vscode from 'vscode';
import type { Prompts } from '../ui/prompts';
import { folderTarget, valueTarget, type Target } from './targets';

export class SettingWriter {
  constructor(private readonly prompts: Prompts) {}

  /** Target for a folder-scoped command. */
  folderTarget(): Target {
    return folderTarget((vscode.workspace.workspaceFolders?.length ?? 0) > 0);
  }

  /** Target for `clippings.<key>` by where its value lives. */
  valueTarget(key: string): Target {
    return valueTarget(vscode.workspace.getConfiguration('clippings').inspect(key));
  }

  get<T>(key: string): T | undefined {
    return vscode.workspace.getConfiguration('clippings').get<T>(key);
  }

  /** Writes `clippings.<key>`; a failure is shown as a warning. Returns success. */
  async write(key: string, value: unknown, target: Target): Promise<boolean> {
    const t = target === 'workspace' ? vscode.ConfigurationTarget.Workspace : vscode.ConfigurationTarget.Global;
    try {
      await vscode.workspace.getConfiguration('clippings').update(key, value, t);
      return true;
    } catch (err) {
      void this.prompts.message('warning', `Clippings: could not update clippings.${key}: ${String(err)}`);
      return false;
    }
  }
}
