import { ChevronDown } from "lucide-react";
import { useTranslation } from "react-i18next";
import type {
  SshAlgorithmCatalog,
  SshAlgorithmGroup,
  SshAlgorithmSelection,
  SshConfig,
} from "../../types";
import { SettingsError } from "./SettingsFooter";

interface SshSettingsProps {
  config: SshConfig;
  catalog: SshAlgorithmCatalog | null;
  catalogLoadFailed: boolean;
  onChange: (patch: Partial<SshConfig>) => void;
  onReloadCatalog: () => void;
}

const ALGORITHM_GROUPS: SshAlgorithmGroup[] = ["kex", "host_key", "cipher", "mac", "compression"];

function recommendedSelection(catalog: SshAlgorithmCatalog): SshAlgorithmSelection {
  return {
    kex: catalog.kex.filter((item) => item.recommended).map((item) => item.name),
    host_key: catalog.host_key.filter((item) => item.recommended).map((item) => item.name),
    cipher: catalog.cipher.filter((item) => item.recommended).map((item) => item.name),
    mac: catalog.mac.filter((item) => item.recommended).map((item) => item.name),
    compression: catalog.compression.filter((item) => item.recommended).map((item) => item.name),
  };
}

function hasCompleteSelection(selection: SshAlgorithmSelection): boolean {
  return ALGORITHM_GROUPS.every((group) => selection[group].length > 0);
}

export function SshSettings({
  config,
  catalog,
  catalogLoadFailed,
  onChange,
  onReloadCatalog,
}: SshSettingsProps) {
  const { t } = useTranslation();

  const setMode = (algorithm_mode: SshConfig["algorithm_mode"]) => {
    if (!catalog) return;
    onChange({
      algorithm_mode,
      algorithms:
        algorithm_mode === "custom" && !hasCompleteSelection(config.algorithms)
          ? recommendedSelection(catalog)
          : config.algorithms,
    });
  };

  const setGroupSelection = (group: SshAlgorithmGroup, values: string[]) => {
    onChange({
      algorithms: {
        ...config.algorithms,
        [group]: values,
      },
    });
  };

  return (
    <div className="settings-ssh">
      <div className="settings-section__title">{t("settings.ssh_settings")}</div>
      <div className="settings-row">
        <div className="settings-ssh__mode">
          <label className="label" htmlFor="settings-ssh-algorithm-mode">
            {t("settings.ssh_algorithm_mode")}
          </label>
          <select
            id="settings-ssh-algorithm-mode"
            className="select"
            value={config.algorithm_mode}
            disabled={!catalog}
            onChange={(event) => setMode(event.target.value as SshConfig["algorithm_mode"])}
          >
            <option value="default">{t("settings.ssh_algorithm_mode_default")}</option>
            <option value="custom">{t("settings.ssh_algorithm_mode_custom")}</option>
          </select>
          <p className="settings-help">{t("settings.ssh_algorithm_mode_desc")}</p>
        </div>
      </div>

      {catalogLoadFailed && (
        <div className="settings-ssh__catalog-error">
          <SettingsError message={t("settings.ssh_algorithm_catalog_failed")} />
          <button type="button" className="btn btn-ghost btn-sm" onClick={onReloadCatalog}>
            {t("settings.reload")}
          </button>
        </div>
      )}

      {catalog && config.algorithm_mode === "custom" && (
        <div className="settings-ssh__custom">
          <div className="settings-ssh__custom-header">
            <p className="settings-help">{t("settings.ssh_algorithm_custom_desc")}</p>
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={() => onChange({ algorithms: recommendedSelection(catalog) })}
            >
              {t("settings.ssh_algorithm_restore_recommended")}
            </button>
          </div>

          {ALGORITHM_GROUPS.map((group) => {
            const selected = config.algorithms[group];
            return (
              <details className="settings-ssh__group" key={group}>
                <summary>
                  <span>{t(`settings.ssh_algorithm_group.${group}`)}</span>
                  <span className="settings-ssh__group-meta">
                    <small>
                      {t("settings.ssh_algorithm_selected_count", { count: selected.length })}
                    </small>
                    <ChevronDown
                      className="settings-ssh__group-chevron"
                      size={14}
                      aria-hidden="true"
                    />
                  </span>
                </summary>
                <div className="settings-ssh__algorithm-list">
                  {catalog[group].map((item) => {
                    const checked = selected.includes(item.name);
                    return (
                      <label className="settings-ssh__algorithm" key={item.name}>
                        <input
                          type="checkbox"
                          checked={checked}
                          disabled={checked && selected.length === 1}
                          onChange={(event) => {
                            setGroupSelection(
                              group,
                              event.target.checked
                                ? [...selected, item.name]
                                : selected.filter((name) => name !== item.name)
                            );
                          }}
                        />
                        <code>{item.name}</code>
                        {item.compatibility && (
                          <small>{t("settings.ssh_algorithm_compatibility")}</small>
                        )}
                      </label>
                    );
                  })}
                </div>
              </details>
            );
          })}
        </div>
      )}
    </div>
  );
}
