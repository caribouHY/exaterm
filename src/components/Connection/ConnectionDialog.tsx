import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import type { AppConfig, ConnectionType, PortInfo, SavedConnection } from "../../types";
import { DEFAULT_TERMINAL_MODE, normalizeTerminalMode } from "../../utils/terminalModes";
import { ConnectionDialogView } from "./ConnectionDialogView";
import { CredentialPromptModal } from "./CredentialPromptModal";
import type {
  ConnectionDialogProps,
  SshCredentialPrompt,
  SshFormActions,
  SshFormState,
  SshHostKeyCheck,
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
import { useConnectionProfileSelection } from "./useConnectionProfileSelection";
import { useSavedConnectionProfiles } from "./useSavedConnectionProfiles";
import { useSshDiagnostics } from "./useSshDiagnostics";
import { useStartupConnectionRequest } from "./useStartupConnectionRequest";
import "./ConnectionDialog.css";

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
  const [hostKeyCheck, setHostKeyCheck] = useState<SshHostKeyCheck | null>(null);
  const [credentialPrompt, setCredentialPrompt] = useState<SshCredentialPrompt | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [selectedProfileIds, setSelectedProfileIds] = useState({ ssh: "", telnet: "" });
  const [sshProfileName, setSshProfileName] = useState("");
  const [telnetProfileName, setTelnetProfileName] = useState("");
  const [pendingStartupConnect, setPendingStartupConnect] = useState(false);

  const [host, setHost] = useState("192.168.1.1");
  const [port, setPort] = useState("22");
  const [username, setUsername] = useState("admin");
  const [authMethod, setAuthMethod] = useState<"password" | "public_key">("password");
  const [privateKeyPath, setPrivateKeyPath] = useState("");
  const [jumpProfileId, setJumpProfileId] = useState("");
  const [jumpCredential, setJumpCredential] = useState("");
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

  const getProfileDisplayName = (profile: SavedConnection) => {
    return profile.id || t("connection.unnamed_profile");
  };

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
      setJumpProfileId,
      setJumpCredential,
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
      setError(getConnectionErrorMessage(caught, t("connection.error")));
    }
  };

  const selectSshAuthFile = async () => {
    try {
      const selected = await open({ multiple: false });
      if (!selected || Array.isArray(selected)) return;
      setPrivateKeyPath(selected);
    } catch {
      // Dialog cancellation and platform errors should not disturb the form.
    }
  };

  const closeCredentialPrompt = () => {
    if (connectingRef.current) return;
    setCredentialPrompt(null);
  };

  const connectionActions = useConnectionActions({
    tab,
    connectingRef,
    setConnecting,
    setError,
    hostKeyCheck,
    setHostKeyCheck,
    credentialPrompt,
    setCredentialPrompt,
    sshProfiles,
    ssh: {
      host,
      port,
      username,
      authMethod,
      privateKeyPath,
      jumpProfileId,
      jumpCredential,
      setJumpCredential,
      encoding,
      terminalMode: sshTerminalMode,
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
      setSshProfileName("");
      setHost(request.host ?? "");
      setUsername(request.username ?? "");
      setPort(String(request.port ?? 22));
      setAuthMethod("password");
      setPrivateKeyPath("");
      setJumpProfileId("");
      setJumpCredential("");
      setEncoding("utf-8");
      setSshTerminalMode(DEFAULT_TERMINAL_MODE);
    },
    []
  );

  const resetDirectTelnet = useCallback((target: string, requestPort?: number | null) => {
    setSelectedProfileIds((current) => ({ ...current, telnet: "" }));
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
    applySshProfile: profileSelection.applySshProfile,
    applyTelnetProfile: profileSelection.applyTelnetProfile,
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

  useConnectionDialogShortcuts({
    connecting,
    credentialPrompt,
    hostKeyCheck,
    onClose,
    onCloseCredentialPrompt: closeCredentialPrompt,
    onCancelHostKeyCheck: () => {
      setHostKeyCheck(null);
    },
    onCredentialSubmit: () => {
      void connectionActions.handleCredentialSubmit();
    },
    onTrustAndConnect: (replace) => {
      void connectionActions.handleTrustAndConnect(replace);
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
        onClose={closeCredentialPrompt}
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

  const sshFormState: SshFormState = {
    selectedProfileId: selectedProfileIds.ssh,
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
  };
  const sshFormActions: SshFormActions = {
    onSelectProfile: profileSelection.handleSelectSshProfile,
    onDeleteProfile: () => {
      void handleDeleteProfile("ssh");
    },
    onProfileNameChange: setSshProfileName,
    onHostChange: setHost,
    onPortChange: setPort,
    onUsernameChange: setUsername,
    onAuthMethodChange: (value) => {
      setAuthMethod(normalizeSshAuthMethod(value));
    },
    onPrivateKeyPathChange: setPrivateKeyPath,
    onSelectPrivateKeyFile: () => {
      void selectSshAuthFile();
    },
    onJumpProfileChange: setJumpProfileId,
    onEncodingChange: (value) => {
      setEncoding(normalizeEncoding(value));
    },
    onTerminalModeChange: (value) => {
      setSshTerminalMode(normalizeTerminalMode(value));
    },
    onMemoChange: setSshMemo,
    onExternalControlEnabledChange: setSshExternalControlEnabled,
    onSaveProfile: handleSaveSshProfile,
  };
  const telnetFormState: TelnetFormState = {
    selectedProfileId: selectedProfileIds.telnet,
    profileName: telnetProfileName,
    host: telnetHost,
    port: telnetPort,
    encoding: telnetEncoding,
    terminalMode: telnetTerminalMode,
    memo: telnetMemo,
    externalControlEnabled: telnetExternalControlEnabled,
  };
  const telnetFormActions: TelnetFormActions = {
    onSelectProfile: profileSelection.handleSelectTelnetProfile,
    onDeleteProfile: () => {
      void handleDeleteProfile("telnet");
    },
    onProfileNameChange: setTelnetProfileName,
    onHostChange: setTelnetHost,
    onPortChange: setTelnetPort,
    onPortEnter: () => {
      void connectionActions.handleConnect();
    },
    onEncodingChange: (value) => {
      setTelnetEncoding(normalizeEncoding(value));
    },
    onTerminalModeChange: (value) => {
      setTelnetTerminalMode(normalizeTerminalMode(value));
    },
    onMemoChange: setTelnetMemo,
    onExternalControlEnabledChange: setTelnetExternalControlEnabled,
    onSaveProfile: handleSaveTelnetProfile,
  };
  const hostKeyTitle =
    hostKeyCheck?.status === "mismatch"
      ? t("connection.host_key_mismatch.title")
      : t("connection.host_key_unknown.title");
  const shortcutText = t("connection.shortcut_ctrl_enter");

  return (
    <ConnectionDialogView
      tab={tab}
      setTab={setTab}
      connecting={connecting}
      error={error}
      hostKeyCheck={hostKeyCheck}
      setHostKeyCheck={setHostKeyCheck}
      shortcutText={shortcutText}
      hostKeyTitle={hostKeyTitle}
      sshProfiles={sshProfiles}
      jumpProfileOptions={jumpProfileOptions}
      telnetProfiles={telnetProfiles}
      getProfileDisplayName={getProfileDisplayName}
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
      }}
      serialActions={{
        onSelectedPortChange: setSelectedPort,
        onBaudRateChange: setBaudRate,
        onDataBitsChange: setDataBits,
        onParityChange: setParity,
        onStopBitsChange: setStopBits,
        onTerminalModeChange: (value) => {
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
