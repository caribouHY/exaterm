import type { PortInfo } from "../../types";

export type SerialPortDetectionStatus = "loading" | "ready" | "failed";

export function createSerialPortDetection(listPorts: () => Promise<PortInfo[]>) {
  const listeners = new Set<() => void>();
  let generation = 0;
  let selectionInitialized = false;
  let preferredPort = "";
  let snapshot: { status: SerialPortDetectionStatus; ports: PortInfo[]; selectedPort: string } = {
    status: "loading",
    ports: [],
    selectedPort: "",
  };

  const publish = (next: typeof snapshot) => {
    snapshot = next;
    for (const listener of listeners) listener();
  };

  return {
    getSnapshot: () => snapshot,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    invalidatePendingRefresh: () => {
      generation++;
    },
    refresh: async () => {
      const request = ++generation;
      publish({ status: "loading", ports: [], selectedPort: "" });
      try {
        const ports = await listPorts();
        if (request !== generation) return;

        // Keep the initial default without silently substituting another device after removal.
        if (!selectionInitialized && ports.length > 0) {
          preferredPort = ports[0].name;
          selectionInitialized = true;
        }
        if (!ports.some((port) => port.name === preferredPort)) preferredPort = "";
        publish({ status: "ready", ports, selectedPort: preferredPort });
      } catch {
        if (request !== generation) return;
        publish({ status: "failed", ports: [], selectedPort: "" });
      }
    },
    select: (name: string) => {
      if (!snapshot.ports.some((port) => port.name === name)) return;
      preferredPort = name;
      publish({ ...snapshot, selectedPort: name });
    },
  };
}
