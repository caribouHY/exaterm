import { invoke } from "@tauri-apps/api/core";
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

      const savedConfig = await invoke<AppConfig>("config_saved_connection_upsert", {
        previousId: selectedProfileId || null,
        profile: nextProfile,
      });
      setConfig(savedConfig);
      onSaved(nextProfile);
    } catch (caught: unknown) {
      setError(getConnectionErrorMessage(caught, t, t("connection.error")));
    }
  };

  return { saveProfile };
};
