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
  SshAuthMethod,
} from "../../types";
import { useTranslation } from "react-i18next";
import "./ConnectionDialog.css";

interface ConnectionDialogProps {
  onClose: () => void;
  onConnect: (
    type: ConnectionType,
    sessionId: string,
    title: string,
    isAutoLogging: boolean,
    encoding?: Encoding
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

const normalizeEncoding = (encoding: string | null | undefined): Encoding => {
  return SSH_ENCODINGS.some((entry) => entry.value === encoding) ? (encoding as Encoding) : "utf-8";
};

const normalizeSshAuthMethod = (authMethod: string | null | undefined): SshAuthMethod => {
  return authMethod === "public_key" ? "public_key" : "password";
};

export default function ConnectionDialog({ onClose, onConnect }: ConnectionDialogProps) {
  const { t } = useTranslation();
  const overlayMouseDownStartedRef = useRef(false);
  const connectingRef = useRef(false);
  const [tab, setTab] = useState<ConnectionType>("ssh");
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState("");
  const [hostKeyCheck, setHostKeyCheck] = useState<HostKeyCheckResult | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [selectedProfileId, setSelectedProfileId] = useState("");
  const [profileName, setProfileName] = useState("");

  // SSH fields
  const [host, setHost] = useState("192.168.1.1");
  const [port, setPort] = useState("22");
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [authMethod, setAuthMethod] = useState<SshAuthMethod>("password");
  const [privateKeyPath, setPrivateKeyPath] = useState("");
  const [keyPassphrase, setKeyPassphrase] = useState("");
  const [encoding, setEncoding] = useState<Encoding>("utf-8");

  // Telnet fields
  const [telnetHost, setTelnetHost] = useState("192.168.1.1");
  const [telnetPort, setTelnetPort] = useState("23");

  // Serial fields
  const [ports, setPorts] = useState<PortInfo[]>([]);
  const [selectedPort, setSelectedPort] = useState("");
  const [baudRate, setBaudRate] = useState("9600");
  const [dataBits, setDataBits] = useState("8");
  const [parity, setParity] = useState("none");
  const [stopBits, setStopBits] = useState("1");

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

  const handleSelectProfile = (id: string) => {
    setSelectedProfileId(id);
    if (!id) {
      setProfileName("");
      setAuthMethod("password");
      setPrivateKeyPath("");
      setKeyPassphrase("");
      setEncoding("utf-8");
      return;
    }

    const profile = sshProfiles.find((entry) => entry.id === id);
    if (!profile) return;

    setProfileName(profile.id);
    setHost(profile.host ?? "");
    setPort(profile.port ? String(profile.port) : "22");
    setUsername(profile.username ?? "");
    setPassword("");
    setAuthMethod(normalizeSshAuthMethod(profile.auth_method));
    setPrivateKeyPath(profile.private_key_path ?? "");
    setKeyPassphrase("");
    setEncoding(normalizeEncoding(profile.encoding));
  };

  const handleSaveProfile = async () => {
    setError("");
    try {
      const sshPort = Number.parseInt(port, 10);
      if (Number.isNaN(sshPort)) {
        throw new Error(t("connection.error"));
      }

      const loaded = config ?? (await loadConfig());
      const trimmedHost = host.trim();
      const trimmedUsername = username.trim();
      const id = profileName.trim();
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
      };
      const existingConnections = loaded.saved_connections ?? [];
      const duplicateProfile = existingConnections.some(
        (entry) =>
          entry.connection_type === "ssh" && entry.id === id && entry.id !== selectedProfileId
      );
      if (duplicateProfile) {
        throw new Error(t("connection.profile_duplicate"));
      }

      const isUpdatingSelectedProfile = Boolean(selectedProfileId);
      const nextConfig: AppConfig = {
        ...loaded,
        saved_connections: isUpdatingSelectedProfile
          ? existingConnections.map((entry) =>
              entry.connection_type === "ssh" && entry.id === selectedProfileId
                ? nextProfile
                : entry
            )
          : [...existingConnections, nextProfile],
      };

      await invoke("config_save", { config: nextConfig });
      setConfig(nextConfig);
      setSelectedProfileId(id);
      setProfileName(nextProfile.id);
      setKeyPassphrase("");
    } catch (e: unknown) {
      const message =
        typeof e === "string" ? e : e instanceof Error ? e.message : t("connection.error");
      setError(message);
    }
  };

  const handleDeleteProfile = async () => {
    if (!selectedProfileId) return;

    setError("");
    try {
      const loaded = config ?? (await loadConfig());
      const nextConfig: AppConfig = {
        ...loaded,
        saved_connections: (loaded.saved_connections ?? []).filter(
          (entry) => entry.id !== selectedProfileId
        ),
      };

      await invoke("config_save", { config: nextConfig });
      setConfig(nextConfig);
      setSelectedProfileId("");
      setProfileName("");
      setAuthMethod("password");
      setPrivateKeyPath("");
      setKeyPassphrase("");
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

  const performSshConnect = async (autoLog: boolean, sshPort: number) => {
    const result = await invoke<{ session_id: string }>("ssh_connect", {
      host,
      port: sshPort,
      username,
      password,
      authMethod,
      privateKeyPath,
      keyPassphrase,
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
    onConnect("ssh", result.session_id, `${username}@${host}`, autoLog, encoding);
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
      const autoLog = await getAutoLogPreference();
      await performSshConnect(autoLog, hostKeyCheck.port);
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
      const autoLog = await getAutoLogPreference();

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
          await performSshConnect(autoLog, sshPort);
          return;
        }

        setHostKeyCheck(result);
        connectingRef.current = false;
        setConnecting(false);
        return;
      }

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
        onConnect("telnet", sessionId, `${telnetHost}:${parsedTelnetPort}`, autoLog);
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
      onConnect("serial", sessionId, selectedPort, autoLog);
    } catch (e: unknown) {
      const message =
        typeof e === "string" ? e : e instanceof Error ? e.message : t("connection.error");
      setError(message);
      connectingRef.current = false;
      setConnecting(false);
    }
  };

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

      if (hostKeyCheck) {
        handleTrustAndConnect(hostKeyCheck.status === "mismatch");
        return;
      }

      handleConnect();
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [connecting, handleConnect, handleTrustAndConnect, hostKeyCheck, onClose]);

  const shortcutText = t("connection.shortcut_ctrl_enter");

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
                    value={selectedProfileId}
                    onChange={(e) => handleSelectProfile(e.target.value)}
                  >
                    <option value="">{t("connection.profile_manual")}</option>
                    {sshProfiles.map((profile) => (
                      <option key={profile.id} value={profile.id}>
                        {getProfileDisplayName(profile)}
                      </option>
                    ))}
                  </select>
                  {selectedProfileId && (
                    <button className="btn btn-danger btn-sm" onClick={handleDeleteProfile}>
                      {t("connection.profile_delete")}
                    </button>
                  )}
                </div>
              </div>
              <div>
                <label className="label">{t("connection.profile_name")}</label>
                <input
                  className="input"
                  value={profileName}
                  onChange={(e) => setProfileName(e.target.value)}
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
              {authMethod === "password" ? (
                <div>
                  <label className="label">{t("connection.password")}</label>
                  <input
                    className="input"
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleConnect()}
                  />
                </div>
              ) : (
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
                  <div>
                    <label className="label">{t("connection.key_passphrase")}</label>
                    <input
                      className="input"
                      type="password"
                      value={keyPassphrase}
                      onChange={(e) => setKeyPassphrase(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && handleConnect()}
                    />
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
              <div className="connection-dialog__profile-actions">
                <button className="btn btn-ghost btn-sm" onClick={handleSaveProfile}>
                  {selectedProfileId
                    ? t("connection.profile_update")
                    : t("connection.profile_save")}
                </button>
                <span>{t("connection.profile_password_notice")}</span>
              </div>
            </>
          ) : tab === "telnet" ? (
            <>
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
