import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { StartupCliRequest } from "../../types";

const REQUEST_AVAILABLE_EVENT = "startup-cli://request-available";

export const startupCliClient = {
  takeNext(windowId: string) {
    return invoke<StartupCliRequest | null>("startup_cli_request_take", { windowId });
  },

  listenRequestAvailable(handler: () => void) {
    return listen(REQUEST_AVAILABLE_EVENT, handler);
  },
};
