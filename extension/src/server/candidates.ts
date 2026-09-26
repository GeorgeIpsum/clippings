// Server resolution order (spec 7.4). Pure: the caller supplies the
// environment, the setting and the filesystem checks.

import { delimiter, join } from 'node:path';

export type CandidateSource = 'environment' | 'setting' | 'bundled' | 'path';

export interface Candidate {
  source: CandidateSource;
  path: string;
  /** For `setting`: the value came from workspace settings, so it needs the user's approval. */
  fromWorkspace?: boolean;
  /** Set when the candidate cannot even be tried, such as a missing bundled binary. */
  unavailable?: string;
}

export interface ServerPathSetting {
  value: string;
  fromWorkspace: boolean;
}

/** The values of `clippings.server.path` at each scope, as `inspect` reports them. */
export interface ServerPathValues {
  globalValue?: string | undefined;
  workspaceValue?: string | undefined;
  workspaceFolderValue?: string | undefined;
}

/**
 * The `server.path` value to try. A workspace value counts only in a trusted
 * workspace (spec 7.1): in an untrusted one it is ignored without a prompt,
 * and the user's own value applies.
 */
export function serverPathSetting(
  values: ServerPathValues | undefined,
  trusted: boolean,
): ServerPathSetting | undefined {
  const workspaceValue = values?.workspaceFolderValue ?? values?.workspaceValue;
  if (workspaceValue && trusted) return { value: workspaceValue, fromWorkspace: true };
  if (values?.globalValue) return { value: values.globalValue, fromWorkspace: false };
  return undefined;
}

export interface CandidateInputs {
  env: Readonly<Record<string, string | undefined>>;
  setting: ServerPathSetting | undefined;
  home: string;
  workspaceFolder: string | undefined;
  extensionPath: string;
  platform: NodeJS.Platform;
  /** Development and test builds also try `clippings` on PATH. */
  development: boolean;
  exists(path: string): boolean;
}

export function binaryName(platform: NodeJS.Platform): string {
  return platform === 'win32' ? 'clippings.exe' : 'clippings';
}

/** Expands a leading `~` and `${workspaceFolder}` in `server.path`. */
export function expandServerPath(value: string, home: string, workspaceFolder: string | undefined): string {
  let path = value.trim();
  if (path === '~' || path.startsWith('~/') || path.startsWith('~\\')) path = home + path.slice(1);
  return path.replaceAll('${workspaceFolder}', workspaceFolder ?? '');
}

/** The first `name` on PATH, like `which`. */
export function findOnPath(
  name: string,
  env: Readonly<Record<string, string | undefined>>,
  exists: (path: string) => boolean,
): string | undefined {
  const dirs = (env['PATH'] ?? env['Path'] ?? '').split(delimiter).filter((d) => d.length > 0);
  for (const dir of dirs) {
    const path = join(dir, name);
    if (exists(path)) return path;
  }
  return undefined;
}

export function candidates(inputs: CandidateInputs): Candidate[] {
  const list: Candidate[] = [];
  const fromEnv = inputs.env['CLIPPINGS_SERVER_PATH'];
  if (fromEnv) list.push({ source: 'environment', path: fromEnv });
  if (inputs.setting && inputs.setting.value.trim()) {
    list.push({
      source: 'setting',
      path: expandServerPath(inputs.setting.value, inputs.home, inputs.workspaceFolder),
      fromWorkspace: inputs.setting.fromWorkspace,
    });
  }
  const name = binaryName(inputs.platform);
  const bundled = join(inputs.extensionPath, 'bin', name);
  const marker = join(inputs.extensionPath, 'bin', 'platform.ok');
  list.push(
    inputs.exists(bundled) && inputs.exists(marker)
      ? { source: 'bundled', path: bundled }
      : { source: 'bundled', path: bundled, unavailable: 'not bundled in this package' },
  );
  if (inputs.development) {
    const onPath = findOnPath(name, inputs.env, inputs.exists);
    list.push(onPath ? { source: 'path', path: onPath } : { source: 'path', path: name, unavailable: 'not on PATH' });
  }
  return list;
}

export function describe(candidate: Candidate): string {
  switch (candidate.source) {
    case 'environment':
      return `CLIPPINGS_SERVER_PATH (${candidate.path})`;
    case 'setting':
      return `clippings.server.path (${candidate.path})`;
    case 'bundled':
      return `bundled server (${candidate.path})`;
    case 'path':
      return `${candidate.path} on PATH`;
  }
}
