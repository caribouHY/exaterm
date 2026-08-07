import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ConnectionHistoryEntry, ConnectionHistoryRecordInput } from "../../types";

const CONNECTION_HISTORY_UPDATED_EVENT = "connection-history://updated";

export const connectionHistoryClient = {
  list() {
    return invoke<ConnectionHistoryEntry[]>("connection_history_list");
  },

  record(input: ConnectionHistoryRecordInput) {
    return invoke<void>("connection_history_record", { input });
  },

  delete(entryId: string) {
    return invoke<void>("connection_history_delete", { entryId });
  },

  clear() {
    return invoke<void>("connection_history_clear");
  },

  listenUpdated(handler: () => void) {
    return listen(CONNECTION_HISTORY_UPDATED_EVENT, handler);
  },
};
