// What the status bar item shows for a server status, the status bar cycle,
// and one-time configuration notices (spec 5.14, 7.7). Pure.

import type { StatusBarMode, StatusParams } from '../protocol';

export interface StatusBarView {
  text: string;
  tooltip: string;
  command: string;
  visible: boolean;
}

export function statusBarView(status: StatusParams): StatusBarView {
  if (status.scanning) {
    return { text: 'Clippings: Scanning...', tooltip: 'Click to interrupt scan', command: 'clippings.stopScan', visible: true };
  }
  if (status.interrupted) {
    return { text: 'Clippings: Scanning interrupted.', tooltip: 'Click to restart', command: 'clippings.refresh', visible: true };
  }
  return { ...status.statusBar, command: 'clippings.onStatusBarClicked' };
}

/** The `cycle` click: total, tags, top three, current file, total. */
export function nextStatusBarMode(current: StatusBarMode): { mode: StatusBarMode; message: string } {
  switch (current) {
    case 'total':
      return { mode: 'tags', message: 'Clippings: Now showing tag counts' };
    case 'tags':
      return { mode: 'top three', message: 'Clippings: Now showing top three tag counts' };
    case 'top three':
      return { mode: 'current file', message: 'Clippings: Now showing total tags in current file' };
    default:
      return { mode: 'total', message: 'Clippings: Now showing total tags' };
  }
}

/** Remembers the last notice shown, so each distinct one is shown once. */
export class OnceNotice {
  private last = '';

  /** The notice to show for `items`, or undefined when there is none or it was just shown. */
  next(items: readonly string[]): string | undefined {
    const key = [...items].sort().join('\n');
    if (key === this.last) return undefined;
    this.last = key;
    return items.length > 0 ? `Clippings: ${items.join('; ')}` : undefined;
  }
}
