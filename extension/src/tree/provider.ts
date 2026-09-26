// The tree data provider over node ID strings (spec 7.5). Children come from
// `clippings/children`; `clippings/treeChanged` names the parents to refetch.

import * as vscode from 'vscode';
import type { ViewNode } from '../protocol';
import { treeItem, type ItemContext } from './items';
import type { NodeCache } from './nodeCache';

export interface ChildrenSource {
  children(parent: string | null): Promise<ViewNode[]>;
}

export class TreeProvider implements vscode.TreeDataProvider<string>, vscode.Disposable {
  private readonly changed = new vscode.EventEmitter<string | string[] | undefined>();
  readonly onDidChangeTreeData = this.changed.event;

  constructor(
    private readonly source: ChildrenSource,
    private readonly cache: NodeCache,
    private readonly ctx: ItemContext,
  ) {}

  async getChildren(element?: string): Promise<string[]> {
    const parent = element ?? null;
    const nodes = await this.source.children(parent);
    this.cache.record(parent, nodes);
    return nodes.map((n) => n.id);
  }

  getTreeItem(element: string): vscode.TreeItem {
    const node = this.cache.get(element);
    if (!node) {
      const item = new vscode.TreeItem('');
      item.id = this.ctx.itemId(element);
      return item;
    }
    return treeItem(node, this.ctx);
  }

  getParent(element: string): string | undefined {
    return this.cache.parent(element) ?? undefined;
  }

  /** Applies `clippings/treeChanged`: `null` is the root; unknown IDs are ignored. */
  refresh(parents: readonly (string | null)[]): void {
    if (parents.includes(null)) return this.changed.fire(undefined);
    const known = parents.filter((p): p is string => p !== null && this.cache.has(p));
    if (known.length > 0) this.changed.fire(known);
  }

  /** Refetches the whole tree. */
  refreshAll(): void {
    this.changed.fire(undefined);
  }

  /** A new server instance: forget every node and refetch. */
  reset(): void {
    this.cache.clear();
    this.changed.fire(undefined);
  }

  dispose(): void {
    this.changed.dispose();
  }
}
