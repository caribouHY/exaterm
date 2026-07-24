import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ConnectionType, SavedConnection } from "../../types";
import {
  FeedbackMessage,
  ModalBody,
  ModalBusy,
  ModalFooter,
  ModalFrame,
  ModalHeader,
  ModalTitle,
} from "../Common";
import { HostKeyConfirmation } from "./HostKeyConfirmation";
import { SerialConnectionForm } from "./SerialConnectionForm";
import { SshConnectionForm } from "./SshConnectionForm";
import { SshDiagnosticsPanel } from "./SshDiagnosticsPanel";
import { TelnetConnectionForm } from "./TelnetConnectionForm";
import type {
  SerialFormState,
  SshFormActions,
  SshFormState,
  SshHostKeyCheck,
  TelnetFormActions,
  TelnetFormState,
} from "./connectionDialogTypes";
import type { useConnectionActions } from "./useConnectionActions";
import type { useSshDiagnostics } from "./useSshDiagnostics";

interface ConnectionDialogViewProps {
  tab: ConnectionType;
  setTab: (value: ConnectionType) => void;
  connecting: boolean;
  error: string;
  hostKeyCheck: SshHostKeyCheck | null;
  setHostKeyCheck: (value: SshHostKeyCheck | null) => void;
  shortcutText: string;
  hostKeyTitle: string;
  sshProfiles: SavedConnection[];
  jumpProfileOptions: SavedConnection[];
  telnetProfiles: SavedConnection[];
  getProfileDisplayName: (profile: SavedConnection) => string;
  sshFormState: SshFormState;
  sshFormActions: SshFormActions;
  telnetFormState: TelnetFormState;
  telnetFormActions: TelnetFormActions;
  serialFormState: SerialFormState;
  serialActions: {
    onSelectedPortChange: (value: string) => void;
    onBaudRateChange: (value: string) => void;
    onDataBitsChange: (value: string) => void;
    onParityChange: (value: string) => void;
    onStopBitsChange: (value: string) => void;
    onTerminalModeChange: (value: string) => void;
  };
  diagnosticsPanelProps: {
    logs: ReturnType<typeof useSshDiagnostics>["logs"];
    expanded: boolean;
    copied: boolean;
    onToggleExpanded: () => void;
    onCopy: () => void;
  };
  connectionActions: ReturnType<typeof useConnectionActions>;
  onClose: () => void;
  onOverlayMouseDown: (event: React.MouseEvent<HTMLDivElement>) => void;
  onOverlayClick: (event: React.MouseEvent<HTMLDivElement>) => void;
}

export function ConnectionDialogView({
  tab,
  setTab,
  connecting,
  error,
  hostKeyCheck,
  setHostKeyCheck,
  shortcutText,
  hostKeyTitle,
  sshProfiles,
  jumpProfileOptions,
  telnetProfiles,
  getProfileDisplayName,
  sshFormState,
  sshFormActions,
  telnetFormState,
  telnetFormActions,
  serialFormState,
  serialActions,
  diagnosticsPanelProps,
  connectionActions,
  onClose,
  onOverlayMouseDown,
  onOverlayClick,
}: ConnectionDialogViewProps) {
  const { t } = useTranslation();

  return (
    <div className="connection-overlay" onMouseDown={onOverlayMouseDown} onClick={onOverlayClick}>
      <ModalFrame
        className="connection-dialog"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <ModalHeader className="connection-dialog__header">
          <ModalTitle className="connection-dialog__title">
            {hostKeyCheck ? hostKeyTitle : t("connection.new")}
          </ModalTitle>
          <button className="btn-icon" onClick={onClose}>
            <X size={16} />
          </button>
        </ModalHeader>

        {!hostKeyCheck && (
          <div className="connection-dialog__tabs">
            <button
              className={`connection-dialog__tab ${tab === "ssh" ? "connection-dialog__tab--active" : ""}`}
              onClick={() => {
                setTab("ssh");
              }}
            >
              {t("connection.ssh")}
            </button>
            <button
              className={`connection-dialog__tab ${tab === "telnet" ? "connection-dialog__tab--active" : ""}`}
              onClick={() => {
                setTab("telnet");
              }}
            >
              {t("connection.telnet")}
            </button>
            <button
              className={`connection-dialog__tab ${tab === "serial" ? "connection-dialog__tab--active" : ""}`}
              onClick={() => {
                setTab("serial");
              }}
            >
              {t("connection.serial")}
            </button>
          </div>
        )}

        <ModalBody className="connection-dialog__body">
          {hostKeyCheck ? (
            <HostKeyConfirmation hostKeyCheck={hostKeyCheck} />
          ) : tab === "ssh" ? (
            <SshConnectionForm
              formState={sshFormState}
              formActions={sshFormActions}
              profileOptions={{
                profiles: sshProfiles,
                jumpProfiles: jumpProfileOptions,
                getDisplayName: getProfileDisplayName,
              }}
            />
          ) : tab === "telnet" ? (
            <TelnetConnectionForm
              formState={telnetFormState}
              formActions={telnetFormActions}
              profileOptions={{
                profiles: telnetProfiles,
                getDisplayName: getProfileDisplayName,
              }}
            />
          ) : (
            <SerialConnectionForm formState={serialFormState} formActions={serialActions} />
          )}
          {error && (
            <FeedbackMessage tone="error" className="connection-dialog__error">
              {error}
            </FeedbackMessage>
          )}
          {tab === "ssh" && <SshDiagnosticsPanel {...diagnosticsPanelProps} />}
        </ModalBody>

        <ModalFooter className="connection-dialog__footer">
          {connecting ? (
            <ModalBusy className="connection-dialog__connecting">
              {t("connection.connecting")}
            </ModalBusy>
          ) : hostKeyCheck ? (
            <>
              <button
                className="btn btn-ghost"
                onClick={() => {
                  setHostKeyCheck(null);
                }}
              >
                {t("connection.cancel")}
              </button>
              <button
                className={`btn ${hostKeyCheck.status === "mismatch" ? "btn-danger" : "btn-primary"}`}
                onClick={() =>
                  void connectionActions.handleTrustAndConnect(hostKeyCheck.status === "mismatch")
                }
              >
                {hostKeyCheck.status === "mismatch"
                  ? t("connection.host_key_replace_connect")
                  : t("connection.host_key_trust_connect")}{" "}
                <span className="connection-dialog__shortcut">{shortcutText}</span>
              </button>
            </>
          ) : (
            <>
              <button className="btn btn-ghost" onClick={onClose}>
                {t("connection.cancel")}
              </button>
              <button
                className="btn btn-primary"
                onClick={() => {
                  void connectionActions.handleConnect();
                }}
              >
                {t("connection.connect")}{" "}
                <span className="connection-dialog__shortcut">{shortcutText}</span>
              </button>
            </>
          )}
        </ModalFooter>
      </ModalFrame>
    </div>
  );
}
