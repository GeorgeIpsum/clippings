// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { ApplySource } from './decorations/manager';
import type { DecorationsParams, StatusParams } from './protocol';
import type { PersistedViewState } from './state/viewState';
import type { StatusBarView } from './status/presentation';
import type { TestItem } from './tree/testItems';
import type { Prompts } from './ui/prompts';

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

export interface DecorationHooks {
  /** Fires after decorations are applied to a document's visible editors. */
  readonly onApplied: vscode.Event<{ uri: string; source: ApplySource }>;
  /** The decorations last applied to a document. */
  entry(uri: string): DecorationsParams | undefined;
  readonly generation: number | undefined;
  readonly styleKeys: string[];
  /** The style keys last set on a specific editor. */
  appliedKeys(editor: vscode.TextEditor): string[] | undefined;
}

export interface TestHooks {
  /** Answers prompts and records messages. */
  readonly prompts: Prompts;
  viewState(): PersistedViewState;
  /** The context keys last set (spec 7.6). */
  contextKeys(): Readonly<Record<string, boolean | string>>;
  /** What the status bar item shows. */
  statusBar(): StatusBarView | undefined;
  readonly decorations: DecorationHooks;
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
