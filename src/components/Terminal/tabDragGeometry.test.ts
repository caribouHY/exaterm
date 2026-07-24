import { describe, expect, it } from "vitest";
import {
  clampContextMenuPosition,
  hasReachedDragThreshold,
  isPointOutsideViewport,
  isValidDropTarget,
  resolveDragRelease,
  resolveForeignDropTarget,
  resolveLocalDropTarget,
  screenToClientPoint,
  type MeasuredTab,
  type RectSnapshot,
  type TabDropTarget,
} from "./tabDragGeometry";

const containerRect: RectSnapshot = {
  left: 100,
  right: 500,
  top: 20,
  bottom: 54,
};

const measuredTabs: MeasuredTab[] = [
  { tabId: "terminal-a", isTerminal: true, left: 100, right: 200 },
  { tabId: "settings", isTerminal: false, left: 200, right: 300 },
  { tabId: "terminal-b", isTerminal: true, left: 300, right: 400 },
];

function localTarget(
  pointX: number,
  currentTarget: TabDropTarget | null = null,
  options: {
    sourceTabId?: string;
    pointY?: number;
    scrollLeft?: number;
    blockedTabIds?: string[];
    tabs?: MeasuredTab[];
  } = {}
) {
  return resolveLocalDropTarget({
    sourceTabId: options.sourceTabId ?? "terminal-a",
    point: { x: pointX, y: options.pointY ?? 30 },
    containerRect,
    scrollLeft: options.scrollLeft ?? 0,
    measuredTabs: options.tabs ?? measuredTabs,
    blockedTabIds: options.blockedTabIds ?? [],
    currentTarget,
  });
}

describe("drag threshold", () => {
  it("starts at four pixels but not immediately before it", () => {
    expect(hasReachedDragThreshold({ x: 0, y: 0 }, { x: 3.99, y: 0 })).toBe(false);
    expect(hasReachedDragThreshold({ x: 0, y: 0 }, { x: 4, y: 0 })).toBe(true);
    expect(hasReachedDragThreshold({ x: 0, y: 0 }, { x: 3, y: 4 })).toBe(true);
  });
});

describe("local drop geometry", () => {
  it("returns null outside the tab strip", () => {
    expect(localTarget(250, null, { pointY: 10 })).toBeNull();
    expect(localTarget(501)).toBeNull();
  });

  it("excludes the source and closing tabs", () => {
    expect(localTarget(150, null, { blockedTabIds: ["settings"] })).toEqual({
      tabId: "terminal-b",
      side: "before",
      indicatorLeft: 200,
      slotIndex: 0,
    });
  });

  it("switches slots at candidate centers", () => {
    expect(localTarget(249)?.tabId).toBe("settings");
    expect(localTarget(249)?.side).toBe("before");
    expect(localTarget(250)?.tabId).toBe("terminal-b");
    expect(localTarget(350)?.side).toBe("after");
  });

  it("keeps the current slot inside the rightward hysteresis range", () => {
    const current = localTarget(249);
    expect(current).not.toBeNull();
    expect(localTarget(260, current)).toBe(current);
    expect(localTarget(264, current)?.tabId).toBe("terminal-b");
  });

  it("keeps the current slot inside the leftward hysteresis range", () => {
    const current = localTarget(264);
    expect(current).not.toBeNull();
    expect(localTarget(240, current)).toBe(current);
    expect(localTarget(236, current)?.tabId).toBe("settings");
  });

  it("includes scroll offset in the indicator position", () => {
    expect(localTarget(249, null, { scrollLeft: 80 })?.indicatorLeft).toBe(180);
  });

  it("returns null when no target remains", () => {
    expect(
      localTarget(150, null, {
        tabs: [{ tabId: "terminal-a", isTerminal: true, left: 100, right: 200 }],
      })
    ).toBeNull();
  });
});

describe("foreign drop geometry", () => {
  it("uses target index zero and the pointer indicator for an empty destination", () => {
    expect(
      resolveForeignDropTarget({
        draggedTabId: "foreign",
        point: { x: 180, y: 30 },
        containerRect,
        scrollLeft: 25,
        measuredTabs: [],
        blockedTabIds: [],
      })
    ).toEqual({
      targetIndex: 0,
      indicatorLeft: 105,
      placement: {
        tabId: "foreign",
        previousTabId: null,
        nextTabId: null,
        visibleSlotIndex: 0,
      },
    });
  });

  it("counts only terminal tabs before a mixed visible slot", () => {
    expect(
      resolveForeignDropTarget({
        draggedTabId: "foreign",
        point: { x: 275, y: 30 },
        containerRect,
        scrollLeft: 0,
        measuredTabs,
        blockedTabIds: [],
      })
    ).toEqual({
      targetIndex: 1,
      indicatorLeft: 200,
      placement: {
        tabId: "foreign",
        previousTabId: "settings",
        nextTabId: "terminal-b",
        visibleSlotIndex: 2,
      },
    });
  });

  it("excludes a matching source and blocked tabs", () => {
    const target = resolveForeignDropTarget({
      draggedTabId: "terminal-a",
      point: { x: 150, y: 30 },
      containerRect,
      scrollLeft: 0,
      measuredTabs,
      blockedTabIds: ["settings"],
    });
    expect(target?.placement.nextTabId).toBe("terminal-b");
    expect(target?.targetIndex).toBe(0);
  });

  it("returns null outside the destination strip", () => {
    expect(
      resolveForeignDropTarget({
        draggedTabId: "foreign",
        point: { x: 99, y: 30 },
        containerRect,
        scrollLeft: 0,
        measuredTabs,
        blockedTabIds: [],
      })
    ).toBeNull();
  });
});

describe("drop validation and release", () => {
  const target: TabDropTarget = {
    tabId: "terminal-b",
    side: "before",
    indicatorLeft: 200,
    slotIndex: 1,
  };

  it("rejects missing, source, or blocked targets", () => {
    expect(isValidDropTarget("terminal-a", target, ["terminal-b"], [])).toBe(true);
    expect(isValidDropTarget("terminal-a", target, [], [])).toBe(false);
    expect(isValidDropTarget("terminal-a", target, ["terminal-b"], ["terminal-b"])).toBe(false);
    expect(isValidDropTarget("terminal-b", target, ["terminal-b"], [])).toBe(false);
  });

  it("prioritizes local reorder and cancels an active cross-window drag", () => {
    expect(
      resolveDragRelease({
        active: true,
        isTerminalTab: true,
        crossWindowStarted: true,
        localTarget: target,
        point: { x: -1, y: 30 },
        viewport: { width: 800, height: 600 },
      })
    ).toEqual({ kind: "reorder", target, cancelCrossWindow: true });
  });

  it("detaches terminal tabs outside the viewport and otherwise cancels", () => {
    expect(
      resolveDragRelease({
        active: true,
        isTerminalTab: true,
        crossWindowStarted: true,
        localTarget: null,
        point: { x: 801, y: 30 },
        viewport: { width: 800, height: 600 },
      })
    ).toEqual({ kind: "cross-window-drop" });
    expect(
      resolveDragRelease({
        active: true,
        isTerminalTab: true,
        crossWindowStarted: true,
        localTarget: null,
        point: { x: 400, y: 30 },
        viewport: { width: 800, height: 600 },
      })
    ).toEqual({ kind: "cancel-cross-window" });
  });

  it("does nothing before drag activation or for an unmatched utility drag", () => {
    expect(
      resolveDragRelease({
        active: false,
        isTerminalTab: true,
        crossWindowStarted: false,
        localTarget: null,
        point: { x: 400, y: 30 },
        viewport: { width: 800, height: 600 },
      })
    ).toEqual({ kind: "none" });
    expect(
      resolveDragRelease({
        active: true,
        isTerminalTab: false,
        crossWindowStarted: false,
        localTarget: null,
        point: { x: -1, y: 30 },
        viewport: { width: 800, height: 600 },
      })
    ).toEqual({ kind: "none" });
  });
});

describe("viewport and overlay geometry", () => {
  it("treats viewport edges as inside", () => {
    expect(isPointOutsideViewport({ x: 0, y: 0 }, { width: 800, height: 600 })).toBe(false);
    expect(isPointOutsideViewport({ x: 800, y: 600 }, { width: 800, height: 600 })).toBe(false);
    expect(isPointOutsideViewport({ x: -1, y: 0 }, { width: 800, height: 600 })).toBe(true);
  });

  it("converts screen coordinates to client coordinates", () => {
    expect(screenToClientPoint({ x: 450, y: 280 }, { x: 120, y: 40 })).toEqual({
      x: 330,
      y: 240,
    });
  });

  it("clamps context menus to the existing viewport margins", () => {
    expect(clampContextMenuPosition({ x: 400, y: 250 }, { width: 800, height: 600 })).toEqual({
      x: 400,
      y: 250,
    });
    expect(clampContextMenuPosition({ x: 790, y: 590 }, { width: 800, height: 600 })).toEqual({
      x: 572,
      y: 464,
    });
    expect(clampContextMenuPosition({ x: -20, y: -10 }, { width: 800, height: 600 })).toEqual({
      x: 8,
      y: 8,
    });
  });
});
