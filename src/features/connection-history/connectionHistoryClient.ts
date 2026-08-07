import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ConnectionHistoryEntry, ConnectionHistoryRecordInput } from "../../types";

const CONNECTION_HISTORY_UPDATED_EVENT = "connection-history://updated";

export const connectionHistoryClient = {
  list() {
    return invoke<ConnectionHistoryEntry[]>("connection_history_list");
  },

  async record(input: ConnectionHistoryRecordInput) {
    await invoke("connection_history_record", { input });
  },

  async delete(entryId: string) {
    await invoke("connection_history_delete", { entryId });
  },

  async clear() {
    await invoke("connection_history_clear");
  },

  listenUpdated(handler: () => void) {
    return listen(CONNECTION_HISTORY_UPDATED_EVENT, handler);
  },
};
