import * as assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

interface Command {
  command: string;
  title: string;
  category: string;
}
interface MenuItem {
  command: string;
  when?: string;
}
interface Setting {
  scope?: string;
  default?: unknown;
}
interface Manifest {
  engines: { vscode: string };
  extensionKind: string[];
  activationEvents: string[];
  capabilities: {
    untrustedWorkspaces: { supported: string; restrictedConfigurations: string[] };
    virtualWorkspaces: boolean;
  };
  contributes: {
    commands: Command[];
    menus: Record<string, MenuItem[]>;
    configuration: { title: string; properties: Record<string, Setting> }[];
    views: Record<string, { id: string; when: string }[]>;
  };
}

// Tests run from out/test/unit; the manifest sits three levels up.
const manifest = JSON.parse(readFileSync(resolve(__dirname, '../../../package.json'), 'utf8')) as Manifest;
const settings = Object.assign({}, ...manifest.contributes.configuration.map((g) => g.properties)) as Record<
  string,
  Setting
>;

describe('manifest', () => {
  it('declares the platform facts from spec 7.1', () => {
    assert.equal(manifest.engines.vscode, '^1.91.0');
    assert.deepEqual(manifest.extensionKind, ['workspace']);
    assert.deepEqual(manifest.activationEvents, ['onStartupFinished']);
    assert.equal(manifest.capabilities.untrustedWorkspaces.supported, 'limited');
    assert.deepEqual(manifest.capabilities.untrustedWorkspaces.restrictedConfigurations, ['clippings.server.path']);
    assert.equal(manifest.capabilities.virtualWorkspaces, false);
    assert.deepEqual(manifest.contributes.views['clippings'], [
      { id: 'clippings-view', name: 'TODOs', when: '!clippings-is-empty' },
    ]);
  });

  it('declares 65 settings, all window scoped except server.path', () => {
    const keys = Object.keys(settings);
    assert.equal(keys.length, 65);
    for (const key of keys) {
      assert.ok(key.startsWith('clippings.'), key);
      const expected = key === 'clippings.server.path' ? 'machine-overridable' : 'window';
      assert.equal(settings[key]?.scope, expected, key);
    }
    assert.equal((settings['clippings.filtering.builtInExcludes']?.default as string[]).length, 25);
  });

  it('declares 37 commands in the Clippings category', () => {
    const commands = manifest.contributes.commands;
    assert.equal(commands.length, 37);
    assert.equal(new Set(commands.map((c) => c.command)).size, 37);
    for (const c of commands) {
      assert.ok(c.command.startsWith('clippings.'), c.command);
      assert.equal(c.category, 'Clippings', c.command);
    }
  });

  it('menus reference declared commands and the clippings view only', () => {
    const declared = new Set(manifest.contributes.commands.map((c) => c.command));
    for (const [menu, items] of Object.entries(manifest.contributes.menus)) {
      for (const item of items) {
        assert.ok(declared.has(item.command), `${menu}: ${item.command}`);
        assert.ok(!item.when?.includes('todo-tree'), `${menu}: ${item.when}`);
        if (menu !== 'commandPalette') assert.match(item.when ?? '', /^view == clippings-view( && |$)/);
      }
    }
  });

  it('guards every disjunct of a menu when clause with the view', () => {
    for (const items of Object.values(manifest.contributes.menus)) {
      for (const item of items) {
        const when = item.when ?? '';
        // An `||` outside parentheses would escape the `view ==` guard.
        let depth = 0;
        for (let i = 0; i < when.length; i++) {
          if (when[i] === '(') depth++;
          if (when[i] === ')') depth--;
          assert.ok(!(depth === 0 && when.startsWith('||', i)), when);
        }
      }
    }
  });
});
