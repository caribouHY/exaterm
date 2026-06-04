import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { ChevronDown, ChevronRight, Copy, FolderOpen, X } from "lucide-react";
import type {
  AppConfig,
  ConnectionType,
  Encoding,
  HostKeyCheckResult,
  PortInfo,
  SavedConnection,
  StartupCliRequest,
  SshAuthMethod,
  TerminalMode,
} from "../../types";
import {
  DEFAULT_TERMINAL_MODE,
  normalizeTerminalMode,
  TERMINAL_MODE_OPTIONS,
} from "../../utils/terminalModes";
import { useTranslation } from "react-i18next";
import "./ConnectionDialog.css";

interface ConnectionDialogProps {
  startupRequest?: StartupCliRequest | null;
  onStartupRequestHandled?: () => void;
  onClose: () => void;
  onConnect: (
    type: ConnectionType,
    sessionId: string,
    title: string,
    isAutoLogging: boolean,
    encoding?: Encoding,
    terminalMode?: TerminalMode
  ) => void | Promise<void>;
}

const SSH_ENCODINGS: { label: string; value: Encoding }[] = [
  { label: "UTF-8", value: "utf-8" },
  { label: "Shift-JIS", value: "shift-jis" },
  { label: "EUC-JP", value: "euc-jp" },
];

const SSH_AUTH_METHODS: { labelKey: string; value: SshAuthMethod }[] = [
  { labelKey: "connection.auth_password", value: "password" },
  { labelKey: "connection.auth_public_key", value: "public_key" },
];

const PRIVATE_KEY_PLACEHOLDER = "C:\\Users\\user\\.ssh\\id_ed25519";

interface SshCredentialPrompt {
  phase: "jump" | "target";
  host: string;
  port: number;
  targetPort?: number;
  username: string;
  authMethod: SshAuthMethod;
  privateKeyPath: string;
  value: string;
  error: string;
}

type SshHostKeyCheck = HostKeyCheckResult & {
  phase: "jump" | "target";
};

interface SshDiagnosticEvent {
  level: "info" | "error";
  message: string;
}

type SshDiagnosticEntry = SshDiagnosticEvent & {
  id: number;
  time: string;
};

const createRequestId = () => {
  return (
    globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`
  );
};

const normalizeEncoding = (encoding: string | null | undefined): Encoding => {
  return SSH_ENCODINGS.some((entry) => entry.value === encoding) ? (encoding as Encoding) : "utf-8";
};

const normalizeSshAuthMethod = (authMethod: string | null | undefined): SshAuthMethod => {
  return authMethod === "public_key" ? "public_key" : "password";
};

const normalizeProfileMemo = (memo: string): string | null => {
  const trimmed = memo.trim();
  return trimmed ? trimmed : null;
};

export default function ConnectionDialog({
  startupRequest,
  onStartupRequestHandled,
  onClose,
  onConnect,
}: ConnectionDialogProps) {
  const { t } = useTranslation();
  const overlayMouseDownStartedRef = useRef(false);
  const connectingRef = useRef(false);
  const startupRequestHandledRef = useRef(false);
  const sshDiagnosticRequestIdRef = useRef<string | null>(null);
  const sshDiagnosticUnlistenRef = useRef<UnlistenFn | null>(null);
  const sshDiagnosticEntryIdRef = useRef(0);
  const [tab, setTab] = useState<ConnectionType>("ssh");
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState("");
  const [hostKeyCheck, setHostKeyCheck] = useState<SshHostKeyCheck | null>(null);
  const [credentialPrompt, setCredentialPrompt] = useState<SshCredentialPrompt | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [selectedProfileIds, setSelectedProfileIds] = useState({ ssh: "", telnet: "" });
  const [sshProfileName, setSshProfileName] = useState("");
  const [telnetProfileName, setTelnetProfileName] = useState("");
  const [pendingStartupConnect, setPendingStartupConnect] = useState(false);
  const [sshDiagnosticLogs, setSshDiagnosticLogs] = useState<SshDiagnosticEntry[]>([]);
  const [sshDiagnosticsExpanded, setSshDiagnosticsExpanded] = useState(false);
  const [sshDiagnosticsCopied, setSshDiagnosticsCopied] = useState(false);

  // SSH fields
  const [host, setHost] = useState("192.168.1.1");
  const [port, setPort] = useState("22");
  const [username, setUsername] = useState("admin");
  const [authMethod, setAuthMethod] = useState<SshAuthMethod>("password");
  const [privateKeyPath, setPrivateKeyPath] = useState("");
  const [jumpProfileId, setJumpProfileId] = useState("");
  const [jumpCredential, setJumpCredential] = useState("");
  const [encoding, setEncoding] = useState<Encoding>("utf-8");
  const [sshTerminalMode, setSshTerminalMode] = useState<TerminalMode>(DEFAULT_TERMINAL_MODE);
  const [sshMemo, setSshMemo] = useState("");

  // Telnet fields
  const [telnetHost, setTelnetHost] = useState("192.168.1.1");
  const [telnetPort, setTelnetPort] = useState("23");
  const [telnetEncoding, setTelnetEncoding] = useState<Encoding>("utf-8");
  const [telnetTerminalMode, setTelnetTerminalMode] = useState<TerminalMode>(DEFAULT_TERMINAL_MODE);
  const [telnetMemo, setTelnetMemo] = useState("");

  // Serial fields
  const [ports, setPorts] = useState<PortInfo[]>([]);
  const [selectedPort, setSelectedPort] = useState("");
  const [baudRate, setBaudRate] = useState("9600");
  const [dataBits, setDataBits] = useState("8");
  const [parity, setParity] = useState("none");
  const [stopBits, setStopBits] = useState("1");
  const [serialTerminalMode, setSerialTerminalMode] = useState<TerminalMode>(DEFAULT_TERMINAL_MODE);

  useEffect(() => {
    connectingRef.current = connecting;
  }, [connecting]);

  const loadConfig = useCallback(async () => {
    const loaded = await invoke<AppConfig>("config_load");
    setConfig(loaded);
    return loaded;
  }, []);

  useEffect(() => {
    loadConfig().catch(() => {});
  }, [loadConfig]);

  useEffect(() => {
    if (tab === "serial") {
      invoke<PortInfo[]>("serial_list_ports")
        .then((p) => {
          setPorts(p);
          if (p.length > 0 && !selectedPort) setSelectedPort(p[0].name);
        })
        .catch(() => {});
    }
  }, [selectedPort, tab]);

  const sshProfiles = (config?.saved_connections ?? []).filter(
    (connection) => connection.connection_type === "ssh"
  );
  const jumpProfileOptions = sshProfiles.filter((profile) => profile.id !== selectedProfileIds.ssh);
  const telnetProfiles = (config?.saved_connections ?? []).filter(
    (connection) => connection.connection_type === "telnet"
  );

  useEffect(() => {
    if (!jumpProfileId) return;
    if (jumpProfileId === selectedProfileIds.ssh) {
      setJumpProfileId("");
      return;
    }
    if (!sshProfiles.some((profile) => profile.id === jumpProfileId)) {
      setJumpProfileId("");
    }
  }, [jumpProfileId, selectedProfileIds.ssh, sshProfiles]);

  const getAutoLogPreference = async () => {
    try {
      const cfg = await invoke<AppConfig>("config_load");
      return cfg.terminal.auto_session_log;
    } catch {
      return false;
    }
  };

  const stopSshDiagnostics = useCallback(() => {
    sshDiagnosticRequestIdRef.current = null;
    sshDiagnosticUnlistenRef.current?.();
    sshDiagnosticUnlistenRef.current = null;
  }, []);

  const startSshDiagnostics = useCallback(async () => {
    stopSshDiagnostics();
    const requestId = createRequestId();
    sshDiagnosticRequestIdRef.current = requestId;
    sshDiagnosticEntryIdRef.current = 0;
    setSshDiagnosticLogs([]);
    setSshDiagnosticsCopied(false);

    const unlisten = await listen<SshDiagnosticEvent>(
      `ssh://connect-diagnostic/${requestId}`,
      (event) => {
        const entryId = sshDiagnosticEntryIdRef.current + 1;
        sshDiagnosticEntryIdRef.current = entryId;
        setSshDiagnosticLogs((current) => [
          ...current,
          {
            id: entryId,
            level: event.payload.level,
            message: event.payload.message,
            time: new Date().toLocaleTimeString(),
          },
        ]);
      }
    );
    sshDiagnosticUnlistenRef.current = unlisten;
    return requestId;
  }, [stopSshDiagnostics]);

  useEffect(() => {
    return () => {
      stopSshDiagnostics();
    };
  }, [stopSshDiagnostics]);

  const currentSshRequestId = () => sshDiagnosticRequestIdRef.current;

  const copySshDiagnostics = async () => {
    if (sshDiagnosticLogs.length === 0) return;
    if (!navigator.clipboard) return;
    const text = sshDiagnosticLogs
      .map((entry) => `[${entry.time}] ${entry.level}: ${entry.message}`)
      .join("\n");
    await navigator.clipboard.writeText(text);
    setSshDiagnosticsCopied(true);
  };

  const getProfileDisplayName = (profile: SavedConnection) => {
    return profile.id || t("connection.unnamed_profile");
  };

  const applySshProfile = useCallback((profile: SavedConnection, overridePort?: number | null) => {
    setSelectedProfileIds((current) => ({ ...current, ssh: profile.id }));
    setSshProfileName(profile.id);
    setHost(profile.host ?? "");
    setPort(String(overridePort ?? profile.port ?? 22));
    setUsername(profile.username ?? "");
    setAuthMethod(normalizeSshAuthMethod(profile.auth_method));
    setPrivateKeyPath(profile.private_key_path ?? "");
    setJumpProfileId(profile.jump_profile_id ?? "");
    setJumpCredential("");
    setEncoding(normalizeEncoding(profile.encoding));
    setSshTerminalMode(normalizeTerminalMode(profile.terminal_mode));
    setSshMemo(profile.memo ?? "");
  }, []);

  const applyTelnetProfile = useCallback(
    (profile: SavedConnection, overridePort?: number | null) => {
      setSelectedProfileIds((current) => ({ ...current, telnet: profile.id }));
      setTelnetProfileName(profile.id);
      setTelnetHost(profile.host ?? "");
      setTelnetPort(String(overridePort ?? profile.port ?? 23));
      setTelnetEncoding(normalizeEncoding(profile.encoding));
      setTelnetTerminalMode(normalizeTerminalMode(profile.terminal_mode));
      setTelnetMemo(profile.memo ?? "");
    },
    []
  );

  const handleSelectSshProfile = (id: string) => {
    setSelectedProfileIds((current) => ({ ...current, ssh: id }));
    if (!id) {
      setSshProfileName("");
      setAuthMethod("password");
      setPrivateKeyPath("");
      setJumpProfileId("");
      setJumpCredential("");
      setEncoding("utf-8");
      setSshTerminalMode(DEFAULT_TERMINAL_MODE);
      setSshMemo("");
      return;
    }

    const profile = sshProfiles.find((entry) => entry.id === id);
    if (!profile) return;

    applySshProfile(profile);
  };

  const handleSelectTelnetProfile = (id: string) => {
    setSelectedProfileIds((current) => ({ ...current, telnet: id }));
    if (!id) {
      setTelnetProfileName("");
      setTelnetEncoding("utf-8");
      setTelnetTerminalMode(DEFAULT_TERMINAL_MODE);
      setTelnetMemo("");
      return;
    }

    const profile = telnetProfiles.find((entry) => entry.id === id);
    if (!profile) return;

    applyTelnetProfile(profile);
  };

  const handleSaveSshProfile = async () => {
    setError("");
    try {
      const sshPort = Number.parseInt(port, 10);
      if (Number.isNaN(sshPort)) {
        throw new Error(t("connection.error"));
      }

      const loaded = config ?? (await loadConfig());
      const trimmedHost = host.trim();
      const trimmedUsername = username.trim();
      const id = sshProfileName.trim();
      if (!id) {
        throw new Error(t("connection.profile_name_required"));
      }
      if (jumpProfileId && jumpProfileId === id) {
        throw new Error(t("connection.jump_profile_self"));
      }
      const nextProfile: SavedConnection = {
        id,
        connection_type: "ssh",
        host: trimmedHost,
        port: sshPort,
        username: trimmedUsername,
        auth_method: authMethod,
        private_key_path: authMethod === "public_key" ? privateKeyPath.trim() : null,
        jump_profile_id: jumpProfileId || null,
        encoding,
        terminal_mode: sshTerminalMode,
        memo: normalizeProfileMemo(sshMemo),
      };
      const existingConnections = loaded.saved_connections ?? [];
      const duplicateProfile = existingConnections.some(
        (entry) =>
          entry.connection_type === "ssh" && entry.id === id && entry.id !== selectedProfileIds.ssh
      );
      if (duplicateProfile) {
        throw new Error(t("connection.profile_duplicate"));
      }

      const isUpdatingSelectedProfile = Boolean(selectedProfileIds.ssh);
      const nextConfig: AppConfig = {
        ...loaded,
        saved_connections: isUpdatingSelectedProfile
          ? existingConnections.map((entry) =>
              entry.connection_type === "ssh" && entry.id === selectedProfileIds.ssh
                ? nextProfile
                : entry
            )
          : [...existingConnections, nextProfile],
      };

      await invoke("config_save", { config: nextConfig });
      setConfig(nextConfig);
      setSelectedProfileIds((current) => ({ ...current, ssh: id }));
      setSshProfileName(nextProfile.id);
    } catch (e: unknown) {
      const message =
        typeof e === "string" ? e : e instanceof Error ? e.message : t("connection.error");
      setError(message);
    }
  };

  const handleSaveTelnetProfile = async () => {
    setError("");
    try {
      const parsedTelnetPort = Number.parseInt(telnetPort, 10);
      if (Number.isNaN(parsedTelnetPort)) {
        throw new Error(t("connection.error"));
      }

      const loaded = config ?? (await loadConfig());
      const trimmedHost = telnetHost.trim();
      const id = telnetProfileName.trim();
      if (!id) {
        throw new Error(t("connection.profile_name_required"));
      }

      const nextProfile: SavedConnection = {
        id,
        connection_type: "telnet",
        host: trimmedHost,
        port: parsedTelnetPort,
        encoding: telnetEncoding,
        terminal_mode: telnetTerminalMode,
        memo: normalizeProfileMemo(telnetMemo),
      };
      const existingConnections = loaded.saved_connections ?? [];
      const duplicateProfile = existingConnections.some(
        (entry) =>
          entry.connection_type === "telnet" &&
          entry.id === id &&
          entry.id !== selectedProfileIds.telnet
      );
      if (duplicateProfile) {
        throw new Error(t("connection.profile_duplicate"));
      }

      const isUpdatingSelectedProfile = Boolean(selectedProfileIds.telnet);
      const nextConfig: AppConfig = {
        ...loaded,
        saved_connections: isUpdatingSelectedProfile
          ? existingConnections.map((entry) =>
              entry.connection_type === "telnet" && entry.id === selectedProfileIds.telnet
                ? nextProfile
                : entry
            )
          : [...existingConnections, nextProfile],
      };

      await invoke("config_save", { config: nextConfig });
      setConfig(nextConfig);
      setSelectedProfileIds((current) => ({ ...current, telnet: id }));
      setTelnetProfileName(nextProfile.id);
    } catch (e: unknown) {
      const message =
        typeof e === "string" ? e : e instanceof Error ? e.message : t("connection.error");
      setError(message);
    }
  };

  const handleDeleteProfile = async (connectionType: "ssh" | "telnet") => {
    const selectedProfileId = selectedProfileIds[connectionType];
    if (!selectedProfileId) return;

    setError("");
    try {
      const loaded = config ?? (await loadConfig());
      const nextConfig: AppConfig = {
        ...loaded,
        saved_connections: (loaded.saved_connections ?? []).filter(
          (entry) => entry.connection_type !== connectionType || entry.id !== selectedProfileId
        ),
      };

      await invoke("config_save", { config: nextConfig });
      setConfig(nextConfig);
      setSelectedProfileIds((current) => ({ ...current, [connectionType]: "" }));
      if (connectionType === "ssh") {
        setSshProfileName("");
        setAuthMethod("password");
        setPrivateKeyPath("");
        setJumpProfileId("");
        setJumpCredential("");
        setSshTerminalMode(DEFAULT_TERMINAL_MODE);
        setSshMemo("");
      } else {
        setTelnetProfileName("");
        setTelnetTerminalMode(DEFAULT_TERMINAL_MODE);
        setTelnetMemo("");
      }
    } catch (e: unknown) {
      const message =
        typeof e === "string" ? e : e instanceof Error ? e.message : t("connection.error");
      setError(message);
    }
  };

  const selectSshAuthFile = async () => {
    try {
      const selected = await open({
        multiple: false,
      });
      if (!selected || Array.isArray(selected)) return;

      setPrivateKeyPath(selected);
    } catch {
      // Dialog cancellation and platform errors should not disturb the form.
    }
  };

  const openCredentialPrompt = (
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
  };

  const closeCredentialPrompt = () => {
    if (connectingRef.current) return;
    setCredentialPrompt(null);
  };

  const performSshConnect = async (
    autoLog: boolean,
    sshPort: number,
    credential: string,
    promptAuthMethod: SshAuthMethod,
    currentJumpCredential: string
  ) => {
    const jumpProfile = sshProfiles.find((profile) => profile.id === jumpProfileId);
    const jumpAuthMethod = normalizeSshAuthMethod(jumpProfile?.auth_method);
    const result = await invoke<{ session_id: string }>("ssh_connect", {
      host,
      port: sshPort,
      username,
      password: promptAuthMethod === "password" ? credential : "",
      authMethod: promptAuthMethod,
      privateKeyPath,
      keyPassphrase: promptAuthMethod === "public_key" ? credential : "",
      jumpProfileId: jumpProfileId || null,
      jumpPassword: jumpAuthMethod === "password" ? currentJumpCredential : "",
      jumpKeyPassphrase: jumpAuthMethod === "public_key" ? currentJumpCredential : "",
      cols: 120,
      rows: 30,
      encoding,
      requestId: currentSshRequestId(),
    });
    if (autoLog) {
      await invoke("logger_start_auto", {
        sessionId: result.session_id,
        connectionType: "ssh",
        target: `${username}@${host}:${sshPort}`,
      });
    }
    await onConnect(
      "ssh",
      result.session_id,
      `${username}@${host}`,
      autoLog,
      encoding,
      sshTerminalMode
    );
  };

  const continueSshConnect = async (sshPort: number, currentJumpCredential = jumpCredential) => {
    if (authMethod === "password") {
      openCredentialPrompt("target", host, sshPort, username, authMethod, privateKeyPath);
      connectingRef.current = false;
      setConnecting(false);
      return;
    }

    const requiresPassphrase = await invoke<boolean>("ssh_private_key_requires_passphrase", {
      privateKeyPath,
    });
    if (requiresPassphrase) {
      openCredentialPrompt("target", host, sshPort, username, authMethod, privateKeyPath);
      connectingRef.current = false;
      setConnecting(false);
      return;
    }

    const autoLog = await getAutoLogPreference();
    await performSshConnect(autoLog, sshPort, "", "public_key", currentJumpCredential);
  };

  const probeTargetHostKey = async (sshPort: number, currentJumpCredential = jumpCredential) => {
    const jumpProfile = sshProfiles.find((profile) => profile.id === jumpProfileId);
    const jumpAuthMethod = normalizeSshAuthMethod(jumpProfile?.auth_method);
    const result = await invoke<HostKeyCheckResult>("ssh_probe_host_key", {
      host,
      port: sshPort,
      jumpProfileId: jumpProfileId || null,
      jumpPassword: jumpAuthMethod === "password" ? currentJumpCredential : "",
      jumpKeyPassphrase: jumpAuthMethod === "public_key" ? currentJumpCredential : "",
      requestId: currentSshRequestId(),
      diagnosticRole: "target",
    });

    if (result.status === "trusted") {
      await continueSshConnect(sshPort, currentJumpCredential);
      return;
    }

    setJumpCredential(currentJumpCredential);
    setHostKeyCheck({ ...result, phase: "target" });
    connectingRef.current = false;
    setConnecting(false);
  };

  const continueAfterJumpTrusted = async (sshPort: number) => {
    const jumpProfile = sshProfiles.find((profile) => profile.id === jumpProfileId);
    if (!jumpProfileId || !jumpProfile) {
      await probeTargetHostKey(sshPort, "");
      return;
    }

    const jumpPort = jumpProfile.port ?? 22;
    const jumpUsername = jumpProfile.username ?? "";
    const jumpPrivateKeyPath = jumpProfile.private_key_path ?? "";
    const jumpAuthMethod = normalizeSshAuthMethod(jumpProfile.auth_method);
    if (!jumpProfile.host || !jumpUsername) {
      throw new Error(t("connection.jump_profile_incomplete", { profile: jumpProfileId }));
    }

    if (jumpAuthMethod === "password") {
      openCredentialPrompt(
        "jump",
        jumpProfile.host,
        jumpPort,
        jumpUsername,
        jumpAuthMethod,
        jumpPrivateKeyPath,
        sshPort
      );
      connectingRef.current = false;
      setConnecting(false);
      return;
    }

    const requiresPassphrase = await invoke<boolean>("ssh_private_key_requires_passphrase", {
      privateKeyPath: jumpPrivateKeyPath,
    });
    if (requiresPassphrase) {
      openCredentialPrompt(
        "jump",
        jumpProfile.host,
        jumpPort,
        jumpUsername,
        jumpAuthMethod,
        jumpPrivateKeyPath,
        sshPort
      );
      connectingRef.current = false;
      setConnecting(false);
      return;
    }

    setJumpCredential("");
    await probeTargetHostKey(sshPort, "");
  };

  const handleCredentialSubmit = async () => {
    if (!credentialPrompt || connectingRef.current) return;

    setCredentialPrompt({ ...credentialPrompt, error: "" });
    connectingRef.current = true;
    setConnecting(true);
    try {
      if (credentialPrompt.phase === "jump") {
        setCredentialPrompt(null);
        setJumpCredential(credentialPrompt.value);
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
        jumpCredential
      );
      setCredentialPrompt(null);
    } catch (e: unknown) {
      const message =
        typeof e === "string" ? e : e instanceof Error ? e.message : t("connection.error");
      setCredentialPrompt({
        ...credentialPrompt,
        value: "",
        error: message,
      });
      connectingRef.current = false;
      setConnecting(false);
    }
  };

  const handleTrustAndConnect = async (replace: boolean) => {
    if (!hostKeyCheck || connectingRef.current) return;

    setError("");
    connectingRef.current = true;
    setConnecting(true);
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
        await continueAfterJumpTrusted(Number.parseInt(port, 10));
      } else {
        await continueSshConnect(checkedPort);
      }
    } catch (e: unknown) {
      const message =
        typeof e === "string" ? e : e instanceof Error ? e.message : t("connection.error");
      setError(message);
      connectingRef.current = false;
      setConnecting(false);
    }
  };

  const handleConnect = async () => {
    if (connectingRef.current) return;

    setError("");
    connectingRef.current = true;
    setConnecting(true);
    try {
      if (tab === "ssh") {
        await startSshDiagnostics();
        const sshPort = Number.parseInt(port, 10);
        if (Number.isNaN(sshPort)) {
          throw new Error(t("connection.error"));
        }

        setJumpCredential("");
        if (jumpProfileId) {
          const jumpProfile = sshProfiles.find((profile) => profile.id === jumpProfileId);
          if (!jumpProfile) {
            throw new Error(t("connection.jump_profile_not_found", { profile: jumpProfileId }));
          }
          if (jumpProfile.jump_profile_id) {
            throw new Error(t("connection.jump_profile_nested"));
          }
          if (!jumpProfile.host || !jumpProfile.username) {
            throw new Error(t("connection.jump_profile_incomplete", { profile: jumpProfileId }));
          }
          const jumpPort = jumpProfile.port ?? 22;
          const jumpResult = await invoke<HostKeyCheckResult>("ssh_probe_host_key", {
            host: jumpProfile.host,
            port: jumpPort,
            jumpProfileId: null,
            jumpPassword: "",
            jumpKeyPassphrase: "",
            requestId: currentSshRequestId(),
            diagnosticRole: "jump",
          });
          if (jumpResult.status === "trusted") {
            await continueAfterJumpTrusted(sshPort);
            return;
          }
          setHostKeyCheck({ ...jumpResult, phase: "jump" });
          connectingRef.current = false;
          setConnecting(false);
          return;
        }

        await probeTargetHostKey(sshPort, "");
        return;
      }

      const autoLog = await getAutoLogPreference();
      stopSshDiagnostics();

      if (tab === "telnet") {
        const parsedTelnetPort = Number.parseInt(telnetPort, 10);
        if (Number.isNaN(parsedTelnetPort)) {
          throw new Error(t("connection.error"));
        }

        const sessionId = await invoke<string>("telnet_connect", {
          host: telnetHost,
          port: parsedTelnetPort,
          cols: 120,
          rows: 30,
          encoding: telnetEncoding,
        });
        if (autoLog) {
          await invoke("logger_start_auto", {
            sessionId,
            connectionType: "telnet",
            target: `${telnetHost}:${parsedTelnetPort}`,
          });
        }
        await onConnect(
          "telnet",
          sessionId,
          `${telnetHost}:${parsedTelnetPort}`,
          autoLog,
          telnetEncoding,
          telnetTerminalMode
        );
        return;
      }

      const sessionId = await invoke<string>("serial_connect", {
        port: selectedPort,
        config: {
          baud_rate: Number.parseInt(baudRate, 10),
          data_bits: Number.parseInt(dataBits, 10),
          parity,
          stop_bits: Number.parseInt(stopBits, 10),
          flow_control: "none",
        },
        encoding: "utf-8",
      });
      if (autoLog) {
        await invoke("logger_start_auto", {
          sessionId,
          connectionType: "serial",
          target: selectedPort,
        });
      }
      await onConnect("serial", sessionId, selectedPort, autoLog, "utf-8", serialTerminalMode);
    } catch (e: unknown) {
      const message =
        typeof e === "string" ? e : e instanceof Error ? e.message : t("connection.error");
      setError(message);
      connectingRef.current = false;
      setConnecting(false);
    }
  };

  useEffect(() => {
    if (!startupRequest || startupRequestHandledRef.current) return;
    if (startupRequest.kind === "ssh" && startupRequest.target_kind === "profile" && !config) {
      return;
    }
    if (startupRequest.kind === "telnet" && !config) return;

    startupRequestHandledRef.current = true;
    onStartupRequestHandled?.();
    setError("");

    if (startupRequest.kind === "telnet") {
      setTab("telnet");
      const target = startupRequest.target.trim();
      const profile = telnetProfiles.find((entry) => entry.id === target);
      if (profile) {
        applyTelnetProfile(profile, startupRequest.port);
        if (!profile.host) {
          setError(t("connection.startup_telnet_profile_incomplete", { profile: target }));
          return;
        }
      } else {
        setSelectedProfileIds((current) => ({ ...current, telnet: "" }));
        setTelnetProfileName("");
        setTelnetHost(target);
        setTelnetPort(String(startupRequest.port ?? 23));
        setTelnetEncoding("utf-8");
        setTelnetTerminalMode(DEFAULT_TERMINAL_MODE);
        if (!target) {
          setError(t("connection.error"));
          return;
        }
      }
      setPendingStartupConnect(true);
      return;
    }

    setTab("ssh");

    if (startupRequest.target_kind === "direct") {
      setSelectedProfileIds((current) => ({ ...current, ssh: "" }));
      setSshProfileName("");
      setHost(startupRequest.host ?? "");
      setUsername(startupRequest.username ?? "");
      setPort(String(startupRequest.port ?? 22));
      setAuthMethod("password");
      setPrivateKeyPath("");
      setJumpProfileId("");
      setJumpCredential("");
      setEncoding("utf-8");
      setSshTerminalMode(DEFAULT_TERMINAL_MODE);
      if (!startupRequest.host || !startupRequest.username) {
        setError(t("connection.error"));
        return;
      }
      setPendingStartupConnect(true);
      return;
    }

    const profileName = startupRequest.profile_name ?? "";
    const profile = sshProfiles.find((entry) => entry.id === profileName);
    if (!profile) {
      setSelectedProfileIds((current) => ({ ...current, ssh: "" }));
      setSshProfileName(profileName);
      setError(t("connection.startup_profile_not_found", { profile: profileName }));
      return;
    }

    applySshProfile(profile, startupRequest.port);
    if (!profile.host || !profile.username) {
      setError(t("connection.startup_profile_incomplete", { profile: profileName }));
      return;
    }

    setPendingStartupConnect(true);
  }, [
    applySshProfile,
    applyTelnetProfile,
    config,
    onStartupRequestHandled,
    sshProfiles,
    startupRequest,
    t,
    telnetProfiles,
  ]);

  useEffect(() => {
    if (!pendingStartupConnect) return;
    setPendingStartupConnect(false);
    handleConnect();
  }, [handleConnect, pendingStartupConnect]);

  const hostKeyTitle =
    hostKeyCheck?.status === "mismatch"
      ? t("connection.host_key_mismatch.title")
      : t("connection.host_key_unknown.title");

  const handleOverlayMouseDown = (e: React.MouseEvent<HTMLDivElement>) => {
    overlayMouseDownStartedRef.current = e.target === e.currentTarget;
  };

  const handleOverlayClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (overlayMouseDownStartedRef.current && e.target === e.currentTarget) {
      onClose();
    }
    overlayMouseDownStartedRef.current = false;
  };

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        if (connecting) return;

        if (credentialPrompt) {
          closeCredentialPrompt();
          return;
        }

        if (hostKeyCheck) {
          setHostKeyCheck(null);
          return;
        }

        onClose();
        return;
      }

      if (event.key !== "Enter" || (!event.ctrlKey && !event.metaKey)) return;

      event.preventDefault();
      if (connecting) return;

      if (credentialPrompt) {
        handleCredentialSubmit();
        return;
      }

      if (hostKeyCheck) {
        handleTrustAndConnect(hostKeyCheck.status === "mismatch");
        return;
      }

      handleConnect();
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    connecting,
    credentialPrompt,
    handleConnect,
    handleCredentialSubmit,
    handleTrustAndConnect,
    hostKeyCheck,
    onClose,
  ]);

  const shortcutText = t("connection.shortcut_ctrl_enter");
  const credentialTitle =
    credentialPrompt?.authMethod === "public_key"
      ? t("connection.key_passphrase_prompt_title")
      : t("connection.password_prompt_title");
  const credentialLabel =
    credentialPrompt?.authMethod === "public_key"
      ? t("connection.key_passphrase")
      : t("connection.password");
  const credentialDescription =
    credentialPrompt?.authMethod === "public_key"
      ? t("connection.key_passphrase_prompt_desc")
      : t("connection.password_prompt_desc");
  const renderSshDiagnostics = () => {
    if (sshDiagnosticLogs.length === 0) return null;

    return (
      <div className="connection-dialog__diagnostics">
        <div className="connection-dialog__diagnostics-header">
          <button
            className="connection-dialog__diagnostics-toggle"
            type="button"
            onClick={() => setSshDiagnosticsExpanded((current) => !current)}
          >
            {sshDiagnosticsExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
            <span>{t("connection.ssh_diagnostics")}</span>
          </button>
          <button
            className="btn btn-ghost btn-sm connection-dialog__diagnostics-copy"
            type="button"
            onClick={() => void copySshDiagnostics()}
            title={t("connection.ssh_diagnostics_copy")}
          >
            <Copy size={13} />
            {sshDiagnosticsCopied
              ? t("connection.ssh_diagnostics_copied")
              : t("connection.ssh_diagnostics_copy")}
          </button>
        </div>
        {sshDiagnosticsExpanded && (
          <div className="connection-dialog__diagnostics-log" role="log" aria-live="polite">
            {sshDiagnosticLogs.map((entry) => (
              <div
                key={entry.id}
                className={`connection-dialog__diagnostics-line connection-dialog__diagnostics-line--${entry.level}`}
              >
                <span className="connection-dialog__diagnostics-time">{entry.time}</span>
                <span>{entry.message}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    );
  };

  if (credentialPrompt) {
    return (
      <div
        className="connection-overlay"
        onMouseDown={(e) => {
          if (e.target === e.currentTarget) closeCredentialPrompt();
        }}
      >
        <div className="connection-credential-modal" onClick={(e) => e.stopPropagation()}>
          <div className="connection-credential-modal__header">
            <span className="connection-dialog__title">{credentialTitle}</span>
            <button className="btn-icon" onClick={closeCredentialPrompt} disabled={connecting}>
              <X size={16} />
            </button>
          </div>
          <div className="connection-credential-modal__body">
            <div className="connection-credential-modal__target">
              {credentialPrompt.username}@{credentialPrompt.host}:{credentialPrompt.port}
            </div>
            <div className="connection-credential-modal__description">{credentialDescription}</div>
            <div>
              <label className="label">{credentialLabel}</label>
              <input
                className="input"
                type="password"
                autoFocus
                value={credentialPrompt.value}
                onChange={(e) =>
                  setCredentialPrompt({
                    ...credentialPrompt,
                    value: e.target.value,
                    error: "",
                  })
                }
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    handleCredentialSubmit();
                  }
                }}
              />
            </div>
            {credentialPrompt.error && (
              <div className="connection-dialog__error">{credentialPrompt.error}</div>
            )}
            {renderSshDiagnostics()}
          </div>
          <div className="connection-dialog__footer">
            {connecting ? (
              <div className="connection-dialog__connecting">
                <div className="connection-dialog__spinner" />
                {t("connection.connecting")}
              </div>
            ) : (
              <>
                <button className="btn btn-ghost" onClick={closeCredentialPrompt}>
                  {t("connection.cancel")}
                </button>
                <button className="btn btn-primary" onClick={handleCredentialSubmit}>
                  {t("connection.connect")}
                </button>
              </>
            )}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      className="connection-overlay"
      onMouseDown={handleOverlayMouseDown}
      onClick={handleOverlayClick}
    >
      <div className="connection-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="connection-dialog__header">
          <span className="connection-dialog__title">
            {hostKeyCheck ? hostKeyTitle : t("connection.new")}
          </span>
          <button className="btn-icon" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        {!hostKeyCheck && (
          <div className="connection-dialog__tabs">
            <button
              className={`connection-dialog__tab ${tab === "ssh" ? "connection-dialog__tab--active" : ""}`}
              onClick={() => setTab("ssh")}
            >
              {t("connection.ssh")}
            </button>
            <button
              className={`connection-dialog__tab ${tab === "telnet" ? "connection-dialog__tab--active" : ""}`}
              onClick={() => setTab("telnet")}
            >
              {t("connection.telnet")}
            </button>
            <button
              className={`connection-dialog__tab ${tab === "serial" ? "connection-dialog__tab--active" : ""}`}
              onClick={() => setTab("serial")}
            >
              {t("connection.serial")}
            </button>
          </div>
        )}

        <div className="connection-dialog__body">
          {hostKeyCheck ? (
            <div className="connection-dialog__host-key">
              <div className="connection-dialog__host-key-message">
                {hostKeyCheck.status === "mismatch"
                  ? t("connection.host_key_mismatch.message", {
                      host: hostKeyCheck.host,
                      port: hostKeyCheck.port,
                    })
                  : t("connection.host_key_unknown.message", {
                      host: hostKeyCheck.host,
                      port: hostKeyCheck.port,
                    })}
              </div>

              <div className="connection-dialog__host-key-grid">
                <div className="connection-dialog__host-key-label">{t("connection.host")}</div>
                <div className="connection-dialog__host-key-value">
                  {hostKeyCheck.host}:{hostKeyCheck.port}
                </div>

                <div className="connection-dialog__host-key-label">
                  {t("connection.host_key_algorithm")}
                </div>
                <div className="connection-dialog__host-key-value">{hostKeyCheck.algorithm}</div>

                <div className="connection-dialog__host-key-label">
                  {t("connection.host_key_fingerprint")}
                </div>
                <div className="connection-dialog__host-key-value connection-dialog__host-key-value--mono">
                  SHA256:{hostKeyCheck.fingerprint}
                </div>

                {hostKeyCheck.known_fingerprint && (
                  <>
                    <div className="connection-dialog__host-key-label">
                      {t("connection.host_key_known_fingerprint")}
                    </div>
                    <div className="connection-dialog__host-key-value connection-dialog__host-key-value--mono">
                      SHA256:{hostKeyCheck.known_fingerprint}
                    </div>
                  </>
                )}
              </div>

              <div className="connection-dialog__host-key-warning">
                {t("connection.host_key_warning")}
              </div>
            </div>
          ) : tab === "ssh" ? (
            <>
              <div className="connection-dialog__profile">
                <label className="label">{t("connection.profile")}</label>
                <div className="connection-dialog__profile-row">
                  <select
                    className="select"
                    value={selectedProfileIds.ssh}
                    onChange={(e) => handleSelectSshProfile(e.target.value)}
                  >
                    <option value="">{t("connection.profile_manual")}</option>
                    {sshProfiles.map((profile) => (
                      <option key={profile.id} value={profile.id}>
                        {getProfileDisplayName(profile)}
                      </option>
                    ))}
                  </select>
                  {selectedProfileIds.ssh && (
                    <button
                      className="btn btn-danger btn-sm"
                      onClick={() => handleDeleteProfile("ssh")}
                    >
                      {t("connection.profile_delete")}
                    </button>
                  )}
                </div>
              </div>
              <div>
                <label className="label">{t("connection.profile_name")}</label>
                <input
                  className="input"
                  value={sshProfileName}
                  onChange={(e) => setSshProfileName(e.target.value)}
                  placeholder={t("connection.profile_name_placeholder")}
                />
              </div>
              <div className="connection-dialog__row">
                <div>
                  <label className="label">{t("connection.host")}</label>
                  <input
                    className="input"
                    value={host}
                    onChange={(e) => setHost(e.target.value)}
                    placeholder="192.168.1.1"
                  />
                </div>
                <div style={{ maxWidth: 100 }}>
                  <label className="label">{t("connection.port")}</label>
                  <input
                    className="input"
                    type="number"
                    value={port}
                    onChange={(e) => setPort(e.target.value)}
                  />
                </div>
              </div>
              <div>
                <label className="label">{t("connection.username")}</label>
                <input
                  className="input"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                />
              </div>
              <div>
                <label className="label">{t("connection.auth_method")}</label>
                <select
                  className="select"
                  style={{ width: "100%" }}
                  value={authMethod}
                  onChange={(e) => setAuthMethod(normalizeSshAuthMethod(e.target.value))}
                >
                  {SSH_AUTH_METHODS.map((entry) => (
                    <option key={entry.value} value={entry.value}>
                      {t(entry.labelKey)}
                    </option>
                  ))}
                </select>
              </div>
              {authMethod === "public_key" && (
                <>
                  <div>
                    <label className="label">{t("connection.private_key_path")}</label>
                    <div className="connection-dialog__file-row">
                      <input
                        className="input"
                        value={privateKeyPath}
                        onChange={(e) => setPrivateKeyPath(e.target.value)}
                        placeholder={PRIVATE_KEY_PLACEHOLDER}
                      />
                      <button
                        className="btn btn-ghost btn-sm"
                        type="button"
                        onClick={selectSshAuthFile}
                        title={t("connection.select_file")}
                      >
                        <FolderOpen size={14} />
                        {t("connection.select_file")}
                      </button>
                    </div>
                  </div>
                </>
              )}
              <div>
                <label className="label">{t("connection.jump_profile")}</label>
                <select
                  className="select"
                  style={{ width: "100%" }}
                  value={jumpProfileId}
                  onChange={(e) => setJumpProfileId(e.target.value)}
                >
                  <option value="">{t("connection.jump_profile_none")}</option>
                  {jumpProfileOptions.map((profile) => (
                    <option key={profile.id} value={profile.id}>
                      {getProfileDisplayName(profile)}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="label">{t("connection.encoding")}</label>
                <select
                  className="select"
                  style={{ width: "100%" }}
                  value={encoding}
                  onChange={(e) => setEncoding(normalizeEncoding(e.target.value))}
                >
                  {SSH_ENCODINGS.map((entry) => (
                    <option key={entry.value} value={entry.value}>
                      {entry.label}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="label">{t("connection.terminal_mode")}</label>
                <select
                  className="select"
                  style={{ width: "100%" }}
                  value={sshTerminalMode}
                  onChange={(e) => setSshTerminalMode(normalizeTerminalMode(e.target.value))}
                >
                  {TERMINAL_MODE_OPTIONS.map((entry) => (
                    <option key={entry.value} value={entry.value}>
                      {t(entry.labelKey)}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="label">{t("connection.profile_memo")}</label>
                <textarea
                  className="input connection-dialog__memo"
                  value={sshMemo}
                  onChange={(e) => setSshMemo(e.target.value)}
                  placeholder={t("connection.profile_memo_placeholder")}
                />
                <div className="connection-dialog__field-help">
                  {t("connection.profile_memo_mcp_notice")}
                </div>
              </div>
              <div className="connection-dialog__profile-actions">
                <button className="btn btn-ghost btn-sm" onClick={handleSaveSshProfile}>
                  {selectedProfileIds.ssh
                    ? t("connection.profile_update")
                    : t("connection.profile_save")}
                </button>
                <span>{t("connection.profile_password_notice")}</span>
              </div>
            </>
          ) : tab === "telnet" ? (
            <>
              <div className="connection-dialog__profile">
                <label className="label">{t("connection.profile")}</label>
                <div className="connection-dialog__profile-row">
                  <select
                    className="select"
                    value={selectedProfileIds.telnet}
                    onChange={(e) => handleSelectTelnetProfile(e.target.value)}
                  >
                    <option value="">{t("connection.profile_manual")}</option>
                    {telnetProfiles.map((profile) => (
                      <option key={profile.id} value={profile.id}>
                        {getProfileDisplayName(profile)}
                      </option>
                    ))}
                  </select>
                  {selectedProfileIds.telnet && (
                    <button
                      className="btn btn-danger btn-sm"
                      onClick={() => handleDeleteProfile("telnet")}
                    >
                      {t("connection.profile_delete")}
                    </button>
                  )}
                </div>
              </div>
              <div>
                <label className="label">{t("connection.profile_name")}</label>
                <input
                  className="input"
                  value={telnetProfileName}
                  onChange={(e) => setTelnetProfileName(e.target.value)}
                  placeholder={t("connection.profile_name_placeholder")}
                />
              </div>
              <div className="connection-dialog__row">
                <div>
                  <label className="label">{t("connection.host")}</label>
                  <input
                    className="input"
                    value={telnetHost}
                    onChange={(e) => setTelnetHost(e.target.value)}
                    placeholder="192.168.1.1"
                  />
                </div>
                <div style={{ maxWidth: 100 }}>
                  <label className="label">{t("connection.port")}</label>
                  <input
                    className="input"
                    type="number"
                    value={telnetPort}
                    onChange={(e) => setTelnetPort(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleConnect()}
                  />
                </div>
              </div>
              <div>
                <label className="label">{t("connection.encoding")}</label>
                <select
                  className="select"
                  style={{ width: "100%" }}
                  value={telnetEncoding}
                  onChange={(e) => setTelnetEncoding(normalizeEncoding(e.target.value))}
                >
                  {SSH_ENCODINGS.map((entry) => (
                    <option key={entry.value} value={entry.value}>
                      {entry.label}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="label">{t("connection.terminal_mode")}</label>
                <select
                  className="select"
                  style={{ width: "100%" }}
                  value={telnetTerminalMode}
                  onChange={(e) => setTelnetTerminalMode(normalizeTerminalMode(e.target.value))}
                >
                  {TERMINAL_MODE_OPTIONS.map((entry) => (
                    <option key={entry.value} value={entry.value}>
                      {t(entry.labelKey)}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="label">{t("connection.profile_memo")}</label>
                <textarea
                  className="input connection-dialog__memo"
                  value={telnetMemo}
                  onChange={(e) => setTelnetMemo(e.target.value)}
                  placeholder={t("connection.profile_memo_placeholder")}
                />
                <div className="connection-dialog__field-help">
                  {t("connection.profile_memo_mcp_notice")}
                </div>
              </div>
              <div className="connection-dialog__profile-actions">
                <button className="btn btn-ghost btn-sm" onClick={handleSaveTelnetProfile}>
                  {selectedProfileIds.telnet
                    ? t("connection.profile_update")
                    : t("connection.profile_save")}
                </button>
                <span>{t("connection.profile_password_notice")}</span>
              </div>
            </>
          ) : (
            <>
              <div>
                <label className="label">{t("connection.port")}</label>
                <select
                  className="select"
                  style={{ width: "100%" }}
                  value={selectedPort}
                  onChange={(e) => setSelectedPort(e.target.value)}
                >
                  {ports.length === 0 && <option value="">{t("connection.no_ports")}</option>}
                  {ports.map((p) => (
                    <option key={p.name} value={p.name}>
                      {p.name} ({p.port_type})
                    </option>
                  ))}
                </select>
              </div>
              <div className="connection-dialog__row">
                <div>
                  <label className="label">{t("connection.baud_rate")}</label>
                  <select
                    className="select"
                    style={{ width: "100%" }}
                    value={baudRate}
                    onChange={(e) => setBaudRate(e.target.value)}
                  >
                    {[
                      "300",
                      "1200",
                      "2400",
                      "4800",
                      "9600",
                      "19200",
                      "38400",
                      "57600",
                      "115200",
                    ].map((r) => (
                      <option key={r} value={r}>
                        {r}
                      </option>
                    ))}
                  </select>
                </div>
                <div>
                  <label className="label">{t("connection.data_bits")}</label>
                  <select
                    className="select"
                    style={{ width: "100%" }}
                    value={dataBits}
                    onChange={(e) => setDataBits(e.target.value)}
                  >
                    {["5", "6", "7", "8"].map((d) => (
                      <option key={d} value={d}>
                        {d}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
              <div className="connection-dialog__row">
                <div>
                  <label className="label">{t("connection.parity")}</label>
                  <select
                    className="select"
                    style={{ width: "100%" }}
                    value={parity}
                    onChange={(e) => setParity(e.target.value)}
                  >
                    <option value="none">{t("connection.parity_none")}</option>
                    <option value="odd">{t("connection.parity_odd")}</option>
                    <option value="even">{t("connection.parity_even")}</option>
                  </select>
                </div>
                <div>
                  <label className="label">{t("connection.stop_bits")}</label>
                  <select
                    className="select"
                    style={{ width: "100%" }}
                    value={stopBits}
                    onChange={(e) => setStopBits(e.target.value)}
                  >
                    <option value="1">1</option>
                    <option value="2">2</option>
                  </select>
                </div>
              </div>
              <div>
                <label className="label">{t("connection.terminal_mode")}</label>
                <select
                  className="select"
                  style={{ width: "100%" }}
                  value={serialTerminalMode}
                  onChange={(e) => setSerialTerminalMode(normalizeTerminalMode(e.target.value))}
                >
                  {TERMINAL_MODE_OPTIONS.map((entry) => (
                    <option key={entry.value} value={entry.value}>
                      {t(entry.labelKey)}
                    </option>
                  ))}
                </select>
              </div>
            </>
          )}
          {error && <div className="connection-dialog__error">{error}</div>}
          {tab === "ssh" && renderSshDiagnostics()}
        </div>

        <div className="connection-dialog__footer">
          {connecting ? (
            <div className="connection-dialog__connecting">
              <div className="connection-dialog__spinner" />
              {t("connection.connecting")}
            </div>
          ) : hostKeyCheck ? (
            <>
              <button className="btn btn-ghost" onClick={() => setHostKeyCheck(null)}>
                {t("connection.cancel")}
              </button>
              <button
                className={`btn ${hostKeyCheck.status === "mismatch" ? "btn-danger" : "btn-primary"}`}
                onClick={() => handleTrustAndConnect(hostKeyCheck.status === "mismatch")}
              >
                {hostKeyCheck.status === "mismatch"
                  ? t("connection.host_key_replace_connect")
                  : t("connection.host_key_trust_connect")}{" "}
                <span className="connection-dialog__shortcut">{shortcutText}</span>
              </button>
            </>
          ) : (
            <>
              <button className="btn btn-ghost" onClick={onClose}>
                {t("connection.cancel")}
              </button>
              <button className="btn btn-primary" onClick={handleConnect}>
                {t("connection.connect")}{" "}
                <span className="connection-dialog__shortcut">{shortcutText}</span>
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
