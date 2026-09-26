import * as assert from 'node:assert/strict';
import type { ThemeColor, Uri } from 'vscode';
import { renderOptions } from '../../decorations/renderOptions';
import { acceptDecorations, keysToSet, reusable, stylesAction } from '../../decorations/rules';
import type { DecorationStyle } from '../../protocol';

describe('decoration rules', () => {
  it('starts a generation on reset and adds only to the current one', () => {
    assert.equal(stylesAction(undefined, { generation: 0, reset: true, styles: {} }), 'reset');
    assert.equal(stylesAction(3, { generation: 3, reset: false, styles: {} }), 'add');
    assert.equal(stylesAction(3, { generation: 2, reset: false, styles: {} }), 'drop');
    assert.equal(stylesAction(undefined, { generation: 0, reset: false, styles: {} }), 'drop');
  });

  it('drops decorations from another generation or an older version', () => {
    assert.equal(acceptDecorations(2, { generation: 2, version: 5 }, 5), true);
    assert.equal(acceptDecorations(2, { generation: 2, version: 6 }, 5), true, 'newer than the document is fine');
    assert.equal(acceptDecorations(2, { generation: 1, version: 5 }, 5), false);
    assert.equal(acceptDecorations(2, { generation: 2, version: 4 }, 5), false);
    assert.equal(acceptDecorations(2, { generation: 2, version: 4 }, undefined), false, 'closed document');
  });

  it('reuses a cached entry only for the same version and generation', () => {
    assert.equal(reusable(1, { generation: 1, version: 3 }, 3), true);
    assert.equal(reusable(1, { generation: 1, version: 3 }, 4), false);
    assert.equal(reusable(2, { generation: 1, version: 3 }, 3), false);
  });

  it('clears keys that were applied before and are now absent', () => {
    assert.deepEqual(keysToSet(['TODO', 'BUG'], { TODO: [], FIXME: [] }).sort(), ['BUG', 'FIXME', 'TODO']);
  });
});

describe('decoration render options', () => {
  const theme = (id: string) => ({ id }) as ThemeColor;
  const base: DecorationStyle = {
    color: { theme: 'editor.background' },
    backgroundColor: { css: 'rgba(255,0,0,0.5)' },
    overviewRulerColor: { css: '#ff0000' },
    overviewRulerLane: 4,
    borderRadius: '0.2em',
    fontStyle: 'normal',
    fontWeight: 'bold',
    textDecoration: '',
    isWholeLine: false,
    gutterIcon: null,
  };

  it('maps colours, ruler and font attributes', () => {
    const options = renderOptions(base, { themeColor: theme, gutterIcon: () => undefined });
    assert.deepEqual(options, {
      borderRadius: '0.2em',
      fontStyle: 'normal',
      fontWeight: 'bold',
      isWholeLine: false,
      color: { id: 'editor.background' },
      backgroundColor: 'rgba(255,0,0,0.5)',
      overviewRulerColor: '#ff0000',
      overviewRulerLane: 4,
    });
  });

  it('omits the ruler without a lane and adds whole-line, decoration and gutter icon', () => {
    const icon = { fsPath: '/icons/todo.svg' } as Uri;
    const options = renderOptions(
      { ...base, overviewRulerLane: null, isWholeLine: true, textDecoration: 'underline', gutterIcon: { kind: 'default' } },
      { themeColor: theme, gutterIcon: () => icon },
    );
    assert.equal(options.overviewRulerColor, undefined);
    assert.equal(options.overviewRulerLane, undefined);
    assert.equal(options.isWholeLine, true);
    assert.equal(options.textDecoration, 'underline');
    assert.equal(options.gutterIconPath, icon);
  });
});
