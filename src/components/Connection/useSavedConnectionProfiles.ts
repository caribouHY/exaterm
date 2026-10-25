import { invoke } from "@tauri-apps/api/core";
import { useRef } from "react";
import type { AppConfig, SavedConnection } from "../../types";
import { getConnectionErrorMessage } from "./connectionProfileUtils";

interface UseSavedConnectionProfilesParams {
  setConfig: (config: AppConfig) => void;
  setError: (value: string) => void;
  t: (key: string) => string;
}

export const useSavedConnectionProfiles = ({
  setConfig,
  setError,
  t,
}: UseSavedConnectionProfilesParams) => {
  const draftVersions = useRef(new Map<string, number>());
  const preserveDraft = (connectionType: "ssh" | "telnet") => {
    draftVersions.current.set(connectionType, (draftVersions.current.get(connectionType) ?? 0) + 1);
  };
  const saveProfile = async (
    selectedProfileId: string,
    createProfile: () => SavedConnection,
    onSaved: (profile: SavedConnection) => void
  ) => {
    setError("");
    try {
      const nextProfile = createProfile();
      if (Number.isNaN(nextProfile.port)) {
        throw new Error(t("connection.error"));
      }
      if (!nextProfile.id) {
        throw new Error(t("connection.profile_name_required"));
      }
      if (nextProfile.connection_type === "ssh" && nextProfile.jump_profile_id === nextProfile.id) {
        throw new Error(t("connection.jump_profile_self"));
      }

      const version = draftVersions.current.get(nextProfile.connection_type) ?? 0;

      const savedConfig = await invoke<AppConfig>("config_saved_connection_upsert", {
        previousId: selectedProfileId || null,
        profile: nextProfile,
      });
      setConfig(savedConfig);
      // Refresh saved data without replacing a copy draft created while this save was pending.
      if ((draftVersions.current.get(nextProfile.connection_type) ?? 0) === version)
        onSaved(nextProfile);
    } catch (caught: unknown) {
      setError(getConnectionErrorMessage(caught, t, t("connection.error")));
    }
  };

  return { saveProfile, preserveDraft };
};
