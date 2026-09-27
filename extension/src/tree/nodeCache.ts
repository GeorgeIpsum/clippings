// The last node received for each ID and each node's parent (spec 7.5).
// Pure, so the tree logic can be tested without VS Code.

import type { ViewNode } from '../protocol';

export class NodeCache {
  private readonly nodes = new Map<string, ViewNode>();
  private readonly parents = new Map<string, string | null>();
  /** The full child ID list `pruneMissing` last saw for a parent. */
  private readonly childIds = new Map<string | null, string[]>();

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
   * Forgets `parent`'s previous children that are absent from `currentIds`,
   * along with their own recorded descendants (spec 7.5). A full
   * `clippings/children` result is the complete child list for `parent`, so
   * a previously recorded child missing from it means VS Code has already
   * dropped that node from its data tree; a stray `treeChanged` naming it
   * later must be filtered out (by `has`) rather than fired, or VS Code logs
   * "Data tree node not found".
   */
  pruneMissing(parent: string | null, currentIds: readonly string[]): void {
    const previous = this.childIds.get(parent) ?? [];
    const kept = new Set(currentIds);
    for (const id of previous) if (!kept.has(id)) this.prune(id);
    this.childIds.set(parent, [...currentIds]);
  }

  private prune(id: string): void {
    for (const child of this.childIds.get(id) ?? []) this.prune(child);
    this.childIds.delete(id);
    this.nodes.delete(id);
    this.parents.delete(id);
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
    this.childIds.clear();
  }
}
