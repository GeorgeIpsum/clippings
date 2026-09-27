// SVG icons rendered in a colour (spec 5.13, 7.7): octicons from
// @primer/octicons, Clippings' own todo icons, and the check-circle. Pure.

import octicons from '@primer/octicons';
import type { IconDescriptor, Settings } from '../protocol';

/** The descriptors that render to an SVG file. */
export type SvgIcon = Extract<IconDescriptor, { kind: 'octicon' | 'todoTree' | 'check' }>;

export function isSvgIcon(icon: IconDescriptor): icon is SvgIcon {
  return icon.kind === 'octicon' || icon.kind === 'todoTree' || icon.kind === 'check';
}

export function isOcticon(name: string): boolean {
  return Object.hasOwn(octicons, name);
}

function attr(value: string): string {
  return value.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

function octicon(name: string, colour: string): string {
  const icon = octicons[isOcticon(name) ? name : 'check'];
  if (!icon) throw new Error('@primer/octicons has no check icon');
  return icon.toSVG({ xmlns: 'http://www.w3.org/2000/svg', fill: attr(colour) });
}

/** Clippings' own todo icon: a rounded square with a tick, outline or filled. */
function todo(filled: boolean, colour: string): string {
  const c = attr(colour);
  const box = filled
    ? `<rect x="1" y="1" width="14" height="14" rx="3.5" fill="${c}"/>`
    : `<rect x="1.75" y="1.75" width="12.5" height="12.5" rx="3" fill="none" stroke="${c}" stroke-width="1.5"/>`;
  const tick = filled ? '#ffffff' : c;
  return (
    '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">' +
    box +
    `<path d="M4.5 8.25l2.25 2.25 4.75-5" fill="none" stroke="${tick}" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>` +
    '</svg>'
  );
}

export function renderSvg(icon: SvgIcon): string {
  switch (icon.kind) {
    case 'octicon':
      return octicon(icon.name, icon.colour);
    case 'todoTree':
      return todo(icon.filled, icon.colour);
    case 'check':
      return octicon('check-circle-fill', icon.colour);
  }
}

/** FNV-1a, 32 bits, as eight hex digits. */
function hash(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, '0');
}

/**
 * A deterministic, filesystem-safe file name per icon and colour. The
 * readable part helps debugging; the hash keeps distinct colours apart.
 */
export function iconFileName(icon: SvgIcon): string {
  const name = icon.kind === 'octicon' ? icon.name : icon.kind === 'todoTree' ? (icon.filled ? 'filled' : 'outline') : 'check';
  const key = `${icon.kind}|${name}|${icon.colour}`;
  const readable = `${icon.kind}-${name}-${icon.colour}`.replace(/[^A-Za-z0-9_-]+/g, '_').slice(0, 48);
  return `${readable}-${hash(key)}.svg`;
}

/**
 * Icon names that are neither octicons, todo-tree's own names, nor codicons
 * (`$(name)`, which VS Code checks itself).
 */
export function invalidIcons(settings: Settings): string[] {
  const names = [
    settings.highlights.defaultHighlight?.icon,
    ...Object.values(settings.highlights.customHighlight ?? {}).map((a) => a?.icon),
  ];
  const bad = names.filter(
    (n): n is string =>
      typeof n === 'string' &&
      n.length > 0 &&
      !n.trim().startsWith('$(') &&
      n !== 'todo-tree' &&
      n !== 'todo-tree-filled' &&
      !isOcticon(n),
  );
  return [...new Set(bad)];
}
