import type { ConnectionType, ManualLogWriteMode, TabInfo } from "../../types";
import type { WorkspaceTabMetadataPatch } from "../workspace-tabs/workspaceClient";

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
  isBackendActive(sessionId: string): Promise<boolean>;
  flush(tabId: string): Promise<void>;
  updateMetadata(tabId: string, patch: WorkspaceTabMetadataPatch): Promise<void>;
  onBusyChange?(busySessionIds: ReadonlySet<string>): void;
}

type PendingMetadata = { kind: "started"; filePath: string } | { kind: "stopped" };

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
  const pendingMetadata = new Map<string, PendingMetadata>();

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

  const updateMetadata = async (tabId: string, patch: WorkspaceTabMetadataPatch) => {
    try {
      await dependencies.updateMetadata(tabId, patch);
    } catch (error) {
      throw operationError("metadata", null, error);
    }
  };

  const commitStart = async (
    target: ManualLogStartTarget,
    options: ManualLogStartOptions
  ): Promise<ManualLogStartResult> => {
    let tab = resolveTab(target.sessionId, target.expectedTabId, true);
    const pending = pendingMetadata.get(target.sessionId);

    if (pending?.kind === "stopped") {
      await updateMetadata(tab.id, {
        isManualLogging: false,
        isManualLoggingPaused: false,
      });
      pendingMetadata.delete(target.sessionId);
      tab = resolveTab(target.sessionId, target.expectedTabId, true);
    }

    const pendingStart = pendingMetadata.get(target.sessionId);
    if (pendingStart?.kind === "started") {
      await updateMetadata(tab.id, {
        isManualLogging: true,
        isManualLoggingPaused: false,
        manualLogFilePath: pendingStart.filePath,
      });
      pendingMetadata.delete(target.sessionId);
      return {
        filePath: pendingStart.filePath,
        alreadyActive: true,
        metadataChanged: true,
      };
    }

    if (tab.isManualLogging && tab.manualLogFilePath) {
      if (tab.isManualLoggingPaused) {
        await updateMetadata(tab.id, { isManualLoggingPaused: false });
        return {
          filePath: tab.manualLogFilePath,
          alreadyActive: true,
          metadataChanged: true,
        };
      }
      return {
        filePath: tab.manualLogFilePath,
        alreadyActive: true,
        metadataChanged: false,
      };
    }

    let filePath: string;
    try {
      filePath = await dependencies.startBackend({
        sessionId: target.sessionId,
        connectionType: target.connectionType,
        target: target.target,
        filePath: options.filePath,
        writeMode: options.writeMode,
      });
    } catch (error) {
      throw operationError("backend", null, error);
    }

    pendingMetadata.set(target.sessionId, { kind: "started", filePath });
    await updateMetadata(tab.id, {
      isManualLogging: true,
      isManualLoggingPaused: false,
      manualLogFilePath: filePath,
    });
    pendingMetadata.delete(target.sessionId);
    return { filePath, alreadyActive: false, metadataChanged: true };
  };

  const beginStart = (target: ManualLogStartTarget): ManualLogStartLease => {
    const release = reserve(target.sessionId);
    try {
      resolveTab(target.sessionId, target.expectedTabId, true);
    } catch (error) {
      release();
      throw error;
    }

    let committed = false;
    return {
      commit: (options) => {
        if (committed) {
          return Promise.reject(operationError("busy", "operation_in_progress"));
        }
        committed = true;
        return commitStart(target, options);
      },
      release,
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
      const tab = resolveTab(target.sessionId, target.expectedTabId, false);
      const pending = pendingMetadata.get(target.sessionId);

      if (pending?.kind === "stopped") {
        await updateMetadata(tab.id, {
          isManualLogging: false,
          isManualLoggingPaused: false,
        });
        pendingMetadata.delete(target.sessionId);
        return { alreadyInactive: true, metadataChanged: true };
      }

      if (!tab.isManualLogging && pending?.kind !== "started") {
        // Metadata recovery is window-local; a moved tab may still have an active backend log.
        let active: boolean;
        try {
          active = await dependencies.isBackendActive(target.sessionId);
        } catch (error) {
          throw operationError("backend", null, error);
        }
        if (!active) return { alreadyInactive: true, metadataChanged: false };
        resolveTab(target.sessionId, target.expectedTabId, false);
      }

      if (tab.isManualLogging) {
        try {
          await dependencies.flush(tab.id);
        } catch (error) {
          throw operationError("flush", null, error);
        }
      }

      try {
        await dependencies.stopBackend(target.sessionId);
      } catch (error) {
        throw operationError("backend", null, error);
      }

      pendingMetadata.set(target.sessionId, { kind: "stopped" });
      await updateMetadata(tab.id, {
        isManualLogging: false,
        isManualLoggingPaused: false,
      });
      pendingMetadata.delete(target.sessionId);
      return { alreadyInactive: false, metadataChanged: true };
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
      const tab = resolveTab(target.sessionId, target.expectedTabId, true);
      if (!tab.isManualLogging) {
        throw operationError("session", "logging_not_active");
      }
      if (Boolean(tab.isManualLoggingPaused) === paused) {
        return { changed: false };
      }
      if (paused) {
        try {
          await dependencies.flush(tab.id);
        } catch (error) {
          throw operationError("flush", null, error);
        }
      }
      await updateMetadata(tab.id, { isManualLoggingPaused: paused });
      return { changed: true };
    } finally {
      release();
    }
  };

  return {
    beginStart,
    start,
    stop,
    setPaused,
    isBusy: (sessionId) => busySessionIds.has(sessionId),
  };
}
