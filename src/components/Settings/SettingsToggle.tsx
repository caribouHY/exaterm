interface SettingsToggleProps {
  id: string;
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

export function SettingsToggle({
  id,
  label,
  description,
  checked,
  onChange,
  disabled = false,
}: SettingsToggleProps) {
  return (
    <div className="settings-toggle-row">
      <label className="settings-toggle-label" htmlFor={id}>
        <span>{label}</span>
        <small id={`${id}-description`}>{description}</small>
      </label>
      <span className="toggle">
        <input
          id={id}
          type="checkbox"
          role="switch"
          aria-describedby={`${id}-description`}
          checked={checked}
          disabled={disabled}
          onChange={(event) => {
            onChange(event.target.checked);
          }}
        />
        <label className="toggle-track" htmlFor={id} aria-hidden="true" />
      </span>
    </div>
  );
}
