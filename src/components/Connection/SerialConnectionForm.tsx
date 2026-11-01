import { useTranslation } from "react-i18next";
import type { SerialFormActions, SerialFormState } from "./connectionDialogTypes";
import { ConnectionSelect } from "./ConnectionSearchSelect";
import { ConnectionFieldError, TerminalModeSelect } from "./ConnectionFormFields";

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
        <ConnectionSelect
          triggerId="connection-serial-port"
          label={t("connection.port")}
          value={formState.selectedPort}
          onChange={formActions.onSelectedPortChange}
          options={
            formState.ports.length === 0
              ? [{ value: "", label: t("connection.no_ports") }]
              : formState.ports.map((port) => ({
                  value: port.name,
                  label: `${port.name} (${port.port_type})`,
                }))
          }
          required
          invalid={Boolean(portError)}
          describedBy={portError ? "connection-serial-port-error" : undefined}
        />
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
