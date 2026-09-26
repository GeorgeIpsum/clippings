// The styles and decorations rules of spec 6.2. Pure.

import type { DecorationsParams, StylesParams } from '../protocol';

/**
 * What to do with a `clippings/styles` message: a reset starts a new
 * generation; other messages add keys to the current generation only.
 */
export function stylesAction(current: number | undefined, msg: StylesParams): 'reset' | 'add' | 'drop' {
  if (msg.reset) return 'reset';
  return msg.generation === current ? 'add' : 'drop';
}

/**
 * Whether to apply a `clippings/decorations` message: its generation must be
 * current and its version no older than the open document's.
 */
export function acceptDecorations(
  current: number | undefined,
  msg: Pick<DecorationsParams, 'generation' | 'version'>,
  documentVersion: number | undefined,
): boolean {
  return msg.generation === current && documentVersion !== undefined && msg.version >= documentVersion;
}

/** Whether a cached entry can be reapplied to an editor that became visible. */
export function reusable(
  current: number | undefined,
  entry: Pick<DecorationsParams, 'generation' | 'version'>,
  documentVersion: number,
): boolean {
  return entry.generation === current && entry.version === documentVersion;
}

/** Keys to set on an editor: the new keys, plus previously applied keys to clear. */
export function keysToSet(previous: Iterable<string>, ranges: Record<string, unknown>): string[] {
  return [...new Set([...previous, ...Object.keys(ranges)])];
}
