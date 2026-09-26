// Crash handling (spec 7.4) for one language client. A crash of a running
// server counts in the connection's `CrashHistory`: under the limit the client
// restarts it, at the limit Clippings shows its own notice with Restart and
// Show Log. A server that exits before it is running is a start failure,
// which the connection handles, so the client never restarts it itself.

import {
  CloseAction,
  ErrorAction,
  type CloseHandlerResult,
  type ErrorHandler,
  type ErrorHandlerResult,
} from 'vscode-languageclient/node';
import { GIVE_UP_MESSAGE, type CrashHistory } from './crashHistory';

export interface CrashEvents {
  /** The server exited before the client reached the running state. */
  startFailed(): void;
  /** Crashes reached the limit; `message` says so. */
  gaveUp(message: string): void;
}

export class CrashPolicy implements ErrorHandler {
  private running = false;

  constructor(
    private readonly history: CrashHistory,
    private readonly events: CrashEvents,
  ) {}

  /** Follows the client's state: only a running server can crash. */
  setRunning(running: boolean): void {
    this.running = running;
  }

  /**
   * Always continues. Writes to a server that just died fail before the
   * connection closes; shutting down on them, as the language client's
   * default does after three, would stop the client without `closed`
   * ever deciding on a restart.
   */
  error(): ErrorHandlerResult {
    return { action: ErrorAction.Continue };
  }

  closed(): CloseHandlerResult {
    if (!this.running) {
      this.events.startFailed();
      return { action: CloseAction.DoNotRestart, handled: true };
    }
    this.running = false;
    if (this.history.record(Date.now()) === 'restart') return { action: CloseAction.Restart, handled: true };
    this.events.gaveUp(GIVE_UP_MESSAGE);
    return { action: CloseAction.DoNotRestart, handled: true };
  }
}
