export interface AppUpdateMetadata {
  version: string;
  currentVersion: string;
  notes?: string | null;
  publishedAt?: string | null;
}

export type AppUpdateCheckSource = "automatic" | "manual";

export type AppUpdateState =
  | { phase: "closed" }
  | { phase: "checking"; source: AppUpdateCheckSource }
  | { phase: "up_to_date" }
  | { phase: "available"; update: AppUpdateMetadata }
  | { phase: "confirm_active_sessions"; update: AppUpdateMetadata; activeSessionCount: number }
  | {
      phase: "downloading";
      update: AppUpdateMetadata;
      downloadedBytes: number;
      contentLength: number | null;
    }
  | { phase: "installing"; update: AppUpdateMetadata }
  | { phase: "error"; source: AppUpdateCheckSource | "install" };

export type AppUpdateAction =
  | { type: "check_started"; source: AppUpdateCheckSource }
  | {
      type: "check_succeeded";
      source: AppUpdateCheckSource;
      update: AppUpdateMetadata | null;
    }
  | { type: "check_failed"; source: AppUpdateCheckSource }
  | { type: "install_started"; update: AppUpdateMetadata }
  | { type: "active_sessions_required"; update: AppUpdateMetadata; count: number }
  | { type: "download_started"; contentLength: number | null }
  | { type: "download_progress"; chunkLength: number }
  | { type: "download_finished" }
  | { type: "install_failed" }
  | { type: "cancel_active_session_confirmation" }
  | { type: "close" };

export const INITIAL_APP_UPDATE_STATE: AppUpdateState = { phase: "closed" };

export function appUpdateReducer(state: AppUpdateState, action: AppUpdateAction): AppUpdateState {
  switch (action.type) {
    case "check_started":
      return action.source === "manual"
        ? { phase: "checking", source: action.source }
        : { phase: "closed" };
    case "check_succeeded":
      if (action.update) return { phase: "available", update: action.update };
      return action.source === "manual" ? { phase: "up_to_date" } : { phase: "closed" };
    case "check_failed":
      return action.source === "manual"
        ? { phase: "error", source: action.source }
        : { phase: "closed" };
    case "install_started":
      return {
        phase: "downloading",
        update: action.update,
        downloadedBytes: 0,
        contentLength: null,
      };
    case "active_sessions_required":
      return {
        phase: "confirm_active_sessions",
        update: action.update,
        activeSessionCount: action.count,
      };
    case "download_started":
      if (state.phase !== "downloading") return state;
      return { ...state, contentLength: action.contentLength };
    case "download_progress":
      if (state.phase !== "downloading") return state;
      return {
        ...state,
        downloadedBytes: state.downloadedBytes + action.chunkLength,
      };
    case "download_finished":
      if (state.phase !== "downloading") return state;
      return { phase: "installing", update: state.update };
    case "install_failed":
      return { phase: "error", source: "install" };
    case "cancel_active_session_confirmation":
      if (state.phase !== "confirm_active_sessions") return state;
      return { phase: "available", update: state.update };
    case "close":
      if (
        state.phase === "checking" ||
        state.phase === "downloading" ||
        state.phase === "installing"
      ) {
        return state;
      }
      return { phase: "closed" };
  }
}

export function shouldCheckForUpdatesOnStartup(
  windowId: string,
  checkOnStartup: boolean,
  alreadyChecked: boolean
): boolean {
  return windowId === "main" && checkOnStartup && !alreadyChecked;
}

export function updateDownloadPercent(
  downloadedBytes: number,
  contentLength: number | null
): number | null {
  if (!contentLength || contentLength <= 0) return null;
  return Math.min(100, Math.max(0, Math.round((downloadedBytes / contentLength) * 100)));
}
