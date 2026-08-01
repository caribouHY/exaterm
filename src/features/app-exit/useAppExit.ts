import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface AppExitCommandError {
  code: string;
  message: string;
  activeSessionCount: number;
}

export function useAppExit() {
  const [activeSessionCount, setActiveSessionCount] = useState<number | null>(null);
  const [isExiting, setIsExiting] = useState(false);

  const exit = useCallback(async (allowActiveSessions: boolean) => {
    setIsExiting(true);
    try {
      await invoke("app_exit", { allowActiveSessions });
      setActiveSessionCount(null);
    } catch (error) {
      const commandError = error as Partial<AppExitCommandError>;
      if (
        !allowActiveSessions &&
        commandError.code === "active_sessions" &&
        typeof commandError.activeSessionCount === "number"
      ) {
        setActiveSessionCount(commandError.activeSessionCount);
      } else {
        console.error("Application exit failed.");
      }
    } finally {
      setIsExiting(false);
    }
  }, []);

  return {
    activeSessionCount,
    isExiting,
    requestExit: () => void exit(false),
    confirmExit: () => void exit(true),
    cancelExit: () => {
      setActiveSessionCount(null);
    },
  };
}
