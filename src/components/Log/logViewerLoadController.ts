import type { LogSession } from "../../types";

type LoadPhase = "loading" | "success" | "error";

export interface LogViewerLoadState {
  sessions: { phase: LoadPhase; data: LogSession[] | null };
  logDir: { phase: LoadPhase; data: string | null };
}

export function createLogViewerLoadState(): LogViewerLoadState {
  return {
    sessions: { phase: "loading", data: null },
    logDir: { phase: "loading", data: null },
  };
}

export function getLogViewerPresentation(state: LogViewerLoadState, isDeleting: boolean) {
  const sessions = state.sessions.data ?? [];
  const isLoading = state.sessions.phase === "loading" || state.logDir.phase === "loading";
  return {
    sessions,
    showEmpty: state.sessions.phase === "success" && sessions.length === 0,
    showTable: sessions.length > 0,
    sessionsErrorKey:
      state.sessions.phase !== "error"
        ? null
        : state.sessions.data === null
          ? "logs.load_failed"
          : "logs.reload_failed",
    logDirErrorKey:
      state.logDir.phase !== "error"
        ? null
        : state.logDir.data === null
          ? "logs.directory_load_failed"
          : "logs.directory_reload_failed",
    refreshDisabled: isLoading || isDeleting,
    deleteDisabled: isDeleting || state.sessions.phase !== "success" || sessions.length === 0,
    refreshLabelKey:
      state.sessions.phase === "error" || state.logDir.phase === "error"
        ? "logs.retry"
        : "logs.refresh",
  };
}

export function createLogViewerLoadController(
  source: {
    getSessions: () => Promise<LogSession[]>;
    getLogDir: () => Promise<string>;
  },
  onChange: (state: LogViewerLoadState) => void
) {
  let state = createLogViewerLoadState();
  let requestId = 0;
  let disposed = false;

  async function loadResource<T>(
    read: () => Promise<T>,
    update: (result: { phase: "success"; data: T } | { phase: "error" }) => void,
    id: number
  ) {
    let result: { phase: "success"; data: T } | { phase: "error" };
    try {
      const data = await read();
      result = { phase: "success", data };
    } catch {
      result = { phase: "error" };
    }
    if (disposed || id !== requestId) return;
    update(result);
    onChange(state);
  }

  return {
    getState: () => state,
    async load() {
      if (disposed) return;
      const id = ++requestId;
      state = {
        sessions: { ...state.sessions, phase: "loading" },
        logDir: { ...state.logDir, phase: "loading" },
      };
      onChange(state);
      await Promise.all([
        loadResource(
          source.getSessions,
          (result) => {
            state = { ...state, sessions: { ...state.sessions, ...result } };
          },
          id
        ),
        loadResource(
          source.getLogDir,
          (result) => {
            state = { ...state, logDir: { ...state.logDir, ...result } };
          },
          id
        ),
      ]);
    },
    dispose() {
      disposed = true;
      requestId++;
    },
  };
}
