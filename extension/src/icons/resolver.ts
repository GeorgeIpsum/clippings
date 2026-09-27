// Maps the server's icon descriptors onto VS Code icons (spec 5.13, 7.7):
// codicons become theme icons; octicons, the todo icons and the check-circle
// become SVG files; the default is the bundled icon.

import * as vscode from 'vscode';
import type { IconDescriptor } from '../protocol';
import type { IconFiles } from './files';
import { isSvgIcon } from './svg';

export type TreeIcon = vscode.ThemeIcon | vscode.Uri;

export class IconResolver {
  constructor(
    private readonly files: IconFiles,
    private readonly defaultIcon: vscode.Uri,
  ) {}

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
        return this.gutterIcon(icon);
    }
  }

  /** A gutter icon file: SVG icons only, as in todo-tree; codicons have none. */
  gutterIcon(icon: IconDescriptor): vscode.Uri | undefined {
    if (icon.kind === 'default') return this.defaultIcon;
    return isSvgIcon(icon) ? vscode.Uri.file(this.files.path(icon)) : undefined;
  }
}
