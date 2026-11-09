import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';

describe('activation', () => {
  it('activates and returns its API', async () => {
    const ext = vscode.extensions.getExtension('shmr.clippings');
    assert.ok(ext, 'extension is installed');
    const api = (await ext.activate()) as { version: string };
    assert.ok(ext.isActive);
    // The manifest is the one source of the version, so a release bump needs no test edit.
    assert.equal(api.version, ext.packageJSON.version);
    assert.match(api.version, /^\d+\.\d+\.\d+$/);
  });
});
