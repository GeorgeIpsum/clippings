// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { StatusParams } from './protocol';
import type { TestItem } from './tree/testItems';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TreeHooks {
  readonly view: vscode.TreeView<string>;
  /** Fires whenever the provider asks VS Code to refetch. */
  readonly onDidChange: vscode.Event<unknown>;
  /** The provider's children of `parent` (top level when omitted), as tree items. */
  items(parent?: string): Promise<TestItem[]>;
  /** The current tree item epoch (spec 7.5). */
  readonly epoch: number;
  /** The line the last todo click flashed. */
  lastFlash(): { uri: string; line: number } | undefined;
}

export interface TestHooks {
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
