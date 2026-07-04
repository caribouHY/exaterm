import { useTranslation } from "react-i18next";
import { normalizeTerminalMode, TERMINAL_MODE_OPTIONS } from "../../utils/terminalModes";
import type { SerialFormActions, SerialFormState } from "./connectionDialogTypes";

interface SerialConnectionFormProps {
  formState: SerialFormState;
  formActions: SerialFormActions;
}

const BAUD_RATES = ["300", "1200", "2400", "4800", "9600", "19200", "38400", "57600", "115200"];

export function SerialConnectionForm({ formState, formActions }: SerialConnectionFormProps) {
  const { t } = useTranslation();

  return (
    <>
      <div>
        <label className="label">{t("connection.port")}</label>
        <select
          className="select"
          style={{ width: "100%" }}
          value={formState.selectedPort}
          onChange={(e) => formActions.onSelectedPortChange(e.target.value)}
        >
          {formState.ports.length === 0 && <option value="">{t("connection.no_ports")}</option>}
          {formState.ports.map((p) => (
            <option key={p.name} value={p.name}>
              {p.name} ({p.port_type})
            </option>
          ))}
        </select>
      </div>
      <div className="connection-dialog__row">
        <div>
          <label className="label">{t("connection.baud_rate")}</label>
          <select
            className="select"
            style={{ width: "100%" }}
            value={formState.baudRate}
            onChange={(e) => formActions.onBaudRateChange(e.target.value)}
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
            onChange={(e) => formActions.onDataBitsChange(e.target.value)}
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
            onChange={(e) => formActions.onParityChange(e.target.value)}
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
            onChange={(e) => formActions.onStopBitsChange(e.target.value)}
          >
            <option value="1">1</option>
            <option value="2">2</option>
          </select>
        </div>
      </div>
      <div>
        <label className="label">{t("connection.terminal_mode")}</label>
        <select
          className="select"
          style={{ width: "100%" }}
          value={formState.terminalMode}
          onChange={(e) => formActions.onTerminalModeChange(normalizeTerminalMode(e.target.value))}
        >
          {TERMINAL_MODE_OPTIONS.map((entry) => (
            <option key={entry.value} value={entry.value}>
              {t(entry.labelKey)}
            </option>
          ))}
        </select>
      </div>
    </>
  );
}
