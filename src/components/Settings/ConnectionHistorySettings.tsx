import { useCallback, useEffect, useState } from "react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import type { AppConfig } from "../../types";
import { connectionHistoryClient } from "../../features/connection-history/connectionHistoryClient";
import { FeedbackMessage } from "../Common";
import { SettingsToggle } from "./SettingsToggle";

interface ConnectionHistorySettingsProps {
  config: AppConfig["connection_history"];
  onChange: (patch: Partial<AppConfig["connection_history"]>) => void;
}

export function ConnectionHistorySettings({ config, onChange }: ConnectionHistorySettingsProps) {
  const { t } = useTranslation();
  const [count, setCount] = useState<number | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [clearResult, setClearResult] = useState<"success" | "error" | "">("");

  const loadCount = useCallback(async () => {
    try {
      setCount((await connectionHistoryClient.list()).length);
      setLoadFailed(false);
    } catch {
      setCount(null);
      setLoadFailed(true);
    }
  }, []);

  useEffect(() => {
    void loadCount();
    const unlisten = connectionHistoryClient.listenUpdated(() => {
      void loadCount();
    });
    return () => {
      void unlisten.then((dispose) => {
        dispose();
      });
    };
  }, [loadCount]);

  const handleClear = async () => {
    const confirmed = await confirm(t("settings.connection_history_clear_confirm"), {
      title: t("settings.connection_history_clear_title"),
      kind: "warning",
      okLabel: t("settings.connection_history_clear"),
      cancelLabel: t("settings.cancel"),
    });
    if (!confirmed) return;

    try {
      setClearing(true);
      setClearResult("");
      await connectionHistoryClient.clear();
      setCount(0);
      setLoadFailed(false);
      setClearResult("success");
    } catch {
      setClearResult("error");
    } finally {
      setClearing(false);
    }
  };

  return (
    <>
      <div className="settings-section__title">{t("settings.connection_history_title")}</div>
      <SettingsToggle
        id="settings-connection-history-enabled"
        label={t("settings.connection_history_enabled")}
        description={t("settings.connection_history_enabled_desc")}
        checked={config.enabled}
        onChange={(enabled) => {
          onChange({ enabled });
        }}
      />
      <div className="settings-connection-history">
        <div>
          <div className="settings-connection-history__count">
            {count === null
              ? t("settings.connection_history_count_unknown")
              : t("settings.connection_history_count", { count })}
          </div>
          <p className="settings-help">{t("settings.connection_history_privacy")}</p>
        </div>
        <button
          className="btn btn-danger btn-sm"
          disabled={clearing}
          onClick={() => void handleClear()}
        >
          {clearing
            ? t("settings.connection_history_clearing")
            : t("settings.connection_history_clear")}
        </button>
      </div>
      {loadFailed && (
        <FeedbackMessage tone="error">
          {t("settings.connection_history_load_failed")}
        </FeedbackMessage>
      )}
      {clearResult && (
        <FeedbackMessage tone={clearResult === "success" ? "success" : "error"}>
          {t(
            clearResult === "success"
              ? "settings.connection_history_clear_success"
              : "settings.connection_history_clear_failed"
          )}
        </FeedbackMessage>
      )}
    </>
  );
}
