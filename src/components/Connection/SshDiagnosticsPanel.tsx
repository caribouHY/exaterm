import { ChevronDown, ChevronRight, Copy } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { SshDiagnosticEntry } from "./connectionDialogTypes";

interface SshDiagnosticsPanelProps {
  logs: SshDiagnosticEntry[];
  expanded: boolean;
  copied: boolean;
  onToggleExpanded: () => void;
  onCopy: () => void;
}

export function SshDiagnosticsPanel({
  logs,
  expanded,
  copied,
  onToggleExpanded,
  onCopy,
}: SshDiagnosticsPanelProps) {
  const { t } = useTranslation();
  if (logs.length === 0) return null;

  return (
    <div className="connection-dialog__diagnostics">
      <div className="connection-dialog__diagnostics-header">
        <button
          className="connection-dialog__diagnostics-toggle"
          type="button"
          onClick={onToggleExpanded}
        >
          {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          <span>{t("connection.ssh_diagnostics")}</span>
        </button>
        <button
          className="btn btn-ghost btn-sm connection-dialog__diagnostics-copy"
          type="button"
          onClick={onCopy}
          title={t("connection.ssh_diagnostics_copy")}
        >
          <Copy size={13} />
          {copied ? t("connection.ssh_diagnostics_copied") : t("connection.ssh_diagnostics_copy")}
        </button>
      </div>
      {expanded && (
        <div className="connection-dialog__diagnostics-log" role="log" aria-live="polite">
          {logs.map((entry) => (
            <div
              key={entry.id}
              className={`connection-dialog__diagnostics-line connection-dialog__diagnostics-line--${entry.level}`}
            >
              <span className="connection-dialog__diagnostics-time">{entry.time}</span>
              <span>{entry.message}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
