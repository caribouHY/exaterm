import { useCallback, useRef, type Dispatch, type SetStateAction } from "react";
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
import {
  getConnectionErrorMessage,
  normalizeSshAuthMethod,
  resolveSshAuthentication,
  usesPrivateKeyAuthentication,
} from "./connectionProfileUtils";
import {
  consumeSshCredential,
  createConnectionRequestId,
  isConnectionCancellation,
  isCurrentConnectionAttempt,
  type ConnectionAttemptAction,
} from "./connectionAttemptModel";
import { parseConnectionPort } from "./connectionFormValidation";
import { finalizeConnectionSession } from "./connectionFinalization";

interface UseConnectionActionsParams {
  tab: ConnectionType;
  canConnect: boolean;
  setError: (value: string) => void;
  credentialPrompt: SshCredentialPrompt | null;
  setCredentialPrompt: Dispatch<SetStateAction<SshCredentialPrompt | null>>;
  attemptDispatch: Dispatch<ConnectionAttemptAction>;
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
    defaultPrivateKeyPath: string;
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
    start: (requestId: string) => Promise<string>;
    stop: () => void;
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

export const useConnectionAttemptController = ({
  tab,
  canConnect,
  setError,
  credentialPrompt,
  setCredentialPrompt,
  attemptDispatch,
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
  const connectionAttemptRef = useRef<{
    connectionType: ConnectionType;
    requestId: string;
    cancelPending: boolean;
    connectInvoked: boolean;
    finalizationPending: boolean;
    connectedSession: {
      type: ConnectionType;
      sessionId: string;
      title: string;
      logState: ConnectionLogState;
      encoding: Encoding;
      terminalMode: TerminalMode;
      connectionInfo?: WorkspaceConnectionInfo;
      history?: ConnectionHistoryRecordInput;
    } | null;
  } | null>(null);

  const openCredentialPrompt = useCallback(
    (
      requestId: string,
      phase: "jump" | "target",
      promptHost: string,
      sshPort: number,
      promptUsername: string,
      promptAuthMethod: SshAuthMethod,
      promptPrivateKeyPath: string,
      targetPort?: number
    ) => {
      setCredentialPrompt({
        requestId,
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
      attemptDispatch({ type: "credential", requestId });
    },
    [attemptDispatch, setCredentialPrompt]
  );

  const finishAttempt = useCallback(
    (requestId: string) => {
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
      const connectionType = connectionAttemptRef.current?.connectionType;
      jumpCredentialRef.current = "";
      connectionAttemptRef.current = null;
      setCredentialPrompt(null);
      if (connectionType === "ssh") diagnostics.stop();
      attemptDispatch({ type: "finish", requestId });
    },
    [attemptDispatch, diagnostics, setCredentialPrompt]
  );

  const finalizeConnectedSession = useCallback(
    async (
      requestId: string,
      connectedSession: NonNullable<
        NonNullable<typeof connectionAttemptRef.current>["connectedSession"]
      >
    ) => {
      const attempt = connectionAttemptRef.current;
      if (attempt?.requestId !== requestId || attempt.finalizationPending) return;
      attempt.connectedSession = connectedSession;
      attempt.cancelPending = false;
      attempt.finalizationPending = true;
      attemptDispatch({ type: "finalize", requestId });
      const history = connectedSession.history;
      try {
        await finalizeConnectionSession({
          registerSession: async () => {
            await onConnect(
              connectedSession.type,
              connectedSession.sessionId,
              connectedSession.title,
              connectedSession.logState,
              connectedSession.encoding,
              connectedSession.terminalMode,
              connectedSession.connectionInfo
            );
          },
          recordHistory: history ? () => recordConnectionHistory(history) : undefined,
        });
        if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
        finishAttempt(requestId);
      } catch (error: unknown) {
        const currentAttempt = connectionAttemptRef.current;
        if (currentAttempt?.requestId !== requestId) return;
        currentAttempt.finalizationPending = false;
        attemptDispatch({
          type: "finalize_failed",
          requestId,
          error: getConnectionErrorMessage(error, t, t("connection.session_registration_failed")),
        });
      }
    },
    [attemptDispatch, finishAttempt, onConnect, t]
  );

  const performSshConnect = useCallback(
    async (
      startLogOnConnection: boolean,
      sshPort: number,
      credential: string,
      promptAuthMethod: SshAuthMethod,
      effectivePrivateKeyPath: string,
      currentJumpCredential: string,
      requestId: string
    ) => {
      const jumpProfile = sshProfiles.find((profile) => profile.id === ssh.jumpProfileId);
      const jumpAuthMethod = normalizeSshAuthMethod(jumpProfile?.auth_method);
      const attempt = connectionAttemptRef.current;
      if (attempt?.requestId !== requestId) return;
      attempt.connectInvoked = true;
      const result = await invoke<{ session_id: string }>("ssh_connect", {
        options: {
          host: ssh.host,
          port: sshPort,
          username: ssh.username,
          password: promptAuthMethod === "password" ? credential : "",
          authMethod: promptAuthMethod,
          privateKeyPath: effectivePrivateKeyPath || null,
          keyPassphrase: usesPrivateKeyAuthentication(promptAuthMethod) ? credential : "",
          jumpProfileId: ssh.jumpProfileId || null,
          jumpPassword: jumpAuthMethod === "password" ? currentJumpCredential : "",
          jumpKeyPassphrase: usesPrivateKeyAuthentication(jumpAuthMethod)
            ? currentJumpCredential
            : "",
          cols: 120,
          rows: 30,
          encoding: ssh.encoding,
          requestId,
        },
      });
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
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
        private_key_path: usesPrivateKeyAuthentication(promptAuthMethod)
          ? ssh.privateKeyPath.trim() || null
          : null,
        jump_profile_id: ssh.jumpProfileId || null,
      };
      await finalizeConnectedSession(requestId, {
        type: "ssh",
        sessionId: result.session_id,
        title: `${ssh.username}@${ssh.host}`,
        logState,
        encoding: ssh.encoding,
        terminalMode: ssh.terminalMode,
        connectionInfo,
        history: shouldRecordConnectionHistory(selectedProfileIds.ssh)
          ? {
              connection_info: connectionInfo,
              encoding: ssh.encoding,
              terminal_mode: ssh.terminalMode,
            }
          : undefined,
      });
    },
    [finalizeConnectedSession, selectedProfileIds.ssh, ssh, sshProfiles]
  );

  const continueSshConnect = useCallback(
    async (
      sshPort: number,
      requestId: string,
      currentJumpCredential = jumpCredentialRef.current
    ) => {
      const { authMethod: effectiveAuthMethod, privateKeyPath: effectivePrivateKeyPath } =
        resolveSshAuthentication(ssh.authMethod, ssh.privateKeyPath, ssh.defaultPrivateKeyPath);
      if (
        !usesPrivateKeyAuthentication(ssh.authMethod) ||
        (ssh.authMethod === "auto" && !effectivePrivateKeyPath)
      ) {
        const startLogOnConnection = await getStartLogOnConnectionPreference();
        if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
        jumpCredentialRef.current = "";
        await performSshConnect(
          startLogOnConnection,
          sshPort,
          "",
          effectiveAuthMethod,
          effectivePrivateKeyPath,
          currentJumpCredential,
          requestId
        );
        return;
      }

      let requiresPassphrase: boolean;
      try {
        requiresPassphrase = await invoke<boolean>("ssh_private_key_requires_passphrase", {
          privateKeyPath: effectivePrivateKeyPath,
        });
      } catch (error: unknown) {
        if (ssh.authMethod !== "auto") throw error;
        const startLogOnConnection = await getStartLogOnConnectionPreference();
        if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
        jumpCredentialRef.current = "";
        await performSshConnect(
          startLogOnConnection,
          sshPort,
          "",
          "auto",
          effectivePrivateKeyPath,
          currentJumpCredential,
          requestId
        );
        return;
      }
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
      if (requiresPassphrase) {
        openCredentialPrompt(
          requestId,
          "target",
          ssh.host,
          sshPort,
          ssh.username,
          ssh.authMethod,
          effectivePrivateKeyPath
        );
        return;
      }

      const startLogOnConnection = await getStartLogOnConnectionPreference();
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
      jumpCredentialRef.current = "";
      await performSshConnect(
        startLogOnConnection,
        sshPort,
        "",
        effectiveAuthMethod,
        effectivePrivateKeyPath,
        currentJumpCredential,
        requestId
      );
    },
    [openCredentialPrompt, performSshConnect, ssh]
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
      const { privateKeyPath: effectiveJumpPrivateKeyPath } = resolveSshAuthentication(
        jumpAuthMethod,
        jumpPrivateKeyPath,
        ssh.defaultPrivateKeyPath
      );
      if (!jumpProfile.host || !jumpUsername) {
        throw new Error(t("connection.jump_profile_incomplete", { profile: ssh.jumpProfileId }));
      }

      const promptForJumpCredential = () => {
        openCredentialPrompt(
          requestId,
          "jump",
          jumpProfile.host ?? "",
          jumpPort,
          jumpUsername,
          jumpAuthMethod,
          effectiveJumpPrivateKeyPath,
          sshPort
        );
      };

      if (
        !usesPrivateKeyAuthentication(jumpAuthMethod) ||
        (jumpAuthMethod === "auto" && !effectiveJumpPrivateKeyPath)
      ) {
        await continueSshConnect(sshPort, requestId, "");
        return;
      }

      let requiresPassphrase: boolean;
      try {
        requiresPassphrase = await invoke<boolean>("ssh_private_key_requires_passphrase", {
          privateKeyPath: effectiveJumpPrivateKeyPath,
        });
      } catch (error: unknown) {
        if (jumpAuthMethod !== "auto") throw error;
        await continueSshConnect(sshPort, requestId, "");
        return;
      }
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
      if (requiresPassphrase) {
        promptForJumpCredential();
        return;
      }

      await continueSshConnect(sshPort, requestId, "");
    },
    [continueSshConnect, openCredentialPrompt, ssh, sshProfiles, t]
  );

  const handleCredentialSubmit = useCallback(async () => {
    if (!credentialPrompt) return;
    const requestId = credentialPrompt.requestId;
    if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;

    const credential = consumeSshCredential(credentialPrompt.value, () => {
      setCredentialPrompt(null);
    });
    attemptDispatch({ type: "resume", requestId });
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
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
      const jumpCredential = jumpCredentialRef.current;
      jumpCredentialRef.current = "";
      await performSshConnect(
        startLogOnConnection,
        credentialPrompt.port,
        credential,
        credentialPrompt.authMethod,
        credentialPrompt.privateKeyPath,
        jumpCredential,
        requestId
      );
    } catch (error: unknown) {
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
      const message = getConnectionErrorMessage(error, t, t("connection.error"));
      finishAttempt(requestId);
      if (!isConnectionCancellation(error)) setError(message);
    }
  }, [
    attemptDispatch,
    credentialPrompt,
    continueSshConnect,
    performSshConnect,
    finishAttempt,
    setCredentialPrompt,
    setError,
    t,
  ]);

  const handleConnect = useCallback(async () => {
    if (connectionAttemptRef.current || !canConnect) return;

    const validatedSshPort = tab === "ssh" ? parseConnectionPort(ssh.port) : 22;
    const validatedTelnetPort = tab === "telnet" ? parseConnectionPort(telnet.port) : 23;
    if (validatedSshPort === null || validatedTelnetPort === null) return;

    setError("");
    const requestId = createConnectionRequestId();
    connectionAttemptRef.current = {
      connectionType: tab,
      requestId,
      cancelPending: false,
      connectInvoked: false,
      finalizationPending: false,
      connectedSession: null,
    };
    attemptDispatch({ type: "begin", connectionType: tab, requestId });
    try {
      if (tab === "ssh") {
        jumpCredentialRef.current = "";
        await diagnostics.start(requestId);
        if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
        attemptDispatch({ type: "connected", requestId });
        await prepareJumpCredentialAndConnect(validatedSshPort, requestId);
        return;
      }

      diagnostics.stop();
      const startLogOnConnection = await getStartLogOnConnectionPreference();
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;

      if (tab === "telnet") {
        const parsedTelnetPort = validatedTelnetPort;
        connectionAttemptRef.current.connectInvoked = true;
        const sessionId = await invoke<string>("telnet_connect", {
          input: {
            host: telnet.host,
            port: parsedTelnetPort,
            cols: 120,
            rows: 30,
            encoding: telnet.encoding,
            requestId,
          },
        });
        if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
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
        await finalizeConnectedSession(requestId, {
          type: "telnet",
          sessionId,
          title: `${telnet.host}:${parsedTelnetPort}`,
          logState,
          encoding: telnet.encoding,
          terminalMode: telnet.terminalMode,
          connectionInfo,
          history: shouldRecordConnectionHistory(selectedProfileIds.telnet)
            ? {
                connection_info: connectionInfo,
                encoding: telnet.encoding,
                terminal_mode: telnet.terminalMode,
              }
            : undefined,
        });
        return;
      }

      connectionAttemptRef.current.connectInvoked = true;
      const sessionId = await invoke<string>("serial_connect", {
        input: {
          port: serial.selectedPort,
          config: {
            baud_rate: Number.parseInt(serial.baudRate, 10),
            data_bits: Number.parseInt(serial.dataBits, 10),
            parity: serial.parity,
            stop_bits: Number.parseInt(serial.stopBits, 10),
            flow_control: "none",
          },
          encoding: "utf-8",
          requestId,
        },
      });
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
      const logState = await startConfiguredConnectionLog(
        startLogOnConnection,
        sessionId,
        "serial",
        serial.selectedPort
      );
      await finalizeConnectedSession(requestId, {
        type: "serial",
        sessionId,
        title: serial.selectedPort,
        logState,
        encoding: "utf-8",
        terminalMode: serial.terminalMode,
      });
    } catch (error: unknown) {
      if (!isCurrentConnectionAttempt(connectionAttemptRef.current, requestId)) return;
      const cancelled = isConnectionCancellation(error);
      const message = getConnectionErrorMessage(error, t, t("connection.error"));
      finishAttempt(requestId);
      if (!cancelled) setError(message);
    }
  }, [
    attemptDispatch,
    canConnect,
    diagnostics,
    finalizeConnectedSession,
    finishAttempt,
    prepareJumpCredentialAndConnect,
    selectedProfileIds.telnet,
    serial,
    setError,
    ssh,
    tab,
    telnet,
    t,
  ]);

  const handleCredentialCancel = useCallback(() => {
    if (!credentialPrompt) return;
    finishAttempt(credentialPrompt.requestId);
  }, [credentialPrompt, finishAttempt]);

  const handleCancelConnection = useCallback(async () => {
    const attempt = connectionAttemptRef.current;
    if (!attempt || attempt.cancelPending || attempt.connectedSession) return;
    if (!attempt.connectInvoked) {
      finishAttempt(attempt.requestId);
      return;
    }

    attempt.cancelPending = true;
    attemptDispatch({ type: "cancel", requestId: attempt.requestId });
    const command = `${attempt.connectionType}_connect_cancel`;
    try {
      const accepted = await invoke<boolean>(command, { requestId: attempt.requestId });
      const currentAttempt = connectionAttemptRef.current;
      if (currentAttempt?.requestId !== attempt.requestId || currentAttempt.connectedSession)
        return;
      if (!accepted) {
        currentAttempt.cancelPending = false;
        attemptDispatch({ type: "resume", requestId: attempt.requestId });
      }
    } catch (error: unknown) {
      const currentAttempt = connectionAttemptRef.current;
      if (currentAttempt?.requestId !== attempt.requestId || currentAttempt.connectedSession)
        return;
      currentAttempt.cancelPending = false;
      attemptDispatch({
        type: "cancel_failed",
        requestId: attempt.requestId,
        error: getConnectionErrorMessage(
          error,
          t,
          t(
            attempt.connectionType === "ssh"
              ? "connection.ssh_cancel_failed"
              : "connection.cancel_failed"
          )
        ),
      });
    }
  }, [attemptDispatch, finishAttempt, t]);

  const handleRetryFinalization = useCallback(async () => {
    const attempt = connectionAttemptRef.current;
    if (!attempt?.connectedSession) return;
    await finalizeConnectedSession(attempt.requestId, attempt.connectedSession);
  }, [finalizeConnectedSession]);

  return {
    handleConnect,
    handleCredentialSubmit,
    handleCredentialCancel,
    handleCancelConnection,
    handleRetryFinalization,
  };
};
