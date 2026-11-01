import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { PopoverMenu, type PopoverMenuItem } from "./PopoverMenu";

function render(items: PopoverMenuItem[]) {
  return renderToStaticMarkup(createElement(PopoverMenu, { items, onAction: vi.fn() }));
}

describe("popover menu selection indicators", () => {
  it("distinguishes current choices from ordinary commands and unselected choices", () => {
    const markup = render([
      { key: "current", label: "Current view", active: true, action: vi.fn() },
      { key: "other", label: "Other view", active: false, action: vi.fn() },
      { key: "command", label: "New connection", action: vi.fn() },
      { key: "disabled", label: "Unavailable command", disabled: true, action: vi.fn() },
    ]);
    const buttons = [...markup.matchAll(/<button\b[^>]*>[\s\S]*?<\/button>/g)].map(
      (match) => match[0]
    );

    expect(buttons).toHaveLength(4);
    expect(buttons[0]).toContain('role="menuitemcheckbox"');
    expect(buttons[0]).toContain('aria-checked="true"');
    expect(buttons[0]).toContain("lucide-check");
    expect(buttons[1]).toContain('aria-checked="false"');
    expect(buttons[1]).not.toContain("lucide-check");
    expect(buttons[2]).toContain('role="menuitem"');
    expect(buttons[2]).not.toContain("aria-checked");
    expect(buttons[2]).not.toContain("lucide-check");
    expect(buttons[3]).toContain('disabled=""');
    expect(buttons[3]).not.toContain("lucide-check");
  });

  it("moves the checkmark when the current choice changes while retaining shortcut labels", () => {
    const choices = (current: string): PopoverMenuItem[] =>
      ["settings", "logs"].map((key) => ({
        key,
        label: key,
        active: key === current,
        shortcut: key === "settings" ? "Ctrl+," : undefined,
        action: vi.fn(),
      }));

    for (const current of ["settings", "logs"]) {
      const markup = render(choices(current));
      const checked = markup.match(/<button\b[^>]*aria-checked="true"[^>]*>[\s\S]*?<\/button>/);
      expect(checked?.[0]).toContain(current);
      expect(checked?.[0]).toContain("lucide-check");
      expect(markup.match(/lucide-check/g)).toHaveLength(1);
      expect(markup).toContain("Ctrl+,");
    }
  });
});
