import { useCallback, useEffect, useReducer, useRef } from "react";
import { appUpdateClient, type AppUpdateCommandError } from "./appUpdateClient";
import {
  INITIAL_APP_UPDATE_STATE,
  appUpdateReducer,
  shouldCheckForUpdatesOnStartup,
  type AppUpdateCheckSource,
  type AppUpdateMetadata,
} from "./updateModel";

interface UseAppUpdateOptions {
  windowId: string;
  checkOnStartup: boolean | null;
}

export function useAppUpdate({ windowId, checkOnStartup }: UseAppUpdateOptions) {
  const [state, dispatch] = useReducer(appUpdateReducer, INITIAL_APP_UPDATE_STATE);
  const startupPreferenceEvaluatedRef = useRef(false);

  const check = useCallback(async (source: AppUpdateCheckSource) => {
    dispatch({ type: "check_started", source });
    try {
      const update = await appUpdateClient.check();
      dispatch({ type: "check_succeeded", source, update });
    } catch {
      console.error("Application update check failed.");
      dispatch({ type: "check_failed", source });
    }
  }, []);

  useEffect(() => {
    if (checkOnStartup === null || startupPreferenceEvaluatedRef.current) return;
    startupPreferenceEvaluatedRef.current = true;
    if (!shouldCheckForUpdatesOnStartup(windowId, checkOnStartup, false)) {
      return;
    }
    void check("automatic");
  }, [check, checkOnStartup, windowId]);

  const install = useCallback(async (update: AppUpdateMetadata, allowActiveSessions: boolean) => {
    dispatch({ type: "install_started", update });
    try {
      await appUpdateClient.install(allowActiveSessions, (event) => {
        switch (event.event) {
          case "Started":
            dispatch({
              type: "download_started",
              contentLength: event.data.contentLength,
            });
            break;
          case "Progress":
            dispatch({
              type: "download_progress",
              chunkLength: event.data.chunkLength,
            });
            break;
          case "Finished":
            dispatch({ type: "download_finished" });
            break;
        }
      });
    } catch (error) {
      const commandError = error as AppUpdateCommandError;
      if (commandError.code === "active_sessions") {
        dispatch({
          type: "active_sessions_required",
          update,
          count: commandError.activeSessionCount ?? 1,
        });
        return;
      }
      console.error("Application update installation failed.");
      dispatch({ type: "install_failed" });
    }
  }, []);

  return {
    state,
    checkManually: () => void check("manual"),
    install: (update: AppUpdateMetadata) => void install(update, false),
    confirmInstallWithActiveSessions: (update: AppUpdateMetadata) => void install(update, true),
    cancelActiveSessionConfirmation: () => dispatch({ type: "cancel_active_session_confirmation" }),
    close: () => dispatch({ type: "close" }),
  };
}
