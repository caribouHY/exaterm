import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SshDiagnosticEntry, SshDiagnosticEvent } from "./connectionDialogTypes";

const createRequestId = () => globalThis.crypto.randomUUID();

export const useSshDiagnostics = () => {
  const requestIdRef = useRef<string | null>(null);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const entryIdRef = useRef(0);
  const [logs, setLogs] = useState<SshDiagnosticEntry[]>([]);
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);

  const stop = useCallback(() => {
    requestIdRef.current = null;
    unlistenRef.current?.();
    unlistenRef.current = null;
  }, []);

  const start = useCallback(async () => {
    stop();
    const requestId = createRequestId();
    requestIdRef.current = requestId;
    entryIdRef.current = 0;
    setLogs([]);
    setCopied(false);

    const unlisten = await listen<SshDiagnosticEvent>(
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
    unlistenRef.current = unlisten;
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
    setExpanded,
    start,
    stop,
    copy,
    currentRequestId: () => requestIdRef.current,
  };
};
