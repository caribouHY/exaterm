import { X } from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { ConnectionHistoryEntry, ConnectionType, SavedConnection } from "../../types";
import {
  FeedbackMessage,
  ModalBody,
  ModalBusy,
  ModalFooter,
  ModalFrame,
  ModalHeader,
  ModalTitle,
} from "../Common";
import { SerialConnectionForm } from "./SerialConnectionForm";
import { SshConnectionForm } from "./SshConnectionForm";
import { SshDiagnosticsPanel } from "./SshDiagnosticsPanel";
import { TelnetConnectionForm } from "./TelnetConnectionForm";
import type {
  SerialFormState,
  SshFormActions,
  SshFormState,
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
  historyError: string;
  shortcutText: string;
  sshProfiles: SavedConnection[];
  sshHistoryEntries: ConnectionHistoryEntry[];
  jumpProfileOptions: SavedConnection[];
  telnetProfiles: SavedConnection[];
  telnetHistoryEntries: ConnectionHistoryEntry[];
  getProfileDisplayName: (profile: SavedConnection) => string;
  getHistoryDisplayName: (entry: ConnectionHistoryEntry) => string;
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
  historyError,
  shortcutText,
  sshProfiles,
  sshHistoryEntries,
  jumpProfileOptions,
  telnetProfiles,
  telnetHistoryEntries,
  getProfileDisplayName,
  getHistoryDisplayName,
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
  const errorRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (tab === "ssh" && error) errorRef.current?.focus();
  }, [error, tab]);

  return (
    <div className="connection-overlay" onMouseDown={onOverlayMouseDown} onClick={onOverlayClick}>
      <ModalFrame
        className="connection-dialog"
        role="dialog"
        ariaModal
        ariaLabelledBy="connection-dialog-title"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <ModalHeader className="connection-dialog__header">
          <ModalTitle className="connection-dialog__title" id="connection-dialog-title">
            {t("connection.new")}
          </ModalTitle>
          <button className="btn-icon" onClick={onClose} aria-label={t("connection.cancel")}>
            <X size={16} />
          </button>
        </ModalHeader>

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

        {tab === "ssh" && error && (
          <div
            ref={errorRef}
            className="connection-dialog__error-banner"
            role="alert"
            tabIndex={-1}
          >
            <FeedbackMessage tone="error">{error}</FeedbackMessage>
          </div>
        )}

        <ModalBody className="connection-dialog__body">
          {tab === "ssh" ? (
            <SshConnectionForm
              formState={sshFormState}
              formActions={sshFormActions}
              profileOptions={{
                profiles: sshProfiles,
                historyEntries: sshHistoryEntries,
                jumpProfiles: jumpProfileOptions,
                getDisplayName: getProfileDisplayName,
                getHistoryDisplayName,
              }}
            />
          ) : tab === "telnet" ? (
            <TelnetConnectionForm
              formState={telnetFormState}
              formActions={telnetFormActions}
              profileOptions={{
                profiles: telnetProfiles,
                historyEntries: telnetHistoryEntries,
                getDisplayName: getProfileDisplayName,
                getHistoryDisplayName,
              }}
            />
          ) : (
            <SerialConnectionForm formState={serialFormState} formActions={serialActions} />
          )}
          {tab !== "ssh" && error && (
            <FeedbackMessage tone="error" className="connection-dialog__error">
              {error}
            </FeedbackMessage>
          )}
          {tab !== "serial" && historyError && (
            <FeedbackMessage tone="error" className="connection-dialog__error">
              {historyError}
            </FeedbackMessage>
          )}
          {tab === "ssh" && <SshDiagnosticsPanel {...diagnosticsPanelProps} />}
        </ModalBody>

        <ModalFooter className="connection-dialog__footer">
          {connecting ? (
            <ModalBusy className="connection-dialog__connecting">
              {t("connection.connecting")}
            </ModalBusy>
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
