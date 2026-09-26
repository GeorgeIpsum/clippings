// Expected views of tests/fixtures/workspace.

/** The default tree view, as `outline` prints it. */
export const DEFAULT_TREE = [
  '(Scan mode: workspace and open files)',
  'workspace',
  '  docs',
  '    plan.md',
  '      [ ] write the guide',
  '      [x] pick a name',
  '  lib',
  '    notes.rs',
  '      TODO first line of a long note',
  '  src',
  '    util',
  '      strings.py',
  '        TODO normalise unicode before comparing',
  '        BUG (bob) drops leading tabs too',
  '    app.ts',
  '      TODO (alice) wire up the router',
  '      FIXME handle the error path',
  '      HACK temporary shim until the router lands',
];
