import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { AppTabInfo, TabInfo } from "../../types";
import TerminalTabs from "./TerminalTabs";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { name: string }) => (options ? `${key} ${options.name}` : key),
  }),
}));

const connected: TabInfo = {
  kind: "terminal",
  id: "connected",
  title: "接続先の長い名前 Lab Terminal",
  connectionType: "ssh",
  sessionId: "sample-session",
  isConnected: true,
  encoding: "utf-8",
  terminalMode: "general",
};

const disconnected: TabInfo = {
  ...connected,
  id: "disconnected",
  title: "Offline terminal",
  isConnected: false,
};

const settingsTab = { kind: "settings", id: "settings" } satisfies AppTabInfo;
const logsTab = { kind: "logs", id: "logs" } satisfies AppTabInfo;
const tabs = [connected, disconnected, settingsTab, logsTab];

function render(overrides: Partial<ComponentProps<typeof TerminalTabs>> = {}) {
  return renderToStaticMarkup(
    createElement(TerminalTabs, {
      tabs,
      activeTabId: connected.id,
      closingTabIds: [],
      isClosingDisconnectedTabs: false,
      onSelectTab: vi.fn(),
      onCloseTab: vi.fn(),
      onCloseDisconnectedTabs: vi.fn(),
      onMoveTabToNewWindow: vi.fn(),
      onOpenSameDestination: vi.fn(),
      onAddTab: vi.fn(),
      onReorderTabs: vi.fn(),
      windowId: "window",
      dragPreview: null,
      onCrossWindowDragStart: vi.fn(),
      onCrossWindowDragUpdate: vi.fn(),
      onCrossWindowDragDrop: vi.fn(),
      onCrossWindowDragCancel: vi.fn(),
      onCrossWindowDragHover: vi.fn(),
      ...overrides,
    })
  );
}

describe("terminal tab accessibility", () => {
  it("associates both terminal states with their selection controls", () => {
    const markup = render();
    expect(markup.match(/aria-current="true"/g)).toHaveLength(1);

    for (const [tab, status] of [
      [connected, "connected"],
      [disconnected, "disconnected"],
    ] as const) {
      const card = render({ tabs: [tab] });
      const descriptionId = card.match(/aria-describedby="([^"]+)"/)?.[1];
      expect(descriptionId).toBeDefined();
      expect(card).toContain(`id="${descriptionId}" hidden="">statusbar.${status}</span>`);
      expect(card).toContain(`terminal-tab__status--${status}" aria-hidden="true"`);
      expect(card).toContain('aria-keyshortcuts="Shift+F10 ContextMenu"');
      expect(card.includes('aria-current="true"')).toBe(tab.isConnected);
    }
  });

  it("places the named close control after the selection button and disables both while closing", () => {
    const card = render({ tabs: [connected], closingTabIds: [connected.id] });
    const [select, close] = card.match(/<button\b[^>]*>[\s\S]*?<\/button>/g) ?? [];
    expect(select).toContain('class="terminal-tab__select"');
    expect(select).toContain(connected.title);
    expect(select).toContain('aria-busy="true"');
    expect(select).toContain('disabled=""');
    expect(select).not.toContain("terminal-tab__close");
    expect(close).toContain('class="terminal-tab__close"');
    expect(close).toContain(`aria-label="terminal.close_named_tab ${connected.title}" disabled=""`);
    expect(render({ tabs: [disconnected], closingTabIds: [connected.id] })).not.toContain(
      'disabled=""'
    );
  });

  it("uses the same current and close controls for utility tabs without a connection state", () => {
    const settings = render({ tabs: [settingsTab], activeTabId: "settings" });
    const logs = render({ tabs: [logsTab], activeTabId: "settings" });
    expect(settings).toContain('aria-current="true"');
    expect(logs).not.toContain("aria-current");
    expect(settings).toContain('aria-label="terminal.close_named_tab settings.title"');
    expect(logs).toContain('aria-label="terminal.close_named_tab logs.title"');
    expect(settings + logs).not.toContain("aria-describedby");
    expect(settings + logs).not.toContain("terminal-tab__status");
  });
});
