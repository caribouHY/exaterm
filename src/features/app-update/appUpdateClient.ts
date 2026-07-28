import { Channel, invoke } from "@tauri-apps/api/core";
import type { AppUpdateMetadata } from "./updateModel";

export interface AppUpdateCommandError {
  code: string;
  message: string;
  activeSessionCount?: number;
}

export type AppUpdateDownloadEvent =
  | { event: "Started"; data: { contentLength: number | null } }
  | { event: "Progress"; data: { chunkLength: number } }
  | { event: "Finished" };

function parseCommandError(error: unknown): AppUpdateCommandError | null {
  if (typeof error === "object" && error !== null && "code" in error) {
    return error as AppUpdateCommandError;
  }
  if (typeof error !== "string") return null;
  try {
    const parsed = JSON.parse(error) as AppUpdateCommandError;
    return typeof parsed.code === "string" ? parsed : null;
  } catch {
    return null;
  }
}

export const appUpdateClient = {
  check() {
    return invoke<AppUpdateMetadata | null>("app_update_check");
  },

  async install(allowActiveSessions: boolean, onEvent: (event: AppUpdateDownloadEvent) => void) {
    const channel = new Channel<AppUpdateDownloadEvent>();
    channel.onmessage = onEvent;
    try {
      await invoke("app_update_install", {
        allowActiveSessions,
        onEvent: channel,
      });
    } catch (error) {
      throw (
        parseCommandError(error) ?? {
          code: "install_failed",
          message: "Update installation failed.",
        }
      );
    }
  },
};
