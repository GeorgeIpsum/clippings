// Tries each candidate in order and returns the first that passes the probe
// (spec 7.4), or fails with every candidate's reason.

import { describe, type Candidate } from './candidates';
import type { ProbeInfo } from './probe';

export interface Resolved {
  candidate: Candidate;
  info: ProbeInfo;
}

export interface CandidateFailure {
  candidate: Candidate;
  reason: string;
}

export class ResolutionError extends Error {
  constructor(readonly failures: CandidateFailure[]) {
    super(
      `Clippings could not start its server. ${failures
        .map((f) => `${describe(f.candidate)}: ${f.reason}`)
        .join('; ')}.`,
    );
  }
}

export interface ResolveDeps {
  probe(path: string): Promise<ProbeInfo>;
  /** Asks, or recalls, whether a workspace-set `server.path` may run. */
  approve(path: string): Promise<boolean>;
  log(message: string): void;
}

export async function resolveServer(list: Candidate[], deps: ResolveDeps): Promise<Resolved> {
  const failures: CandidateFailure[] = [];
  for (const candidate of list) {
    if (candidate.unavailable) {
      failures.push({ candidate, reason: candidate.unavailable });
      continue;
    }
    if (candidate.fromWorkspace && !(await deps.approve(candidate.path))) {
      failures.push({ candidate, reason: 'not allowed to run from workspace settings' });
      continue;
    }
    try {
      const info = await deps.probe(candidate.path);
      deps.log(`Using ${describe(candidate)}: clippings ${info.version} (${info.target})`);
      return { candidate, info };
    } catch (err) {
      const reason = err instanceof Error ? err.message : String(err);
      deps.log(`Skipping ${describe(candidate)}: ${reason}`);
      failures.push({ candidate, reason });
    }
  }
  throw new ResolutionError(failures);
}
