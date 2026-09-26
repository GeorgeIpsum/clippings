// Where commands write settings (spec 7.6). Pure.

export type Target = 'global' | 'workspace';

/** Scan mode commands and the count, badge and compact folder toggles. */
export function folderTarget(hasFolder: boolean): Target {
  return hasFolder ? 'workspace' : 'global';
}

/** addTag, removeTag and the status bar clicks: where the value already lives. */
export function valueTarget(inspected: { workspaceValue?: unknown } | undefined): Target {
  return inspected?.workspaceValue !== undefined ? 'workspace' : 'global';
}
