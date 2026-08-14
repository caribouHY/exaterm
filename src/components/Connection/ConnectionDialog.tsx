import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import type {
  AppConfig,
  ConnectionHistoryEntry,
  ConnectionType,
  PortInfo,
  SavedConnection,
  SshAuthMethod,
} from "../../types";
import { DEFAULT_TERMINAL_MODE, normalizeTerminalMode } from "../../utils/terminalModes";
import { ConnectionDialogView } from "./ConnectionDialogView";
import { ConnectionProgressDialog } from "./ConnectionProgressDialog";
import { CredentialPromptModal } from "./CredentialPromptModal";
import type {
  ConnectionDialogProps,
  SshCredentialPrompt,
  SshFormActions,
  SshFormState,
  TelnetFormActions,
  TelnetFormState,
} from "./connectionDialogTypes";
import {
  createSshProfile,
  createTelnetProfile,
  getConnectionErrorMessage,
  normalizeEncoding,
  normalizeSshAuthMethod,
  removeSavedProfile,
} from "./connectionProfileUtils";
import { useConnectionActions } from "./useConnectionActions";
import { useConnectionDialogShortcuts } from "./useConnectionDialogShortcuts";
import { formatHistoryEntryLabel, parseConnectionSource } from "./connectionHistoryModel";
import { useConnectionHistory } from "./useConnectionHistory";
import { useConnectionProfileSelection } from "./useConnectionProfileSelection";
import { useSavedConnectionProfiles } from "./useSavedConnectionProfiles";
import { useSshDiagnostics } from "./useSshDiagnostics";
import { useStartupConnectionRequest } from "./useStartupConnectionRequest";
import {
  initialSshConnectionAttemptState,
  sshConnectionAttemptReducer,
} from "./sshConnectionAttemptModel";
import { connectionAttemptReducer, initialConnectionAttemptState } from "./connectionAttemptModel";
import {
  isActiveConnectionFormValid,
  validateSerialConnectionForm,
  validateSshConnectionForm,
  validateTelnetConnectionForm,
} from "./connectionFormValidation";
import "./ConnectionDialog.css";

export default function ConnectionDialog({
  initialValues,
  startupRequest,
  onStartupRequestHandled,
  onClose,
  onConnect,
}: ConnectionDialogProps) {
  const { t, i18n } = useTranslation();
  const overlayMouseDownStartedRef = useRef(false);
  const connectingRef = useRef(false);
  const startupRequestHandledRef = useRef(false);
  const initialValuesAppliedRef = useRef(false);
  const [tab, setTab] = useState<ConnectionType>("ssh");
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState("");
  const [credentialPrompt, setCredentialPrompt] = useState<SshCredentialPrompt | null>(null);
  const [sshAttempt, dispatchSshAttempt] = useReducer(
    sshConnectionAttemptReducer,
    initialSshConnectionAttemptState
  );
  const [connectionAttempt, dispatchConnectionAttempt] = useReducer(
    connectionAttemptReducer,
    initialConnectionAttemptState
  );
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [selectedProfileIds, setSelectedProfileIds] = useState({ ssh: "", telnet: "" });
  const [selectedHistoryIds, setSelectedHistoryIds] = useState({ ssh: "", telnet: "" });
  const [sshProfileName, setSshProfileName] = useState("");
  const [telnetProfileName, setTelnetProfileName] = useState("");
  const [pendingStartupConnect, setPendingStartupConnect] = useState(false);

  const [host, setHost] = useState("192.168.1.1");
  const [port, setPort] = useState("22");
  const [username, setUsername] = useState("admin");
  const [authMethod, setAuthMethod] = useState<SshAuthMethod>("auto");
  const [privateKeyPath, setPrivateKeyPath] = useState("");
  const [jumpProfileId, setJumpProfileId] = useState("");
  const [missingInitialJumpProfileId, setMissingInitialJumpProfileId] = useState("");
  const [encoding, setEncoding] = useState<"utf-8" | "shift-jis" | "euc-jp">("utf-8");
  const [sshTerminalMode, setSshTerminalMode] = useState(DEFAULT_TERMINAL_MODE);
  const [sshMemo, setSshMemo] = useState("");
  const [sshExternalControlEnabled, setSshExternalControlEnabled] = useState(true);

  const [telnetHost, setTelnetHost] = useState("192.168.1.1");
  const [telnetPort, setTelnetPort] = useState("23");
  const [telnetEncoding, setTelnetEncoding] = useState<"utf-8" | "shift-jis" | "euc-jp">("utf-8");
  const [telnetTerminalMode, setTelnetTerminalMode] = useState(DEFAULT_TERMINAL_MODE);
  const [telnetMemo, setTelnetMemo] = useState("");
  const [telnetExternalControlEnabled, setTelnetExternalControlEnabled] = useState(true);

  const [ports, setPorts] = useState<PortInfo[]>([]);
  const [selectedPort, setSelectedPort] = useState("");
  const [baudRate, setBaudRate] = useState("9600");
  const [dataBits, setDataBits] = useState("8");
  const [parity, setParity] = useState("none");
  const [stopBits, setStopBits] = useState("1");
  const [serialTerminalMode, setSerialTerminalMode] = useState(DEFAULT_TERMINAL_MODE);
  const diagnostics = useSshDiagnostics();
  const connectionHistory = useConnectionHistory();

  useEffect(() => {
    setSelectedHistoryIds((current) => {
      const ssh = connectionHistory.sshEntries.some((entry) => entry.id === current.ssh)
        ? current.ssh
        : "";
      const telnet = connectionHistory.telnetEntries.some((entry) => entry.id === current.telnet)
        ? current.telnet
        : "";
      return ssh === current.ssh && telnet === current.telnet ? current : { ssh, telnet };
    });
  }, [connectionHistory.sshEntries, connectionHistory.telnetEntries]);

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
    if (tab !== "serial") return;
    invoke<PortInfo[]>("serial_list_ports")
      .then((availablePorts) => {
        setPorts(availablePorts);
        if (availablePorts.length > 0 && !selectedPort) {
          setSelectedPort(availablePorts[0].name);
        }
      })
      .catch(() => {});
  }, [selectedPort, tab]);

  const sshProfiles = (config?.saved_connections ?? []).filter(
    (connection) => connection.connection_type === "ssh"
  );
  const telnetProfiles = (config?.saved_connections ?? []).filter(
    (connection) => connection.connection_type === "telnet"
  );
  const jumpProfileOptions = sshProfiles.filter((profile) => profile.id !== selectedProfileIds.ssh);

  const handleJumpProfileChange = useCallback((value: string) => {
    setMissingInitialJumpProfileId("");
    setError("");
    setJumpProfileId(value);
  }, []);

  useEffect(() => {
    if (!jumpProfileId) return;
    if (jumpProfileId === selectedProfileIds.ssh) {
      setMissingInitialJumpProfileId("");
      setJumpProfileId("");
      return;
    }
    if (!sshProfiles.some((profile) => profile.id === jumpProfileId)) {
      if (jumpProfileId === missingInitialJumpProfileId) return;
      setJumpProfileId("");
    }
  }, [jumpProfileId, missingInitialJumpProfileId, selectedProfileIds.ssh, sshProfiles]);

  useEffect(() => {
    if (!initialValues || initialValuesAppliedRef.current || startupRequest || !config) return;
    initialValuesAppliedRef.current = true;
    setError("");

    if (initialValues.connectionInfo.kind === "ssh") {
      const info = initialValues.connectionInfo;
      const missingJumpProfileId =
        info.jump_profile_id && !sshProfiles.some((profile) => profile.id === info.jump_profile_id)
          ? info.jump_profile_id
          : "";
      setTab("ssh");
      setSelectedProfileIds((current) => ({ ...current, ssh: "" }));
      setSelectedHistoryIds((current) => ({ ...current, ssh: "" }));
      setSshProfileName("");
      setHost(info.host);
      setPort(String(info.port));
      setUsername(info.username);
      setAuthMethod(normalizeSshAuthMethod(info.auth_method));
      setPrivateKeyPath(info.private_key_path ?? "");
      setJumpProfileId(info.jump_profile_id ?? "");
      setMissingInitialJumpProfileId(missingJumpProfileId);
      setEncoding(initialValues.encoding);
      setSshTerminalMode(initialValues.terminalMode);
      if (missingJumpProfileId) {
        setError(t("connection.jump_profile_not_found", { profile: missingJumpProfileId }));
      }
      return;
    }

    const info = initialValues.connectionInfo;
    setTab("telnet");
    setSelectedProfileIds((current) => ({ ...current, telnet: "" }));
    setSelectedHistoryIds((current) => ({ ...current, telnet: "" }));
    setTelnetProfileName("");
    setTelnetHost(info.host);
    setTelnetPort(String(info.port));
    setTelnetEncoding(initialValues.encoding);
    setTelnetTerminalMode(initialValues.terminalMode);
  }, [config, initialValues, sshProfiles, startupRequest, t]);

  const getProfileDisplayName = (profile: SavedConnection) => {
    return profile.id || t("connection.unnamed_profile");
  };

  const getHistoryDisplayName = (entry: ConnectionHistoryEntry) =>
    formatHistoryEntryLabel(entry, i18n.resolvedLanguage ?? i18n.language);

  const profileSelection = useConnectionProfileSelection({
    sshProfiles,
    telnetProfiles,
    selectedProfileIds,
    setSelectedProfileIds,
    sshSetters: {
      setProfileName: setSshProfileName,
      setHost,
      setPort,
      setUsername,
      setAuthMethod,
      setPrivateKeyPath,
      setJumpProfileId: handleJumpProfileChange,
      setEncoding,
      setTerminalMode: setSshTerminalMode,
      setMemo: setSshMemo,
      setExternalControlEnabled: setSshExternalControlEnabled,
    },
    telnetSetters: {
      setProfileName: setTelnetProfileName,
      setHost: setTelnetHost,
      setPort: setTelnetPort,
      setEncoding: setTelnetEncoding,
      setTerminalMode: setTelnetTerminalMode,
      setMemo: setTelnetMemo,
      setExternalControlEnabled: setTelnetExternalControlEnabled,
    },
  });

  const applySshHistory = (entry: ConnectionHistoryEntry) => {
    if (entry.connection_info.kind !== "ssh") return;
    const info = entry.connection_info;
    const missingJumpProfileId =
      info.jump_profile_id && !sshProfiles.some((profile) => profile.id === info.jump_profile_id)
        ? info.jump_profile_id
        : "";
    setError("");
    setSelectedProfileIds((current) => ({ ...current, ssh: "" }));
    setSelectedHistoryIds((current) => ({ ...current, ssh: entry.id }));
    setSshProfileName("");
    setHost(info.host);
    setPort(String(info.port));
    setUsername(info.username);
    setAuthMethod(normalizeSshAuthMethod(info.auth_method));
    setPrivateKeyPath(info.private_key_path ?? "");
    setJumpProfileId(info.jump_profile_id ?? "");
    setMissingInitialJumpProfileId(missingJumpProfileId);
    setEncoding(entry.encoding);
    setSshTerminalMode(entry.terminal_mode);
    setSshMemo("");
    setSshExternalControlEnabled(true);
    if (missingJumpProfileId) {
      setError(t("connection.jump_profile_not_found", { profile: missingJumpProfileId }));
    }
  };

  const applyTelnetHistory = (entry: ConnectionHistoryEntry) => {
    if (entry.connection_info.kind !== "telnet") return;
    setError("");
    setSelectedProfileIds((current) => ({ ...current, telnet: "" }));
    setSelectedHistoryIds((current) => ({ ...current, telnet: entry.id }));
    setTelnetProfileName("");
    setTelnetHost(entry.connection_info.host);
    setTelnetPort(String(entry.connection_info.port));
    setTelnetEncoding(entry.encoding);
    setTelnetTerminalMode(entry.terminal_mode);
    setTelnetMemo("");
    setTelnetExternalControlEnabled(true);
  };

  const handleSelectSshSource = (value: string) => {
    const source = parseConnectionSource(value);
    setMissingInitialJumpProfileId("");
    setError("");
    if (source.kind === "history") {
      const entry = connectionHistory.sshEntries.find((candidate) => candidate.id === source.id);
      if (entry) applySshHistory(entry);
      return;
    }
    setSelectedHistoryIds((current) => ({ ...current, ssh: "" }));
    profileSelection.handleSelectSshProfile(source.kind === "profile" ? source.id : "");
  };

  const handleSelectTelnetSource = (value: string) => {
    const source = parseConnectionSource(value);
    setError("");
    if (source.kind === "history") {
      const entry = connectionHistory.telnetEntries.find((candidate) => candidate.id === source.id);
      if (entry) applyTelnetHistory(entry);
      return;
    }
    setSelectedHistoryIds((current) => ({ ...current, telnet: "" }));
    profileSelection.handleSelectTelnetProfile(source.kind === "profile" ? source.id : "");
  };

  const handleDeleteHistory = async (connectionType: "ssh" | "telnet") => {
    const entryId = connectionType === "ssh" ? selectedHistoryIds.ssh : selectedHistoryIds.telnet;
    if (!entryId || !(await connectionHistory.deleteEntry(entryId))) return;
    setSelectedHistoryIds((current) => ({ ...current, [connectionType]: "" }));
  };

  const savedProfiles = useSavedConnectionProfiles({ config, loadConfig, setConfig, setError, t });

  const handleSaveSshProfile = () => {
    void savedProfiles.saveProfile(
      selectedProfileIds.ssh,
      () =>
        createSshProfile({
          profileName: sshProfileName,
          host,
          port,
          username,
          authMethod,
          privateKeyPath,
          jumpProfileId,
          encoding,
          terminalMode: sshTerminalMode,
          memo: sshMemo,
          externalControlEnabled: sshExternalControlEnabled,
        }),
      (profile) => {
        setSelectedProfileIds((current) => ({ ...current, ssh: profile.id }));
        setSelectedHistoryIds((current) => ({ ...current, ssh: "" }));
        setSshProfileName(profile.id);
      }
    );
  };

  const handleSaveTelnetProfile = () => {
    void savedProfiles.saveProfile(
      selectedProfileIds.telnet,
      () =>
        createTelnetProfile({
          profileName: telnetProfileName,
          host: telnetHost,
          port: telnetPort,
          encoding: telnetEncoding,
          terminalMode: telnetTerminalMode,
          memo: telnetMemo,
          externalControlEnabled: telnetExternalControlEnabled,
        }),
      (profile) => {
        setSelectedProfileIds((current) => ({ ...current, telnet: profile.id }));
        setSelectedHistoryIds((current) => ({ ...current, telnet: "" }));
        setTelnetProfileName(profile.id);
      }
    );
  };

  const handleDeleteProfile = async (connectionType: "ssh" | "telnet") => {
    const selectedProfileId = selectedProfileIds[connectionType];
    if (!selectedProfileId) return;

    setError("");
    try {
      const loaded = config ?? (await loadConfig());
      const nextConfig = removeSavedProfile(loaded, connectionType, selectedProfileId);
      await invoke("config_save", { config: nextConfig });
      setConfig(nextConfig);
      setSelectedProfileIds((current) => ({ ...current, [connectionType]: "" }));
      if (connectionType === "ssh") {
        profileSelection.resetSshProfileFields();
      } else {
        profileSelection.resetTelnetProfileFields();
      }
    } catch (caught: unknown) {
      setError(getConnectionErrorMessage(caught, t, t("connection.error")));
    }
  };

  const selectSshAuthFile = async () => {
    try {
      const selected = await open({ multiple: false });
      if (!selected || Array.isArray(selected)) return;
      setError("");
      setPrivateKeyPath(selected);
    } catch {
      // Dialog cancellation and platform errors should not disturb the form.
    }
  };

  const formValidation = {
    ssh: validateSshConnectionForm({ host, port, username, authMethod, privateKeyPath }),
    telnet: validateTelnetConnectionForm({ host: telnetHost, port: telnetPort }),
    serial: validateSerialConnectionForm({ selectedPort }),
  };
  const canConnect = isActiveConnectionFormValid(tab, formValidation);

  const connectionActions = useConnectionActions({
    tab,
    canConnect,
    connectingRef,
    setConnecting,
    setError,
    credentialPrompt,
    setCredentialPrompt,
    sshAttemptDispatch: dispatchSshAttempt,
    connectionAttemptDispatch: dispatchConnectionAttempt,
    selectedProfileIds,
    sshProfiles,
    ssh: {
      host,
      port,
      username,
      authMethod,
      privateKeyPath,
      jumpProfileId,
      encoding,
      terminalMode: sshTerminalMode,
      defaultPrivateKeyPath: config?.ssh.default_private_key_path ?? "",
    },
    telnet: {
      host: telnetHost,
      port: telnetPort,
      encoding: telnetEncoding,
      terminalMode: telnetTerminalMode,
    },
    serial: {
      selectedPort,
      baudRate,
      dataBits,
      parity,
      stopBits,
      terminalMode: serialTerminalMode,
    },
    diagnostics,
    onConnect,
    t,
  });

  const resetDirectSsh = useCallback(
    (request: Extract<NonNullable<typeof startupRequest>, { kind: "ssh" }>) => {
      setSelectedProfileIds((current) => ({ ...current, ssh: "" }));
      setSelectedHistoryIds((current) => ({ ...current, ssh: "" }));
      setSshProfileName("");
      setHost(request.host ?? "");
      setUsername(request.username ?? "");
      setPort(String(request.port ?? 22));
      setAuthMethod("auto");
      setPrivateKeyPath("");
      setJumpProfileId("");
      setEncoding("utf-8");
      setSshTerminalMode(DEFAULT_TERMINAL_MODE);
    },
    []
  );

  const resetDirectTelnet = useCallback((target: string, requestPort?: number | null) => {
    setSelectedProfileIds((current) => ({ ...current, telnet: "" }));
    setSelectedHistoryIds((current) => ({ ...current, telnet: "" }));
    setTelnetProfileName("");
    setTelnetHost(target);
    setTelnetPort(String(requestPort ?? 23));
    setTelnetEncoding("utf-8");
    setTelnetTerminalMode(DEFAULT_TERMINAL_MODE);
  }, []);

  useStartupConnectionRequest({
    startupRequest,
    startupRequestHandledRef,
    configReady: Boolean(config),
    sshProfiles,
    telnetProfiles,
    onStartupRequestHandled,
    setError,
    setTab,
    setSelectedProfileIds,
    applySshProfile: (profile, overridePort) => {
      setSelectedHistoryIds((current) => ({ ...current, ssh: "" }));
      profileSelection.applySshProfile(profile, overridePort);
    },
    applyTelnetProfile: (profile, overridePort) => {
      setSelectedHistoryIds((current) => ({ ...current, telnet: "" }));
      profileSelection.applyTelnetProfile(profile, overridePort);
    },
    resetDirectSsh,
    resetDirectTelnet,
    setSshProfileName,
    setPendingStartupConnect,
    t,
  });

  useEffect(() => {
    if (!pendingStartupConnect) return;
    setPendingStartupConnect(false);
    void connectionActions.handleConnect();
  }, [connectionActions, pendingStartupConnect]);

  useEffect(() => {
    if (!diagnostics.progress) return;
    dispatchSshAttempt({ type: "progress", ...diagnostics.progress });
  }, [diagnostics.progress]);

  useConnectionDialogShortcuts({
    connecting,
    canConnect,
    credentialPrompt,
    onClose,
    onCloseCredentialPrompt: connectionActions.handleCredentialCancel,
    onCredentialSubmit: () => {
      void connectionActions.handleCredentialSubmit();
    },
    onConnect: () => {
      void connectionActions.handleConnect();
    },
  });

  const handleOverlayMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
    overlayMouseDownStartedRef.current = event.target === event.currentTarget;
  };

  const handleOverlayClick = (event: React.MouseEvent<HTMLDivElement>) => {
    if (overlayMouseDownStartedRef.current && event.target === event.currentTarget) {
      onClose();
    }
    overlayMouseDownStartedRef.current = false;
  };

  const diagnosticsPanelProps = {
    logs: diagnostics.logs,
    expanded: diagnostics.expanded,
    copied: diagnostics.copied,
    onToggleExpanded: () => {
      diagnostics.setExpanded((current) => !current);
    },
    onCopy: () => {
      void diagnostics.copy();
    },
  };

  if (credentialPrompt) {
    return (
      <CredentialPromptModal
        credentialPrompt={credentialPrompt}
        connecting={connecting}
        diagnostics={diagnosticsPanelProps}
        onClose={connectionActions.handleCredentialCancel}
        onSubmit={() => {
          void connectionActions.handleCredentialSubmit();
        }}
        onValueChange={(value) => {
          setCredentialPrompt({
            ...credentialPrompt,
            value,
            error: "",
          });
        }}
      />
    );
  }

  if (tab === "ssh" && sshAttempt.status !== "editing") {
    const sshProgressLabelKey =
      sshAttempt.status === "cancelling"
        ? "connection.progress_cancelling"
        : sshAttempt.status === "preparing" || sshAttempt.progress === null
          ? "connection.progress_preparing"
          : `connection.ssh_progress_${sshAttempt.progress.phase}`;
    return (
      <ConnectionProgressDialog
        connectionType="ssh"
        target={`${username}@${host}:${port}`}
        statusLabel={t(sshProgressLabelKey)}
        cancelling={sshAttempt.status === "cancelling"}
        cancelError={sshAttempt.cancelError}
        roleLabel={sshAttempt.progress?.target === "jump" ? t("connection.ssh_progress_jump") : ""}
        diagnostics={diagnosticsPanelProps}
        onCancel={() => {
          void connectionActions.handleCancelSshConnect();
        }}
      />
    );
  }

  if (connectionAttempt.status !== "editing" && connectionAttempt.connectionType) {
    const connectionType = connectionAttempt.connectionType;
    const target = connectionType === "telnet" ? `${telnetHost}:${telnetPort}` : selectedPort;
    const statusLabelKey =
      connectionAttempt.status === "cancelling"
        ? "connection.progress_cancelling"
        : connectionType === "serial"
          ? "connection.serial_progress_opening"
          : "connection.progress_connecting";
    return (
      <ConnectionProgressDialog
        connectionType={connectionType}
        target={target}
        statusLabel={t(statusLabelKey)}
        cancelling={connectionAttempt.status === "cancelling"}
        cancelError={connectionAttempt.cancelError}
        onCancel={() => {
          void connectionActions.handleCancelConnection();
        }}
      />
    );
  }

  const sshFormState: SshFormState = {
    selectedProfileId: selectedProfileIds.ssh,
    selectedHistoryId: selectedHistoryIds.ssh,
    profileName: sshProfileName,
    host,
    port,
    username,
    authMethod,
    privateKeyPath,
    jumpProfileId,
    encoding,
    terminalMode: sshTerminalMode,
    memo: sshMemo,
    externalControlEnabled: sshExternalControlEnabled,
    validationErrors: formValidation.ssh.errors,
  };
  const sshFormActions: SshFormActions = {
    onSelectSource: handleSelectSshSource,
    onDeleteProfile: () => {
      void handleDeleteProfile("ssh");
    },
    onDeleteHistory: () => {
      void handleDeleteHistory("ssh");
    },
    onProfileNameChange: (value) => {
      setError("");
      setSshProfileName(value);
    },
    onHostChange: (value) => {
      setError("");
      setHost(value);
    },
    onPortChange: (value) => {
      setError("");
      setPort(value);
    },
    onUsernameChange: (value) => {
      setError("");
      setUsername(value);
    },
    onAuthMethodChange: (value) => {
      setError("");
      setAuthMethod(normalizeSshAuthMethod(value));
    },
    onPrivateKeyPathChange: (value) => {
      setError("");
      setPrivateKeyPath(value);
    },
    onSelectPrivateKeyFile: () => {
      void selectSshAuthFile();
    },
    onJumpProfileChange: handleJumpProfileChange,
    onEncodingChange: (value) => {
      setError("");
      setEncoding(normalizeEncoding(value));
    },
    onTerminalModeChange: (value) => {
      setError("");
      setSshTerminalMode(normalizeTerminalMode(value));
    },
    onMemoChange: (value) => {
      setError("");
      setSshMemo(value);
    },
    onExternalControlEnabledChange: (value) => {
      setError("");
      setSshExternalControlEnabled(value);
    },
    onSaveProfile: handleSaveSshProfile,
  };
  const telnetFormState: TelnetFormState = {
    selectedProfileId: selectedProfileIds.telnet,
    selectedHistoryId: selectedHistoryIds.telnet,
    profileName: telnetProfileName,
    host: telnetHost,
    port: telnetPort,
    encoding: telnetEncoding,
    terminalMode: telnetTerminalMode,
    memo: telnetMemo,
    externalControlEnabled: telnetExternalControlEnabled,
    validationErrors: formValidation.telnet.errors,
  };
  const telnetFormActions: TelnetFormActions = {
    onSelectSource: handleSelectTelnetSource,
    onDeleteProfile: () => {
      void handleDeleteProfile("telnet");
    },
    onDeleteHistory: () => {
      void handleDeleteHistory("telnet");
    },
    onProfileNameChange: (value) => {
      setError("");
      setTelnetProfileName(value);
    },
    onHostChange: (value) => {
      setError("");
      setTelnetHost(value);
    },
    onPortChange: (value) => {
      setError("");
      setTelnetPort(value);
    },
    onPortEnter: () => {
      if (canConnect) void connectionActions.handleConnect();
    },
    onEncodingChange: (value) => {
      setError("");
      setTelnetEncoding(normalizeEncoding(value));
    },
    onTerminalModeChange: (value) => {
      setError("");
      setTelnetTerminalMode(normalizeTerminalMode(value));
    },
    onMemoChange: (value) => {
      setError("");
      setTelnetMemo(value);
    },
    onExternalControlEnabledChange: (value) => {
      setError("");
      setTelnetExternalControlEnabled(value);
    },
    onSaveProfile: handleSaveTelnetProfile,
  };
  const shortcutText = t("connection.shortcut_ctrl_enter");

  return (
    <ConnectionDialogView
      tab={tab}
      setTab={(value) => {
        setError("");
        setTab(value);
      }}
      connecting={connecting}
      canConnect={canConnect}
      error={error}
      historyError={
        connectionHistory.error
          ? t(
              connectionHistory.error === "delete"
                ? "connection.history_delete_failed"
                : "connection.history_load_failed"
            )
          : ""
      }
      shortcutText={shortcutText}
      sshProfiles={sshProfiles}
      sshHistoryEntries={connectionHistory.sshEntries}
      jumpProfileOptions={jumpProfileOptions}
      telnetProfiles={telnetProfiles}
      telnetHistoryEntries={connectionHistory.telnetEntries}
      getProfileDisplayName={getProfileDisplayName}
      getHistoryDisplayName={getHistoryDisplayName}
      sshFormState={sshFormState}
      sshFormActions={sshFormActions}
      telnetFormState={telnetFormState}
      telnetFormActions={telnetFormActions}
      serialFormState={{
        ports,
        selectedPort,
        baudRate,
        dataBits,
        parity,
        stopBits,
        terminalMode: serialTerminalMode,
        validationErrors: formValidation.serial.errors,
      }}
      serialActions={{
        onSelectedPortChange: (value) => {
          setError("");
          setSelectedPort(value);
        },
        onBaudRateChange: (value) => {
          setError("");
          setBaudRate(value);
        },
        onDataBitsChange: (value) => {
          setError("");
          setDataBits(value);
        },
        onParityChange: (value) => {
          setError("");
          setParity(value);
        },
        onStopBitsChange: (value) => {
          setError("");
          setStopBits(value);
        },
        onTerminalModeChange: (value) => {
          setError("");
          setSerialTerminalMode(normalizeTerminalMode(value));
        },
      }}
      diagnosticsPanelProps={diagnosticsPanelProps}
      connectionActions={connectionActions}
      onClose={onClose}
      onOverlayMouseDown={handleOverlayMouseDown}
      onOverlayClick={handleOverlayClick}
    />
  );
}
