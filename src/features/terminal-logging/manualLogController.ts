import type { ConnectionType, ManualLogWriteMode, TabInfo } from "../../types";

export type ManualLogOperationStage = "busy" | "session" | "flush" | "backend" | "metadata";

export type ManualLogOperationCode =
  | "operation_in_progress"
  | "session_not_found"
  | "session_changed"
  | "session_disconnected"
  | "logging_not_active";

export class ManualLogOperationError extends Error {
  constructor(
    public readonly stage: ManualLogOperationStage,
    public readonly code: ManualLogOperationCode | null,
    public readonly cause: unknown
  ) {
    super(code ?? `manual_log_${stage}_failed`);
    this.name = "ManualLogOperationError";
  }
}

export interface ManualLogStartTarget {
  sessionId: string;
  expectedTabId?: string;
  connectionType: ConnectionType;
  target: string;
}

export interface ManualLogStartOptions {
  filePath: string | null;
  writeMode: ManualLogWriteMode;
}

export interface ManualLogStartResult {
  filePath: string;
  alreadyActive: boolean;
  metadataChanged: boolean;
}

export interface ManualLogStopTarget {
  sessionId: string;
  expectedTabId?: string;
}

export interface ManualLogStopResult {
  alreadyInactive: boolean;
  metadataChanged: boolean;
}

export interface ManualLogPauseResult {
  changed: boolean;
}

export interface ManualLogStartLease {
  commit(options: ManualLogStartOptions): Promise<ManualLogStartResult>;
  release(): void;
}

export interface ManualLogControllerDependencies {
  getTabBySessionId(sessionId: string): TabInfo | null;
  startBackend(input: {
    sessionId: string;
    connectionType: ConnectionType;
    target: string;
    filePath: string | null;
    writeMode: ManualLogWriteMode;
  }): Promise<string>;
  stopBackend(sessionId: string): Promise<void>;
  getBackendStatus(sessionId: string): Promise<ManualLogStatus>;
  setBackendPaused(sessionId: string, paused: boolean): Promise<void>;
  onBusyChange?(busySessionIds: ReadonlySet<string>): void;
}

export interface ManualLogStatus {
  active: boolean;
  paused: boolean;
  filePath: string | null;
}

export interface ManualLogController {
  beginStart(target: ManualLogStartTarget): ManualLogStartLease;
  start(
    target: ManualLogStartTarget,
    options: ManualLogStartOptions
  ): Promise<ManualLogStartResult>;
  stop(target: ManualLogStopTarget): Promise<ManualLogStopResult>;
  setPaused(target: ManualLogStopTarget, paused: boolean): Promise<ManualLogPauseResult>;
  isBusy(sessionId: string): boolean;
}

function operationError(
  stage: ManualLogOperationStage,
  code: ManualLogOperationCode | null,
  cause: unknown = null
) {
  return new ManualLogOperationError(stage, code, cause);
}

export function manualLogOperationCause(error: unknown): unknown {
  return error instanceof ManualLogOperationError && error.cause ? error.cause : error;
}

export function createManualLogController(
  dependencies: ManualLogControllerDependencies
): ManualLogController {
  const busySessionIds = new Set<string>();
  const publishBusy = () => {
    dependencies.onBusyChange?.(new Set(busySessionIds));
  };

  const reserve = (sessionId: string) => {
    if (busySessionIds.has(sessionId)) {
      throw operationError("busy", "operation_in_progress");
    }
    busySessionIds.add(sessionId);
    publishBusy();

    let released = false;
    return () => {
      if (released) return;
      released = true;
      busySessionIds.delete(sessionId);
      publishBusy();
    };
  };

  const resolveTab = (
    sessionId: string,
    expectedTabId: string | undefined,
    requireConnected: boolean
  ) => {
    const tab = dependencies.getTabBySessionId(sessionId);
    if (!tab) {
      throw operationError("session", "session_not_found");
    }
    if (expectedTabId && tab.id !== expectedTabId) {
      throw operationError("session", "session_changed");
    }
    if (requireConnected && !tab.isConnected) {
      throw operationError("session", "session_disconnected");
    }
    return tab;
  };

  const backend = async <T>(operation: () => Promise<T>): Promise<T> => {
    try {
      return await operation();
    } catch (error) {
      throw operationError("backend", null, error);
    }
  };

  const commitStart = async (
    target: ManualLogStartTarget,
    options: ManualLogStartOptions
  ): Promise<ManualLogStartResult> => {
    resolveTab(target.sessionId, target.expectedTabId, true);
    const status = await backend(() => dependencies.getBackendStatus(target.sessionId));
    resolveTab(target.sessionId, target.expectedTabId, true);
    const filePath = await backend(() => dependencies.startBackend({ ...target, ...options }));
    return { filePath, alreadyActive: status.active, metadataChanged: true };
  };

  const beginStart = (target: ManualLogStartTarget): ManualLogStartLease => {
    const releaseReservation = reserve(target.sessionId);
    try {
      resolveTab(target.sessionId, target.expectedTabId, true);
    } catch (error) {
      releaseReservation();
      throw error;
    }
    let committed = false;
    let released = false;
    return {
      commit: (options) => {
        if (committed || released) {
          return Promise.reject(operationError("busy", "operation_in_progress"));
        }
        committed = true;
        return commitStart(target, options);
      },
      release: () => {
        released = true;
        releaseReservation();
      },
    };
  };

  const start = async (target: ManualLogStartTarget, options: ManualLogStartOptions) => {
    const lease = beginStart(target);
    try {
      return await lease.commit(options);
    } finally {
      lease.release();
    }
  };

  const stop = async (target: ManualLogStopTarget): Promise<ManualLogStopResult> => {
    const release = reserve(target.sessionId);
    try {
      resolveTab(target.sessionId, target.expectedTabId, false);
      const status = await backend(() => dependencies.getBackendStatus(target.sessionId));
      resolveTab(target.sessionId, target.expectedTabId, false);
      await backend(() => dependencies.stopBackend(target.sessionId));
      return { alreadyInactive: !status.active, metadataChanged: true };
    } finally {
      release();
    }
  };

  const setPaused = async (
    target: ManualLogStopTarget,
    paused: boolean
  ): Promise<ManualLogPauseResult> => {
    const release = reserve(target.sessionId);
    try {
      resolveTab(target.sessionId, target.expectedTabId, true);
      const status = await backend(() => dependencies.getBackendStatus(target.sessionId));
      resolveTab(target.sessionId, target.expectedTabId, true);
      if (!status.active) throw operationError("session", "logging_not_active");
      await backend(() => dependencies.setBackendPaused(target.sessionId, paused));
      return { changed: status.paused !== paused };
    } finally {
      release();
    }
  };

  return { beginStart, start, stop, setPaused, isBusy: (id) => busySessionIds.has(id) };
}
