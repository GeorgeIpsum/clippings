// Maps the server's icon descriptors onto VS Code icons (spec 7.7).

import * as vscode from 'vscode';
import type { IconDescriptor } from '../protocol';

export type TreeIcon = vscode.ThemeIcon | vscode.Uri;

export class IconResolver {
  /** An icon for a tree item, or undefined for none. */
  treeIcon(icon: IconDescriptor | null): TreeIcon | undefined {
    if (!icon) return undefined;
    switch (icon.kind) {
      case 'codicon':
        return new vscode.ThemeIcon(icon.name, icon.colour ? new vscode.ThemeColor(icon.colour) : undefined);
      case 'folder':
        return vscode.ThemeIcon.Folder;
      case 'file':
        return vscode.ThemeIcon.File;
      default:
        return undefined;
    }
  }
}
