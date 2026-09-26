// A plain snapshot of tree items for the test hooks.

import * as vscode from 'vscode';

export interface TestItem {
  id: string;
  itemId: string;
  label: string;
  description: string | undefined;
  tooltip: string | undefined;
  contextValue: string | undefined;
  state: 'none' | 'collapsed' | 'expanded';
  icon: string | undefined;
  command: { command: string; arguments: unknown[] } | undefined;
}

function iconName(icon: vscode.TreeItem['iconPath']): string | undefined {
  if (icon instanceof vscode.ThemeIcon) return `$(${icon.id})`;
  if (icon instanceof vscode.Uri) return icon.fsPath;
  return undefined;
}

export function testItem(id: string, item: vscode.TreeItem): TestItem {
  const states = { 0: 'none', 1: 'collapsed', 2: 'expanded' } as const;
  return {
    id,
    itemId: item.id ?? '',
    label: typeof item.label === 'string' ? item.label : (item.label?.label ?? ''),
    description: typeof item.description === 'string' ? item.description : undefined,
    tooltip: typeof item.tooltip === 'string' ? item.tooltip : undefined,
    contextValue: item.contextValue,
    state: states[item.collapsibleState ?? 0],
    icon: iconName(item.iconPath),
    command: item.command ? { command: item.command.command, arguments: item.command.arguments ?? [] } : undefined,
  };
}
