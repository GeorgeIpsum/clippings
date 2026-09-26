// Client-side expansion state (spec 7.5): a map from node ID to expanded,
// persisted in workspace storage, and the epoch that prefixes tree item IDs
// so VS Code forgets its own expansion state when the whole tree resets.

import type { ViewNode } from '../protocol';
import type { ViewStateStore } from '../state/viewState';

export class Expansion {
  private nodes: Record<string, boolean>;
  private epoch: number;
  /** Bump the epoch at the next root refresh, once the server has re-rendered. */
  private bumpAtRoot = false;

  constructor(private readonly store: ViewStateStore) {
    this.nodes = store.expandedNodes();
    this.epoch = store.epoch();
  }

  get currentEpoch(): number {
    return this.epoch;
  }

  /** The tree item ID for a node: the node ID prefixed with the epoch. */
  itemId(id: string): string {
    return `${this.epoch}/${id}`;
  }

  expanded(node: ViewNode): boolean {
    return this.nodes[node.id] ?? node.defaultExpanded;
  }

  set(id: string, expanded: boolean): void {
    this.nodes[id] = expanded;
    void this.store.setExpandedNodes(this.nodes);
  }

  /**
   * Forgets every node's state and starts a new epoch. When the server is
   * about to re-render the tree with new defaults (`awaitServer`), the epoch
   * changes at that root refresh, so VS Code reads the new defaults under
   * new IDs; otherwise it changes now and the caller refreshes the root.
   */
  reset(awaitServer: boolean): void {
    this.nodes = {};
    void this.store.setExpandedNodes(this.nodes);
    if (awaitServer) this.bumpAtRoot = true;
    else this.bump();
  }

  /** Called before the provider refetches the root for `clippings/treeChanged`. */
  onRootRefresh(): void {
    if (!this.bumpAtRoot) return;
    this.bumpAtRoot = false;
    this.bump();
  }

  private bump(): void {
    this.epoch += 1;
    void this.store.bumpEpoch();
  }
}
