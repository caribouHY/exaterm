import { useTranslation } from "react-i18next";
import type { ShortcutBinding, ShortcutConfig } from "../../types";
import { formatShortcut } from "../../features/shortcuts/shortcutModel";
import appIcon from "../../../src-tauri/icons/icon.png";
import "./TerminalView.css";

interface TerminalEmptyStateProps {
  isActive: boolean;
  shortcuts: ShortcutConfig;
  onOpenConnection: () => void;
}

export default function TerminalEmptyState({
  isActive,
  shortcuts,
  onOpenConnection,
}: TerminalEmptyStateProps) {
  const { t } = useTranslation();

  return (
    <div className={`terminal-view ${!isActive ? "terminal-view--hidden" : ""}`}>
      <div className="terminal-view__empty">
        <img className="terminal-view__empty-icon" src={appIcon} alt="" aria-hidden="true" />
        <div className="terminal-view__empty-title">ExaTerm</div>
        <div className="terminal-view__empty-desc">{t("terminal.empty_desc")}</div>
        <button className="btn btn-primary" onClick={onOpenConnection}>
          {t("connection.new")}
        </button>
        <div className="terminal-view__empty-shortcuts">
          {(
            [
              { binding: shortcuts.new_connection, label: t("connection.new") },
              { binding: shortcuts.new_window, label: t("titlebar.menu.new_window") },
              { binding: shortcuts.open_settings, label: t("titlebar.menu.settings") },
            ] satisfies Array<{ binding: ShortcutBinding | null; label: string }>
          ).map(({ binding, label }) => {
            const shortcut = formatShortcut(binding);
            return shortcut ? (
              <div className="terminal-view__shortcut" key={String(label)}>
                <span className="terminal-view__key">{shortcut}</span>
                <span>{label}</span>
              </div>
            ) : null;
          })}
        </div>
      </div>
    </div>
  );
}
