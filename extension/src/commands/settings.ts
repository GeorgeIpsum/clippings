// Commands that change settings: scan modes, tags and the tree toggles
// (spec 7.6).

import * as vscode from 'vscode';
import type { SettingWriter } from '../config/writes';
import type { ScanMode } from '../protocol';
import type { Prompts } from '../ui/prompts';

export function registerSettingCommands(writer: SettingWriter, prompts: Prompts): vscode.Disposable[] {
  const scanMode = (mode: ScanMode) => () => writer.write('tree.scanMode', mode, writer.folderTarget());
  const toggle = (key: string) => () => writer.write(key, !writer.get<boolean>(key), writer.folderTarget());
  const tags = () => writer.get<string[]>('general.tags') ?? [];

  const commands: Record<string, (...args: unknown[]) => unknown> = {
    'clippings.scanOpenFilesOnly': scanMode('open files'),
    'clippings.scanCurrentFileOnly': scanMode('current file'),
    'clippings.scanWorkspaceAndOpenFiles': scanMode('workspace'),
    'clippings.scanWorkspaceOnly': scanMode('workspace only'),
    'clippings.toggleItemCounts': toggle('tree.showCountsInTree'),
    'clippings.toggleBadges': toggle('tree.showBadges'),
    'clippings.toggleCompactFolders': toggle('tree.disableCompactFolders'),
    'clippings.addTag': async () => {
      const tag = await prompts.input({ prompt: 'New tag', placeHolder: 'e.g. FIXME' });
      if (!tag || tags().includes(tag)) return;
      await writer.write('general.tags', [...tags(), tag], writer.valueTarget('general.tags'));
    },
    'clippings.removeTag': async () => {
      const remove = await prompts.pick(tags(), {
        canPickMany: true,
        matchOnDescription: true,
        matchOnDetail: true,
        placeHolder: 'Select tags to remove',
      });
      if (!remove || remove.length === 0) return;
      await writer.write(
        'general.tags',
        tags().filter((t) => !remove.includes(t)),
        writer.valueTarget('general.tags'),
      );
    },
  };
  return Object.entries(commands).map(([id, run]) => vscode.commands.registerCommand(id, run));
}
