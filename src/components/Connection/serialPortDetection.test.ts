import { describe, expect, it, vi } from "vitest";
import type { PortInfo } from "../../types";
import { validateSerialConnectionForm } from "./connectionFormValidation";
import { createSerialPortDetection } from "./serialPortDetection";

const com3 = { name: "COM3", port_type: "USB" };
const com4 = { name: "COM4", port_type: "USB" };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe("Serial port detection", () => {
  it("keeps a listed selection and clears it when the port disappears", async () => {
    const listPorts = vi
      .fn<() => Promise<PortInfo[]>>()
      .mockResolvedValueOnce([com3, com4])
      .mockResolvedValueOnce([com4, com3])
      .mockResolvedValueOnce([com3])
      .mockResolvedValueOnce([com3, com4]);
    const detection = createSerialPortDetection(listPorts);
    const canConnect = () => validateSerialConnectionForm(detection.getSnapshot()).isValid;

    await detection.refresh();
    expect(detection.getSnapshot().selectedPort).toBe("COM3");
    detection.select("COM4");
    expect(listPorts).toHaveBeenCalledTimes(1);

    const pending = detection.refresh();
    expect(canConnect()).toBe(false);
    await pending;
    expect(detection.getSnapshot().selectedPort).toBe("COM4");
    expect(canConnect()).toBe(true);

    await detection.refresh();
    expect(detection.getSnapshot().selectedPort).toBe("");
    detection.select("COM4");
    expect(canConnect()).toBe(false);

    await detection.refresh();
    expect(detection.getSnapshot().selectedPort).toBe("");
    detection.select("COM3");
    expect(canConnect()).toBe(true);
  });

  it("distinguishes failure from an empty list and retains a choice until it can verify it", async () => {
    const listPorts = vi
      .fn<() => Promise<PortInfo[]>>()
      .mockResolvedValueOnce([com3])
      .mockRejectedValueOnce(new Error("enumeration failed"))
      .mockResolvedValueOnce([com3])
      .mockResolvedValueOnce([]);
    const detection = createSerialPortDetection(listPorts);
    await detection.refresh();
    await detection.refresh();
    expect(detection.getSnapshot()).toEqual({ status: "failed", ports: [], selectedPort: "" });
    await detection.refresh();
    expect(detection.getSnapshot().selectedPort).toBe("COM3");
    await detection.refresh();
    expect(detection.getSnapshot()).toEqual({ status: "ready", ports: [], selectedPort: "" });
  });

  it("ignores an older success or failure that arrives after re-detection", async () => {
    const oldSuccess = deferred<PortInfo[]>();
    const oldFailure = deferred<PortInfo[]>();
    const listPorts = vi
      .fn<() => Promise<PortInfo[]>>()
      .mockResolvedValueOnce([com4])
      .mockReturnValueOnce(oldSuccess.promise)
      .mockReturnValueOnce(oldFailure.promise)
      .mockResolvedValueOnce([com3]);
    const detection = createSerialPortDetection(listPorts);
    await detection.refresh();
    const first = detection.refresh();
    const second = detection.refresh();
    await detection.refresh();

    oldSuccess.resolve([com4]);
    oldFailure.reject(new Error("old failure"));
    await Promise.all([first, second]);
    expect(detection.getSnapshot()).toEqual({
      status: "ready",
      ports: [com3],
      selectedPort: "",
    });
  });

  it("ignores a request invalidated when leaving Serial", async () => {
    const response = deferred<PortInfo[]>();
    const detection = createSerialPortDetection(() => response.promise);
    const pending = detection.refresh();
    const snapshot = detection.getSnapshot();
    detection.invalidatePendingRefresh();
    response.resolve([com3]);
    await pending;
    expect(detection.getSnapshot()).toBe(snapshot);
  });
});
