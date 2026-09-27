// A root-level `afterEach` for every integration suite (spec 12.5).
//
// `build.mjs` bundles `index.ts` as its own entry point (alongside every
// `*.test.ts`), and `.vscode-test.mjs` lists the compiled output next to
// the test glob so Mocha loads it. Calling `afterEach` here, outside any
// `describe`, attaches it to Mocha's root suite, so it runs after every
// test in every file regardless of load order. (Mocha's newer root-hook
// -plugin mechanism -- exporting `mochaHooks` -- needs `--require`
// handling that `@vscode/test-cli`'s runner does not do; it silently
// no-ops a `mochaHooks` export from an ordinary spec file.)
//
// `Prompts` (spec 12.5) is one FIFO shared by the whole run: an answer a
// test queues with `script()` but no prompt consumes would otherwise sit
// there and be read by the next test that shows one, in any file. Fail
// the test that left it, and clear it so one leak can't cascade into
// later tests.

import * as assert from 'node:assert/strict';
import { getApi } from './helpers';

afterEach(async function () {
  const api = await getApi();
  const pending = api.test.prompts.pending();
  api.test.prompts.clearScript();
  assert.deepEqual(
    pending,
    [],
    `${this.currentTest?.fullTitle()} left scripted prompt answers unconsumed: ${JSON.stringify(pending)}`,
  );
});
