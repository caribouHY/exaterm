const GENERAL_TERMINAL_MODE_DEFINITION = {
  cliValue: "general",
  labelKey: "terminal_mode.general",
  sortName: "General",
  value: "general",
} as const;

const DEVICE_TERMINAL_MODE_DEFINITIONS = [
  {
    cliValue: "cisco-ios",
    labelKey: "terminal_mode.cisco_ios",
    sortName: "Cisco IOS",
    value: "cisco_ios",
  },
  {
    cliValue: "arista-eos",
    labelKey: "terminal_mode.arista_eos",
    sortName: "Arista EOS",
    value: "arista_eos",
  },
  {
    cliValue: "juniper-junos",
    labelKey: "terminal_mode.juniper_junos",
    sortName: "Juniper Junos",
    value: "juniper_junos",
  },
  { cliValue: "vyos", labelKey: "terminal_mode.vyos", sortName: "VyOS", value: "vyos" },
  {
    cliValue: "fujitsu-sir",
    labelKey: "terminal_mode.fujitsu_sir",
    sortName: "Fujitsu Si-R",
    value: "fujitsu_sir",
  },
  {
    cliValue: "allied-telesis-awplus",
    labelKey: "terminal_mode.allied_telesis_awplus",
    sortName: "Allied Telesis AW+",
    value: "allied_telesis_awplus",
  },
  {
    cliValue: "furukawa-fitelnet",
    labelKey: "terminal_mode.furukawa_fitelnet",
    sortName: "Furukawa FITELnet",
    value: "furukawa_fitelnet",
  },
] as const;

export const TERMINAL_MODE_CATALOG = [
  GENERAL_TERMINAL_MODE_DEFINITION,
  ...DEVICE_TERMINAL_MODE_DEFINITIONS,
] as const;

export type TerminalMode = (typeof TERMINAL_MODE_CATALOG)[number]["value"];
export type TerminalModeLabelKey = (typeof TERMINAL_MODE_CATALOG)[number]["labelKey"];

export interface TerminalModeOption {
  cliValue: string;
  label: string;
  value: TerminalMode;
}

export const DEFAULT_TERMINAL_MODE: TerminalMode = GENERAL_TERMINAL_MODE_DEFINITION.value;

const TERMINAL_MODE_VALUES = new Set<string>(
  TERMINAL_MODE_CATALOG.map((definition) => definition.value)
);
const TERMINAL_MODE_SORTER = new Intl.Collator("en", { sensitivity: "base" });

export function isTerminalMode(value: unknown): value is TerminalMode {
  return typeof value === "string" && TERMINAL_MODE_VALUES.has(value);
}

export function normalizeTerminalMode(terminalMode: string | null | undefined): TerminalMode {
  return isTerminalMode(terminalMode) ? terminalMode : DEFAULT_TERMINAL_MODE;
}

export function getTerminalModeOptions(
  translate: (labelKey: TerminalModeLabelKey) => string
): TerminalModeOption[] {
  const generalOption = {
    cliValue: GENERAL_TERMINAL_MODE_DEFINITION.cliValue,
    label: translate(GENERAL_TERMINAL_MODE_DEFINITION.labelKey),
    value: GENERAL_TERMINAL_MODE_DEFINITION.value,
  };
  const deviceOptions = DEVICE_TERMINAL_MODE_DEFINITIONS.map((definition) => ({
    cliValue: definition.cliValue,
    label: translate(definition.labelKey),
    sortName: definition.sortName,
    value: definition.value,
  })).sort(
    (left, right) =>
      TERMINAL_MODE_SORTER.compare(left.sortName, right.sortName) ||
      left.value.localeCompare(right.value)
  );

  return [
    generalOption,
    ...deviceOptions.map(({ cliValue, label, value }) => ({
      cliValue,
      label,
      value,
    })),
  ];
}
