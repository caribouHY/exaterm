import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, X } from "lucide-react";
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
  ) => void;
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
  port: number;
  authMethod: SshAuthMethod;
  value: string;
  error: string;
}

const normalizeEncoding = (encoding: string | null | undefined): Encoding => {
  return SSH_ENCODINGS.some((entry) => entry.value === encoding) ? (encoding as Encoding) : "utf-8";
};

const normalizeSshAuthMethod = (authMethod: string | null | undefined): SshAuthMethod => {
  return authMethod === "public_key" ? "public_key" : "password";
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
  const [tab, setTab] = useState<ConnectionType>("ssh");
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState("");
  const [hostKeyCheck, setHostKeyCheck] = useState<HostKeyCheckResult | null>(null);
  const [credentialPrompt, setCredentialPrompt] = useState<SshCredentialPrompt | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [selectedProfileIds, setSelectedProfileIds] = useState({ ssh: "", telnet: "" });
  const [sshProfileName, setSshProfileName] = useState("");
  const [telnetProfileName, setTelnetProfileName] = useState("");
  const [pendingStartupConnect, setPendingStartupConnect] = useState(false);

  // SSH fields
  const [host, setHost] = useState("192.168.1.1");
  const [port, setPort] = useState("22");
  const [username, setUsername] = useState("admin");
  const [authMethod, setAuthMethod] = useState<SshAuthMethod>("password");
  const [privateKeyPath, setPrivateKeyPath] = useState("");
  const [encoding, setEncoding] = useState<Encoding>("utf-8");
  const [sshTerminalMode, setSshTerminalMode] = useState<TerminalMode>(DEFAULT_TERMINAL_MODE);

  // Telnet fields
  const [telnetHost, setTelnetHost] = useState("192.168.1.1");
  const [telnetPort, setTelnetPort] = useState("23");
  const [telnetEncoding, setTelnetEncoding] = useState<Encoding>("utf-8");
  const [telnetTerminalMode, setTelnetTerminalMode] = useState<TerminalMode>(DEFAULT_TERMINAL_MODE);

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
  const telnetProfiles = (config?.saved_connections ?? []).filter(
    (connection) => connection.connection_type === "telnet"
  );

  const getAutoLogPreference = async () => {
    try {
      const cfg = await invoke<AppConfig>("config_load");
      return cfg.terminal.auto_session_log;
    } catch {
      return false;
    }
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
    setEncoding(normalizeEncoding(profile.encoding));
    setSshTerminalMode(normalizeTerminalMode(profile.terminal_mode));
  }, []);

  const handleSelectSshProfile = (id: string) => {
    setSelectedProfileIds((current) => ({ ...current, ssh: id }));
    if (!id) {
      setSshProfileName("");
      setAuthMethod("password");
      setPrivateKeyPath("");
      setEncoding("utf-8");
      setSshTerminalMode(DEFAULT_TERMINAL_MODE);
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
      return;
    }

    const profile = telnetProfiles.find((entry) => entry.id === id);
    if (!profile) return;

    setTelnetProfileName(profile.id);
    setTelnetHost(profile.host ?? "");
    setTelnetPort(profile.port ? String(profile.port) : "23");
    setTelnetEncoding(normalizeEncoding(profile.encoding));
    setTelnetTerminalMode(normalizeTerminalMode(profile.terminal_mode));
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
      const nextProfile: SavedConnection = {
        id,
        connection_type: "ssh",
        host: trimmedHost,
        port: sshPort,
        username: trimmedUsername,
        auth_method: authMethod,
        private_key_path: authMethod === "public_key" ? privateKeyPath.trim() : null,
        encoding,
        terminal_mode: sshTerminalMode,
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
        setSshTerminalMode(DEFAULT_TERMINAL_MODE);
      } else {
        setTelnetProfileName("");
        setTelnetTerminalMode(DEFAULT_TERMINAL_MODE);
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

  const openCredentialPrompt = (sshPort: number) => {
    setCredentialPrompt({
      port: sshPort,
      authMethod,
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
    promptAuthMethod: SshAuthMethod
  ) => {
    const result = await invoke<{ session_id: string }>("ssh_connect", {
      host,
      port: sshPort,
      username,
      password: promptAuthMethod === "password" ? credential : "",
      authMethod: promptAuthMethod,
      privateKeyPath,
      keyPassphrase: promptAuthMethod === "public_key" ? credential : "",
      cols: 120,
      rows: 30,
    });
    if (autoLog) {
      await invoke("logger_start_auto", {
        sessionId: result.session_id,
        connectionType: "ssh",
        target: `${username}@${host}:${sshPort}`,
      });
    }
    onConnect("ssh", result.session_id, `${username}@${host}`, autoLog, encoding, sshTerminalMode);
  };

  const handleCredentialSubmit = async () => {
    if (!credentialPrompt || connectingRef.current) return;

    setCredentialPrompt({ ...credentialPrompt, error: "" });
    connectingRef.current = true;
    setConnecting(true);
    try {
      const autoLog = await getAutoLogPreference();
      await performSshConnect(
        autoLog,
        credentialPrompt.port,
        credentialPrompt.value,
        credentialPrompt.authMethod
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
      setHostKeyCheck(null);
      openCredentialPrompt(hostKeyCheck.port);
      connectingRef.current = false;
      setConnecting(false);
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
        const sshPort = Number.parseInt(port, 10);
        if (Number.isNaN(sshPort)) {
          throw new Error(t("connection.error"));
        }

        const result = await invoke<HostKeyCheckResult>("ssh_probe_host_key", {
          host,
          port: sshPort,
        });

        if (result.status === "trusted") {
          openCredentialPrompt(sshPort);
          connectingRef.current = false;
          setConnecting(false);
          return;
        }

        setHostKeyCheck(result);
        connectingRef.current = false;
        setConnecting(false);
        return;
      }

      const autoLog = await getAutoLogPreference();

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
        });
        if (autoLog) {
          await invoke("logger_start_auto", {
            sessionId,
            connectionType: "telnet",
            target: `${telnetHost}:${parsedTelnetPort}`,
          });
        }
        onConnect(
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
      });
      if (autoLog) {
        await invoke("logger_start_auto", {
          sessionId,
          connectionType: "serial",
          target: selectedPort,
        });
      }
      onConnect("serial", sessionId, selectedPort, autoLog, "utf-8", serialTerminalMode);
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
    if (startupRequest.target_kind === "profile" && !config) return;

    startupRequestHandledRef.current = true;
    onStartupRequestHandled?.();
    setTab("ssh");
    setError("");

    if (startupRequest.target_kind === "direct") {
      setSelectedProfileIds((current) => ({ ...current, ssh: "" }));
      setSshProfileName("");
      setHost(startupRequest.host ?? "");
      setUsername(startupRequest.username ?? "");
      setPort(String(startupRequest.port ?? 22));
      setAuthMethod("password");
      setPrivateKeyPath("");
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
  }, [applySshProfile, config, onStartupRequestHandled, sshProfiles, startupRequest, t]);

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
              {username}@{host}:{credentialPrompt.port}
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
