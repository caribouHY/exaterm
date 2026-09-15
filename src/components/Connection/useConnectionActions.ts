import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { createConnectionAttemptController } from "./connectionAttemptController";
import {
  createConnectionDependencies,
  type ConnectionInput,
  type ConnectionServices,
} from "./connectionAttemptServices";

export const useConnectionActions = (
  params: ConnectionInput & Omit<ConnectionServices, "invoke"> & { canConnect: boolean }
) => {
  const servicesRef = useRef(params);
  servicesRef.current = params;
  const [controller] = useState(() =>
    createConnectionAttemptController(
      createConnectionDependencies({
        invoke,
        diagnostics: {
          start: (requestId) => servicesRef.current.diagnostics.start(requestId),
          stop: () => servicesRef.current.diagnostics.stop(),
        },
        onConnect: (...args) => servicesRef.current.onConnect(...args),
        t: (...args) => servicesRef.current.t(...args),
      })
    )
  );
  const snapshot = useSyncExternalStore(controller.subscribe, controller.getSnapshot);
  const lifetime = useRef(0);
  useEffect(() => {
    const generation = ++lifetime.current;
    // StrictMode replays effects without discarding this controller.
    return () => {
      queueMicrotask(() => {
        if (lifetime.current === generation) controller.dispose();
      });
    };
  }, [controller]);
  return {
    snapshot,
    handleConnect: () => {
      if (!params.canConnect) return Promise.resolve();
      const { tab, selectedProfileIds, sshProfiles, ssh, telnet, serial } = params;
      const target =
        tab === "ssh"
          ? `${ssh.username}@${ssh.host}:${ssh.port}`
          : tab === "telnet"
            ? `${telnet.host}:${telnet.port}`
            : serial.selectedPort;
      return controller.start(
        tab,
        { tab, selectedProfileIds, sshProfiles, ssh, telnet, serial },
        target
      );
    },
    handleCredentialSubmit: controller.submitCredential,
    handleCredentialCancel: controller.cancel,
    handleCancelConnection: controller.cancel,
    retryRegistration: controller.retryRegistration,
  };
};
