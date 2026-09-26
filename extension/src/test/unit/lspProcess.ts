// A minimal JSON-RPC client over a spawned `clippings lsp`, for tests that
// check the wire shapes without VS Code.

import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

export function serverPath(): string {
  const path =
    process.env['CLIPPINGS_SERVER_PATH'] ??
    resolve(__dirname, '../../../../target/debug/clippings' + (process.platform === 'win32' ? '.exe' : ''));
  if (!existsSync(path)) throw new Error(`no server binary at ${path}; run \`pnpm dev\``);
  return path;
}

export interface Incoming {
  id?: number | string;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: { message: string };
}

export class LspProcess {
  private readonly child: ChildProcessWithoutNullStreams;
  private buffer = Buffer.alloc(0);
  private nextId = 1;
  readonly received: Incoming[] = [];
  private waiters: (() => void)[] = [];

  constructor(path = serverPath()) {
    this.child = spawn(path, ['lsp'], { stdio: 'pipe' });
    this.child.stdout.on('data', (chunk: Buffer) => this.onData(chunk));
  }

  private onData(chunk: Buffer): void {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    for (;;) {
      const end = this.buffer.indexOf('\r\n\r\n');
      if (end < 0) return;
      const header = this.buffer.subarray(0, end).toString('ascii');
      const length = Number(/Content-Length: (\d+)/i.exec(header)?.[1]);
      if (this.buffer.length < end + 4 + length) return;
      const body = this.buffer.subarray(end + 4, end + 4 + length).toString('utf8');
      this.buffer = this.buffer.subarray(end + 4 + length);
      this.received.push(JSON.parse(body) as Incoming);
      for (const w of this.waiters.splice(0)) w();
    }
  }

  private write(message: object): void {
    const body = Buffer.from(JSON.stringify({ jsonrpc: '2.0', ...message }), 'utf8');
    this.child.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
    this.child.stdin.write(body);
  }

  notify(method: string, params: unknown): void {
    this.write({ method, params });
  }

  async request(method: string, params: unknown): Promise<unknown> {
    const id = this.nextId++;
    this.write({ id, method, params });
    const reply = await this.waitFor((m) => m.id === id && m.method === undefined);
    if (reply.error) throw new Error(reply.error.message);
    return reply.result;
  }

  /** Resolves with the first received message, past or future, that matches. */
  async waitFor(match: (m: Incoming) => boolean, timeoutMs = 10_000): Promise<Incoming> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const found = this.received.find(match);
      if (found) return found;
      if (Date.now() > deadline) throw new Error('timed out waiting for a server message');
      await new Promise<void>((resolveWait) => {
        this.waiters.push(resolveWait);
        setTimeout(resolveWait, 100);
      });
    }
  }

  async close(): Promise<void> {
    const exited = new Promise((r) => this.child.once('exit', r));
    try {
      await this.request('shutdown', null);
      this.notify('exit', null);
    } finally {
      this.child.stdin.end();
    }
    await exited;
  }
}
