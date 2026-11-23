import { useTranslation } from "react-i18next";
import type { SerialFormActions, SerialFormState } from "./connectionDialogTypes";
import { ConnectionSelect } from "./ConnectionSearchSelect";
import { ConnectionFieldError, TerminalModeSelect } from "./ConnectionFormFields";
import type { SerialPortDetectionStatus } from "./serialPortDetection";

interface SerialConnectionFormProps {
  formState: SerialFormState;
  formActions: SerialFormActions;
}

const BAUD_RATES = ["300", "1200", "2400", "4800", "9600", "19200", "38400", "57600", "115200"];
const EMPTY_PORT_LIST_LABELS: Record<SerialPortDetectionStatus, string> = {
  loading: "connection.serial_ports_loading",
  failed: "connection.serial_ports_load_failed",
  ready: "connection.no_ports",
};

export function SerialConnectionForm({ formState, formActions }: SerialConnectionFormProps) {
  const { t } = useTranslation();
  const portError = formState.validationErrors.selectedPort;

  return (
    <>
      <div>
        <div className="connection-dialog__serial-port-row">
          <ConnectionSelect
            triggerId="connection-serial-port"
            label={t("connection.port")}
            value={formState.selectedPort}
            onChange={formActions.onSelectedPortChange}
            options={
              formState.ports.length === 0
                ? [{ value: "", label: t(EMPTY_PORT_LIST_LABELS[formState.portDetectionStatus]) }]
                : formState.ports.map((port) => ({
                    value: port.name,
                    label: `${port.name} (${port.port_type})`,
                  }))
            }
            required
            invalid={Boolean(portError)}
            describedBy={portError ? "connection-serial-port-error" : undefined}
          />
          <button
            className="btn btn-ghost btn-sm"
            type="button"
            onClick={formActions.onRefreshPorts}
          >
            {t("connection.serial_ports_refresh")}
          </button>
        </div>
        <ConnectionFieldError id="connection-serial-port-error" error={portError} />
      </div>
      <div className="connection-dialog__row">
        <ConnectionSelect
          label={t("connection.baud_rate")}
          value={formState.baudRate}
          options={BAUD_RATES.map((rate) => ({ value: rate, label: rate }))}
          onChange={formActions.onBaudRateChange}
        />
        <ConnectionSelect
          label={t("connection.data_bits")}
          value={formState.dataBits}
          options={["5", "6", "7", "8"].map((bits) => ({ value: bits, label: bits }))}
          onChange={formActions.onDataBitsChange}
        />
      </div>
      <div className="connection-dialog__row">
        <ConnectionSelect
          label={t("connection.parity")}
          value={formState.parity}
          options={["none", "odd", "even"].map((parity) => ({
            value: parity,
            label: t(`connection.parity_${parity}`),
          }))}
          onChange={formActions.onParityChange}
        />
        <ConnectionSelect
          label={t("connection.stop_bits")}
          value={formState.stopBits}
          options={["1", "2"].map((bits) => ({ value: bits, label: bits }))}
          onChange={formActions.onStopBitsChange}
        />
      </div>
      <TerminalModeSelect
        value={formState.terminalMode}
        onChange={formActions.onTerminalModeChange}
      />
    </>
  );
}
