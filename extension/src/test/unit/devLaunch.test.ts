import * as assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Local development (spec 8.3): F5 builds the debug server and points the
// extension at it.
const root = resolve(__dirname, '../../../..');
const read = (path: string) => JSON.parse(readFileSync(resolve(root, path), 'utf8')) as unknown;

interface Launch {
  configurations: {
    name: string;
    type: string;
    args: string[];
    env: Record<string, string>;
    windows: { env: Record<string, string> };
    preLaunchTask: string;
  }[];
}
interface Tasks {
  tasks: { label: string; command: string; options: { cwd: string } }[];
}

describe('local development', () => {
  it('runs the extension against the debug server with backtraces', () => {
    const launch = read('.vscode/launch.json') as Launch;
    const run = launch.configurations.find((c) => c.name === 'Run Extension');
    assert.ok(run);
    assert.equal(run.type, 'extensionHost');
    assert.deepEqual(run.args, ['--extensionDevelopmentPath=${workspaceFolder}/extension']);
    assert.equal(run.env['CLIPPINGS_SERVER_PATH'], '${workspaceFolder}/target/debug/clippings');
    assert.equal(run.env['RUST_BACKTRACE'], '1');
    assert.match(run.windows.env['CLIPPINGS_SERVER_PATH'] ?? '', /clippings\.exe$/);

    const tasks = read('.vscode/tasks.json') as Tasks;
    const build = tasks.tasks.find((t) => t.label === run.preLaunchTask);
    assert.ok(build, 'the pre-launch task exists');
    assert.equal(build.command, 'pnpm dev && pnpm build');
  });

  it('builds the debug server with pnpm dev', () => {
    const manifest = read('extension/package.json') as { scripts: Record<string, string> };
    assert.equal(manifest.scripts['dev'], 'cargo build --manifest-path ../Cargo.toml -p clippings');
  });
});
