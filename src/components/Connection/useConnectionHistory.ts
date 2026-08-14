import { useCallback, useEffect, useMemo, useState } from "react";
import type { ConnectionHistoryEntry } from "../../types";
import { connectionHistoryClient } from "../../features/connection-history/connectionHistoryClient";
import { historyEntriesForType } from "./connectionHistoryModel";

export function useConnectionHistory() {
  const [entries, setEntries] = useState<ConnectionHistoryEntry[]>([]);
  const [error, setError] = useState("");

  const load = useCallback(async () => {
    try {
      setEntries(await connectionHistoryClient.list());
      setError("");
    } catch {
      setError("load");
    }
  }, []);

  useEffect(() => {
    void load();
    const unlisten = connectionHistoryClient.listenUpdated(() => {
      void load();
    });
    return () => {
      void unlisten.then((dispose) => {
        dispose();
      });
    };
  }, [load]);

  const deleteEntry = useCallback(async (entryId: string) => {
    try {
      await connectionHistoryClient.delete(entryId);
      setEntries((current) => current.filter((entry) => entry.id !== entryId));
      setError("");
      return true;
    } catch {
      setError("delete");
      return false;
    }
  }, []);

  const sshEntries = useMemo(() => historyEntriesForType(entries, "ssh"), [entries]);
  const telnetEntries = useMemo(() => historyEntriesForType(entries, "telnet"), [entries]);

  return {
    sshEntries,
    telnetEntries,
    error,
    deleteEntry,
  };
}
