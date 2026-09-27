import * as assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildConfiguration } from '../../config/configuration';
import { IconFiles } from '../../icons/files';
import { iconFileName, invalidIcons, isOcticon, renderSvg } from '../../icons/svg';

describe('icon svgs', () => {
  it('renders octicons in the colour, falling back to check', () => {
    const bug = renderSvg({ kind: 'octicon', name: 'bug', colour: '#ff0000' });
    assert.match(bug, /^<svg /);
    assert.match(bug, /fill="#ff0000"/);
    assert.match(bug, /octicon-bug/);
    const unknown = renderSvg({ kind: 'octicon', name: 'no-such-icon', colour: 'red' });
    assert.match(unknown, /octicon-check"/);
  });

  it('renders the todo icons and the check-circle', () => {
    const outline = renderSvg({ kind: 'todoTree', filled: false, colour: 'blue' });
    const filled = renderSvg({ kind: 'todoTree', filled: true, colour: 'blue' });
    assert.notEqual(outline, filled);
    assert.match(outline, /stroke="blue"/);
    assert.match(filled, /<rect[^>]*fill="blue"/);
    assert.match(renderSvg({ kind: 'check', colour: 'green' }), /octicon-check-circle-fill.*fill="green"|fill="green".*octicon-check-circle-fill/);
  });

  it('escapes colours for the attribute', () => {
    assert.match(renderSvg({ kind: 'check', colour: 'x" onload="y' }), /fill="x&#34; onload=&#34;y"/);
  });

  it('names files deterministically, safely and per colour', () => {
    const a = iconFileName({ kind: 'octicon', name: 'bug', colour: 'rgba(255, 0, 0, 0.5)' });
    assert.equal(a, iconFileName({ kind: 'octicon', name: 'bug', colour: 'rgba(255, 0, 0, 0.5)' }));
    assert.match(a, /^octicon-bug-rgba_255_0_0_0_5_-[0-9a-f]{8}\.svg$/);
    assert.notEqual(a, iconFileName({ kind: 'octicon', name: 'bug', colour: 'rgba(255,0,0,0.5)' }));
    assert.notEqual(
      iconFileName({ kind: 'todoTree', filled: true, colour: 'red' }),
      iconFileName({ kind: 'todoTree', filled: false, colour: 'red' }),
    );
    assert.doesNotMatch(iconFileName({ kind: 'check', colour: '../../etc' }), /\.\.|\//);
  });

  it('knows the octicon set', () => {
    assert.ok(isOcticon('flame'));
    assert.ok(!isOcticon('constructor'));
    assert.ok(!isOcticon('flames'));
  });
});

describe('icon files', () => {
  it('writes each icon once and remembers it', () => {
    const dir = mkdtempSync(join(tmpdir(), 'clippings-icons-'));
    try {
      const files = new IconFiles(join(dir, 'icons'));
      const path = files.path({ kind: 'octicon', name: 'flame', colour: 'orange' });
      assert.match(readFileSync(path, 'utf8'), /fill="orange"/);
      const written = statSync(path).mtimeMs;
      assert.equal(files.path({ kind: 'octicon', name: 'flame', colour: 'orange' }), path);
      assert.equal(new IconFiles(join(dir, 'icons')).path({ kind: 'octicon', name: 'flame', colour: 'orange' }), path);
      assert.equal(statSync(path).mtimeMs, written);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe('icon validation', () => {
  it('reports names that are not octicons, codicons or todo-tree icons', () => {
    const settings = buildConfiguration({
      groups: {
        general: {},
        highlights: {
          defaultHighlight: { icon: 'nope' },
          customHighlight: {
            A: { icon: 'bug' },
            B: { icon: '$(beaker)' },
            C: { icon: 'todo-tree' },
            D: { icon: 'todo-tree-filled' },
            E: { icon: 'nope' },
            F: { icon: 'also-bad' },
            G: {},
          },
        },
        filtering: {},
        tree: {},
        regex: {},
      },
      filesExclude: {},
      searchExclude: {},
      explorerCompactFolders: false,
      viewState: { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] },
    });
    assert.deepEqual(invalidIcons(settings), ['nope', 'also-bad']);
  });
});
