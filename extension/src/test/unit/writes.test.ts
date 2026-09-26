import * as assert from 'node:assert/strict';
import { folderTarget, valueTarget } from '../../config/targets';

describe('setting write targets', () => {
  it('writes folder-scoped commands to the workspace only when a folder is open', () => {
    assert.equal(folderTarget(true), 'workspace');
    assert.equal(folderTarget(false), 'global');
  });

  it('writes value-scoped commands where the value already lives', () => {
    assert.equal(valueTarget({ workspaceValue: ['TODO'] }), 'workspace');
    assert.equal(valueTarget({ workspaceValue: false }), 'workspace');
    assert.equal(valueTarget({}), 'global');
    assert.equal(valueTarget(undefined), 'global');
  });
});
