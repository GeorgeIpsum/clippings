import * as assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { buildConfiguration } from '../../config/configuration';
import {
  Method,
  PROTOCOL_VERSION,
  type ChildrenResult,
  type DecorationsParams,
  type DecorationStyle,
  type StatusParams,
  type StylesParams,
  type ViewNode,
} from '../../protocol';
import { LspProcess } from './lspProcess';

// Each list must name every field of its TypeScript type (the compiler checks
// that) and exactly the fields the server sends (the tests check that).
const statusFields = {
  instance: 1, scanning: 1, interrupted: 1, needsScan: 1, error: 1, warnings: 1,
  statusBar: 1, badge: 1, viewTitle: 1, hasSubTags: 1, isEmpty: 1,
} satisfies Record<keyof StatusParams, 1>;
const nodeFields = {
  id: 1, label: 1, description: 1, tooltip: 1, icon: 1, hasChildren: 1, defaultExpanded: 1,
  contextValue: 1, resourceUri: 1, command: 1,
} satisfies Record<keyof ViewNode, 1>;
const styleFields = {
  color: 1, backgroundColor: 1, overviewRulerColor: 1, overviewRulerLane: 1, borderRadius: 1, fontStyle: 1,
  fontWeight: 1, textDecoration: 1, isWholeLine: 1, gutterIcon: 1,
} satisfies Record<keyof DecorationStyle, 1>;
const stylesFields = { generation: 1, reset: 1, styles: 1 } satisfies Record<keyof StylesParams, 1>;
const decorationsFields = { uri: 1, version: 1, generation: 1, ranges: 1 } satisfies Record<keyof DecorationsParams, 1>;

function keys(value: unknown): string[] {
  return Object.keys(value as object).sort();
}

describe('protocol mirror against the real server', function () {
  this.timeout(20_000);
  let dir: string;
  let server: LspProcess;

  before(async () => {
    dir = mkdtempSync(join(tmpdir(), 'clippings-protocol-'));
    writeFileSync(join(dir, 'a.ts'), '// TODO first\n// FIXME second\n');
    server = new LspProcess();
    const settings = buildConfiguration({
      groups: { general: {}, highlights: { customHighlight: { TODO: { gutterIcon: true } } }, filtering: {}, tree: {}, regex: {} },
      filesExclude: {},
      searchExclude: {},
      explorerCompactFolders: false,
      viewState: { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] },
    });
    await server.request('initialize', {
      processId: process.pid,
      workspaceFolders: [{ uri: pathToFileURL(dir).href, name: 'w' }],
      capabilities: {},
      initializationOptions: { protocolVersion: PROTOCOL_VERSION, settings },
    });
    server.notify('initialized', {});
  });

  after(async () => {
    await server.close();
    rmSync(dir, { recursive: true, force: true });
  });

  it('status carries exactly the StatusParams fields', async () => {
    const status = await server.waitFor((m) => m.method === Method.status);
    assert.deepEqual(keys(status.params), keys(statusFields));
  });

  it('children nodes carry exactly the ViewNode fields', async () => {
    await server.waitFor((m) => m.method === Method.treeChanged);
    const result = (await server.request(Method.children, { parent: null })) as ChildrenResult;
    assert.ok(result.nodes.length > 0);
    for (const node of result.nodes) assert.deepEqual(keys(node), keys(nodeFields));
    const root = result.nodes.find((n) => n.contextValue === 'folder');
    assert.ok(root, 'a workspace root node');
    assert.deepEqual(root.icon, { kind: 'codicon', name: 'window', colour: null });
  });

  it('styles and decorations carry their fields and tagged colours', async () => {
    const uri = pathToFileURL(join(dir, 'a.ts')).href;
    server.notify('textDocument/didOpen', {
      textDocument: { uri, languageId: 'typescript', version: 3, text: '// TODO first\n// FIXME second\n' },
    });
    const decorations = await server.waitFor((m) => m.method === Method.decorations);
    const params = decorations.params as DecorationsParams;
    assert.deepEqual(keys(params), keys(decorationsFields));
    assert.equal(params.uri, uri);
    assert.equal(params.version, 3);
    assert.deepEqual(keys(params.ranges), ['FIXME', 'TODO']);
    const styles = server.received
      .filter((m) => m.method === Method.styles)
      .map((m) => m.params as StylesParams)
      .find((s) => !s.reset);
    assert.ok(styles, 'a non-reset styles message');
    assert.deepEqual(keys(styles), keys(stylesFields));
    const todo = styles.styles['TODO'];
    assert.ok(todo);
    assert.deepEqual(keys(todo), keys(styleFields));
    assert.deepEqual(todo.color, { theme: 'editor.background' });
    assert.deepEqual(todo.gutterIcon, { kind: 'check', colour: 'green' });
  });
});
