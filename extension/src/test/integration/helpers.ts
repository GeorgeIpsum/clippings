// Shared helpers for the integration tests. Waits are driven by real
// signals from the extension, never by fixed sleeps.

import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import type { TestItem } from '../../tree/testItems';

export async function getApi(): Promise<ClippingsApi> {
  const ext = vscode.extensions.getExtension<ClippingsApi>('clippings-dev.clippings');
  if (!ext) throw new Error('extension not installed');
  return ext.activate();
}

/**
 * Resolves with the first truthy value of `check`, evaluated now and again
 * after each event from `events`.
 */
export function waitFor<T>(
  what: string,
  check: () => T | Promise<T>,
  events: vscode.Event<unknown>[],
  timeoutMs = 15_000,
): Promise<NonNullable<T>> {
  return new Promise((resolve, reject) => {
    let done = false;
    let running = false;
    let again = false;
    const subscriptions: vscode.Disposable[] = [];
    const finish = (error: Error | undefined, value?: NonNullable<T>) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      subscriptions.forEach((s) => s.dispose());
      if (error) reject(error);
      else resolve(value as NonNullable<T>);
    };
    const timer = setTimeout(() => finish(new Error(`timed out waiting for ${what}`)), timeoutMs);
    const evaluate = async (): Promise<void> => {
      if (running) {
        again = true;
        return;
      }
      running = true;
      try {
        do {
          again = false;
          const value = await check();
          if (value) return finish(undefined, value as NonNullable<T>);
        } while (again && !done);
      } catch (err) {
        finish(err instanceof Error ? err : new Error(String(err)));
      } finally {
        running = false;
      }
    };
    for (const event of events) subscriptions.push(event(() => void evaluate()));
    void evaluate();
  });
}

/** Resolves like `work`, or rejects if it takes longer than `timeoutMs`. */
export function withTimeout<T>(what: string, work: Thenable<T>, timeoutMs = 20_000): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`timed out waiting for ${what}`)), timeoutMs);
    work.then(
      (value) => (clearTimeout(timer), resolve(value)),
      (err: unknown) => (clearTimeout(timer), reject(err instanceof Error ? err : new Error(String(err)))),
    );
  });
}

/** Waits until the server is running and has finished scanning. */
export async function whenIdle(api: ClippingsApi): Promise<void> {
  const s = api.test.server;
  await waitFor('an idle server', () => s.running && s.status()?.scanning === false, [s.onStatus, s.onRunning]);
}

export function workspacePath(...parts: string[]): string {
  const root = process.env['CLIPPINGS_TEST_WORKSPACE'];
  if (!root) throw new Error('CLIPPINGS_TEST_WORKSPACE is not set');
  return [root, ...parts].join('/');
}

/**
 * The tree as indented lines, `label` or `label  description` for status
 * nodes, walking every node with children down to `depth` levels.
 */
export async function outline(api: ClippingsApi, depth = 10, parent?: string, indent = ''): Promise<string[]> {
  const lines: string[] = [];
  for (const item of await api.test.tree.items(parent)) {
    lines.push(indent + (item.label || `(${item.description ?? ''})`));
    if (item.state !== 'none' && depth > 1) lines.push(...(await outline(api, depth - 1, item.id, indent + '  ')));
  }
  return lines;
}

/** Waits until the tree's outline equals `expected`. */
export async function treeBecomes(api: ClippingsApi, expected: string[], depth = 10): Promise<void> {
  let last: string[] = [];
  try {
    await waitFor(
      'the expected tree',
      async () => {
        last = await outline(api, depth);
        return last.join('\n') === expected.join('\n');
      },
      [api.test.tree.onDidChange, api.test.server.onStatus],
    );
  } catch (err) {
    throw new Error(`${(err as Error).message}\nexpected:\n${expected.join('\n')}\nactual:\n${last.join('\n')}`);
  }
}

/** Finds a tree item by the labels on the path to it. */
export async function itemAt(api: ClippingsApi, ...labels: string[]): Promise<TestItem> {
  let parent: string | undefined;
  let found: TestItem | undefined;
  for (const label of labels) {
    found = (await api.test.tree.items(parent)).find((i) => i.label === label);
    if (!found) throw new Error(`no tree item ${labels.join(' > ')}`);
    parent = found.id;
  }
  if (!found) throw new Error('no labels');
  return found;
}

/** Updates a `clippings.*` setting and waits until VS Code reports the change. */
export async function setSetting(
  key: string,
  value: unknown,
  target = vscode.ConfigurationTarget.Global,
): Promise<void> {
  const changed = new Promise<void>((resolve) => {
    const sub = vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration(`clippings.${key}`)) {
        sub.dispose();
        resolve();
      }
    });
  });
  const current = vscode.workspace.getConfiguration('clippings').inspect(key);
  const existing =
    target === vscode.ConfigurationTarget.Global ? current?.globalValue : current?.workspaceValue;
  if (JSON.stringify(existing) === JSON.stringify(value)) return;
  await vscode.workspace.getConfiguration('clippings').update(key, value, target);
  await changed;
}

/** Resolves with the next event that matches, or rejects after a timeout. */
export function nextEvent<T>(
  what: string,
  event: vscode.Event<T>,
  matches: (e: T) => boolean = () => true,
  timeoutMs = 15_000,
): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      sub.dispose();
      reject(new Error(`timed out waiting for ${what}`));
    }, timeoutMs);
    const sub = event((e) => {
      if (!matches(e)) return;
      clearTimeout(timer);
      sub.dispose();
      resolve(e);
    });
  });
}
