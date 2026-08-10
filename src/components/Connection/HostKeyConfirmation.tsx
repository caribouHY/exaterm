import { useTranslation } from "react-i18next";
import type { HostKeyCheckResult } from "../../types";
import { FeedbackMessage } from "../Common";

interface HostKeyConfirmationProps {
  hostKeyCheck: HostKeyCheckResult;
}

export function HostKeyConfirmation({ hostKeyCheck }: HostKeyConfirmationProps) {
  const { t } = useTranslation();

  return (
    <div className="ssh-host-key-prompt__content">
      <div className="ssh-host-key-prompt__message">
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

      <div className="ssh-host-key-prompt__grid">
        <div className="ssh-host-key-prompt__label">{t("connection.host")}</div>
        <div className="ssh-host-key-prompt__value">
          {hostKeyCheck.host}:{hostKeyCheck.port}
        </div>

        <div className="ssh-host-key-prompt__label">{t("connection.host_key_algorithm")}</div>
        <div className="ssh-host-key-prompt__value">{hostKeyCheck.algorithm}</div>

        <div className="ssh-host-key-prompt__label">{t("connection.host_key_fingerprint")}</div>
        <div className="ssh-host-key-prompt__value ssh-host-key-prompt__value--mono">
          SHA256:{hostKeyCheck.fingerprint}
        </div>

        {hostKeyCheck.known_fingerprint && (
          <>
            <div className="ssh-host-key-prompt__label">
              {t("connection.host_key_known_fingerprint")}
            </div>
            <div className="ssh-host-key-prompt__value ssh-host-key-prompt__value--mono">
              SHA256:{hostKeyCheck.known_fingerprint}
            </div>
          </>
        )}
      </div>

      <FeedbackMessage tone="warning" className="ssh-host-key-prompt__warning">
        {t("connection.host_key_warning")}
      </FeedbackMessage>
    </div>
  );
}
