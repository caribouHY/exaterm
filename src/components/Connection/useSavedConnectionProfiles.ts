import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, SavedConnection } from "../../types";
import {
  getConnectionErrorMessage,
  hasDuplicateProfile,
  upsertSavedProfile,
} from "./connectionProfileUtils";

interface UseSavedConnectionProfilesParams {
  config: AppConfig | null;
  loadConfig: () => Promise<AppConfig>;
  setConfig: (config: AppConfig) => void;
  setError: (value: string) => void;
  t: (key: string) => string;
}

export const useSavedConnectionProfiles = ({
  config,
  loadConfig,
  setConfig,
  setError,
  t,
}: UseSavedConnectionProfilesParams) => {
  const saveProfile = async (
    selectedProfileId: string,
    createProfile: () => SavedConnection,
    onSaved: (profile: SavedConnection) => void
  ) => {
    setError("");
    try {
      const loaded = config ?? (await loadConfig());
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

      const existingConnections = loaded.saved_connections;
      if (hasDuplicateProfile(existingConnections, nextProfile, selectedProfileId)) {
        throw new Error(t("connection.profile_duplicate"));
      }

      const nextConfig = upsertSavedProfile(loaded, nextProfile, selectedProfileId);
      await invoke("config_save", { config: nextConfig });
      setConfig(nextConfig);
      onSaved(nextProfile);
    } catch (caught: unknown) {
      setError(getConnectionErrorMessage(caught, t, t("connection.error")));
    }
  };

  return { saveProfile };
};
