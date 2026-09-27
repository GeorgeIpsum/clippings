// Every prompt and message the commands show goes through here, so the
// integration tests can answer prompts and read messages (spec 12.5).

import * as vscode from 'vscode';

export type Shown =
  | { kind: 'input'; options: vscode.InputBoxOptions }
  | { kind: 'pick'; items: string[]; options: vscode.QuickPickOptions }
  | { kind: 'info' | 'warning' | 'error'; message: string; actions: string[] };

export class Prompts {
  private readonly answers: unknown[] = [];
  /** Everything shown, most recent last. */
  readonly shown: Shown[] = [];

  /** Test hook: queues answers for the next prompts or message actions. */
  script(...answers: unknown[]): void {
    this.answers.push(...answers);
  }

  /** Test hook: answers queued by `script` that no prompt has consumed yet. */
  pending(): readonly unknown[] {
    return [...this.answers];
  }

  /** Test hook: discards any queued answers, so one test's leak can't reach the next. */
  clearScript(): void {
    this.answers.length = 0;
  }

  private scripted<T>(): { answer: T | undefined } | undefined {
    return this.answers.length > 0 ? { answer: this.answers.shift() as T | undefined } : undefined;
  }

  async input(options: vscode.InputBoxOptions): Promise<string | undefined> {
    this.shown.push({ kind: 'input', options });
    const s = this.scripted<string>();
    return s ? s.answer : vscode.window.showInputBox(options);
  }

  async pick(items: string[], options: vscode.QuickPickOptions & { canPickMany: true }): Promise<string[] | undefined>;
  async pick(items: string[], options: vscode.QuickPickOptions): Promise<string | undefined>;
  async pick(items: string[], options: vscode.QuickPickOptions): Promise<string | string[] | undefined> {
    this.shown.push({ kind: 'pick', items, options });
    const s = this.scripted<string | string[]>();
    return s ? s.answer : vscode.window.showQuickPick(items, options);
  }

  /** A quick pick of labelled items, answering with the chosen label. */
  async pickItem(items: vscode.QuickPickItem[], options: vscode.QuickPickOptions): Promise<string | undefined> {
    this.shown.push({ kind: 'pick', items: items.map((i) => i.label), options });
    const s = this.scripted<string>();
    if (s) return s.answer;
    return (await vscode.window.showQuickPick(items, options))?.label;
  }

  async message(kind: 'info' | 'warning' | 'error', message: string, ...actions: string[]): Promise<string | undefined> {
    this.shown.push({ kind, message, actions });
    if (actions.length > 0) {
      const s = this.scripted<string>();
      if (s) return s.answer;
    }
    const show =
      kind === 'info'
        ? vscode.window.showInformationMessage
        : kind === 'warning'
          ? vscode.window.showWarningMessage
          : vscode.window.showErrorMessage;
    return show(message, ...actions);
  }
}
