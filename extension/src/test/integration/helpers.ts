// Shared helpers for the integration tests. Waits are driven by real
// signals from the extension, never by fixed sleeps.

import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';

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
