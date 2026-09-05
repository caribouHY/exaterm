import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../types";
import { isDirectConnectControlDisabled, normalizeExternalControlConfig } from "./settingsModel";

describe("external control settings", () => {
  it("defaults a missing direct connection permission to disabled", () => {
    const config = {
      external_control: {
        enabled: true,
        connect_enabled: true,
        mcp_enabled: true,
        cli_enabled: true,
      },
    } as AppConfig;

    expect(normalizeExternalControlConfig(config).external_control.direct_connect_enabled).toBe(
      false
    );
  });

  it.each([
    [false, false, true],
    [false, true, true],
    [true, false, true],
    [true, true, false],
  ])(
    "uses the master and new-connection permissions (%s, %s)",
    (externalControlEnabled, connectEnabled, expected) => {
      expect(isDirectConnectControlDisabled(externalControlEnabled, connectEnabled)).toBe(expected);
    }
  );
});
