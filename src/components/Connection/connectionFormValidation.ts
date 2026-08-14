import type { ConnectionType, SshAuthMethod } from "../../types";

export type ConnectionValidationError = "required" | "port_range" | "serial_port_required";

export interface SshConnectionValidationErrors {
  host?: ConnectionValidationError;
  port?: ConnectionValidationError;
  username?: ConnectionValidationError;
  privateKeyPath?: ConnectionValidationError;
}

export interface TelnetConnectionValidationErrors {
  host?: ConnectionValidationError;
  port?: ConnectionValidationError;
}

export interface SerialConnectionValidationErrors {
  selectedPort?: ConnectionValidationError;
}

export interface ConnectionFormValidationState {
  ssh: {
    isValid: boolean;
    errors: SshConnectionValidationErrors;
  };
  telnet: {
    isValid: boolean;
    errors: TelnetConnectionValidationErrors;
  };
  serial: {
    isValid: boolean;
    errors: SerialConnectionValidationErrors;
  };
}

interface SshConnectionValidationInput {
  host: string;
  port: string;
  username: string;
  authMethod: SshAuthMethod;
  privateKeyPath: string;
}

interface TelnetConnectionValidationInput {
  host: string;
  port: string;
}

interface SerialConnectionValidationInput {
  selectedPort: string;
}

const isBlank = (value: string): boolean => value.trim() === "";

export const parseConnectionPort = (value: string): number | null => {
  const trimmed = value.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const port = Number(trimmed);
  return Number.isInteger(port) && port >= 1 && port <= 65_535 ? port : null;
};

export const validateSshConnectionForm = ({
  host,
  port,
  username,
  authMethod,
  privateKeyPath,
}: SshConnectionValidationInput): ConnectionFormValidationState["ssh"] => {
  const errors: SshConnectionValidationErrors = {};
  if (isBlank(host)) errors.host = "required";
  if (parseConnectionPort(port) === null) errors.port = "port_range";
  if (isBlank(username)) errors.username = "required";
  if (authMethod === "public_key" && isBlank(privateKeyPath)) {
    errors.privateKeyPath = "required";
  }
  return { isValid: Object.keys(errors).length === 0, errors };
};

export const validateTelnetConnectionForm = ({
  host,
  port,
}: TelnetConnectionValidationInput): ConnectionFormValidationState["telnet"] => {
  const errors: TelnetConnectionValidationErrors = {};
  if (isBlank(host)) errors.host = "required";
  if (parseConnectionPort(port) === null) errors.port = "port_range";
  return { isValid: Object.keys(errors).length === 0, errors };
};

export const validateSerialConnectionForm = ({
  selectedPort,
}: SerialConnectionValidationInput): ConnectionFormValidationState["serial"] => {
  const errors: SerialConnectionValidationErrors = {};
  if (isBlank(selectedPort)) errors.selectedPort = "serial_port_required";
  return { isValid: Object.keys(errors).length === 0, errors };
};

export const isActiveConnectionFormValid = (
  connectionType: ConnectionType,
  validation: ConnectionFormValidationState
): boolean => {
  switch (connectionType) {
    case "ssh":
      return validation.ssh.isValid;
    case "telnet":
      return validation.telnet.isValid;
    case "serial":
      return validation.serial.isValid;
  }
};
