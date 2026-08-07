import { useCallback, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  ConnectionHistoryRecordInput,
  ConnectionType,
  Encoding,
  HostKeyCheckResult,
  SavedConnection,
  SshAuthMethod,
  TerminalMode,
  WorkspaceConnectionInfo,
} from "../../types";
import { connectionHistoryClient } from "../../features/connection-history/connectionHistoryClient";
import type { SshCredentialPrompt, SshHostKeyCheck } from "./connectionDialogTypes";
import { getConnectionErrorMessage, normalizeSshAuthMethod } from "./connectionProfileUtils";

interface UseConnectionActionsParams {
  tab: ConnectionType;
  connectingRef: MutableRefObject<boolean>;
  setConnecting: (value: boolean) => void;
  setError: (value: string) => void;
  hostKeyCheck: SshHostKeyCheck | null;
  setHostKeyCheck: (value: SshHostKeyCheck | null) => void;
  credentialPrompt: SshCredentialPrompt | null;
  setCredentialPrompt: Dispatch<SetStateAction<SshCredentialPrompt | null>>;
  sshProfiles: SavedConnection[];
  ssh: {
    host: string;
    port: string;
    username: string;
    authMethod: SshAuthMethod;
    privateKeyPath: string;
    jumpProfileId: string;
    jumpCredential: string;
    setJumpCredential: (value: string) => void;
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
    isAutoLogging: boolean,
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

const getAutoLogPreference = async () => {
  try {
    const cfg = await invoke<AppConfig>("config_load");
    return cfg.terminal.auto_session_log;
  } catch {
    return false;
  }
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
  hostKeyCheck,
  setHostKeyCheck,
  credentialPrompt,
  setCredentialPrompt,
  sshProfiles,
  ssh,
  telnet,
  serial,
  diagnostics,
  onConnect,
  t,
}: UseConnectionActionsParams) => {
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
    },
    [setCredentialPrompt]
  );

  const performSshConnect = useCallback(
    async (
      autoLog: boolean,
      sshPort: number,
      credential: string,
      promptAuthMethod: SshAuthMethod,
      currentJumpCredential: string
    ) => {
      const jumpProfile = sshProfiles.find((profile) => profile.id === ssh.jumpProfileId);
      const jumpAuthMethod = normalizeSshAuthMethod(jumpProfile?.auth_method);
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
          requestId: diagnostics.currentRequestId(),
        },
      });
      if (autoLog) {
        await invoke("logger_start_auto", {
          sessionId: result.session_id,
          connectionType: "ssh",
          target: `${ssh.username}@${ssh.host}:${sshPort}`,
        });
      }
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
        autoLog,
        ssh.encoding,
        ssh.terminalMode,
        connectionInfo
      );
      recordConnectionHistory({
        connection_info: connectionInfo,
        encoding: ssh.encoding,
        terminal_mode: ssh.terminalMode,
      });
    },
    [diagnostics, onConnect, ssh, sshProfiles]
  );

  const continueSshConnect = useCallback(
    async (sshPort: number, currentJumpCredential = ssh.jumpCredential) => {
      if (ssh.authMethod === "password") {
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

      const requiresPassphrase = await invoke<boolean>("ssh_private_key_requires_passphrase", {
        privateKeyPath: ssh.privateKeyPath,
      });
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

      const autoLog = await getAutoLogPreference();
      await performSshConnect(autoLog, sshPort, "", "public_key", currentJumpCredential);
    },
    [openCredentialPrompt, performSshConnect, setBusy, ssh]
  );

  const probeTargetHostKey = useCallback(
    async (sshPort: number, currentJumpCredential = ssh.jumpCredential) => {
      const jumpProfile = sshProfiles.find((profile) => profile.id === ssh.jumpProfileId);
      const jumpAuthMethod = normalizeSshAuthMethod(jumpProfile?.auth_method);
      const result = await invoke<HostKeyCheckResult>("ssh_probe_host_key", {
        options: {
          host: ssh.host,
          port: sshPort,
          jumpProfileId: ssh.jumpProfileId || null,
          jumpPassword: jumpAuthMethod === "password" ? currentJumpCredential : "",
          jumpKeyPassphrase: jumpAuthMethod === "public_key" ? currentJumpCredential : "",
          requestId: diagnostics.currentRequestId(),
          diagnosticRole: "target",
        },
      });

      if (result.status === "trusted") {
        await continueSshConnect(sshPort, currentJumpCredential);
        return;
      }

      ssh.setJumpCredential(currentJumpCredential);
      setHostKeyCheck({ ...result, phase: "target" });
      setBusy(false);
    },
    [continueSshConnect, diagnostics, setBusy, setHostKeyCheck, ssh, sshProfiles]
  );

  const continueAfterJumpTrusted = useCallback(
    async (sshPort: number) => {
      const jumpProfile = sshProfiles.find((profile) => profile.id === ssh.jumpProfileId);
      if (!ssh.jumpProfileId || !jumpProfile) {
        await probeTargetHostKey(sshPort, "");
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
        promptForJumpCredential();
        return;
      }

      const requiresPassphrase = await invoke<boolean>("ssh_private_key_requires_passphrase", {
        privateKeyPath: jumpPrivateKeyPath,
      });
      if (requiresPassphrase) {
        promptForJumpCredential();
        return;
      }

      ssh.setJumpCredential("");
      await probeTargetHostKey(sshPort, "");
    },
    [openCredentialPrompt, probeTargetHostKey, setBusy, ssh, sshProfiles, t]
  );

  const handleCredentialSubmit = useCallback(async () => {
    if (!credentialPrompt || connectingRef.current) return;

    setCredentialPrompt({ ...credentialPrompt, error: "" });
    setBusy(true);
    try {
      if (credentialPrompt.phase === "jump") {
        setCredentialPrompt(null);
        ssh.setJumpCredential(credentialPrompt.value);
        await probeTargetHostKey(
          credentialPrompt.targetPort ?? credentialPrompt.port,
          credentialPrompt.value
        );
        return;
      }

      const autoLog = await getAutoLogPreference();
      await performSshConnect(
        autoLog,
        credentialPrompt.port,
        credentialPrompt.value,
        credentialPrompt.authMethod,
        ssh.jumpCredential
      );
      setCredentialPrompt(null);
    } catch (error: unknown) {
      setCredentialPrompt({
        ...credentialPrompt,
        value: "",
        error: getConnectionErrorMessage(error, t("connection.error")),
      });
      setBusy(false);
    }
  }, [
    connectingRef,
    credentialPrompt,
    performSshConnect,
    probeTargetHostKey,
    setBusy,
    setCredentialPrompt,
    ssh,
    t,
  ]);

  const handleTrustAndConnect = useCallback(
    async (replace: boolean) => {
      if (!hostKeyCheck || connectingRef.current) return;

      setError("");
      setBusy(true);
      try {
        await invoke("ssh_trust_host_key", {
          host: hostKeyCheck.host,
          port: hostKeyCheck.port,
          replace,
        });
        const phase = hostKeyCheck.phase;
        const checkedPort = hostKeyCheck.port;
        setHostKeyCheck(null);
        if (phase === "jump") {
          await continueAfterJumpTrusted(parsePort(ssh.port, t("connection.error")));
        } else {
          await continueSshConnect(checkedPort);
        }
      } catch (error: unknown) {
        setError(getConnectionErrorMessage(error, t("connection.error")));
        setBusy(false);
      }
    },
    [
      connectingRef,
      continueAfterJumpTrusted,
      continueSshConnect,
      hostKeyCheck,
      setBusy,
      setError,
      setHostKeyCheck,
      ssh.port,
      t,
    ]
  );

  const handleConnect = useCallback(async () => {
    if (connectingRef.current) return;

    setError("");
    setBusy(true);
    try {
      if (tab === "ssh") {
        await diagnostics.start();
        const sshPort = parsePort(ssh.port, t("connection.error"));
        ssh.setJumpCredential("");

        if (ssh.jumpProfileId) {
          const jumpProfile = sshProfiles.find((profile) => profile.id === ssh.jumpProfileId);
          if (!jumpProfile) {
            throw new Error(t("connection.jump_profile_not_found", { profile: ssh.jumpProfileId }));
          }
          if (jumpProfile.jump_profile_id) {
            throw new Error(t("connection.jump_profile_nested"));
          }
          if (!jumpProfile.host || !jumpProfile.username) {
            throw new Error(
              t("connection.jump_profile_incomplete", { profile: ssh.jumpProfileId })
            );
          }

          const jumpResult = await invoke<HostKeyCheckResult>("ssh_probe_host_key", {
            options: {
              host: jumpProfile.host,
              port: jumpProfile.port ?? 22,
              jumpProfileId: null,
              jumpPassword: "",
              jumpKeyPassphrase: "",
              requestId: diagnostics.currentRequestId(),
              diagnosticRole: "jump",
            },
          });
          if (jumpResult.status === "trusted") {
            await continueAfterJumpTrusted(sshPort);
            return;
          }
          setHostKeyCheck({ ...jumpResult, phase: "jump" });
          setBusy(false);
          return;
        }

        await probeTargetHostKey(sshPort, "");
        return;
      }

      const autoLog = await getAutoLogPreference();
      diagnostics.stop();

      if (tab === "telnet") {
        const parsedTelnetPort = parsePort(telnet.port, t("connection.error"));
        const sessionId = await invoke<string>("telnet_connect", {
          host: telnet.host,
          port: parsedTelnetPort,
          cols: 120,
          rows: 30,
          encoding: telnet.encoding,
        });
        if (autoLog) {
          await invoke("logger_start_auto", {
            sessionId,
            connectionType: "telnet",
            target: `${telnet.host}:${parsedTelnetPort}`,
          });
        }
        const connectionInfo: WorkspaceConnectionInfo = {
          kind: "telnet",
          host: telnet.host,
          port: parsedTelnetPort,
        };
        await onConnect(
          "telnet",
          sessionId,
          `${telnet.host}:${parsedTelnetPort}`,
          autoLog,
          telnet.encoding,
          telnet.terminalMode,
          connectionInfo
        );
        recordConnectionHistory({
          connection_info: connectionInfo,
          encoding: telnet.encoding,
          terminal_mode: telnet.terminalMode,
        });
        return;
      }

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
      });
      if (autoLog) {
        await invoke("logger_start_auto", {
          sessionId,
          connectionType: "serial",
          target: serial.selectedPort,
        });
      }
      await onConnect(
        "serial",
        sessionId,
        serial.selectedPort,
        autoLog,
        "utf-8",
        serial.terminalMode
      );
    } catch (error: unknown) {
      setError(getConnectionErrorMessage(error, t("connection.error")));
      setBusy(false);
    }
  }, [
    connectingRef,
    continueAfterJumpTrusted,
    diagnostics,
    onConnect,
    probeTargetHostKey,
    serial,
    setBusy,
    setError,
    setHostKeyCheck,
    ssh,
    sshProfiles,
    tab,
    telnet,
    t,
  ]);

  return {
    handleConnect,
    handleCredentialSubmit,
    handleTrustAndConnect,
  };
};
