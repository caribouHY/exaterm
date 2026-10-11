import { describe, expect, it, vi } from "vitest";
import { createManualLogBufferWriter } from "./manualLogBufferWriter";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe("manualLogBufferWriter", () => {
  it("drains a flush queued after the write loop exits but before settlement", async () => {
    const first = deferred<undefined>();
    const last = deferred<undefined>();
    const writes: string[] = [];
    const writer = createManualLogBufferWriter(async (data) => {
      writes.push(data);
      await (data === "first" ? first.promise : last.promise);
    });
    const append = writer.append("first");
    first.resolve(undefined);
    await Promise.resolve();
    await Promise.resolve();
    let flushed = false;
    const flush = writer.flush("last").then(() => {
      flushed = true;
    });
    await append;
    expect(writes).toEqual(["first", "last"]);
    expect(flushed).toBe(false);
    last.resolve(undefined);
    await flush;
    expect(flushed).toBe(true);
  });

  it("retains a failed flush and retries it before newer output", async () => {
    const writes: string[] = [];
    const write = vi
      .fn<(data: string) => Promise<void>>()
      .mockRejectedValueOnce(new Error("append failed"))
      .mockImplementation(async (data) => {
        writes.push(data);
      });
    const writer = createManualLogBufferWriter(write);

    await expect(writer.flush("partial")).rejects.toThrow("append failed");
    await writer.append("\nnewer");

    expect(writes).toEqual(["partial\nnewer"]);
  });

  it("includes output queued while a flush is in flight before resolving", async () => {
    const first = deferred<undefined>();
    const writes: string[] = [];
    const write = vi.fn(async (data: string) => {
      writes.push(data);
      if (writes.length === 1) await first.promise;
    });
    const writer = createManualLogBufferWriter(write);

    const flush = writer.flush("first");
    const append = writer.append("second");
    first.resolve(undefined);
    await Promise.all([flush, append]);

    expect(writes).toEqual(["first", "second"]);
  });
});
