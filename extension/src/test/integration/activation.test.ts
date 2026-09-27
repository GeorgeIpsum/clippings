import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';

describe('activation', () => {
  it('activates and returns its API', async () => {
    const ext = vscode.extensions.getExtension('shmr.clippings');
    assert.ok(ext, 'extension is installed');
    const api = (await ext.activate()) as { version: string };
    assert.ok(ext.isActive);
    assert.equal(api.version, '0.1.0');
  });
});
