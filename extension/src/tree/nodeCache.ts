// The last node received for each ID and each node's parent (spec 7.5).
// Pure, so the tree logic can be tested without VS Code.

import type { ViewNode } from '../protocol';

export class NodeCache {
  private readonly nodes = new Map<string, ViewNode>();
  private readonly parents = new Map<string, string | null>();

  get(id: string): ViewNode | undefined {
    return this.nodes.get(id);
  }

  has(id: string): boolean {
    return this.nodes.has(id);
  }

  /** The node's parent ID, `null` for a top-level node, `undefined` if unknown. */
  parent(id: string): string | null | undefined {
    return this.parents.get(id);
  }

  get size(): number {
    return this.nodes.size;
  }

  /** Records the children of `parent` (`null` for the top level). */
  record(parent: string | null, children: readonly ViewNode[]): void {
    for (const node of children) {
      this.nodes.set(node.id, node);
      this.parents.set(node.id, parent);
    }
  }

  /** Records a path from `clippings/find`: each node is the parent of the next. */
  recordPath(path: readonly ViewNode[]): void {
    let parent: string | null = null;
    for (const node of path) {
      this.record(parent, [node]);
      parent = node.id;
    }
  }

  /**
   * The node's own key: its ID without the parent's ID and the `/` after it
   * (spec 5.12). Keys can contain `/`, so this needs the recorded parent.
   */
  ownKey(id: string): string {
    const parent = this.parents.get(id);
    return parent && id.startsWith(parent + '/') ? id.slice(parent.length + 1) : id;
  }

  clear(): void {
    this.nodes.clear();
    this.parents.clear();
  }
}
