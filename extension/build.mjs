// Bundles the extension into dist/ and the tests into out/test/.
// esbuild only transpiles; `pnpm typecheck` runs the type checker.
import * as esbuild from 'esbuild';
import { readdirSync, rmSync, statSync } from 'node:fs';
import { join } from 'node:path';

const watch = process.argv.includes('--watch');
const production = process.argv.includes('--production');

function testEntries(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return testEntries(path);
    return name.endsWith('.test.ts') || name === 'index.ts' ? [path] : [];
  });
}

const common = {
  bundle: true,
  platform: 'node',
  format: 'cjs',
  target: 'node20',
  external: ['vscode', 'mocha'],
  sourcemap: !production,
  minify: production,
  logLevel: 'info',
};

rmSync('out', { recursive: true, force: true });
const contexts = [
  await esbuild.context({ ...common, entryPoints: ['src/extension.ts'], outfile: 'dist/extension.js' }),
];
if (!production) {
  contexts.push(
    await esbuild.context({
      ...common,
      entryPoints: testEntries('src/test'),
      outdir: 'out/test',
      outbase: 'src/test',
    }),
  );
}

if (watch) {
  await Promise.all(contexts.map((c) => c.watch()));
} else {
  await Promise.all(contexts.map((c) => c.rebuild()));
  await Promise.all(contexts.map((c) => c.dispose()));
}
