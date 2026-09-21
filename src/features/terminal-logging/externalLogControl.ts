import type { ConnectionType, ManualLogWriteMode } from "../../types";
import { backendCommandErrorMessage } from "../backend-errors/backendCommandError";
import {
  manualLogOperationCause,
  ManualLogOperationError,
  type ManualLogController,
} from "./manualLogController";

export interface ExternalLogControlRequestPayload {
  request_id: string;
  session_id: string;
  connection_type: ConnectionType;
  target: string;
  file_path: string | null;
  write_mode: ManualLogWriteMode | null;
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
  onSubmitError(error: unknown): void;
}

export interface ExternalLogControlHandlers {
  start(payload: ExternalLogControlRequestPayload): Promise<void>;
  stop(payload: ExternalLogControlRequestPayload): Promise<void>;
  pause(payload: ExternalLogControlRequestPayload): Promise<void>;
  resume(payload: ExternalLogControlRequestPayload): Promise<void>;
}

export function externalLogControlErrorMessage(
  error: unknown,
  action: "start" | "stop" | "pause" | "resume"
): string {
  if (error instanceof ManualLogOperationError) {
    switch (error.code) {
      case "session_not_found":
      case "session_changed":
        return "Session not found.";
      case "session_disconnected":
        return "Session is disconnected.";
      case "operation_in_progress":
        return "A log operation is already in progress for this session.";
      case "logging_not_active":
        return "Manual logging is not active.";
    }
  }
  const fallback = {
    start: "Failed to start the external control log.",
    stop: "Failed to stop the external control log.",
    pause: "Failed to pause the external control log.",
    resume: "Failed to resume the external control log.",
  }[action];
  return backendCommandErrorMessage(manualLogOperationCause(error), fallback);
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
          {
            filePath: payload.file_path,
            writeMode: payload.write_mode ?? "overwrite",
          }
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
          error: externalLogControlErrorMessage(error, "start"),
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
          error: externalLogControlErrorMessage(error, "stop"),
        });
      }
    },
    pause: async (payload) => {
      try {
        const result = await dependencies.controller.setPaused(
          { sessionId: payload.session_id },
          true
        );
        if (result.changed) {
          await dependencies.waitForUiUpdate();
        }
        await respond({ requestId: payload.request_id, filePath: null, error: null });
      } catch (error) {
        await respond({
          requestId: payload.request_id,
          filePath: null,
          error: externalLogControlErrorMessage(error, "pause"),
        });
      }
    },
    resume: async (payload) => {
      try {
        const result = await dependencies.controller.setPaused(
          { sessionId: payload.session_id },
          false
        );
        if (result.changed) {
          await dependencies.waitForUiUpdate();
        }
        await respond({ requestId: payload.request_id, filePath: null, error: null });
      } catch (error) {
        await respond({
          requestId: payload.request_id,
          filePath: null,
          error: externalLogControlErrorMessage(error, "resume"),
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

  register("external-control://log-start-request", (payload) => handlers.start(payload));
  register("external-control://log-stop-request", (payload) => handlers.stop(payload));
  register("external-control://log-pause-request", (payload) => handlers.pause(payload));
  register("external-control://log-resume-request", (payload) => handlers.resume(payload));

  return () => {
    acceptingEvents = false;
    for (const unlisten of unlisteners.splice(0)) {
      unlisten();
    }
  };
}
