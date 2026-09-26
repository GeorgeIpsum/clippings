// Temporary filter globs from the folder and file context menus, scopes and
// Remove Filter (spec 5.3, inventory 6.2). Pure.

import { fileURLToPath } from 'node:url';

/** globset::escape: wraps each glob metacharacter in brackets. */
export function escapeGlob(path: string): string {
  return path.replace(/[?*[\]{}]/g, (c) => `[${c}]`);
}

/** A path with `/` separators, as the server matches globs. */
export function slashPath(path: string, platform: NodeJS.Platform = process.platform): string {
  return platform === 'win32' ? path.replaceAll('\\', '/') : path;
}

/** Only Show This Folder (`/*`), or And Subfolders and Hide This Folder (`/**\/*`). */
export function folderGlob(path: string, recursive: boolean): string {
  return escapeGlob(slashPath(path)) + (recursive ? '/**/*' : '/*');
}

/** Hide This File. */
export function fileGlob(path: string): string {
  return escapeGlob(slashPath(path));
}

/**
 * The filesystem path of a tree root, folder or file node, from its own key
 * (spec 5.12): `w:<folder uri>`, `d:<path>` or `f:<path>`. A file node of a
 * non-`file` document has no path.
 */
export function nodePath(ownKey: string): string | undefined {
  const value = ownKey.slice(2);
  if (ownKey.startsWith('w:')) {
    try {
      return fileURLToPath(value);
    } catch {
      return undefined;
    }
  }
  if (ownKey.startsWith('d:')) return value;
  // A scheme has two or more characters; a Windows drive letter has one.
  if (ownKey.startsWith('f:')) return /^[a-z][a-z0-9+.-]+:/i.test(value) ? undefined : value;
  return undefined;
}

/** A scope's globs, a comma-separated string or an array (todo-tree's `toGlobArray`). */
export function toGlobArray(value: unknown): string[] {
  if (typeof value === 'string') {
    return value
      .split(',')
      .map((g) => g.trim())
      .filter((g) => g.length > 0);
  }
  return Array.isArray(value) ? value.filter((g): g is string => typeof g === 'string') : [];
}

export interface Scope {
  name: string;
  includeGlobs: string[];
  excludeGlobs: string[];
}

export function scopesOf(value: unknown): Scope[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter((s): s is Record<string, unknown> => !!s && typeof s === 'object' && typeof s['name'] === 'string')
    .map((s) => ({
      name: s['name'] as string,
      includeGlobs: toGlobArray(s['includeGlobs']),
      excludeGlobs: toGlobArray(s['excludeGlobs']),
    }));
}

/** Whether a scope's globs equal the current temporary globs. */
export function isCurrentScope(scope: Scope, include: string[], exclude: string[]): boolean {
  return (
    JSON.stringify(scope.includeGlobs) === JSON.stringify(include) &&
    JSON.stringify(scope.excludeGlobs) === JSON.stringify(exclude)
  );
}

export type Removal = { kind: 'clear' } | { kind: 'include'; glob: string } | { kind: 'exclude'; glob: string };

export const CLEAR_TREE_FILTER = 'Clear Tree Filter';

/** Remove Filter's choices, labelled as todo-tree labels them. */
export function removalChoices(filter: string, include: string[], exclude: string[]): Map<string, Removal> {
  const choices = new Map<string, Removal>();
  if (filter) choices.set(CLEAR_TREE_FILTER, { kind: 'clear' });
  for (const glob of exclude) {
    const label = glob.endsWith('/**/*')
      ? `Exclude Folder: ${glob.slice(0, -5)}`
      : glob.includes('*')
        ? `Exclude: ${glob}`
        : `Exclude File: ${glob}`;
    choices.set(label, { kind: 'exclude', glob });
  }
  for (const glob of include) {
    const label = glob.endsWith('/**/*')
      ? `Include Folder and Subfolders: ${glob.slice(0, -5)}`
      : glob.endsWith('/*')
        ? `Include Folder: ${glob.slice(0, -2)}`
        : `Include: ${glob}`;
    choices.set(label, { kind: 'include', glob });
  }
  return choices;
}
