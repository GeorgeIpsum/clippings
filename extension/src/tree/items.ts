// Turns a rendered `ViewNode` into a VS Code tree item (spec 5.12, 7.5).

import * as vscode from 'vscode';
import type { IconResolver } from '../icons/resolver';
import type { ViewNode } from '../protocol';

export interface ItemContext {
  icons: IconResolver;
  /** The tree item ID for a node ID. */
  itemId(id: string): string;
  /** Whether a node with children shows expanded. */
  expanded(node: ViewNode): boolean;
}

export function treeItem(node: ViewNode, ctx: ItemContext): vscode.TreeItem {
  const state = !node.hasChildren
    ? vscode.TreeItemCollapsibleState.None
    : ctx.expanded(node)
      ? vscode.TreeItemCollapsibleState.Expanded
      : vscode.TreeItemCollapsibleState.Collapsed;
  const item = new vscode.TreeItem(node.label, state);
  item.id = ctx.itemId(node.id);
  if (node.description !== null) item.description = node.description;
  if (node.tooltip !== null) item.tooltip = node.tooltip;
  if (node.contextValue !== null) item.contextValue = node.contextValue;
  if (node.resourceUri !== null) item.resourceUri = vscode.Uri.parse(node.resourceUri);
  const icon = ctx.icons.treeIcon(node.icon);
  if (icon) item.iconPath = icon;
  const command = node.command;
  if (command?.kind === 'reveal') {
    item.command = {
      title: 'Reveal In File',
      command: 'clippings.revealInFile',
      arguments: [command.uri, command.position],
    };
  } else if (command?.kind === 'openUrl') {
    item.command = { title: 'Open URL', command: 'clippings.openUrl', arguments: [command.url] };
  }
  return item;
}
