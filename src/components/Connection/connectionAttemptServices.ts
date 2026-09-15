import type {
  AppConfig,
  ConnectionType,
  Encoding,
  TerminalMode,
  WorkspaceConnectionInfo,
  SavedConnection,
} from "../../types";
import type { ConnectionDialogProps, ProfileSelectionState } from "./connectionDialogTypes";
import type { AttemptDependencies } from "./connectionAttemptController";
import {
  prepareSshConnection,
  type ConnectionInvoke,
  type SshAttemptInput,
} from "./sshConnectionPreparation";
import { startConnectionLog } from "../../features/terminal-logging/connectionLogModel";
import { connectionHistoryClient } from "../../features/connection-history/connectionHistoryClient";
import { shouldRecordConnectionHistory } from "./connectionHistoryModel";
import { getConnectionErrorMessage, usesPrivateKeyAuthentication } from "./connectionProfileUtils";

import { isConnectionCancellation } from "./connectionProtocolEvents";

export interface ConnectionInput {
  tab: ConnectionType;
  selectedProfileIds: ProfileSelectionState;
  sshProfiles: SavedConnection[];
  ssh: SshAttemptInput;
  telnet: { host: string; port: string; encoding: Encoding; terminalMode: TerminalMode };
  serial: {
    selectedPort: string;
    baudRate: string;
    dataBits: string;
    parity: string;
    stopBits: string;
    terminalMode: TerminalMode;
  };
}
export interface ConnectionServices {
  invoke: ConnectionInvoke;
  diagnostics: { start: (requestId: string) => Promise<unknown>; stop: () => void };
  onConnect: ConnectionDialogProps["onConnect"];
  t: (key: string, options?: Record<string, unknown>) => string;
}
export function createConnectionDependencies(
  services: ConnectionServices
): AttemptDependencies<ConnectionInput> {
  const { invoke } = services;
  return {
    createRequestId: () => globalThis.crypto.randomUUID(),
    cancel: (type, requestId) => invoke<boolean>(`${type}_connect_cancel`, { requestId }),
    disconnect: (type, sessionId) => invoke<void>(`${type}_disconnect`, { sessionId }),
    stopDiagnostics: () => services.diagnostics.stop(),
    errorMessage: (error) =>
      getConnectionErrorMessage(error, services.t, services.t("connection.error")),
    isCancellation: (error) => isConnectionCancellation(error),
    prepare: async (input, context) => {
      const { tab, ssh, telnet, serial } = input;
      if (tab === "ssh") await services.diagnostics.start(context.requestId);
      else services.diagnostics.stop();
      context.assertActive();
      let logEnabled = false;
      try {
        logEnabled = (await invoke<AppConfig>("config_load")).terminal.auto_session_log;
      } catch {
        /* Logging remains opt-in when configuration is unavailable. */
      }
      context.assertActive();
      let connect: (requestId: string) => Promise<string>;
      let clearCredentials: (() => void) | undefined;
      let info: WorkspaceConnectionInfo | undefined;
      let title: string;
      let target: string;
      let encoding: Encoding;
      let terminalMode: TerminalMode;
      if (tab === "ssh") {
        const prepared = await prepareSshConnection(ssh, input.sshProfiles, context, invoke);
        connect = prepared.connect;
        clearCredentials = prepared.clearCredentials;
        info = {
          kind: "ssh",
          host: ssh.host,
          port: Number(ssh.port),
          username: ssh.username,
          auth_method: prepared.authMethod,
          private_key_path: usesPrivateKeyAuthentication(prepared.authMethod)
            ? ssh.privateKeyPath.trim() || null
            : null,
          jump_profile_id: ssh.jumpProfileId || null,
        };
        title = `${ssh.username}@${ssh.host}`;
        target = `${title}:${ssh.port}`;
        encoding = ssh.encoding;
        terminalMode = ssh.terminalMode;
      } else if (tab === "telnet") {
        connect = (requestId) =>
          invoke<string>("telnet_connect", {
            input: {
              host: telnet.host,
              port: Number(telnet.port),
              cols: 120,
              rows: 30,
              encoding: telnet.encoding,
              requestId,
            },
          });
        info = { kind: "telnet", host: telnet.host, port: Number(telnet.port) };
        title = target = `${telnet.host}:${telnet.port}`;
        encoding = telnet.encoding;
        terminalMode = telnet.terminalMode;
      } else {
        connect = (requestId) =>
          invoke<string>("serial_connect", {
            input: {
              port: serial.selectedPort,
              config: {
                baud_rate: Number(serial.baudRate),
                data_bits: Number(serial.dataBits),
                parity: serial.parity,
                stop_bits: Number(serial.stopBits),
                flow_control: "none",
              },
              encoding: "utf-8",
              requestId,
            },
          });
        title = target = serial.selectedPort;
        encoding = "utf-8";
        terminalMode = serial.terminalMode;
      }
      return {
        type: tab,
        connect,
        clearCredentials,
        startLog: (sessionId) =>
          startConnectionLog(logEnabled, () =>
            invoke<string>("logger_start_on_connection", { sessionId, connectionType: tab, target })
          ),
        register: async (sessionId, log) => {
          await services.onConnect(tab, sessionId, title, log, encoding, terminalMode, info);
        },
        recordHistory: async () => {
          if (
            info &&
            tab !== "serial" &&
            shouldRecordConnectionHistory(input.selectedProfileIds[tab])
          )
            await connectionHistoryClient.record({
              connection_info: info,
              encoding,
              terminal_mode: terminalMode,
            });
        },
      };
    },
  };
}
