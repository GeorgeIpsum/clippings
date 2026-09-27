import * as assert from 'node:assert/strict';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';

const COUNT = 1000;
const TARGET_MS = 30;

function manyTodos(version: number): string {
  return Array.from({ length: COUNT }, (_, i) => `// TODO item ${i} version ${version}\n`).join('');
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? NaN;
}

describe('extension host performance (spec 13)', function () {
  this.timeout(120_000);
  let api: ClippingsApi;
  const path = workspacePath('perf', 'many.ts');

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    mkdirSync(workspacePath('perf'), { recursive: true });
    writeFileSync(path, manyTodos(0));
    await waitFor(
      'the perf file in the tree',
      async () => (await api.test.tree.items()).length > 0 && (await itemAt(api, 'workspace', 'perf', 'many.ts').catch(() => undefined)),
      [api.test.tree.onDidChange],
    );
    await vscode.commands.executeCommand('clippings-view.focus');
  });

  after(async () => {
    rmSync(workspacePath('perf'), { recursive: true, force: true });
    await treeBecomes(api, DEFAULT_TREE);
  });

  it(`applies a delta that refreshes ${COUNT} visible nodes in under ${TARGET_MS} ms of host time`, async () => {
    const perf = api.test.tree.perf;
    const file = await itemAt(api, 'workspace', 'perf', 'many.ts');
    perf.reset();
    await api.test.tree.view.reveal(file.id, { expand: true, focus: false, select: false });
    await perf.whenItems(COUNT);

    const busy: number[] = [];
    const wall: number[] = [];
    for (let version = 1; version <= 7; version++) {
      perf.reset();
      writeFileSync(path, manyTodos(version));
      await perf.whenItems(COUNT);
      busy.push(perf.busyMs);
      wall.push(perf.lastItemAt - perf.lastChangeAt);
    }
    const children = await api.test.tree.items(file.id);
    assert.equal(children.length, COUNT);
    assert.equal(children[0]?.label, 'TODO item 0 version 7');

    // The JSON-RPC layer parses the children response on the host too.
    const nodes = children.map((c) => api.test.tree.node(c.id));
    assert.ok(nodes.every((n) => n !== undefined));
    const parse: number[] = [];
    let payload = '';
    for (let i = 0; i < 7; i++) {
      // A fresh string each time, as each response is.
      payload = JSON.stringify({ jsonrpc: '2.0', id: i, result: { nodes } });
      const start = performance.now();
      JSON.parse(payload);
      parse.push(performance.now() - start);
    }

    const host = median(busy) + median(parse);
    console.log(
      `perf: ${COUNT} nodes, provider ${median(busy).toFixed(2)} ms, JSON parse ${median(parse).toFixed(2)} ms ` +
        `(${(payload.length / 1024).toFixed(0)} KiB), host total ${host.toFixed(2)} ms, ` +
        `end-to-end incl. server round trip ${median(wall).toFixed(2)} ms; provider runs ${busy.map((b) => b.toFixed(1)).join(', ')}`,
    );
    assert.ok(host < TARGET_MS, `host time ${host.toFixed(2)} ms is over ${TARGET_MS} ms`);
  });
});
