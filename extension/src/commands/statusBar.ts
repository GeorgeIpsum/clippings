// The status bar click (spec 7.7): reveal the view, toggle highlights, or
// cycle the status bar mode.

import * as vscode from 'vscode';
import { statusBarClickBehaviour } from '../config/clientSettings';
import type { ConfigurationSync } from '../config/sync';
import type { SettingWriter } from '../config/writes';
import { nextStatusBarMode } from '../status/presentation';
import type { Prompts } from '../ui/prompts';

export interface StatusBarDeps {
  sync: ConfigurationSync;
  writer: SettingWriter;
  prompts: Prompts;
  view: vscode.TreeView<string>;
}

export function registerStatusBarCommand({ sync, writer, prompts, view }: StatusBarDeps): vscode.Disposable {
  return vscode.commands.registerCommand('clippings.onStatusBarClicked', async () => {
    const settings = sync.current;
    switch (statusBarClickBehaviour(settings)) {
      case 'reveal':
        if (!view.visible) await vscode.commands.executeCommand('clippings-view.focus');
        return;
      case 'toggle highlights':
        await writer.write('highlights.enabled', !settings.highlights.enabled, writer.valueTarget('highlights.enabled'));
        return;
      case 'cycle': {
        const { mode, message } = nextStatusBarMode(settings.general.statusBar);
        if (await writer.write('general.statusBar', mode, writer.valueTarget('general.statusBar'))) {
          void prompts.message('info', message);
        }
        return;
      }
    }
  });
}
