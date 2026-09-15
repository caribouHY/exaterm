import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { SshConnectionProgressUpdate, SshDiagnosticEntry } from "./connectionDialogTypes";
import { createSshDiagnosticSubscription } from "./sshDiagnosticSubscription";

export const useSshDiagnostics = () => {
  const entryIdRef = useRef(0);
  const [logs, setLogs] = useState<SshDiagnosticEntry[]>([]);
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);
  const [progress, setProgress] = useState<SshConnectionProgressUpdate | null>(null);

  const [subscription] = useState(() =>
    createSshDiagnosticSubscription(
      listen,
      (event) => {
        const id = ++entryIdRef.current;
        setLogs((current) => [
          ...current,
          {
            id,
            level: event.level,
            message: event.message,
            time: new Date().toLocaleTimeString(),
          },
        ]);
      },
      (requestId, value) => {
        setProgress({ requestId, progress: value });
      }
    )
  );
  const stop = subscription.stop;
  const start = useCallback(
    (requestId: string) => {
      entryIdRef.current = 0;
      setLogs([]);
      setCopied(false);
      setProgress(null);
      return subscription.start(requestId);
    },
    [subscription]
  );

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
  };
};
