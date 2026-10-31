import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import {
  EncodingSelect,
  ProfileGroupField,
  ProfileSelector,
  TerminalModeSelect,
} from "./ConnectionFormFields";
import { ConnectionSearchSelect, ConnectionSelect } from "./ConnectionSearchSelect";
import { SerialConnectionForm } from "./SerialConnectionForm";

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

describe("consistent connection dropdowns", () => {
  it("uses labeled select-only comboboxes for fixed choices", () => {
    const markup = renderToStaticMarkup(
      createElement(ConnectionSelect, {
        label: "Authentication",
        value: "auto",
        options: [
          { value: "auto", label: "Automatic" },
          { value: "password", label: "Password" },
        ],
        onChange: vi.fn(),
      })
    );
    expect(markup).toContain('role="combobox"');
    expect(markup).toContain('aria-haspopup="listbox"');
    expect(markup).toContain('aria-expanded="false"');
    expect(markup).toContain("Automatic");
    expect(markup).not.toMatch(/<select(?:\s|>)/);
    expect(markup).not.toContain('aria-autocomplete="list"');
  });

  it("uses the same picker for encoding and terminal mode", () => {
    for (const markup of [
      renderToStaticMarkup(createElement(EncodingSelect, { value: "utf-8", onChange: vi.fn() })),
      renderToStaticMarkup(
        createElement(TerminalModeSelect, { value: "general", onChange: vi.fn() })
      ),
    ]) {
      expect(markup).toContain('role="combobox"');
      expect(markup).not.toMatch(/<select(?:\s|>)/);
    }
  });

  it("preserves Serial port validation and no-port feedback with custom pickers", () => {
    const markup = renderToStaticMarkup(
      createElement(SerialConnectionForm, {
        formState: {
          ports: [],
          selectedPort: "",
          baudRate: "9600",
          dataBits: "8",
          parity: "none",
          stopBits: "1",
          terminalMode: "general",
          validationErrors: { selectedPort: "serial_port_required" },
        },
        formActions: {
          onSelectedPortChange: vi.fn(),
          onBaudRateChange: vi.fn(),
          onDataBitsChange: vi.fn(),
          onParityChange: vi.fn(),
          onStopBitsChange: vi.fn(),
          onTerminalModeChange: vi.fn(),
        },
      })
    );
    expect(markup).not.toMatch(/<select(?:\s|>)/);
    expect(markup.match(/role="combobox"/g)).toHaveLength(6);
    expect(markup).toContain('id="connection-serial-port"');
    expect(markup).toContain('aria-required="true"');
    expect(markup).toContain('aria-invalid="true"');
    expect(markup).toContain('aria-describedby="connection-serial-port-error"');
    expect(markup).toContain("connection.no_ports");
    expect(markup).toContain("connection.validation_serial_port_required");
  });
});

describe("saved profile group fields", () => {
  it("does not describe an unavailable profile as a manual or direct connection", () => {
    const markup = renderToStaticMarkup(
      createElement(ConnectionSearchSelect, {
        label: "Jump profile",
        value: "profile:removed",
        sections: [
          {
            key: "direct",
            options: [{ value: "", label: "Direct", searchText: "", alwaysVisible: true }],
          },
        ],
        onChange: vi.fn(),
      })
    );
    expect(markup).toContain("connection.profile_unavailable");
    expect(markup).not.toContain("connection.profile_manual");
  });
  it("renders the selected profile independently of the closed search menu", () => {
    const markup = renderToStaticMarkup(
      createElement(ProfileSelector, {
        selectedProfileId: "router",
        selectedHistoryId: "",
        profiles: [
          { id: "legacy", connection_type: "ssh" },
          { id: "router", connection_type: "ssh", group: "Site <A>" },
        ],
        historyEntries: [],
        getDisplayName: (profile) => profile.id,
        getHistoryDisplayName: (entry) => entry.id,
        onSelectSource: vi.fn(),
        onDeleteProfile: vi.fn(),
        onDeleteHistory: vi.fn(),
        onCopyProfile: vi.fn(),
      })
    );
    expect(markup).toContain('aria-haspopup="listbox"');
    expect(markup).toContain('aria-expanded="false"');
    expect(markup).toContain("router");
    expect(markup).toContain("connection.profile_delete");
  });

  it("offers existing group names and allows a new name or an empty value", () => {
    const markup = renderToStaticMarkup(
      createElement(ProfileGroupField, {
        value: "New group",
        groupNames: ["Site A", "Lab"],
        onChange: vi.fn(),
      })
    );
    expect(markup).toContain('value="New group"');
    expect(markup).toContain('role="combobox"');
    expect(markup).toContain('aria-expanded="false"');
    expect(markup).toContain('aria-autocomplete="list"');
    expect(markup).toContain('placeholder="connection.profile_ungrouped"');
    expect(markup).not.toContain("required");
  });
});

describe("profile duplication controls", () => {
  function renderSelector(overrides: Partial<Parameters<typeof ProfileSelector>[0]> = {}) {
    return renderToStaticMarkup(
      createElement(ProfileSelector, {
        selectedProfileId: "router",
        selectedHistoryId: "",
        profiles: [{ id: "router", connection_type: "ssh" }],
        historyEntries: [],
        getDisplayName: (profile) => profile.id,
        getHistoryDisplayName: (entry) => entry.id,
        onSelectSource: vi.fn(),
        onDeleteProfile: vi.fn(),
        onDeleteHistory: vi.fn(),
        onCopyProfile: vi.fn(),
        ...overrides,
      })
    );
  }

  it("offers duplication only for a saved profile, alongside deletion", () => {
    expect(renderSelector()).toContain("connection.profile_copy");
    expect(renderSelector()).toContain("connection.profile_delete");
    expect(renderSelector({ selectedProfileId: "" })).not.toContain("connection.profile_copy");
    expect(renderSelector({ selectedHistoryId: "recent", selectedProfileId: "" })).not.toContain(
      "connection.profile_copy"
    );
  });

  it("disables duplication when a selected saved profile is unavailable", () => {
    expect(renderSelector({ profiles: [] })).toContain('disabled=""');
  });
});
