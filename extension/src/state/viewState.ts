// Persisted view state in workspace storage (spec 7.8): the view buttons
// the user clicked, the tree filter, temporary globs, the expansion map and
// the tree item epoch.

/** The subset of `vscode.Memento` the store needs. */
export interface Memento {
  get<T>(key: string): T | undefined;
  update(key: string, value: unknown): PromiseLike<void>;
}

/** View buttons whose clicked value overrides the matching `tree.*` setting. */
export interface ViewFlags {
  flat?: boolean;
  tagsOnly?: boolean;
  expanded?: boolean;
  groupedByTag?: boolean;
  groupedBySubTag?: boolean;
}

export interface PersistedViewState extends ViewFlags {
  currentFilter: string;
  filtered: boolean;
  includeGlobs: string[];
  excludeGlobs: string[];
}

const FLAG_KEYS = ['flat', 'tagsOnly', 'expanded', 'groupedByTag', 'groupedBySubTag'] as const;

/** Every workspace storage key the extension owns. */
export const WORKSPACE_KEYS = [
  ...FLAG_KEYS,
  'currentFilter',
  'filtered',
  'includeGlobs',
  'excludeGlobs',
  'expandedNodes',
  'epoch',
] as const;

function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
}

function flag(value: unknown): boolean | undefined {
  return typeof value === 'boolean' ? value : undefined;
}

export class ViewStateStore {
  constructor(private readonly memento: Memento) {}

  snapshot(): PersistedViewState {
    const m = this.memento;
    const state: PersistedViewState = {
      currentFilter: typeof m.get('currentFilter') === 'string' ? (m.get<string>('currentFilter') ?? '') : '',
      filtered: m.get('filtered') === true,
      includeGlobs: strings(m.get('includeGlobs')),
      excludeGlobs: strings(m.get('excludeGlobs')),
    };
    for (const key of FLAG_KEYS) {
      const value = flag(m.get(key));
      if (value !== undefined) state[key] = value;
    }
    return state;
  }

  async setFlags(flags: ViewFlags): Promise<void> {
    for (const key of FLAG_KEYS) {
      if (key in flags) await this.memento.update(key, flags[key]);
    }
  }

  /** Sets the tree filter; empty or undefined clears it. */
  async setFilter(text: string | undefined): Promise<void> {
    await this.memento.update('currentFilter', text ?? '');
    await this.memento.update('filtered', !!text);
  }

  async setGlobs(includeGlobs: string[], excludeGlobs: string[]): Promise<void> {
    await this.memento.update('includeGlobs', includeGlobs);
    await this.memento.update('excludeGlobs', excludeGlobs);
  }

  /** Expansion state by node ID. */
  expandedNodes(): Record<string, boolean> {
    const value = this.memento.get<unknown>('expandedNodes');
    return value && typeof value === 'object' ? { ...(value as Record<string, boolean>) } : {};
  }

  async setExpandedNodes(nodes: Record<string, boolean>): Promise<void> {
    await this.memento.update('expandedNodes', nodes);
  }

  epoch(): number {
    const value = this.memento.get<unknown>('epoch');
    return typeof value === 'number' && Number.isSafeInteger(value) ? value : 0;
  }

  async bumpEpoch(): Promise<number> {
    const next = this.epoch() + 1;
    await this.memento.update('epoch', next);
    return next;
  }

  /** Clears everything but the epoch, which only ever grows. */
  async reset(): Promise<void> {
    for (const key of WORKSPACE_KEYS) {
      if (key !== 'epoch') await this.memento.update(key, undefined);
    }
  }
}
