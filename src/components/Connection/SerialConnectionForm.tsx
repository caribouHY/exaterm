import { useTranslation } from "react-i18next";
import type { SerialFormActions, SerialFormState } from "./connectionDialogTypes";
import {
  ConnectionFieldError,
  ConnectionFieldLabel,
  TerminalModeSelect,
} from "./ConnectionFormFields";

interface SerialConnectionFormProps {
  formState: SerialFormState;
  formActions: SerialFormActions;
}

const BAUD_RATES = ["300", "1200", "2400", "4800", "9600", "19200", "38400", "57600", "115200"];

export function SerialConnectionForm({ formState, formActions }: SerialConnectionFormProps) {
  const { t } = useTranslation();
  const portError = formState.validationErrors.selectedPort;

  return (
    <>
      <div>
        <ConnectionFieldLabel htmlFor="connection-serial-port" required>
          {t("connection.port")}
        </ConnectionFieldLabel>
        <select
          id="connection-serial-port"
          className="select"
          style={{ width: "100%" }}
          value={formState.selectedPort}
          onChange={(event) => {
            formActions.onSelectedPortChange(event.target.value);
          }}
          required
          aria-invalid={Boolean(portError)}
          aria-describedby={portError ? "connection-serial-port-error" : undefined}
        >
          {formState.ports.length === 0 && <option value="">{t("connection.no_ports")}</option>}
          {formState.ports.map((p) => (
            <option key={p.name} value={p.name}>
              {p.name} ({p.port_type})
            </option>
          ))}
        </select>
        <ConnectionFieldError id="connection-serial-port-error" error={portError} />
      </div>
      <div className="connection-dialog__row">
        <div>
          <label className="label">{t("connection.baud_rate")}</label>
          <select
            className="select"
            style={{ width: "100%" }}
            value={formState.baudRate}
            onChange={(event) => {
              formActions.onBaudRateChange(event.target.value);
            }}
          >
            {BAUD_RATES.map((rate) => (
              <option key={rate} value={rate}>
                {rate}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="label">{t("connection.data_bits")}</label>
          <select
            className="select"
            style={{ width: "100%" }}
            value={formState.dataBits}
            onChange={(event) => {
              formActions.onDataBitsChange(event.target.value);
            }}
          >
            {["5", "6", "7", "8"].map((dataBits) => (
              <option key={dataBits} value={dataBits}>
                {dataBits}
              </option>
            ))}
          </select>
        </div>
      </div>
      <div className="connection-dialog__row">
        <div>
          <label className="label">{t("connection.parity")}</label>
          <select
            className="select"
            style={{ width: "100%" }}
            value={formState.parity}
            onChange={(event) => {
              formActions.onParityChange(event.target.value);
            }}
          >
            <option value="none">{t("connection.parity_none")}</option>
            <option value="odd">{t("connection.parity_odd")}</option>
            <option value="even">{t("connection.parity_even")}</option>
          </select>
        </div>
        <div>
          <label className="label">{t("connection.stop_bits")}</label>
          <select
            className="select"
            style={{ width: "100%" }}
            value={formState.stopBits}
            onChange={(event) => {
              formActions.onStopBitsChange(event.target.value);
            }}
          >
            <option value="1">1</option>
            <option value="2">2</option>
          </select>
        </div>
      </div>
      <TerminalModeSelect
        value={formState.terminalMode}
        onChange={formActions.onTerminalModeChange}
      />
    </>
  );
}
