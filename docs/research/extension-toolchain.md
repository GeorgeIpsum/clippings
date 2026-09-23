# Extension toolchain research

Research artifact generated 2026-09-23. Scope: exact, verified toolchain facts for building the Clippings VS Code extension (spec sections 3, 4, 6, 7, 8.2, 8.3, 12.5), following on from `docs/research/rust-vscode-bridge-survey.md`.

Method: `npm view` against the live registry (dated 2026-09-23), unpacked npm tarballs (via `npm pack`) read directly rather than trusted from memory, the real `vscode-languageclient@10.1.1` and `vscode-jsonrpc@9.0.2` compiled sources, the real `@types/vscode@1.91.0` `index.d.ts`, the real installed `gruntfuggly.todo-tree-0.0.226` extension bundle at `~/.vscode/extensions/`, and the ten-extension local clones already fetched for the bridge survey under `scratchpad/ref2/`. Two facts (§2's xvfb CI recipes) came from the `code.visualstudio.com` docs via the WebFetch tool, which returns an LLM-summarized extraction rather than raw HTML; those two are cross-checked against a primary source and are called out below. Everything else was read directly from package source or `.d.ts` files, with file:line references into the unpacked tarball.

All unpacked packages live under the session scratchpad, `scratchpad/lc-check/`, and will not survive past this session; treat the package name@version in each citation as the reproducible pointer (`npm pack <pkg>@<version>` reproduces the same files).

---

## 1. vscode-languageclient 10.x

**Version.** Latest 10.x on npm (2026-09-23): **10.1.1** (`dist-tags.latest`). Full 10.x line: 10.0.0 through 10.0.1, 10.1.0, 10.1.1 (`npm view vscode-languageclient versions --json`). There is also a `next` dist-tag at `10.0.0-next.22`, not relevant here.

**`engines.vscode`.** Both `10.0.0` and `10.1.1` declare `"engines": { "vscode": "^1.91.0" }` (`npm view vscode-languageclient@10.1.1 engines --json`; confirmed again from the unpacked tarball's own `package.json`). Per spec 7.1, **`engines.vscode` should be `^1.91.0`**.

**Matching `@types/vscode`.** `@types/vscode@1.91.0` exists on the registry and is the natural pairing for `engines.vscode: ^1.91.0` (`npm view @types/vscode versions --json` lists `1.90.0, 1.91.0, 1.92.0, ...`). Latest `@types/vscode` overall is **1.138.0**, which matches the locally installed VS Code CLI (`code --version` → `1.138.0`), confirming the registry is current. Recommendation: pin `@types/vscode` to `^1.91.0` to match the declared floor — using the latest typings while declaring an older floor risks referencing APIs unavailable to users on the minimum version. Every VS Code API used elsewhere in this document (`ViewBadge`, `registerTextDocumentContentProvider`, `inspect()`, etc.) is already present in the `1.91.0` typings, so there is no functional reason to go newer.

**ESM or CJS, and importing from an esbuild CJS bundle.** `vscode-languageclient@10.1.1`'s own `package.json` has **no `"type"` field**, so it defaults to CommonJS. The unpacked `lib/node/main.js` starts with `"use strict"` and uses `require(...)`/`__exportStar` (TypeScript-compiled CJS output), not `import`/`export`. Source: unpacked tarball, `package/lib/node/main.js:1-10`, `package/package.json` (no `type` key). The package's `exports` map is:
```json
"exports": {
  ".":        { "types": "./lib/common/api.d.ts", "default": "./lib/common/api.js" },
  "./node":   { "types": "./lib/node/main.d.ts",   "node": "./lib/node/main.js" },
  "./browser":{ "types": "./lib/browser/main.d.ts","browser": "./lib/browser/main.js" }
}
```
So `require('vscode-languageclient/node')` resolves to CJS `lib/node/main.js` under Node's `"node"` export condition, and works unmodified from an esbuild-bundled CJS extension. In `extension/esbuild.config` terms: `import { LanguageClient, ... } from 'vscode-languageclient/node'` compiles (via `tsc`'s `esModuleInterop`) to a `require('vscode-languageclient/node')`, and esbuild's bundler (format `cjs`, platform `node`) inlines that CJS module directly — no special handling needed, unlike some ESM-only libraries.

**`LanguageClient` construction for a stdio `Executable` server with env vars.** From `lib/node/main.d.ts` (unpacked `vscode-languageclient@10.1.1`, lines ~31-104):
```ts
export interface ExecutableOptions { cwd?: string; env?: any; detached?: boolean; shell?: boolean }
export interface Executable { command: string; transport?: Transport; args?: string[]; options?: ExecutableOptions }
export type ServerOptions =
  | Executable
  | { run: Executable; debug: Executable }
  | { run: NodeModule; debug: NodeModule }
  | NodeModule
  | (() => Promise<ChildProcess | StreamInfo | MessageTransports | ChildProcessInfo>);

export declare class LanguageClient extends BaseLanguageClient {
  constructor(id: string, name: string, serverOptions: ServerOptions, clientOptions: LanguageClientOptions, forceDebug?: boolean);
  restart(): Promise<void>;
  get serverProcess(): ChildProcess | undefined;
}
```
`Executable.transport` defaults to stdio when omitted (no `--stdio` flag is appended automatically the way Biome's client does it; clippings' `clippings lsp` subcommand should speak JSON-RPC over stdio unconditionally). Example matching spec 7.4 ("Spawn: `clippings lsp` through the language client, with `RUST_BACKTRACE=1` and the log level in the environment"):
```ts
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';

const serverOptions: ServerOptions = {
  command: serverPath,
  args: ['lsp'],
  options: {
    env: { ...process.env, RUST_BACKTRACE: '1', CLIPPINGS_LOG: logLevel },
  },
};

const client = new LanguageClient('clippings', 'Clippings', serverOptions, clientOptions);
```

**`documentSelector` built from `general.schemes`, including notebook cells.** Build it as a plain array of scheme filters:
```ts
const documentSelector = schemes.map(scheme => ({ scheme }));
```
`DocumentFilter.scheme` alone (no `language`/`pattern`) matches any document with that scheme regardless of language — `@types/vscode@1.91.0`, `index.d.ts:2310-2338` (`DocumentFilter`), `:2353` (`DocumentSelector = DocumentFilter | string | ReadonlyArray<...>`).

**Notebook-cell answer (verified from source, not inferred):** a plain `{ scheme: 'vscode-notebook-cell' }` entry in `documentSelector` is **sufficient** — the client does *not* need `notebookDocumentSync`/`notebookDocumentOptions` for clippings' design. Trace:
- `vscode-languageclient@10.1.1`'s ordinary text-sync feature (`DidOpenTextDocumentFeature` etc., built on `TextDocumentEventFeature`) fires `textDocument/didOpen`/`didChange`/`didClose` for **any** document (including notebook-cell documents) that matches `documentSelector`, gated only by `TextDocumentEventFeature.matches()`:
  ```js
  // package/lib/common/features.js
  matches(data) {
      if (this._client.hasDedicatedTextSynchronizationFeature(this._textDocument(data))) {
          return false;
      }
      return !this._selectorFilter || this._selectorFilter(this._selectors.values(), data);
  }
  ```
- `hasDedicatedTextSynchronizationFeature` only returns `true` when the **server** has dynamically registered the LSP 3.17 `notebookDocumentSync` capability *and* that registration's notebook selector covers the cell's parent notebook:
  ```js
  // package/lib/common/client.js:1570-1575
  hasDedicatedTextSynchronizationFeature(textDocument) {
      const feature = this.getFeature(NotebookDocumentSyncRegistrationType.method);
      if (feature === undefined || !(feature instanceof NotebookDocumentSyncFeature)) return false;
      return feature.handles(textDocument);
  }
  ```
- Since a `clippings lsp` server that never declares `notebookDocumentSync` in its `initialize` response leaves that feature unregistered, `hasDedicatedTextSynchronizationFeature` is always `false`, so ordinary `didOpen`/`didChange`/`didClose` fire normally for `vscode-notebook-cell`-scheme documents, keyed by their own cell URI — exactly what spec 5.8 wants ("Buffer results ... keyed by the full URI, so notebook cells ... are separate entries").

The dedicated `notebookDocument/*` protocol (`NotebookDocumentSyncFeature`, `notebookDocumentOptions` on `LanguageClientOptions`) syncs the *notebook document as a structural unit* (cell array diffs) and is a different, heavier mechanism the spec does not need — file: `package/lib/common/notebook.d.ts:53-95`.

**`initializationOptions`.** `LanguageClientOptions.initializationOptions?: any | (() => any)` — `client.d.ts:260`. Passed verbatim as `InitializeParams.initializationOptions` on `initialize`.

**Custom `ErrorHandler` (10.x signatures).** From `client.d.ts:65-139`:
```ts
export declare enum ErrorAction { Continue = 1, Shutdown = 2 }
export type ErrorHandlerResult = { action: ErrorAction; message?: string; handled?: boolean };

export declare enum CloseAction { DoNotRestart = 1, Restart = 2 }
export type CloseHandlerResult = { action: CloseAction; message?: string; handled?: boolean };

export interface ErrorHandler {
  error(error: Error, message: Message | undefined, count: number | undefined): ErrorHandlerResult | Promise<ErrorHandlerResult>;
  closed(): CloseHandlerResult | Promise<CloseHandlerResult>;
}
```
`handled: true` suppresses the client's own popup for that event (see below); `handled` unset/`false` lets the client show its default one-button ("Go to output") notification via `window.showErrorMessage`/`showWarningMessage`.

**The built-in default handler already matches spec 7.4's "5 crashes in 3 minutes" exactly.** `client.js:173-202` plus `809-813`:
```js
class DefaultErrorHandler {
  error(_error, _message, count) { return count && count <= 3 ? { action: ErrorAction.Continue } : { action: ErrorAction.Shutdown }; }
  closed() {
    this.restarts.push(Date.now());
    if (this.restarts.length <= this.maxRestartCount) return { action: CloseAction.Restart };
    const diff = this.restarts.at(-1) - this.restarts[0];
    if (diff <= 3 * 60 * 1000) return { action: CloseAction.DoNotRestart, message: `The ${name} server crashed ${maxRestartCount + 1} times in the last 3 minutes...` };
    this.restarts.shift();
    return { action: CloseAction.Restart };
  }
}
createDefaultErrorHandler(maxRestartCount) { return new DefaultErrorHandler(this, maxRestartCount ?? 4); }
```
With the default `maxRestartCount = 4`, the handler restarts on each of the first 5 crashes-within-3-minutes and gives up on the 5th within that window — precisely spec 7.4's policy. `client.createDefaultErrorHandler()` (no args) can be used as-is for the counting logic.

**Gotcha for the spec's exact UX ("shows a notification with Restart and Show Log").** The library's own give-up notification is hardcoded to a single "Go to output" button:
```js
// client.js showNotificationMessage()
void window.showErrorMessage(message, 'Go to output').then(sel => { if (sel !== undefined) this.outputChannel.show(true); });
```
(reached via `handleConnectionClosed()`'s `this.error(handlerResult.message ?? ..., undefined, handlerResult.handled === true ? false : 'force')`, `client.js:1441-1478`.) To get clippings' own "Restart"/"Show Log" buttons, the extension needs a thin custom `ErrorHandler` whose `closed()` reuses the same restart-counting logic but returns `{ action: CloseAction.DoNotRestart, handled: true }` on give-up (suppressing the built-in popup), then shows its own `window.showWarningMessage(msg, 'Restart', 'Show Log')`.

**`outputChannel`, `traceOutputChannel`, `trace.server` convention.** `LanguageClientOptions.outputChannel?: LogOutputChannel`, `outputChannelName?: string`, `traceOutputChannel?: LogOutputChannel` (`client.d.ts:251-253`). If `outputChannel` is omitted, the client lazily creates one via `window.createOutputChannel(name, { log: true })` on first access (`client.js:531-536`); if `traceOutputChannel` is omitted it falls back to the same `outputChannel` getter (`client.js:540-542`).

The `trace.server` setting is **consumed automatically** by the client, keyed off the client's `id` (the first constructor argument):
```js
// client.js refreshTrace(), :1508-1531
const config = workspace.getConfiguration(this._id);
const traceConfig = config.get('trace.server', 'messages');
```
So constructing `new LanguageClient('clippings', 'Clippings', serverOptions, clientOptions)` automatically wires up `clippings.trace.server` with **no extra extension code**, as long as clippings declares that setting (`off`/`messages`/`verbose`) under the `clippings.*` namespace, matching spec 7.2's `trace.server` entry. **Gotcha:** trace is only actually emitted when *both* `trace.server !== off` *and* the trace output channel's `LogOutputChannel` level is `Trace` (`this._traceLogLevel !== LogLevel.Trace ? Off : Messages`, `client.js:1508-1509`) — the user must also raise the Clippings output channel's log level (via its gear icon → Set Log Level) for `trace.server` to have any visible effect. This is current 10.x behavior, not older versions' "the setting alone is enough."

**Typed `NotificationType`/`RequestType` and `onNotification`.**
```ts
sendRequest<P, R, E>(type: RequestType<P, R, E>, params: P, token?: CancellationToken): Promise<R>;
onRequest<P, R, E>(type: RequestType<P, R, E>, handler: RequestHandler<P, R, E>): Disposable;
sendNotification<P>(type: NotificationType<P>, params?: P): Promise<void>;
onNotification<P>(type: NotificationType<P>, handler: NotificationHandler<P>): Disposable;
```
(`client.d.ts:367-388`.) `NotificationType`/`RequestType` are re-exported from `vscode-languageserver-protocol` through `vscode-languageclient/node`, so `import { NotificationType, RequestType } from 'vscode-languageclient/node'` works directly. Example matching protocol.ts (spec 6.2):
```ts
interface StatusParams { instance: string; scanning: boolean; /* ... */ }
const StatusNotification = new NotificationType<StatusParams>('clippings/status');
client.onNotification(StatusNotification, params => { /* ... */ });

interface ChildrenParams { parent: string | null }
interface ChildrenResult { nodes: ViewNode[] }
const ChildrenRequest = new RequestType<ChildrenParams, ChildrenResult, void>('clippings/children');
const { nodes } = await client.sendRequest(ChildrenRequest, { parent: null }, token);
```

**Stop and restart.** `BaseLanguageClient.start(): Promise<void>` and `stop(timeout?: number): Promise<void>` (`client.d.ts:407,412`); the Node `LanguageClient` subclass adds `restart(): Promise<void>` (`node/main.d.ts:104`), used by spec 7.4's `clippings.restartServer` command and the "schemes/server.path/logLevel/trace.server changed → restart" rule.

**`client/registerCapability` for `workspace/didChangeWatchedFiles` — confirmed automatic.** `FileSystemWatcherFeature` is one of the client's built-in features, registered unconditionally in the constructor path:
```js
// client.js:1577-1591, registerBuiltinFeatures()
this.registerFeature(new FileSystemWatcherFeature(this, event => this.notifyFileEvent(event)));
```
So when the server sends `client/registerCapability` for `workspace/didChangeWatchedFiles` with a `RelativePattern`-style registration, `vscode-languageclient` creates the VS Code `FileSystemWatcher`(s) itself — no extension code is required, confirming spec 6.1/5.10's assumption.

**Does the client send `$/cancelRequest` when a `CancellationToken` fires?** Yes, confirmed transitively through `vscode-jsonrpc@9.0.2` (a dependency of `vscode-languageclient@10.1.1` via `vscode-languageserver-protocol`):
```js
// vscode-jsonrpc@9.0.2, lib/common/connection.js
var CancelNotification; (function (CancelNotification) { CancelNotification.type = new NotificationType('$/cancelRequest'); })(...)
CancellationSenderStrategy.Message = { sendCancellation(conn, id) { return conn.sendNotification(CancelNotification.type, { id }); }, cleanup(_) {} }
// sendRequest(): disposable = token.onCancellationRequested(() => { ... }) // connection.js:1107
```
`client.sendRequest(type, params, token)` passes the token straight through to the underlying JSON-RPC connection, which hooks `token.onCancellationRequested` and sends `$/cancelRequest` via the default `CancellationSenderStrategy` unless overridden by `clientOptions.connectionOptions.cancellationStrategy`.

---

## 2. `@vscode/test-electron` and `@vscode/test-cli`

**Versions (2026-09-23, `npm view <pkg> version`):** `@vscode/test-electron@3.1.0`, `@vscode/test-cli@0.0.15`. Both declare `"engines": { "node": ">=22" }` — matches the locally installed Node 22.18.0. `@vscode/test-cli` is ESM (`"type": "module"`, `main: "out/index.cjs"` dual-published, `bin: { "vscode-test": "out/bin.mjs" }`); `@vscode/test-electron` is CJS (no `type` field, `main: "./out/index.js"`).

**Current recommended setup.** The official docs (code.visualstudio.com/api/working-with-extensions/testing-extension, fetched 2026-09-23) present `@vscode/test-cli` as the primary path: `npm install --save-dev @vscode/test-cli @vscode/test-electron`, and describe `@vscode/test-electron`'s `runTests` as the "advanced setup" for custom needs. `@vscode/test-cli` depends on `@vscode/test-electron` for the actual VS Code download/launch and on `mocha ^11.7.6` internally, and exposes the `vscode-test` CLI, which is a Mocha-like runner.

**`.vscode-test.mjs` config**, from `@vscode/test-cli@0.0.15`'s own README and `out/config.d.cts` (both read from the unpacked tarball):
```js
// .vscode-test.mjs
import { defineConfig } from '@vscode/test-cli';

export default defineConfig({
  files: 'out/test/**/*.test.js',
  version: 'stable',                          // or 'insiders' / a pinned version string
  extensionDevelopmentPath: import.meta.dirname,
  workspaceFolder: `${import.meta.dirname}/../tests/fixtures/workspace`,
  launchArgs: ['--disable-extensions'],
  mocha: { timeout: 20000, ui: 'bdd' },
});
```
Full `IDesktopTestConfiguration` fields (verified from `config.d.cts`): `files`, `version`, `extensionDevelopmentPath`, `workspaceFolder`, `mocha` (`Mocha.MochaOptions` + `preload`/`reporter`), `label`, `srcDir`, `platform: 'desktop'`, `desktopPlatform`, `launchArgs: string[]`, `env`, `useInstallation: { fromMachine: boolean } | { fromPath?: string }`, `download: { reporter, timeout }`, `installExtensions: string[]`, `skipExtensionDependencies`.

**`launchArgs` like `--disable-extensions`.** Confirmed in both `config.d.cts`'s doc comment ("A list of launch arguments passed to VS Code executable, in addition to `--extensionDevelopmentPath` and `--extensionTestsPath`... See `code --help` for possible arguments") and `@vscode/test-electron@3.1.0`'s own README usage sample:
```ts
await runTests({
  vscodeExecutablePath, extensionDevelopmentPath, extensionTestsPath,
  launchArgs: [testWorkspace, '--disable-extensions'], // disables all extensions except the one under test
  extensionTestsEnv: { foo: 'bar' },
});
```

**Headless on macOS locally, and Linux CI with xvfb.** macOS needs nothing special — the real window server is present even on CI runners, so `vscode-test`/`runTests` launches an ordinary (visible or backgrounded) VS Code window and no virtual framebuffer is required. Linux needs a virtual X display because Electron/VS Code cannot start without one:
- Azure Pipelines recipe, taken directly from `@vscode/test-electron@3.1.0`'s own shipped `pipeline.yml` (its own CI, unpacked from the npm tarball):
  ```yaml
  - bash: |
      /usr/bin/Xvfb :99 -screen 0 1024x768x24 > /dev/null 2>&1 &
      echo ">>> Started xvfb"
    displayName: Start xvfb
    condition: eq(variables['Agent.OS'], 'Linux')
  - script: cd sample && npm run test
    env:
      DISPLAY: ':99.0'
  ```
- GitHub Actions equivalent, per `code.visualstudio.com/api/working-with-extensions/continuous-integration` (fetched via WebFetch's summarizer, so treat as slightly lower-confidence than the pipeline.yml above, but it is the officially documented idiom and consistent with widespread practice):
  ```yaml
  - run: xvfb-run -a npm test
    if: runner.os == 'Linux'
  - run: npm test
    if: runner.os != 'Linux'
  ```
  `xvfb-run` is the wrapper script from the Debian/Ubuntu `xvfb` package (`apt-get install -y xvfb`), which starts a throwaway `Xvfb` server, sets `DISPLAY`, runs the given command, and tears the display down afterward — a shorthand for the manual `Xvfb :99 ... & DISPLAY=:99.0 ...` dance shown in the Azure Pipelines recipe.

**How tests get at extension internals (activate() return-value hook).** Standard VS Code mechanism, confirmed from `@types/vscode@1.91.0`'s `index.d.ts`:
```ts
export interface Extension<T> {
  readonly exports: T;
  activate(): Thenable<T>;
}
export function getExtension<T = any>(extensionId: string): Extension<T> | undefined; // extensions.getExtension
```
(`index.d.ts:7620-7668`, `:16448`.) The extension's `activate(context)` returns a plain object (its "public API"); a test does:
```ts
const ext = vscode.extensions.getExtension('clippings-dev.clippings')!;
const api = await ext.activate(); // or ext.exports if already active
const decorations = api.getAppliedDecorationsForTest(uri); // whatever test hook clippings' activate() exposes
```
This matches spec 12.5 ("decorations through a test hook").

---

## 3. esbuild

**Version.** `esbuild@0.28.2` (`npm view esbuild version`), `"engines": { "node": ">=18" }`.

**`BuildOptions` fields**, verified against the unpacked `esbuild@0.28.2` package's `lib/main.d.ts` (`entry-points`, `bundle`, `external`, `platform`, `format`, `target`, `sourcemap`, `minify`, `outfile` all present as documented, each linking to `esbuild.github.io/api/#<name>`):
```ts
interface BuildOptions extends CommonOptions {
  bundle?: boolean;
  outfile?: string;
  external?: string[];
  entryPoints?: (string | { in: string; out: string })[] | Record<string, string>;
}
interface CommonOptions {
  sourcemap?: boolean | 'linked' | 'inline' | 'external' | 'both';
  format?: Format;      // 'iife' | 'cjs' | 'esm'
  target?: string | string[];
  platform?: Platform;  // 'browser' | 'node' | 'neutral'
  minify?: boolean;
}
```

**Minimal `build.mjs`** for a VS Code extension (entry, `external: ['vscode']`, platform node, format cjs, target matching `engines.vscode`'s Node runtime, sourcemap for dev):
```js
// extension/build.mjs
import * as esbuild from 'esbuild';

const watch = process.argv.includes('--watch');
const production = process.argv.includes('--production');

const ctx = await esbuild.context({
  entryPoints: ['src/extension.ts'],
  bundle: true,
  outfile: 'dist/extension.js',
  external: ['vscode'],
  platform: 'node',
  format: 'cjs',
  target: 'node22',          // matches the extension host's bundled Node (>=22 per engines.vscode ^1.91.0's Electron/Node)
  sourcemap: !production,
  minify: production,
});

if (watch) {
  await ctx.watch();
} else {
  await ctx.rebuild();
  await ctx.dispose();
}
```
`external: ['vscode']` is required because the `vscode` module is a virtual module injected by the extension host at runtime, not an npm package — esbuild must leave `require('vscode')` calls untouched rather than trying to resolve/bundle them.

**Type-checking note.** esbuild transpiles TypeScript but does not type-check it (documented at `esbuild.github.io/content-types/#typescript`); the standard companion step is `tsc --noEmit` as a separate script/CI job. TypeScript latest on npm (2026-09-23) is `typescript@7.0.2` (`"engines": { "node": ">=16.20.0" }`), used here purely for type-checking since esbuild owns the actual compile/bundle.

---

## 4. `@primer/octicons`

**Version.** `@primer/octicons@19.38.0` (`npm view @primer/octicons version`). Package is CJS (`main: index.js`), one runtime dependency: `object-assign@^4.1.1`.

**Icon set size.** The published `build/data.json` inside `@primer/octicons@19.38.0` has **397 keys total: 394 canonical icons + 3 aliases** (e.g. `play` aliases `triangle-circle`), counted directly from the unpacked tarball with Python/`json`.

**API to get an SVG string with a fill colour.** Verified from the package's own `index.js` (unpacked tarball) and its README:
```js
const octicons = require('@primer/octicons');
octicons['alert'].toSVG(options?: { width?: number; height?: number; class?: string; 'aria-label'?: string; [attr: string]: any })
```
`toSVG` picks the closest natural pixel size (`closestNaturalHeight`, default 16) for the requested `width`/`height`, merges the per-size default attributes (`version`, `width`, `height`, `viewBox`, `class: "octicon octicon-<name>"`, `aria-hidden`, `data-component`) with the caller's `options`, and serializes every resulting key as a literal HTML/SVG attribute on the root `<svg>` element, e.g. `<svg width="16" height="16" viewBox="0 0 16 16" ...>${path}</svg>`.

**`fill` is not documented in the README** (the README only documents `class`, `aria-label`, `width`, `height` as recognized options) but **works** because of the generic attribute pass-through: any extra key in the `options` object that `toSVG`'s `htmlAttributes()` helper doesn't specially handle is still merged into the attribute set and rendered onto the `<svg>` tag (`index.js`'s `attrObj = objectAssign({}, defaultOptions, options)` then `attributes.push(\`${option}="${attrObj[option]}"\`)` for every key). Since the icon's `<path>` elements carry no `fill` of their own, they inherit the root `<svg fill="...">`'s value per normal SVG presentation-attribute inheritance — so `octicons[name].toSVG({ fill, width, height })` does exactly what todo-tree relies on, even though it's an undocumented (but stable, source-confirmed) convention rather than a formally supported option.

**Confirmed against todo-tree's actual installed bundle** (`~/.vscode/extensions/gruntfuggly.todo-tree-0.0.226/dist/extension.js`, read directly — minified, one `toSVG` call site found):
```js
var w = n.join(e.globalStorageUri.fsPath, "todo-" + v + "-" + p + ".svg");
if (!i.existsSync(w)) {
  var b = '<?xml version="1.0" encoding="iso-8859-1"?>\n' + u[v].toSVG({ xmlns: "http://www.w3.org/2000/svg", fill: d });
  i.writeFileSync(w, b);
}
```
So todo-tree calls `octicons[iconName].toSVG({ xmlns: "http://www.w3.org/2000/svg", fill: colour })` (no explicit `width`/`height`, defaulting to the 16px drawing), prefixes an XML declaration, and writes the result to a deterministic path under `context.globalStorageUri` named `todo-<name>-<colour>.svg`, **only if that file doesn't already exist** — a cache-by-file-existence pattern. This matches spec 7.7 verbatim: "gutter icons use deterministic file paths, and a missing icon file is written synchronously the first time its name and colour appear." Clippings' extension-side icon renderer can reuse this exact call shape.

---

## 5. VS Code API facts (verified against `@types/vscode@1.91.0`, unpacked `index.d.ts`, 19,267 lines)

- **`TreeDataProvider<T>`** (`index.d.ts:11487-11520`): `onDidChangeTreeData?`, `getTreeItem(element): TreeItem | Thenable<TreeItem>`, `getChildren(element?): ProviderResult<T[]>`, and **`getParent?(element): ProviderResult<T>`**, whose doc comment states plainly: *"This method should be implemented in order to access `TreeView.reveal` API."* — required, exactly as spec 7.5 assumes.
- **`TreeItem.id`** (`index.d.ts:11402-11405`): *"Optional id for the tree item that has to be unique across tree. The id is used to preserve the selection and expansion state of the tree item. If not provided, an id is generated using the tree item's label. Note that when labels change, ids will change and that selection and expansion state cannot be kept stable anymore."* This directly validates clippings' design of explicit, stable, epoch-prefixed node IDs (spec 7.5) rather than relying on label-derived ids.
- **`TreeView<T>`** (`index.d.ts:11395-11476`): `onDidExpandElement: Event<TreeViewExpansionEvent<T>>`, `onDidCollapseElement: Event<TreeViewExpansionEvent<T>>`, `message?: string`, `title?: string`, `description?: string`, `badge?: ViewBadge | undefined`, `reveal(element, { select?, focus?, expand? }): Thenable<void>` (reveal's doc: *"NOTE: The TreeDataProvider ... must implement getParent method to access this API"*, and expand accepts a number up to 3 levels).
- **`ViewBadge`** (`index.d.ts:11369-11378`): `{ readonly tooltip: string; readonly value: number }`.
- **`TextEditorDecorationType` options mapping** — `DecorationRenderOptions extends ThemableDecorationRenderOptions` (`index.d.ts:953-1163`): `isWholeLine?: boolean` (default `false`), `rangeBehavior?: DecorationRangeBehavior` (default `OpenOpen`), `overviewRulerLane?: OverviewRulerLane`, plus per-theme `light`/`dark` overrides. `ThemableDecorationRenderOptions` carries `gutterIconPath?: string | Uri`, `gutterIconSize?: string`, `overviewRulerColor?: string | ThemeColor`, and the CSS-like set (`backgroundColor`, `border*`, `outline*`, `fontStyle`/`fontWeight`, `textDecoration`, `color`, `opacity`, `letterSpacing`, `before`/`after` attachment render options).
- **`StatusBarItem` alignment and priority** (`index.d.ts:7172-7260`, `10894-10904`): `enum StatusBarAlignment { Left = 1, Right = 2 }`; `window.createStatusBarItem(id?, alignment?, priority?)` or `createStatusBarItem(alignment?, priority?)`; `StatusBarItem.priority: number | undefined` — *"Higher value means the item should be shown more to the left."*
- **`workspace.registerTextDocumentContentProvider`** (`index.d.ts:1812-1834`, `:13421`): `registerTextDocumentContentProvider(scheme: string, provider: TextDocumentContentProvider): Disposable`, where `TextDocumentContentProvider` is `{ onDidChange?: Event<Uri>; provideTextDocumentContent(uri, token): ProviderResult<string> }` — matches spec 7.6's read-only `clippings-export` scheme document.
- **`commands.executeCommand('setContext', ...)`** — not a typed API surface at all; `setContext` is a built-in VS Code command invoked purely by string id through the generic `commands.executeCommand<T>(command: string, ...rest: any[]): Thenable<T>`. There is no dedicated `setContext` function in `vscode.d.ts`.
- **`workspace.getConfiguration().inspect()`** (`index.d.ts:6492-6526`): `inspect<T>(section): { key, defaultValue?, globalValue?, workspaceValue?, workspaceFolderValue?, defaultLanguageValue?, globalLanguageValue?, workspaceLanguageValue?, workspaceFolderLanguageValue?, languageIds? } | undefined` — exactly the shape needed for the todo-tree settings importer (spec 7.3) to detect explicit global/workspace values.
- **`ConfigurationTarget`** (`index.d.ts:6371-6386`): `enum ConfigurationTarget { Global = 1, Workspace = 2, WorkspaceFolder = 3 }`.
- **`ExtensionContext.workspaceState`/`globalState`/`globalStorageUri`** (`index.d.ts:7702-7814`): `workspaceState: Memento`; `globalState: Memento & { setKeysForSync(keys): void }`; `globalStorageUri: Uri` (the non-deprecated replacement for `globalStoragePath`).
- **`window.showWarningMessage` with buttons** (`index.d.ts:10582-10617`): overloads `showWarningMessage<T extends string>(message, ...items: T[]): Thenable<T | undefined>` and the `MessageOptions`/`MessageItem` variants — this is the mechanism for spec 7.4's crash notification and 7.3's Import/Not Now/Never prompt.
- **Restricted settings and workspace trust.** `capabilities.untrustedWorkspaces.restrictedConfigurations` is a `package.json` manifest key, not part of `vscode.d.ts`; confirmed both from a real shipped extension (`ruff-vscode`'s actual `package.json`, `scratchpad/ref2/ruff-vscode/package.json:41-51`):
  ```json
  "capabilities": {
    "untrustedWorkspaces": {
      "supported": "limited",
      "restrictedConfigurations": ["ruff.path", "ruff.importStrategy", "ruff.interpreter", "ruff.configuration"]
    }
  }
  ```
  and from VS Code core's own type for that manifest field (full-source clone under `scratchpad/ref/...(vs-code-speech...)/src/vs/platform/extensions/common/extensions.ts:299-301`):
  ```ts
  export type ExtensionUntrustedWorkspaceSupport =
    | { supported: true }
    | { supported: false; description: string }
    | { supported: 'limited'; description: string; restrictedConfigurations?: string[] };
  ```
  Note the internal type marks `description` required alongside `"limited"`, but the real ruff-vscode manifest above omits it and is a shipped, working extension — so `description` is effectively optional in practice even though core's stricter internal type wants it; include one anyway for Marketplace clarity. This matches spec 7.1 exactly: `"capabilities.untrustedWorkspaces": { "supported": "limited", "restrictedConfigurations": ["clippings.server.path"] }`.
  `workspace.isTrusted: boolean` and `workspace.onDidGrantWorkspaceTrust: Event<void>` are real typed APIs — `@types/vscode@1.91.0`, `index.d.ts:13687,13691`.

---

## 6. vsce packaging (local dev only, per spec 8.3)

**Version.** `@vscode/vsce@4.0.0` (`npm view @vscode/vsce version`), `"engines": { "node": ">= 22" }`.

**`.vscodeignore` whitelist pattern.** Verified from two real, currently-shipping Rust-backed extensions' actual `.vscodeignore` files (both still present in the survey's local clones):
```
# rust-analyzer, editors/code/.vscodeignore (scratchpad/ref2/rust-analyzer/editors/code/.vscodeignore)
**
!icon.png
!language-configuration.json
!LICENSE
!node_modules/@hpcc-js/wasm/dist/graphvizlib.wasm
!out/main.js
!package.json
!server
!README.md
```
```
# Harper, packages/vscode-plugin/.vscodeignore (scratchpad/ref2/harper/packages/vscode-plugin/.vscodeignore)
**/**
!build/extension.js
!package.json
!LICENSE
!bin
!icon.png
```
Both use gitignore-style **"exclude everything with `**`, then whitelist specific files/directories with `!`"** rather than an explicit denylist (taplo's `.vscodeignore`, by contrast, is a denylist — both styles work with vsce's `ignore`-package-based file collector, `out/package.js:1254-1372` in the unpacked `@vscode/vsce@4.0.0`, which prepends a small `defaultIgnore` list and layers the file's patterns on top). For clippings, the whitelist form matching spec 8.2 ("`.vscodeignore` whitelists `dist/**` and `bin/**`") is:
```
**
!dist/**
!bin/**
!package.json
!LICENSE
!README.md
```

**`vsce package --no-dependencies`.** Confirmed source-level from the unpacked `@vscode/vsce@4.0.0` (`out/main.js:96-97`, `out/npm.js:150-190`): the `--dependencies`/`--no-dependencies` flag pair is described in vsce's own CLI help text as *"Enable/Disable dependency detection via npm or yarn"* — **there is no pnpm awareness anywhere in vsce's source** (`grep -rn pnpm out/*.js` inside the unpacked package returns nothing). Without `--no-dependencies`, vsce's `getDependencies()` either walks a Yarn tree (if `yarn.lock`/`.yarnrc`/`.yarnrc.yaml`/`.pnp.cjs`/`.yarn` is detected, `out/npm.js:170-179`) or shells out to `npm list` — neither understands pnpm's symlinked/content-addressable `node_modules` layout, so under a pnpm-managed `node_modules` this detection can misbehave (wrong or incomplete dependency subset, spurious errors). Passing `--no-dependencies` sets `dependencies === 'none'`, which short-circuits `getDependencies()` to just `[cwd]` — no `node_modules` walk at all (`out/npm.js:180-183`).

Since clippings bundles the extension with esbuild (`external: ['vscode']`, everything else inlined into `dist/extension.js`), the packaged VSIX never needs `node_modules` at runtime, so the fix is two-sided: `.vscodeignore` should never whitelist `node_modules/`, and `vsce package --no-dependencies --target <target>` is the correct, not-a-workaround invocation — it's simply telling vsce there is nothing to detect. This matches spec 8.2's `vsce package --no-dependencies --target <target>` exactly.

---

## 7. pnpm specifics

**Installed locally:** pnpm `10.27.0`, Node `22.18.0` (`pnpm --version`, `node --version`) — matches spec's "pnpm 10" and the `node >=22` floor shared by `@vscode/vsce`, `@vscode/test-electron`, and `@vscode/test-cli`.

**`packageManager` field.** Per pnpm's own current docs (Context7 `/websites/pnpm_io`, sourced from `pnpm.io/blog/releases/11.23` and `pnpm.io/package-managers`): Corepack reads only the top-level `"packageManager"` field in `package.json` and **requires an exact version, not a semver range** — `pnpm init` itself now pins the exact installed version rather than a `^`-range specifically because Corepack rejects a range there ("expected a semver version"). So clippings' `extension/package.json` (or a root `package.json`, if a workspace is used) should carry:
```json
"packageManager": "pnpm@10.27.0"
```
not `"pnpm@^10.0.0"`.

**Workspace or a single package in `extension/`.** pnpm reads workspace membership **only** from a root `pnpm-workspace.yaml`, never from a `workspaces` array in `package.json` (that's the npm/yarn convention) — per pnpm docs: *"pnpm reads the workspace from `pnpm-workspace.yaml`, not from the `workspaces` field of the root package.json. ... a root manifest that declares a non-empty `workspaces` array in a project with no `pnpm-workspace.yaml` gets a warning, because such an install silently links no project at all."* Given today's repo layout has one real Node package (`extension/`) and one optional-CI-only Node runner (`tests/oracle/`, spec 4 and 12.3) that doesn't share dependencies with it, the simpler choice is a **standalone `extension/package.json` with no root `pnpm-workspace.yaml`** — `cd extension && pnpm install`. A `pnpm-workspace.yaml` with:
```yaml
packages:
  - 'extension'
  - 'tests/oracle'
```
only pays off if a single root-level `pnpm install` / one shared lockfile across both is wanted later.

**The `vsce` + pnpm `--no-dependencies` gotcha** is the same fact as section 6 above, restated in pnpm terms: because vsce's dependency-detection code path only understands npm's or Yarn's `node_modules`/lockfile shape (never pnpm's), any `vsce package` invocation in this repo must pass `--no-dependencies` — omitting it risks vsce either erroring against pnpm's `node_modules` symlink tree or silently producing a wrong dependency subset. This is not conditional on how the package is structured (workspace or standalone); it applies either way.

---

## Pinned versions recommended (all current on npm as of 2026-09-23)

| Package | Version | Source |
|---|---|---|
| `vscode-languageclient` | `10.1.1` | `npm view vscode-languageclient version` |
| `@types/vscode` | `^1.91.0` | matches `engines.vscode` floor below; exists on npm |
| `engines.vscode` | `^1.91.0` | `vscode-languageclient@10.1.1` and `@10.0.0`'s own `engines.vscode` |
| `@vscode/test-electron` | `3.1.0` | `npm view @vscode/test-electron version` |
| `@vscode/test-cli` | `0.0.15` | `npm view @vscode/test-cli version` |
| `mocha` | `12.0.2` (latest); `@vscode/test-cli@0.0.15` itself pins `mocha@^11.7.6` internally | `npm view mocha version`; `@vscode/test-cli` `package.json` dependencies |
| `esbuild` | `0.28.2` | `npm view esbuild version` |
| `typescript` | `7.0.2` | `npm view typescript version` |
| `@primer/octicons` | `19.38.0` | `npm view @primer/octicons version` |
| `@vscode/vsce` | `4.0.0` | `npm view @vscode/vsce version` |

Note on `mocha`: `@vscode/test-cli` bundles its own `mocha` dependency (`^11.7.6`) and drives it internally through the `vscode-test` CLI/`defineConfig({ mocha: {...} })`, so an extension using `@vscode/test-cli` does not need its own top-level `mocha` devDependency at all unless it wants direct control over the Mocha version (in which case pin to whatever satisfies `^11.7.6`, or accept the newer standalone `12.0.2` only if driving Mocha directly outside `@vscode/test-cli`).

## Unverified / flagged

- The GitHub Actions `xvfb-run -a npm test` snippet (section 2) was extracted via the WebFetch tool's summarizer from `code.visualstudio.com/api/working-with-extensions/continuous-integration`, not from raw HTML or a repository file — treat it as documented-but-not-independently-grepped, unlike the Azure Pipelines `Xvfb`/`DISPLAY` recipe immediately above it, which was read directly from `@vscode/test-electron@3.1.0`'s own shipped `pipeline.yml`.
- `@types/vscode`'s recommended pin (`^1.91.0`, matching `engines.vscode`) is a convention (matching the floor so TypeScript can't reference APIs newer than the declared minimum), not a hard requirement enforced by tooling — flagging in case the team prefers latest typings instead.
- `mocha@12.0.2` being "latest" is unusually high for the ecosystem's historical cadence; sanity-checked only via `npm view`, not cross-referenced against Mocha's own changelog/release notes for breaking changes since Mocha 10/11.
- TypeScript `7.0.2` (the native/Go-ported compiler line Microsoft previously previewed as `@typescript/native-preview`) was verified only for `version`/`engines` via `npm view`; this document does not verify whether `tsc --noEmit` CLI behavior, `tsconfig.json` option support, or output are unchanged from the classic 5.x line, since clippings only needs it for type-checking behind esbuild's bundling, not for emit.
