import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { SshConfig } from "../../types";
import { finalizeBoundedInteger, parseBoundedInteger } from "./terminalSettingsModel";
import { SettingsToggle } from "./SettingsToggle";

interface SshKeepaliveSettingsProps {
  config: SshConfig;
  onChange: (patch: Partial<SshConfig>) => void;
}

function KeepaliveNumber({
  field,
  label,
  value,
  max,
  disabled,
  onChange,
}: {
  field: "keepalive_interval_secs" | "keepalive_max";
  label: string;
  value: number;
  max: number;
  disabled: boolean;
  onChange: (patch: Partial<SshConfig>) => void;
}) {
  const [input, setInput] = useState(String(value));
  const [editing, setEditing] = useState(false);
  useEffect(() => {
    if (!editing) setInput(String(value));
  }, [value, editing]);
  const id = `settings-ssh-${field}`;
  return (
    <div>
      <label className="label" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        className="input"
        type="number"
        min={1}
        max={max}
        step={1}
        disabled={disabled}
        value={input}
        onFocus={() => setEditing(true)}
        onChange={(event) => {
          setInput(event.target.value);
          const parsed = parseBoundedInteger(event.target.value, 1, max);
          if (parsed !== null) onChange({ [field]: parsed });
        }}
        onBlur={() => {
          const finalized = finalizeBoundedInteger(input, value, 1, max);
          setInput(String(finalized));
          onChange({ [field]: finalized });
          setEditing(false);
        }}
      />
    </div>
  );
}

export function SshKeepaliveSettings({ config, onChange }: SshKeepaliveSettingsProps) {
  const { t } = useTranslation();
  return (
    <>
      <SettingsToggle
        id="settings-ssh-keepalive"
        label={t("settings.ssh_keepalive")}
        description={t("settings.ssh_keepalive_desc")}
        checked={config.keepalive_enabled}
        onChange={(enabled) => onChange({ keepalive_enabled: enabled })}
      />
      <div className="settings-row">
        <KeepaliveNumber
          field="keepalive_interval_secs"
          label={t("settings.ssh_keepalive_interval")}
          value={config.keepalive_interval_secs}
          max={3600}
          disabled={!config.keepalive_enabled}
          onChange={onChange}
        />
        <KeepaliveNumber
          field="keepalive_max"
          label={t("settings.ssh_keepalive_max")}
          value={config.keepalive_max}
          max={100}
          disabled={!config.keepalive_enabled}
          onChange={onChange}
        />
      </div>
    </>
  );
}
