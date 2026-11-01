import type { TabInfo } from "../../types";

export interface ConnectionLogState {
  isLogging: boolean;
  filePath: string | null;
  startFailed: boolean;
}

export async function startConnectionLog(
  enabled: boolean,
  start: () => Promise<string>
): Promise<ConnectionLogState> {
  if (!enabled) return { isLogging: false, filePath: null, startFailed: false };

  try {
    const filePath = await start();
    return { isLogging: true, filePath, startFailed: false };
  } catch {
    return { isLogging: false, filePath: null, startFailed: true };
  }
}

export function connectionLogTarget(tab: Pick<TabInfo, "title" | "connectionInfo">): string {
  const info = tab.connectionInfo;
  if (!info) return tab.title;
  const endpoint = `${info.host}:${info.port}`;
  return info.kind === "ssh" ? `${info.username}@${endpoint}` : endpoint;
}
