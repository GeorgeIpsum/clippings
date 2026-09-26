// A server decoration style as VS Code decoration options (spec 5.13).
// Pure: the caller supplies theme colours and the gutter icon.

import type { DecorationRenderOptions, ThemeColor, Uri } from 'vscode';
import type { Colour, DecorationStyle } from '../protocol';

export interface RenderDeps {
  themeColor(id: string): ThemeColor;
  /** The gutter icon file, when the descriptor renders to SVG. */
  gutterIcon(style: DecorationStyle): Uri | undefined;
}

function colour(value: Colour | null, deps: RenderDeps): string | ThemeColor | undefined {
  if (!value) return undefined;
  return 'theme' in value ? deps.themeColor(value.theme) : value.css;
}

export function renderOptions(style: DecorationStyle, deps: RenderDeps): DecorationRenderOptions {
  const options: DecorationRenderOptions = {
    borderRadius: style.borderRadius,
    fontStyle: style.fontStyle,
    fontWeight: style.fontWeight,
    isWholeLine: style.isWholeLine,
  };
  const fg = colour(style.color, deps);
  const bg = colour(style.backgroundColor, deps);
  if (fg !== undefined) options.color = fg;
  if (bg !== undefined) options.backgroundColor = bg;
  if (style.textDecoration) options.textDecoration = style.textDecoration;
  if (style.overviewRulerLane !== null) {
    const ruler = colour(style.overviewRulerColor, deps);
    if (ruler !== undefined) options.overviewRulerColor = ruler;
    // The server's lane numbers are VS Code's OverviewRulerLane values.
    options.overviewRulerLane = style.overviewRulerLane;
  }
  const gutter = deps.gutterIcon(style);
  if (gutter) options.gutterIconPath = gutter;
  return options;
}
