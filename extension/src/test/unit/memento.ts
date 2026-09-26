import type { Memento } from '../../state/viewState';

/** An in-memory `vscode.Memento` for unit tests. */
export class MemoryMemento implements Memento {
  readonly values = new Map<string, unknown>();

  get<T>(key: string): T | undefined {
    return this.values.get(key) as T | undefined;
  }

  update(key: string, value: unknown): Promise<void> {
    if (value === undefined) this.values.delete(key);
    else this.values.set(key, structuredClone(value));
    return Promise.resolve();
  }
}
