import * as assert from 'node:assert/strict';
import { DEFAULT_SCHEMES, documentSelector, schemeList } from '../../config/schemes';

describe('general.schemes', () => {
  it('selects one filter per configured scheme', () => {
    assert.deepEqual(documentSelector(['file', 'vscode-notebook-cell']), [
      { scheme: 'file' },
      { scheme: 'vscode-notebook-cell' },
    ]);
    assert.deepEqual(documentSelector([]), []);
  });

  it('falls back to the defaults for a value of the wrong type', () => {
    for (const value of ['file', undefined, null, 3, { file: true }, ['file', 3]]) {
      assert.deepEqual(schemeList(value), DEFAULT_SCHEMES, JSON.stringify(value));
    }
    assert.deepEqual(DEFAULT_SCHEMES, ['file', 'ssh', 'untitled', 'vscode-notebook-cell']);
  });
});
