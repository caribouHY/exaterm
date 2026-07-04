import { useCallback, type Dispatch, type SetStateAction } from "react";
import type { Encoding, SavedConnection, SshAuthMethod, TerminalMode } from "../../types";
import { DEFAULT_TERMINAL_MODE, normalizeTerminalMode } from "../../utils/terminalModes";
import type { ProfileSelectionState } from "./connectionDialogTypes";
import {
  normalizeEncoding,
  normalizeProfileExternalControlEnabled,
  normalizeSshAuthMethod,
} from "./connectionProfileUtils";

interface UseConnectionProfileSelectionParams {
  sshProfiles: SavedConnection[];
  telnetProfiles: SavedConnection[];
  selectedProfileIds: ProfileSelectionState;
  setSelectedProfileIds: Dispatch<SetStateAction<ProfileSelectionState>>;
  sshSetters: {
    setProfileName: (value: string) => void;
    setHost: (value: string) => void;
    setPort: (value: string) => void;
    setUsername: (value: string) => void;
    setAuthMethod: (value: SshAuthMethod) => void;
    setPrivateKeyPath: (value: string) => void;
    setJumpProfileId: (value: string) => void;
    setJumpCredential: (value: string) => void;
    setEncoding: (value: Encoding) => void;
    setTerminalMode: (value: TerminalMode) => void;
    setMemo: (value: string) => void;
    setExternalControlEnabled: (value: boolean) => void;
  };
  telnetSetters: {
    setProfileName: (value: string) => void;
    setHost: (value: string) => void;
    setPort: (value: string) => void;
    setEncoding: (value: Encoding) => void;
    setTerminalMode: (value: TerminalMode) => void;
    setMemo: (value: string) => void;
    setExternalControlEnabled: (value: boolean) => void;
  };
}

export const useConnectionProfileSelection = ({
  sshProfiles,
  telnetProfiles,
  selectedProfileIds,
  setSelectedProfileIds,
  sshSetters,
  telnetSetters,
}: UseConnectionProfileSelectionParams) => {
  const applySshProfile = useCallback(
    (profile: SavedConnection, overridePort?: number | null) => {
      setSelectedProfileIds((current) => ({ ...current, ssh: profile.id }));
      sshSetters.setProfileName(profile.id);
      sshSetters.setHost(profile.host ?? "");
      sshSetters.setPort(String(overridePort ?? profile.port ?? 22));
      sshSetters.setUsername(profile.username ?? "");
      sshSetters.setAuthMethod(normalizeSshAuthMethod(profile.auth_method));
      sshSetters.setPrivateKeyPath(profile.private_key_path ?? "");
      sshSetters.setJumpProfileId(profile.jump_profile_id ?? "");
      sshSetters.setJumpCredential("");
      sshSetters.setEncoding(normalizeEncoding(profile.encoding));
      sshSetters.setTerminalMode(normalizeTerminalMode(profile.terminal_mode));
      sshSetters.setMemo(profile.memo ?? "");
      sshSetters.setExternalControlEnabled(
        normalizeProfileExternalControlEnabled(profile.external_control_enabled)
      );
    },
    [setSelectedProfileIds, sshSetters]
  );

  const applyTelnetProfile = useCallback(
    (profile: SavedConnection, overridePort?: number | null) => {
      setSelectedProfileIds((current) => ({ ...current, telnet: profile.id }));
      telnetSetters.setProfileName(profile.id);
      telnetSetters.setHost(profile.host ?? "");
      telnetSetters.setPort(String(overridePort ?? profile.port ?? 23));
      telnetSetters.setEncoding(normalizeEncoding(profile.encoding));
      telnetSetters.setTerminalMode(normalizeTerminalMode(profile.terminal_mode));
      telnetSetters.setMemo(profile.memo ?? "");
      telnetSetters.setExternalControlEnabled(
        normalizeProfileExternalControlEnabled(profile.external_control_enabled)
      );
    },
    [setSelectedProfileIds, telnetSetters]
  );

  const resetSshProfileFields = useCallback(() => {
    sshSetters.setProfileName("");
    sshSetters.setAuthMethod("password");
    sshSetters.setPrivateKeyPath("");
    sshSetters.setJumpProfileId("");
    sshSetters.setJumpCredential("");
    sshSetters.setEncoding("utf-8");
    sshSetters.setTerminalMode(DEFAULT_TERMINAL_MODE);
    sshSetters.setMemo("");
    sshSetters.setExternalControlEnabled(true);
  }, [sshSetters]);

  const resetTelnetProfileFields = useCallback(() => {
    telnetSetters.setProfileName("");
    telnetSetters.setEncoding("utf-8");
    telnetSetters.setTerminalMode(DEFAULT_TERMINAL_MODE);
    telnetSetters.setMemo("");
    telnetSetters.setExternalControlEnabled(true);
  }, [telnetSetters]);

  const handleSelectSshProfile = useCallback(
    (id: string) => {
      setSelectedProfileIds((current) => ({ ...current, ssh: id }));
      if (!id) {
        resetSshProfileFields();
        return;
      }
      const profile = sshProfiles.find((entry) => entry.id === id);
      if (profile) applySshProfile(profile);
    },
    [applySshProfile, resetSshProfileFields, setSelectedProfileIds, sshProfiles]
  );

  const handleSelectTelnetProfile = useCallback(
    (id: string) => {
      setSelectedProfileIds((current) => ({ ...current, telnet: id }));
      if (!id) {
        resetTelnetProfileFields();
        return;
      }
      const profile = telnetProfiles.find((entry) => entry.id === id);
      if (profile) applyTelnetProfile(profile);
    },
    [applyTelnetProfile, resetTelnetProfileFields, setSelectedProfileIds, telnetProfiles]
  );

  return {
    applySshProfile,
    applyTelnetProfile,
    resetSshProfileFields,
    resetTelnetProfileFields,
    handleSelectSshProfile,
    handleSelectTelnetProfile,
    selectedSshProfileId: selectedProfileIds.ssh,
    selectedTelnetProfileId: selectedProfileIds.telnet,
  };
};
