// Finds the server binary for this window (spec 7.4).

import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import * as vscode from 'vscode';
import { ServerPathApprovals, type Decision } from './approvals';
import { candidates, serverPathSetting } from './candidates';
import { ensureExecutable, probe } from './probe';
import { resolveServer, type Resolved } from './resolve';

async function ask(path: string): Promise<Decision | undefined> {
  const choice = await vscode.window.showWarningMessage(
    `This workspace sets clippings.server.path to ${path}. Allow Clippings to run it?`,
    { modal: true, detail: 'Clippings remembers your choice for this path.' },
    'Allow',
    'Deny',
  );
  return choice === 'Allow' ? 'allow' : choice === 'Deny' ? 'deny' : undefined;
}

export function locateServer(context: vscode.ExtensionContext, log: (message: string) => void): Promise<Resolved> {
  const list = candidates({
    env: process.env,
    setting: serverPathSetting(
      vscode.workspace.getConfiguration('clippings').inspect<string>('server.path'),
      vscode.workspace.isTrusted,
    ),
    home: homedir(),
    workspaceFolder: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
    extensionPath: context.extensionPath,
    platform: process.platform,
    development: context.extensionMode !== vscode.ExtensionMode.Production,
    exists: existsSync,
  });
  const approvals = new ServerPathApprovals(context.globalState, ask);
  return resolveServer(list, {
    probe: (path) => {
      if (list.some((c) => c.source === 'bundled' && c.path === path)) ensureExecutable(path);
      return probe(path);
    },
    approve: (path) => approvals.approve(path),
    log,
  });
}
