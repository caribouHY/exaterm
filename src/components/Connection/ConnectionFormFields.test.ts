import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { ProfileGroupField, ProfileSelector } from "./ConnectionFormFields";

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

describe("saved profile group fields", () => {
  it("renders groups as headings while keeping profile source IDs unchanged", () => {
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
      })
    );
    expect(markup).toContain('label="connection.profile_saved / Site &lt;A&gt;"');
    expect(markup).toContain('label="connection.profile_saved / connection.profile_ungrouped"');
    expect(markup).toContain('value="profile:router" selected=""');
    expect(markup).toContain('value=""');
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
    expect(markup).not.toContain("<datalist");
    expect(markup).toContain('placeholder="connection.profile_ungrouped"');
    expect(markup).not.toContain("required");
  });
});
