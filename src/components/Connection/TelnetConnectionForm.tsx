import { useTranslation } from "react-i18next";
import type {
  TelnetFormActions,
  TelnetFormState,
  NetworkProfileOptions,
} from "./connectionDialogTypes";
import {
  ConnectionFieldError,
  ConnectionFieldLabelHtml,
  EncodingSelect,
  TerminalModeSelect,
} from "./ConnectionFormFields";
import { NetworkConnectionForm } from "./NetworkConnectionForm";

interface TelnetConnectionFormProps {
  formState: TelnetFormState;
  formActions: TelnetFormActions;
  profileOptions: NetworkProfileOptions;
}

export function TelnetConnectionForm({
  formState,
  formActions,
  profileOptions,
}: TelnetConnectionFormProps) {
  const { t } = useTranslation();
  const errors = formState.validationErrors;
  return (
    <NetworkConnectionForm
      formState={formState}
      formActions={formActions}
      profileOptions={profileOptions}
      advancedFields={
        <>
          <EncodingSelect value={formState.encoding} onChange={formActions.onEncodingChange} />
          <TerminalModeSelect
            value={formState.terminalMode}
            onChange={formActions.onTerminalModeChange}
          />
        </>
      }
    >
      <div className="connection-dialog__row">
        <div>
          <ConnectionFieldLabelHtml htmlFor="connection-telnet-host" required>
            {t("connection.host")}
          </ConnectionFieldLabelHtml>
          <input
            id="connection-telnet-host"
            className="input connection-dialog__destination-input"
            value={formState.host}
            onChange={(event) => {
              formActions.onHostChange(event.target.value);
            }}
            placeholder="192.168.1.1"
            required
            aria-invalid={Boolean(errors.host)}
            aria-describedby={errors.host ? "connection-telnet-host-error" : undefined}
          />
          <ConnectionFieldError id="connection-telnet-host-error" error={errors.host} />
        </div>
        <div style={{ maxWidth: 100 }}>
          <ConnectionFieldLabelHtml htmlFor="connection-telnet-port" required>
            {t("connection.port")}
          </ConnectionFieldLabelHtml>
          <input
            id="connection-telnet-port"
            className="input"
            type="number"
            value={formState.port}
            onChange={(event) => {
              formActions.onPortChange(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                formActions.onPortEnter();
              }
            }}
            required
            min={1}
            max={65535}
            step={1}
            aria-invalid={Boolean(errors.port)}
            aria-describedby={errors.port ? "connection-telnet-port-error" : undefined}
          />
          <ConnectionFieldError id="connection-telnet-port-error" error={errors.port} />
        </div>
      </div>
    </NetworkConnectionForm>
  );
}
