import { describe, expect, it, vi } from "vitest";
import {
  createExternalSessionFocusController,
  type SessionFocusRequest,
} from "./externalSessionFocus";
import type { TabInfo } from "../../types";

const tab: TabInfo = {
  kind: "terminal",
  id: "tab",
  sessionId: "session",
  title: "Terminal",
  connectionType: "ssh",
  isConnected: false,
  encoding: "utf-8",
  terminalMode: "general",
};
const request: SessionFocusRequest = {
  request_id: "request",
  session_id: "session",
  tab_id: "tab",
  snapshot: {
    revision: 1,
    window_id: "main",
    window: {
      window_id: "main",
      label: "main",
      tab_order: ["tab"],
      active_tab_id: "tab",
    },
    tabs: [],
  },
};

function harness(accepted = true) {
  const dependencies = {
    select: vi.fn(() => accepted),
    submit: vi.fn(async () => {}),
    focusTerminal: vi.fn(),
    onError: vi.fn(),
  };
  return { ...dependencies, controller: createExternalSessionFocusController(dependencies) };
}

describe("external session focus", () => {
  it.each(["other-tab", "settings", "logs", "tab"])(
    "waits for a commit when selecting from %s",
    (previousTab) => {
      const h = harness();
      h.controller.afterCommit({ activeTabId: previousTab, tabs: [tab], closingTabIds: [] }, 0);
      h.controller.receive(request, 1);
      expect(h.submit).not.toHaveBeenCalled();
      h.controller.afterCommit({ activeTabId: previousTab, tabs: [tab], closingTabIds: [] }, 0);
      expect(h.submit).not.toHaveBeenCalled();
      h.controller.afterCommit({ activeTabId: "tab", tabs: [tab], closingTabIds: [] }, 1);
      expect(h.submit).toHaveBeenCalledExactlyOnceWith({
        requestId: "request",
        tabId: "tab",
        selected: true,
      });
      expect(h.focusTerminal).toHaveBeenCalledWith("tab");
      h.controller.afterCommit({ activeTabId: "tab", tabs: [tab], closingTabIds: [] }, 1);
      expect(h.submit).toHaveBeenCalledTimes(1);
    }
  );

  it.each([
    { activeTabId: "other", tabs: [tab], closingTabIds: [] },
    { activeTabId: "tab", tabs: [], closingTabIds: [] },
    { activeTabId: "tab", tabs: [tab], closingTabIds: ["tab"] },
    { activeTabId: "tab", tabs: [{ ...tab, sessionId: "different" }], closingTabIds: [] },
  ])("rejects a changed selection or removed/moved/closing tab", (state) => {
    const h = harness();
    h.controller.receive(request, 1);
    h.controller.afterCommit(state, 1);
    expect(h.submit).toHaveBeenCalledWith({ requestId: "request", tabId: "tab", selected: false });
    expect(h.focusTerminal).not.toHaveBeenCalled();
  });

  it("rejects selection failure and releases pending requests on disposal", () => {
    const h = harness(false);
    h.controller.receive(request, 1);
    h.controller.afterCommit({ activeTabId: "tab", tabs: [tab], closingTabIds: [] }, 1);
    expect(h.submit).toHaveBeenCalledWith({ requestId: "request", tabId: "tab", selected: false });
    h.controller.receive({ ...request, request_id: "second" }, 2);
    h.controller.dispose();
    h.controller.receive({ ...request, request_id: "third" }, 3);
    expect(h.submit).toHaveBeenCalledTimes(2);
    expect(h.select).toHaveBeenCalledTimes(2);
  });
});
