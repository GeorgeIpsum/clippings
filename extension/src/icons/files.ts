// Rendered icons on disk (spec 7.7): one file per icon and colour under
// global storage, written synchronously the first time it is needed and
// remembered in memory.

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { iconFileName, renderSvg, type SvgIcon } from './svg';

export class IconFiles {
  private readonly known = new Map<string, string>();

  constructor(private readonly dir: string) {}

  /** The path of the icon's SVG file, writing it if missing. */
  path(icon: SvgIcon): string {
    const name = iconFileName(icon);
    const cached = this.known.get(name);
    if (cached) return cached;
    const path = join(this.dir, name);
    if (!existsSync(path)) {
      mkdirSync(this.dir, { recursive: true });
      writeFileSync(path, renderSvg(icon));
    }
    this.known.set(name, path);
    return path;
  }
}
