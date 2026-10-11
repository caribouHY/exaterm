export interface ManualLogBufferWriter {
  append(data: string): Promise<void>;
  flush(data: string): Promise<void>;
}

export function createManualLogBufferWriter(
  write: (data: string) => Promise<void>
): ManualLogBufferWriter {
  let pending = "";
  let draining: Promise<void> | null = null;

  const drain = (): Promise<void> => {
    // Output may arrive after the loop exits but before its promise settles.
    if (draining) return draining.then(drain);
    if (!pending) return Promise.resolve();

    const operation = (async () => {
      while (pending) {
        const batch = pending;
        await write(batch);
        pending = pending.slice(batch.length);
      }
    })();
    const settled = operation.finally(() => {
      if (draining === settled) {
        draining = null;
      }
    });
    draining = settled;
    return settled;
  };

  const enqueue = (data: string) => {
    pending += data;
    return drain();
  };

  return {
    append: enqueue,
    flush: enqueue,
  };
}
