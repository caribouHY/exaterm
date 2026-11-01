import type { WorkspaceSnapshot } from "../../types";
import type { ManualLogControllerDependencies, ManualLogStatus } from "./manualLogController";

interface ManualLogOperationResult {
  status: ManualLogStatus;
  snapshot: WorkspaceSnapshot | null;
}

type Invoke = <T>(command: string, args: Record<string, unknown>) => Promise<T>;

export function createManualLogBackendClient(
  invoke: Invoke,
  applySnapshot: (snapshot: WorkspaceSnapshot) => void
): Pick<
  ManualLogControllerDependencies,
  "startBackend" | "stopBackend" | "getBackendStatus" | "setBackendPaused"
> {
  const operate = async (command: string, args: Record<string, unknown>) => {
    const result = await invoke<ManualLogOperationResult>(command, args);
    if (result.snapshot) applySnapshot(result.snapshot);
    return result.status;
  };
  return {
    startBackend: async (input) => {
      const status = await operate("logger_start_manual", input);
      if (!status.filePath) throw new Error("Log capture unavailable");
      return status.filePath;
    },
    stopBackend: async (sessionId) => {
      await operate("logger_stop_manual", { sessionId });
    },
    getBackendStatus: (sessionId) => invoke("logger_get_manual_status", { sessionId }),
    setBackendPaused: async (sessionId, paused) => {
      await operate("logger_set_manual_paused", { sessionId, paused });
    },
  };
}
