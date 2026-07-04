import { useTranslation } from "react-i18next";
import type { SshHostKeyCheck } from "./connectionDialogTypes";

interface HostKeyConfirmationProps {
  hostKeyCheck: SshHostKeyCheck;
}

export function HostKeyConfirmation({ hostKeyCheck }: HostKeyConfirmationProps) {
  const { t } = useTranslation();

  return (
    <div className="connection-dialog__host-key">
      <div className="connection-dialog__host-key-message">
        {hostKeyCheck.status === "mismatch"
          ? t("connection.host_key_mismatch.message", {
              host: hostKeyCheck.host,
              port: hostKeyCheck.port,
            })
          : t("connection.host_key_unknown.message", {
              host: hostKeyCheck.host,
              port: hostKeyCheck.port,
            })}
      </div>

      <div className="connection-dialog__host-key-grid">
        <div className="connection-dialog__host-key-label">{t("connection.host")}</div>
        <div className="connection-dialog__host-key-value">
          {hostKeyCheck.host}:{hostKeyCheck.port}
        </div>

        <div className="connection-dialog__host-key-label">
          {t("connection.host_key_algorithm")}
        </div>
        <div className="connection-dialog__host-key-value">{hostKeyCheck.algorithm}</div>

        <div className="connection-dialog__host-key-label">
          {t("connection.host_key_fingerprint")}
        </div>
        <div className="connection-dialog__host-key-value connection-dialog__host-key-value--mono">
          SHA256:{hostKeyCheck.fingerprint}
        </div>

        {hostKeyCheck.known_fingerprint && (
          <>
            <div className="connection-dialog__host-key-label">
              {t("connection.host_key_known_fingerprint")}
            </div>
            <div className="connection-dialog__host-key-value connection-dialog__host-key-value--mono">
              SHA256:{hostKeyCheck.known_fingerprint}
            </div>
          </>
        )}
      </div>

      <div className="connection-dialog__host-key-warning">{t("connection.host_key_warning")}</div>
    </div>
  );
}
