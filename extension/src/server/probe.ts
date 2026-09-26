// `clippings probe` (spec 7.4): checks that a candidate binary runs and
// speaks this client's protocol version.

import { execFile } from 'node:child_process';
import { chmodSync, statSync } from 'node:fs';
import { PROTOCOL_VERSION } from '../protocol';

export const PROBE_TIMEOUT_MS = 15_000;

export interface ProbeInfo {
  version: string;
  target: string;
  protocolVersion: number;
}

/** A candidate that failed the probe, with a reason for the aggregated error. */
export class ProbeError extends Error {}

function parse(stdout: string): ProbeInfo {
  let value: unknown;
  try {
    value = JSON.parse(stdout);
  } catch {
    throw new ProbeError(`printed something other than JSON: ${JSON.stringify(stdout.slice(0, 80))}`);
  }
  const info = value as Partial<ProbeInfo> | null;
  if (
    !info ||
    typeof info.version !== 'string' ||
    typeof info.target !== 'string' ||
    typeof info.protocolVersion !== 'number'
  ) {
    throw new ProbeError(`printed unexpected JSON: ${stdout.trim().slice(0, 80)}`);
  }
  return { version: info.version, target: info.target, protocolVersion: info.protocolVersion };
}

export function probe(path: string, timeoutMs = PROBE_TIMEOUT_MS): Promise<ProbeInfo> {
  return new Promise((resolve, reject) => {
    execFile(path, ['probe'], { timeout: timeoutMs, windowsHide: true }, (error, stdout, stderr) => {
      if (error) {
        const e = error as NodeJS.ErrnoException & { killed?: boolean; code?: number | string };
        if (e.killed) return reject(new ProbeError(`timed out after ${timeoutMs / 1000} s`));
        if (typeof e.code === 'string') return reject(new ProbeError(`did not start (${e.code})`));
        const detail = stderr.trim().split('\n').pop() ?? '';
        return reject(new ProbeError(`exited with code ${e.code ?? 'unknown'}${detail ? `: ${detail}` : ''}`));
      }
      try {
        const info = parse(stdout);
        if (info.protocolVersion !== PROTOCOL_VERSION) {
          throw new ProbeError(
            `speaks protocol version ${info.protocolVersion}, but this extension needs ${PROTOCOL_VERSION}`,
          );
        }
        resolve(info);
      } catch (err) {
        reject(err);
      }
    });
  });
}

/** Makes a bundled binary executable if packaging lost the mode bits. */
export function ensureExecutable(path: string): void {
  if (process.platform === 'win32') return;
  const mode = statSync(path).mode;
  if ((mode & 0o111) === 0) chmodSync(path, mode | 0o755);
}
