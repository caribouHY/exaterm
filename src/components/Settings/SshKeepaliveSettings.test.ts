import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { SshConfig } from "../../types";
import { SshKeepaliveSettings } from "./SshKeepaliveSettings";
import { finalizeBoundedInteger, parseBoundedInteger } from "./terminalSettingsModel";

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

describe("SSH keepalive settings", () => {
  it.each([false, true])(
    "renders numeric controls enabled only when keepalive is %s",
    (enabled) => {
      const config: SshConfig = {
        algorithm_mode: "default",
        algorithms: { kex: [], host_key: [], cipher: [], mac: [], compression: [] },
        default_private_key_path: "",
        keepalive_enabled: enabled,
        keepalive_interval_secs: 30,
        keepalive_max: 3,
      };
      const change = vi.fn();
      const html = renderToStaticMarkup(
        createElement(SshKeepaliveSettings, { config, onChange: change })
      );
      expect(html.match(/disabled=""/g)?.length ?? 0).toBe(enabled ? 0 : 2);
      expect(html).toContain('value="30"');
      expect(html).toContain('value="3"');
      expect(html).toContain('max="3600"');
      expect(html).toContain('max="100"');
      expect(change).not.toHaveBeenCalled();
    }
  );

  it.each([3600, 100])("validates integer input with maximum %s", (max) => {
    for (const value of ["", "0", "-1", "1.5", "1e2", String(max + 1)]) {
      expect(parseBoundedInteger(value, 1, max)).toBeNull();
    }
    expect(parseBoundedInteger("1", 1, max)).toBe(1);
    expect(parseBoundedInteger(String(max), 1, max)).toBe(max);
    expect(finalizeBoundedInteger("", 3, 1, max)).toBe(3);
    expect(finalizeBoundedInteger("0", 3, 1, max)).toBe(1);
    expect(finalizeBoundedInteger(String(max + 1), 3, 1, max)).toBe(max);
  });
});
