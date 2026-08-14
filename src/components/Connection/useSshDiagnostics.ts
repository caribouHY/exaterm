import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  SshConnectionProgressUpdate,
  SshDiagnosticEntry,
  SshDiagnosticEvent,
} from "./connectionDialogTypes";
import type { SshConnectionProgressEvent } from "./sshConnectionAttemptModel";

const createRequestId = () => globalThis.crypto.randomUUID();

export const useSshDiagnostics = () => {
  const requestIdRef = useRef<string | null>(null);
  const unlistenRefs = useRef<UnlistenFn[]>([]);
  const listenerGenerationRef = useRef(0);
  const entryIdRef = useRef(0);
  const [logs, setLogs] = useState<SshDiagnosticEntry[]>([]);
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);
  const [progress, setProgress] = useState<SshConnectionProgressUpdate | null>(null);

  const stop = useCallback(() => {
    listenerGenerationRef.current += 1;
    requestIdRef.current = null;
    for (const unlisten of unlistenRefs.current) unlisten();
    unlistenRefs.current = [];
  }, []);

  const start = useCallback(async () => {
    stop();
    const listenerGeneration = listenerGenerationRef.current;
    const requestId = createRequestId();
    requestIdRef.current = requestId;
    entryIdRef.current = 0;
    setLogs([]);
    setCopied(false);
    setProgress(null);

    const unlistenDiagnostics = await listen<SshDiagnosticEvent>(
      `ssh://connect-diagnostic/${requestId}`,
      (event) => {
        const entryId = entryIdRef.current + 1;
        entryIdRef.current = entryId;
        setLogs((current) => [
          ...current,
          {
            id: entryId,
            level: event.payload.level,
            message: event.payload.message,
            time: new Date().toLocaleTimeString(),
          },
        ]);
      }
    );
    let unlistenProgress: UnlistenFn;
    try {
      unlistenProgress = await listen<SshConnectionProgressEvent>(
        `ssh://connect-progress/${requestId}`,
        (event) => {
          setProgress({ requestId, progress: event.payload });
        }
      );
    } catch (error) {
      unlistenDiagnostics();
      throw error;
    }
    if (listenerGeneration !== listenerGenerationRef.current) {
      unlistenDiagnostics();
      unlistenProgress();
      return requestId;
    }
    unlistenRefs.current = [unlistenDiagnostics, unlistenProgress];
    return requestId;
  }, [stop]);

  const copy = useCallback(async () => {
    if (logs.length === 0) return;
    const text = logs.map((entry) => `[${entry.time}] ${entry.level}: ${entry.message}`).join("\n");
    await navigator.clipboard.writeText(text);
    setCopied(true);
  }, [logs]);

  useEffect(() => stop, [stop]);

  return {
    logs,
    expanded,
    copied,
    progress,
    setExpanded,
    start,
    stop,
    copy,
    currentRequestId: () => requestIdRef.current,
  };
};
