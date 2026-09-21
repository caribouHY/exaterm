import type { StartupCliRequest } from "../../types";

interface StartupCliRequestCoordinatorOptions {
  takeNext: () => Promise<StartupCliRequest | null>;
  onRequest: (request: StartupCliRequest) => void;
  onError: (error: unknown) => void;
}

export interface StartupCliRequestCoordinator {
  check(): void;
  setBlocked(blocked: boolean): void;
}

export function createStartupCliRequestCoordinator({
  takeNext,
  onRequest,
  onError,
}: StartupCliRequestCoordinatorOptions): StartupCliRequestCoordinator {
  let blocked = false;
  let checking = false;
  let checkPending = false;

  const check = () => {
    checkPending = true;
    if (blocked || checking) return;

    checkPending = false;
    checking = true;
    void takeNext()
      .then((request) => {
        if (!request) return;
        blocked = true;
        onRequest(request);
      })
      .catch(onError)
      .finally(() => {
        checking = false;
        if (checkPending && !blocked) check();
      });
  };

  return {
    check,
    setBlocked(nextBlocked) {
      blocked = nextBlocked;
      if (!blocked) check();
    },
  };
}
