import type { SshDiagnosticEvent } from "./connectionDialogTypes";
import type { SshConnectionProgressEvent } from "./connectionProtocolEvents";

export type DiagnosticEventListener = <T>(
  event: string,
  handler: (event: { payload: T }) => void
) => Promise<() => void>;

export function createSshDiagnosticSubscription(
  listen: DiagnosticEventListener,
  onDiagnostic: (event: SshDiagnosticEvent) => void,
  onProgress: (requestId: string, progress: SshConnectionProgressEvent) => void
) {
  let generation = 0;
  let unlisteners: Array<() => void> = [];

  const stop = () => {
    generation += 1;
    const previous = unlisteners;
    unlisteners = [];
    for (const unlisten of previous) unlisten();
  };

  const start = async (requestId: string) => {
    stop();
    const current = generation;
    const subscribe = async <T>(event: string, receive: (payload: T) => void) => {
      const unlisten = await listen<T>(event, ({ payload }) => {
        if (generation === current) receive(payload);
      });
      if (generation !== current) {
        unlisten();
        return false;
      }
      unlisteners.push(unlisten);
      return true;
    };

    try {
      if (
        !(await subscribe<SshDiagnosticEvent>(
          `ssh://connect-diagnostic/${requestId}`,
          onDiagnostic
        ))
      ) {
        return requestId;
      }
      if (generation !== current) return requestId;
      await subscribe<SshConnectionProgressEvent>(
        `ssh://connect-progress/${requestId}`,
        (progress) => {
          onProgress(requestId, progress);
        }
      );
      return requestId;
    } catch (error) {
      // A failed obsolete subscription must not stop the replacement attempt.
      if (generation === current) stop();
      throw error;
    }
  };

  return { start, stop };
}
