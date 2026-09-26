import * as assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import {
  escapeGlob,
  fileGlob,
  folderGlob,
  isCurrentScope,
  nodePath,
  removalChoices,
  scopesOf,
  slashPath,
  toGlobArray,
} from '../../filters/globs';

describe('temporary globs', () => {
  it('escapes metacharacters as globset does', () => {
    assert.equal(escapeGlob('/w/app/[slug]/a*b?{c}'), '/w/app/[[]slug[]]/a[*]b[?][{]c[}]');
    assert.equal(escapeGlob('/plain/path!'), '/plain/path!');
  });

  it('builds folder and file globs', () => {
    assert.equal(folderGlob('/w/src', false), '/w/src/*');
    assert.equal(folderGlob('/w/[id]', true), '/w/[[]id[]]/**/*');
    assert.equal(fileGlob('/w/a.ts'), '/w/a.ts');
  });

  it('uses forward slashes on Windows only', () => {
    assert.equal(slashPath('C:\\w\\src', 'win32'), 'C:/w/src');
    assert.equal(slashPath('/w/a\\b', 'linux'), '/w/a\\b');
  });

  it('reads node paths from own keys', () => {
    const root = process.platform === 'win32' ? 'C:\\work\\space' : '/work/space';
    assert.equal(nodePath(`w:${pathToFileURL(root).href}`), root);
    assert.equal(nodePath('d:/w/src'), '/w/src');
    assert.equal(nodePath('f:/w/src/a.ts'), '/w/src/a.ts');
    assert.equal(nodePath('f:C:\\w\\a.ts'), 'C:\\w\\a.ts');
    assert.equal(nodePath('f:untitled:Untitled-1'), undefined);
    assert.equal(nodePath('g:TODO'), undefined);
    assert.equal(nodePath('w:not a uri'), undefined);
  });
});

describe('scopes', () => {
  it('accepts comma-separated strings or arrays', () => {
    assert.deepEqual(toGlobArray(' a/** , b/*,,'), ['a/**', 'b/*']);
    assert.deepEqual(toGlobArray(['x', 1]), ['x']);
    assert.deepEqual(toGlobArray(undefined), []);
  });

  it('reads named scopes and spots the current one', () => {
    const scopes = scopesOf([{ name: 'src', includeGlobs: 'src/**' }, { excludeGlobs: [] }, 'bad', { name: 'all' }]);
    assert.deepEqual(scopes, [
      { name: 'src', includeGlobs: ['src/**'], excludeGlobs: [] },
      { name: 'all', includeGlobs: [], excludeGlobs: [] },
    ]);
    assert.equal(isCurrentScope(scopes[0]!, ['src/**'], []), true);
    assert.equal(isCurrentScope(scopes[0]!, [], []), false);
  });
});

describe('remove filter choices', () => {
  it('labels each filter as todo-tree does', () => {
    const choices = removalChoices('fix', ['/w/a/**/*', '/w/b/*', '*.ts'], ['/w/c/**/*', '/w/d.ts', '**/*.md']);
    assert.deepEqual(
      [...choices.keys()],
      [
        'Clear Tree Filter',
        'Exclude Folder: /w/c',
        'Exclude File: /w/d.ts',
        'Exclude: **/*.md',
        'Include Folder and Subfolders: /w/a',
        'Include Folder: /w/b',
        'Include: *.ts',
      ],
    );
    assert.deepEqual(choices.get('Include Folder: /w/b'), { kind: 'include', glob: '/w/b/*' });
    assert.ok(!removalChoices('', [], []).has('Clear Tree Filter'));
  });
});
