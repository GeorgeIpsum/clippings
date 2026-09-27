# Clippings extension: follow-ups

Known issues left after plan 3's per-task and whole-branch reviews. None of them blocks normal use. Plans 4 and 5 should pick them up where they touch the same code.

## Check before a release

- Clicking a todo inside a notebook cell opens the cell. `tree/open.ts` relies on `vscode.open` resolving a `vscode-notebook-cell` URI. The tree and the decorations are tested, the click is not.
- The extension with no folder open (spec 11.2). There is no integration test for it.

## Server lifecycle (`server/connection.ts`, `crashPolicy.ts`)

- The fix depends on vscode-languageclient 10.1.1 internals: the `Client.error` and `Client.stop` overrides, and the `_onStart` catch. Recheck them when the library is upgraded.
- A crash can still be recorded after `stop()` or deactivate if it lands during retire's kill wait, which can fire the give-up notice after a stop. Check the abort signal before recording.
- The `Client.error` override also silences the library's forced "connection erroring" toast for a running server that sends bad messages; only the log keeps it. Make that choice explicit in `CrashPolicy`.
- An ordinary cancel logs an error-level "couldn't create connection" line.
- vscode-jsonrpc occasionally logs `ERR_STREAM_DESTROYED` when a server dies during `initialize`, about once in 20 runs. It cannot be caught outside the library.
- A server that hangs after it is running has no request watchdog. Restart Server recovers it.

## Configuration

- One wrong-typed setting makes the whole configuration fall back to the defaults (spec 6.3). The warning now names the setting. A per-group fallback would keep the user's other settings.
- The `clippings/configure` unit test (`an_unreadable_configuration_is_a_status_warning`) does not pin the field path in the warning.
- A configuration change that touches a restart setting and `tree.expanded` in one save skips the expansion reset.
- `CLIPPINGS_SERVER_PATH` is not trimmed or skipped when blank, unlike `server.path`.

## Tree and decorations

- `NodeCache.pruneMissing` does not order requests per parent. If VS Code ever answered two `getChildren` calls for one parent out of order, it could prune a live node.
- The expansion map keeps entries for nodes that no longer exist.
- The unknown-style-key warning and the reapply of a replaced key have no automated test, because the server cannot send those sequences.

## Commands and prompts

- The server-path Allow/Deny prompt in `server/locate.ts` calls `showWarningMessage` directly rather than going through `ui/prompts.ts`.
- The on-demand import's overwrite prompt can also appear on the activation-offer path if a `clippings.*` value is written while the offer is open.
- `revealInFile` does nothing on an unexpected URI argument, reachable only through a hand-typed `executeCommand`.
- The Open Settings call in the status controller's error notice is fire-and-forget, with no catch.
- Remove Filter with nothing selected does not push the configuration. It has no visible effect.

## Tests

- No test drives `TodoTreeImporter.run()` with a failing writer. Narrow its writer parameter to an interface and add a unit test.
- No test exports twice to the same URI while the first export is still open.
- The prompt-leak `afterEach` would stop running without a warning if `out/test/integration/index.js` stopped being built, because test-cli ignores a missing literal path.
- There are no tests for `CLIPPINGS_LOG=off`, for a trust grant restarting the server (runs use `--disable-workspace-trust`), or for the export close release (VS Code fires `onDidCloseTextDocument` lazily).
- `server.test.ts` "shows the log" only checks that the command does not throw.

## Code tidiness

- The five view-state flag keys are listed in both `config/configuration.ts` and `state/viewState.ts`.
- The Status relay repeats the session guard inline instead of using `relay()`.
- Several lines copied from the plan exceed 120 columns (`serverResolution.test.ts`, `ui/prompts.ts`, `context/keys.ts`, `icons/svg.ts` and the icon tests). There is no formatter to catch them.
