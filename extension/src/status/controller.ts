// The status bar item, the view's title, badge and message, and the
// configuration notices, all driven by `clippings/status` (spec 7.5, 7.7).

import * as vscode from 'vscode';
import { NEEDS_SCAN_MESSAGE } from '../commands/scan';
import type { StatusParams } from '../protocol';
import type { Prompts } from '../ui/prompts';
import { OnceNotice, statusBarView, type StatusBarView } from './presentation';

export class StatusController implements vscode.Disposable {
  private readonly item = vscode.window.createStatusBarItem('clippings.status', vscode.StatusBarAlignment.Left, 0);
  private readonly warnings = new OnceNotice();
  private readonly errors = new OnceNotice();
  /** What the item shows, for the tests: VS Code cannot report it. */
  shown: StatusBarView | undefined;

  constructor(
    private readonly view: vscode.TreeView<string>,
    private readonly prompts: Prompts,
  ) {
    this.item.name = 'Clippings';
  }

  update(status: StatusParams): void {
    const bar = statusBarView(status);
    this.item.text = bar.text;
    this.item.tooltip = bar.tooltip;
    this.item.command = bar.command;
    if (bar.visible) this.item.show();
    else this.item.hide();
    this.shown = bar;

    this.view.title = status.viewTitle;
    this.view.badge = status.badge.value > 0 ? { value: status.badge.value, tooltip: status.badge.tooltip } : undefined;
    this.view.message = status.needsScan && !status.scanning ? NEEDS_SCAN_MESSAGE : undefined;

    const warning = this.warnings.next(status.warnings);
    if (warning) void this.prompts.message('warning', warning);
    const error = this.errors.next(status.error ? [status.error] : []);
    if (error) void this.showError(error);
  }

  private async showError(message: string): Promise<void> {
    const choice = await this.prompts.message('warning', message, 'Open Settings');
    if (choice === 'Open Settings') await vscode.commands.executeCommand('workbench.action.openSettings', 'clippings');
  }

  dispose(): void {
    this.item.dispose();
  }
}
