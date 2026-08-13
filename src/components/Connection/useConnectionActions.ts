import {
  useCallback,
  useRef,
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  ConnectionHistoryRecordInput,
  ConnectionType,
  Encoding,
  SavedConnection,
  SshAuthMethod,
  TerminalMode,
  WorkspaceConnectionInfo,
} from "../../types";
import { connectionHistoryClient } from "../../features/connection-history/connectionHistoryClient";
import { startConnectionLog } from "../../features/terminal-logging/connectionLogModel";
import type { ConnectionLogState } from "../../features/terminal-logging/connectionLogModel";
import type { ProfileSelectionState, SshCredentialPrompt } from "./connectionDialogTypes";
import { shouldRecordConnectionHistory } from "./connectionHistoryModel";
import { getConnectionErrorMessage, normalizeSshAuthMethod } from "./connectionProfileUtils";
import {
  createConnectionRequestId,
  isConnectionCancellation,
  isCurrentConnectionAttempt,
  type BasicConnectionType,
  type ConnectionAttemptAction,
} from "./connectionAttemptModel";
import {
  consumeSshCredential,
  isCurrentSshConnectionAttempt,
  isSshConnectionCancellation,
  type SshConnectionAttemptAction,
} from "./sshConnectionAttemptModel";

interface UseConnectionActionsParams {
  tab: ConnectionType;
  connectingRef: MutableRefObject<boolean>;
  setConnecting: (value: boolean) => void;
  setError: (value: string) => void;
  credentialPrompt: SshCredentialPrompt | null;
  setCredentialPrompt: Dispatch<SetStateAction<SshCredentialPrompt | null>>;
  sshAttemptDispatch: Dispatch<SshConnectionAttemptAction>;
  connectionAttemptDispatch: Dispatch<ConnectionAttemptAction>;
  selectedProfileIds: ProfileSelectionState;
  sshProfiles: SavedConnection[];
  ssh: {
    host: string;
    port: string;
    username: string;
    authMethod: SshAuthMethod;
    privateKeyPath: string;
    jumpProfileId: string;
    encoding: Encoding;
    terminalMode: TerminalMode;
  };
  telnet: {
    host: string;
    port: string;
    encoding: Encoding;
    terminalMode: TerminalMode;
  };
  serial: {
    selectedPort: string;
    baudRate: string;
    dataBits: string;
    parity: string;
    stopBits: string;
    terminalMode: TerminalMode;
  };
  diagnostics: {
    start: () => Promise<string>;
    stop: () => void;
    currentRequestId: () => string | null;
  };
  onConnect: (
    type: ConnectionType,
    sessionId: string,
    title: string,
    logState: ConnectionLogState,
    encoding?: Encoding,
    terminalMode?: TerminalMode,
    connectionInfo?: WorkspaceConnectionInfo
  ) => void | Promise<void>;
  t: (key: string, options?: Record<string, unknown>) => string;
}

const parsePort = (value: string, errorMessage: string) => {
  const parsed = Number.parseInt(value, 10);
  if (Number.isNaN(parsed)) {
    throw new Error(errorMessage);
  }
  return parsed;
};

const getStartLogOnConnectionPreference = async () => {
  try {
    const cfg = await invoke<AppConfig>("config_load");
    return cfg.terminal.auto_session_log;
  } catch {
    return false;
  }
};

const startConfiguredConnectionLog = async (
  enabled: boolean,
  sessionId: string,
  connectionType: ConnectionType,
  target: string
): Promise<ConnectionLogState> => {
  const state = await startConnectionLog(enabled, () =>
    invoke<string>("logger_start_on_connection", { sessionId, connectionType, target })
  );
  if (state.startFailed) console.error("Failed to start the connection log.");
  return state;
};

const recordConnectionHistory = (input: ConnectionHistoryRecordInput) => {
  void connectionHistoryClient.record(input).catch(() => {
    console.warn("Failed to save connection history.");
  });
};

export const useConnectionActions = ({
  tab,
  connectingRef,
  setConnecting,
  setError,
  credentialPrompt,
  setCredentialPrompt,
  sshAttemptDispatch,
  connectionAttemptDispatch,
  selectedProfileIds,
  sshProfiles,
  ssh,
  telnet,
  serial,
  diagnostics,
  onConnect,
  t,
}: UseConnectionActionsParams) => {
  const jumpCredentialRef = useRef("");
  const sshConnectInvokedRef = useRef(false);
  const cancellingRequestIdRef = useRef<string | null>(null);
  const connectionAttemptRef = useRef<{
    connectionType: BasicConnectionType;
    requestId: string;
    cancelPending: boolean;
    connectInvoked: boolean;
  } | null>(null);
  const setBusy = useCallback(
    (value: boolean) => {
      connectingRef.current = value;
      setConnecting(value);
    },
    [connectingRef, setConnecting]
  );

  const openCredentialPrompt = useCallback(
    (
      phase: "jump" | "target",
      promptHost: string,
      sshPort: number,
      promptUsername: string,
      promptAuthMethod: SshAuthMethod,
      promptPrivateKeyPath: string,
      targetPort?: number
    ) => {
      setCredentialPrompt({
        phase,
        host: promptHost,
        port: sshPort,
        targetPort,
        username: promptUsername,
        authMethod: promptAuthMethod,
        privateKeyPath: promptPrivateKeyPath,
        value: "",
        error: "",
      });
      sshAttemptDispatch({ type: "credential" });
    },
    [setCredentialPrompt, sshAttemptDispatch]
  );

  const performSshConnect = useCallback(
    async (
      startLogOnConnection: boolean,
      sshPort: number,
      credential: string,
      promptAuthMethod: SshAuthMethod,
      currentJumpCredential: string,
      requestId: string
    ) => {
      const jumpProfile = sshProfiles.find((profile) => profile.id === ssh.jumpProfileId);
      const jumpAuthMethod = normalizeSshAuthMethod(jumpProfile?.auth_method);
      sshConnectInvokedRef.current = true;
      const result = await invoke<{ session_id: string }>("ssh_connect", {
        options: {
          host: ssh.host,
          port: sshPort,
          username: ssh.username,
          password: promptAuthMethod === "password" ? credential : "",
          authMethod: promptAuthMethod,
          privateKeyPath: ssh.privateKeyPath,
          keyPassphrase: promptAuthMethod === "public_key" ? credential : "",
          jumpProfileId: ssh.jumpProfileId || null,
          jumpPassword: jumpAuthMethod === "password" ? currentJumpCredential : "",
          jumpKeyPassphrase: jumpAuthMethod === "public_key" ? currentJumpCredential : "",
          cols: 120,
          rows: 30,
          encoding: ssh.encoding,
          requestId,
        },
      });
      const logState = await startConfiguredConnectionLog(
        startLogOnConnection,
        result.session_id,
        "ssh",
        `${ssh.username}@${ssh.host}:${sshPort}`
      );
      const connectionInfo: WorkspaceConnectionInfo = {
        kind: "ssh",
        host: ssh.host,
        port: sshPort,
        username: ssh.username,
        auth_method: promptAuthMethod,
        private_key_path: ssh.privateKeyPath || null,
        jump_profile_id: ssh.jumpProfileId || null,
      };
      await onConnect(
        "ssh",
        result.session_id,
        `${ssh.username}@${ssh.host}`,
        logState,
        ssh.encoding,
        ssh.terminalMode,
        connectionInfo
      );
      if (shouldRecordConnectionHistory(selectedProfileIds.ssh)) {
        recordConnectionHistory({
          connection_info: connectionInfo,
          encoding: ssh.encoding,
          terminal_mode: ssh.terminalMode,
        });
      }
    },
    [onConnect, selectedProfileIds.ssh, ssh, sshProfiles]
  );

  const continueSshConnect = useCallback(
    async (
      sshPort: number,
      requestId: string,
      currentJumpCredential = jumpCredentialRef.current
    ) => {
      if (ssh.authMethod === "password") {
        const startLogOnConnection = await getStartLogOnConnectionPreference();
        if (!isCurrentSshConnectionAttempt(diagnostics.currentRequestId(), requestId)) return;
        jumpCredentialRef.current = "";
        await performSshConnect(
          startLogOnConnection,
          sshPort,
          "",
          "password",
          currentJumpCredential,
          requestId
        );
        return;
      }

      const requiresPassphrase = await invoke<boolean>("ssh_private_key_requires_passphrase", {
        privateKeyPath: ssh.privateKeyPath,
      });
      if (!isCurrentSshConnectionAttempt(diagnostics.currentRequestId(), requestId)) return;
      if (requiresPassphrase) {
        openCredentialPrompt(
          "target",
          ssh.host,
          sshPort,
          ssh.username,
          ssh.authMethod,
          ssh.privateKeyPath
        );
        setBusy(false);
        return;
      }

      const startLogOnConnection = await getStartLogOnConnectionPreference();
      if (!isCurrentSshConnectionAttempt(diagnostics.currentRequestId(), requestId)) return;
      jumpCredentialRef.current = "";
      await performSshConnect(
        startLogOnConnection,
        sshPort,
        "",
        "public_key",
        currentJumpCredential,
        requestId
      );
    },
    [diagnostics, openCredentialPrompt, performSshConnect, setBusy, ssh]
  );

  const prepareJumpCredentialAndConnect = useCallback(
    async (sshPort: number, requestId: string) => {
      const jumpProfile = sshProfiles.find((profile) => profile.id === ssh.jumpProfileId);
      if (!ssh.jumpProfileId || !jumpProfile) {
        await continueSshConnect(sshPort, requestId, "");
        return;
      }

      const jumpPort = jumpProfile.port ?? 22;
      const jumpUsername = jumpProfile.username ?? "";
      const jumpPrivateKeyPath = jumpProfile.private_key_path ?? "";
      const jumpAuthMethod = normalizeSshAuthMethod(jumpProfile.auth_method);
      if (!jumpProfile.host || !jumpUsername) {
        throw new Error(t("connection.jump_profile_incomplete", { profile: ssh.jumpProfileId }));
      }

      const promptForJumpCredential = () => {
        openCredentialPrompt(
          "jump",
          jumpProfile.host ?? "",
          jumpPort,
          jumpUsername,
          jumpAuthMethod,
          jumpPrivateKeyPath,
          sshPort
        );
        setBusy(false);
      };

      if (jumpAuthMethod === "password") {
        await continueSshConnect(sshPort, requestId, "");
        return;
      }

      const requiresPassphrase = await invoke<boolean>("ssh_private_key_requires_passphrase", {
        privateKeyPath: jumpPrivateKeyPath,
      });
      if (!isCurrentSshConnectionAttempt(diagnostics.currentRequestId(), requestId)) return;
      if (requiresPassphrase) {
        promptForJumpCredential();
        return;
      }

      await continueSshConnect(sshPort, requestId, "");
    },
    [continueSshConnect, diagnostics, openCredentialPrompt, setBusy, ssh, sshProfiles, t]
  );

  const finishSshAttempt = useCallback(() => {
    jumpCredentialRef.current = "";
    sshConnectInvokedRef.current = false;
    cancellingRequestIdRef.current = null;
    setCredentialPrompt(null);
    diagnostics.stop();
    sshAttemptDispatch({ type: "finish" });
    setBusy(false);
  }, [diagnostics, setBusy, setCredentialPrompt, sshAttemptDispatch]);

  const finishConnectionAttempt = useCallback(
    (requestId: string) => {
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
      connectionAttemptRef.current = null;
      connectionAttemptDispatch({ type: "finish", requestId });
      setBusy(false);
    },
    [connectionAttemptDispatch, setBusy]
  );

  const handleCredentialSubmit = useCallback(async () => {
    if (!credentialPrompt || connectingRef.current) return;
    const requestId = diagnostics.currentRequestId();
    if (!requestId) {
      finishSshAttempt();
      return;
    }

    const credential = consumeSshCredential(credentialPrompt.value, () => {
      setCredentialPrompt(null);
    });
    sshAttemptDispatch({ type: "resume" });
    setBusy(true);
    try {
      if (credentialPrompt.phase === "jump") {
        jumpCredentialRef.current = credential;
        await continueSshConnect(
          credentialPrompt.targetPort ?? credentialPrompt.port,
          requestId,
          credential
        );
        return;
      }

      const startLogOnConnection = await getStartLogOnConnectionPreference();
      if (!isCurrentSshConnectionAttempt(diagnostics.currentRequestId(), requestId)) return;
      const jumpCredential = jumpCredentialRef.current;
      jumpCredentialRef.current = "";
      await performSshConnect(
        startLogOnConnection,
        credentialPrompt.port,
        credential,
        credentialPrompt.authMethod,
        jumpCredential,
        requestId
      );
    } catch (error: unknown) {
      if (!isCurrentSshConnectionAttempt(diagnostics.currentRequestId(), requestId)) return;
      const message = getConnectionErrorMessage(error, t, t("connection.error"));
      finishSshAttempt();
      if (!isSshConnectionCancellation(error)) setError(message);
    }
  }, [
    connectingRef,
    credentialPrompt,
    continueSshConnect,
    diagnostics,
    performSshConnect,
    finishSshAttempt,
    setBusy,
    setCredentialPrompt,
    setError,
    sshAttemptDispatch,
    t,
  ]);

  const handleConnect = useCallback(async () => {
    if (connectingRef.current) return;

    setError("");
    setBusy(true);
    let sshRequestId: string | null = null;
    let connectionRequestId: string | null = null;
    try {
      if (tab === "ssh") {
        sshConnectInvokedRef.current = false;
        cancellingRequestIdRef.current = null;
        jumpCredentialRef.current = "";
        sshAttemptDispatch({ type: "begin" });
        const diagnosticsStart = diagnostics.start();
        sshRequestId = diagnostics.currentRequestId();
        const requestId = await diagnosticsStart;
        if (!isCurrentSshConnectionAttempt(diagnostics.currentRequestId(), requestId)) return;
        sshAttemptDispatch({ type: "started", requestId });
        const sshPort = parsePort(ssh.port, t("connection.error"));

        await prepareJumpCredentialAndConnect(sshPort, requestId);
        return;
      }

      diagnostics.stop();
      const connectionType: BasicConnectionType = tab === "telnet" ? "telnet" : "serial";
      connectionRequestId = createConnectionRequestId();
      connectionAttemptRef.current = {
        connectionType,
        requestId: connectionRequestId,
        cancelPending: false,
        connectInvoked: false,
      };
      connectionAttemptDispatch({
        type: "begin",
        connectionType,
        requestId: connectionRequestId,
      });
      const startLogOnConnection = await getStartLogOnConnectionPreference();
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, connectionRequestId)) return;

      if (tab === "telnet") {
        const parsedTelnetPort = parsePort(telnet.port, t("connection.error"));
        connectionAttemptRef.current.connectInvoked = true;
        const sessionId = await invoke<string>("telnet_connect", {
          host: telnet.host,
          port: parsedTelnetPort,
          cols: 120,
          rows: 30,
          encoding: telnet.encoding,
          requestId: connectionRequestId,
        });
        const logState = await startConfiguredConnectionLog(
          startLogOnConnection,
          sessionId,
          "telnet",
          `${telnet.host}:${parsedTelnetPort}`
        );
        const connectionInfo: WorkspaceConnectionInfo = {
          kind: "telnet",
          host: telnet.host,
          port: parsedTelnetPort,
        };
        await onConnect(
          "telnet",
          sessionId,
          `${telnet.host}:${parsedTelnetPort}`,
          logState,
          telnet.encoding,
          telnet.terminalMode,
          connectionInfo
        );
        if (shouldRecordConnectionHistory(selectedProfileIds.telnet)) {
          recordConnectionHistory({
            connection_info: connectionInfo,
            encoding: telnet.encoding,
            terminal_mode: telnet.terminalMode,
          });
        }
        finishConnectionAttempt(connectionRequestId);
        return;
      }

      connectionAttemptRef.current.connectInvoked = true;
      const sessionId = await invoke<string>("serial_connect", {
        port: serial.selectedPort,
        config: {
          baud_rate: Number.parseInt(serial.baudRate, 10),
          data_bits: Number.parseInt(serial.dataBits, 10),
          parity: serial.parity,
          stop_bits: Number.parseInt(serial.stopBits, 10),
          flow_control: "none",
        },
        encoding: "utf-8",
        requestId: connectionRequestId,
      });
      const logState = await startConfiguredConnectionLog(
        startLogOnConnection,
        sessionId,
        "serial",
        serial.selectedPort
      );
      await onConnect(
        "serial",
        sessionId,
        serial.selectedPort,
        logState,
        "utf-8",
        serial.terminalMode
      );
      finishConnectionAttempt(connectionRequestId);
    } catch (error: unknown) {
      const staleSshAttempt =
        tab === "ssh" &&
        sshRequestId !== null &&
        !isCurrentSshConnectionAttempt(diagnostics.currentRequestId(), sshRequestId);
      if (staleSshAttempt) return;
      const staleConnectionAttempt =
        tab !== "ssh" &&
        connectionRequestId !== null &&
        !isCurrentConnectionAttempt(connectionAttemptRef.current, connectionRequestId);
      if (staleConnectionAttempt) return;
      const cancelled =
        tab === "ssh" ? isSshConnectionCancellation(error) : isConnectionCancellation(error);
      const message = getConnectionErrorMessage(error, t, t("connection.error"));
      if (tab === "ssh") finishSshAttempt();
      else if (connectionRequestId) finishConnectionAttempt(connectionRequestId);
      else setBusy(false);
      if (!cancelled) setError(message);
    }
  }, [
    connectingRef,
    connectionAttemptDispatch,
    diagnostics,
    finishConnectionAttempt,
    finishSshAttempt,
    onConnect,
    prepareJumpCredentialAndConnect,
    selectedProfileIds.telnet,
    serial,
    setBusy,
    setError,
    ssh,
    sshAttemptDispatch,
    sshProfiles,
    tab,
    telnet,
    t,
  ]);

  const handleCredentialCancel = useCallback(() => {
    if (connectingRef.current) return;
    finishSshAttempt();
  }, [connectingRef, finishSshAttempt]);

  const handleCancelSshConnect = useCallback(async () => {
    const requestId = diagnostics.currentRequestId();
    if (!requestId) {
      finishSshAttempt();
      return;
    }
    if (cancellingRequestIdRef.current === requestId) return;
    if (!sshConnectInvokedRef.current) {
      finishSshAttempt();
      return;
    }

    cancellingRequestIdRef.current = requestId;
    sshAttemptDispatch({ type: "cancel" });
    try {
      const accepted = await invoke<boolean>("ssh_connect_cancel", { requestId });
      if (!isCurrentSshConnectionAttempt(diagnostics.currentRequestId(), requestId)) return;
      if (!accepted) {
        cancellingRequestIdRef.current = null;
        sshAttemptDispatch({ type: "resume" });
      }
    } catch (error: unknown) {
      if (!isCurrentSshConnectionAttempt(diagnostics.currentRequestId(), requestId)) return;
      cancellingRequestIdRef.current = null;
      sshAttemptDispatch({
        type: "cancel_failed",
        error: getConnectionErrorMessage(error, t, t("connection.ssh_cancel_failed")),
      });
    }
  }, [diagnostics, finishSshAttempt, sshAttemptDispatch, t]);

  const handleCancelConnection = useCallback(async () => {
    const attempt = connectionAttemptRef.current;
    if (!attempt || attempt.cancelPending) return;
    if (!attempt.connectInvoked) {
      finishConnectionAttempt(attempt.requestId);
      return;
    }

    attempt.cancelPending = true;
    connectionAttemptDispatch({ type: "cancel", requestId: attempt.requestId });
    const command =
      attempt.connectionType === "telnet" ? "telnet_connect_cancel" : "serial_connect_cancel";
    try {
      const accepted = await invoke<boolean>(command, { requestId: attempt.requestId });
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, attempt.requestId)) return;
      if (!accepted) {
        attempt.cancelPending = false;
        connectionAttemptDispatch({ type: "resume", requestId: attempt.requestId });
      }
    } catch (error: unknown) {
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, attempt.requestId)) return;
      attempt.cancelPending = false;
      connectionAttemptDispatch({
        type: "cancel_failed",
        requestId: attempt.requestId,
        error: getConnectionErrorMessage(error, t, t("connection.cancel_failed")),
      });
    }
  }, [connectionAttemptDispatch, finishConnectionAttempt, t]);

  return {
    handleConnect,
    handleCredentialSubmit,
    handleCredentialCancel,
    handleCancelSshConnect,
    handleCancelConnection,
  };
};
