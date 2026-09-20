import type { ConnectionType } from "../../types";
import type { ManualLogController } from "./manualLogController";

export interface ExternalLogControlRequestPayload {
  request_id: string;
  session_id: string;
  connection_type: ConnectionType;
  target: string;
}

export interface ExternalLogControlResponse {
  requestId: string;
  filePath: string | null;
  error: string | null;
}

interface ExternalLogControlHandlersDependencies {
  controller: ManualLogController;
  waitForUiUpdate(): Promise<void>;
  submit(response: ExternalLogControlResponse): Promise<void>;
  errorMessage(error: unknown, action: "start" | "stop"): string;
  onSubmitError(error: unknown): void;
}

export interface ExternalLogControlHandlers {
  start(payload: ExternalLogControlRequestPayload): Promise<void>;
  stop(payload: ExternalLogControlRequestPayload): Promise<void>;
}

export function createExternalLogControlHandlers(
  dependencies: ExternalLogControlHandlersDependencies
): ExternalLogControlHandlers {
  const respond = async (response: ExternalLogControlResponse) => {
    try {
      await dependencies.submit(response);
    } catch (error) {
      dependencies.onSubmitError(error);
    }
  };

  return {
    start: async (payload) => {
      try {
        const result = await dependencies.controller.start(
          {
            sessionId: payload.session_id,
            connectionType: payload.connection_type,
            target: payload.target,
          },
          { filePath: null, writeMode: "overwrite" }
        );
        if (result.metadataChanged) {
          await dependencies.waitForUiUpdate();
        }
        await respond({
          requestId: payload.request_id,
          filePath: result.filePath,
          error: null,
        });
      } catch (error) {
        await respond({
          requestId: payload.request_id,
          filePath: null,
          error: dependencies.errorMessage(error, "start"),
        });
      }
    },
    stop: async (payload) => {
      try {
        const result = await dependencies.controller.stop({ sessionId: payload.session_id });
        if (result.metadataChanged) {
          await dependencies.waitForUiUpdate();
        }
        await respond({ requestId: payload.request_id, filePath: null, error: null });
      } catch (error) {
        await respond({
          requestId: payload.request_id,
          filePath: null,
          error: dependencies.errorMessage(error, "stop"),
        });
      }
    },
  };
}

type Unlisten = () => void;
type EventListener<T> = (event: { payload: T }) => void;
type Listen = <T>(event: string, listener: EventListener<T>) => Promise<Unlisten>;

export function subscribeExternalLogControl(
  listen: Listen,
  handlers: ExternalLogControlHandlers,
  onRegistrationError: (error: unknown) => void
): () => void {
  let acceptingEvents = true;
  const unlisteners: Unlisten[] = [];

  const register = (
    event: string,
    handler: (payload: ExternalLogControlRequestPayload) => Promise<void>
  ) => {
    void listen<ExternalLogControlRequestPayload>(event, (received) => {
      if (!acceptingEvents) return;
      void handler(received.payload);
    })
      .then((unlisten) => {
        if (acceptingEvents) {
          unlisteners.push(unlisten);
        } else {
          unlisten();
        }
      })
      .catch(onRegistrationError);
  };

  register("external-control://log-start-request", handlers.start);
  register("external-control://log-stop-request", handlers.stop);

  return () => {
    acceptingEvents = false;
    for (const unlisten of unlisteners.splice(0)) {
      unlisten();
    }
  };
}
