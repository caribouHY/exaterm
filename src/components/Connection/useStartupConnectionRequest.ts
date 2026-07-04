import { useEffect, type MutableRefObject } from "react";
import type { SavedConnection, StartupCliRequest } from "../../types";
import type { ProfileSelectionState } from "./connectionDialogTypes";

interface UseStartupConnectionRequestParams {
  startupRequest?: StartupCliRequest | null;
  startupRequestHandledRef: MutableRefObject<boolean>;
  configReady: boolean;
  sshProfiles: SavedConnection[];
  telnetProfiles: SavedConnection[];
  onStartupRequestHandled?: () => void;
  setError: (value: string) => void;
  setTab: (value: "ssh" | "telnet" | "serial") => void;
  setSelectedProfileIds: React.Dispatch<React.SetStateAction<ProfileSelectionState>>;
  applySshProfile: (profile: SavedConnection, overridePort?: number | null) => void;
  applyTelnetProfile: (profile: SavedConnection, overridePort?: number | null) => void;
  resetDirectSsh: (request: Extract<StartupCliRequest, { kind: "ssh" }>) => void;
  resetDirectTelnet: (target: string, port?: number | null) => void;
  setSshProfileName: (value: string) => void;
  setPendingStartupConnect: (value: boolean) => void;
  t: (key: string, options?: Record<string, unknown>) => string;
}

export const useStartupConnectionRequest = ({
  startupRequest,
  startupRequestHandledRef,
  configReady,
  sshProfiles,
  telnetProfiles,
  onStartupRequestHandled,
  setError,
  setTab,
  setSelectedProfileIds,
  applySshProfile,
  applyTelnetProfile,
  resetDirectSsh,
  resetDirectTelnet,
  setSshProfileName,
  setPendingStartupConnect,
  t,
}: UseStartupConnectionRequestParams) => {
  useEffect(() => {
    if (!startupRequest || startupRequestHandledRef.current) return;
    if (startupRequest.kind === "ssh" && startupRequest.target_kind === "profile" && !configReady) {
      return;
    }
    if (startupRequest.kind === "telnet" && !configReady) return;

    startupRequestHandledRef.current = true;
    onStartupRequestHandled?.();
    setError("");

    if (startupRequest.kind === "telnet") {
      const target = startupRequest.target.trim();
      const profile = telnetProfiles.find((entry) => entry.id === target);
      setTab("telnet");
      if (profile) {
        applyTelnetProfile(profile, startupRequest.port);
        if (!profile.host) {
          setError(t("connection.startup_telnet_profile_incomplete", { profile: target }));
          return;
        }
      } else {
        resetDirectTelnet(target, startupRequest.port);
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
      resetDirectSsh(startupRequest);
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
    configReady,
    onStartupRequestHandled,
    resetDirectSsh,
    resetDirectTelnet,
    setError,
    setPendingStartupConnect,
    setSelectedProfileIds,
    setSshProfileName,
    setTab,
    sshProfiles,
    startupRequest,
    startupRequestHandledRef,
    t,
    telnetProfiles,
  ]);
};
