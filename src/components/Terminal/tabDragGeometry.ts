import type { ForeignTabPlacement } from "../../types";

export const DRAG_START_DISTANCE = 4;
export const DROP_SLOT_HYSTERESIS_PX = 14;

const CONTEXT_MENU_WIDTH = 220;
const CONTEXT_MENU_MAX_HEIGHT = 128;
const CONTEXT_MENU_VIEWPORT_MARGIN = 8;

export interface Point {
  x: number;
  y: number;
}

export interface RectSnapshot {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

export interface ViewportSize {
  width: number;
  height: number;
}

export interface MeasuredTab {
  tabId: string;
  isTerminal: boolean;
  left: number;
  right: number;
}

export type TabDropSide = "before" | "after";

export interface TabDropTarget {
  tabId: string;
  side: TabDropSide;
  indicatorLeft: number;
  slotIndex: number;
}

export interface ForeignDropTarget {
  targetIndex: number;
  indicatorLeft: number;
  placement: ForeignTabPlacement;
}

export type DragReleaseDecision =
  | {
      kind: "reorder";
      target: TabDropTarget;
      cancelCrossWindow: boolean;
    }
  | { kind: "cross-window-drop" }
  | { kind: "cancel-cross-window" }
  | { kind: "none" };

interface ResolveLocalDropTargetOptions {
  sourceTabId: string;
  point: Point;
  containerRect: RectSnapshot;
  scrollLeft: number;
  measuredTabs: readonly MeasuredTab[];
  blockedTabIds: readonly string[];
  currentTarget: TabDropTarget | null;
}

interface ResolveForeignDropTargetOptions {
  draggedTabId: string;
  point: Point;
  containerRect: RectSnapshot;
  scrollLeft: number;
  measuredTabs: readonly MeasuredTab[];
  blockedTabIds: readonly string[];
}

interface ResolveDragReleaseOptions {
  active: boolean;
  isTerminalTab: boolean;
  crossWindowStarted: boolean;
  localTarget: TabDropTarget | null;
  point: Point;
  viewport: ViewportSize;
}

function isPointInsideRect(point: Point, rect: RectSnapshot) {
  return (
    point.y >= rect.top && point.y <= rect.bottom && point.x >= rect.left && point.x <= rect.right
  );
}

function centerX(tab: MeasuredTab) {
  return tab.left + (tab.right - tab.left) / 2;
}

function availableTabs(
  measuredTabs: readonly MeasuredTab[],
  sourceTabId: string,
  blockedTabIds: readonly string[]
) {
  const blocked = new Set(blockedTabIds);
  return measuredTabs.filter((tab) => tab.tabId !== sourceTabId && !blocked.has(tab.tabId));
}

function findSlotIndex(tabs: readonly MeasuredTab[], pointerX: number) {
  const slotIndex = tabs.findIndex((tab) => pointerX < centerX(tab));
  return slotIndex === -1 ? tabs.length : slotIndex;
}

function tabAt(tabs: readonly MeasuredTab[], targetIndex: number) {
  return tabs.find((_, index) => index === targetIndex);
}

export function hasReachedDragThreshold(start: Point, current: Point) {
  return Math.hypot(current.x - start.x, current.y - start.y) >= DRAG_START_DISTANCE;
}

export function resolveLocalDropTarget({
  sourceTabId,
  point,
  containerRect,
  scrollLeft,
  measuredTabs,
  blockedTabIds,
  currentTarget,
}: ResolveLocalDropTargetOptions): TabDropTarget | null {
  if (!isPointInsideRect(point, containerRect)) return null;

  const candidates = availableTabs(measuredTabs, sourceTabId, blockedTabIds);
  if (candidates.length === 0) return null;

  const slotIndex = findSlotIndex(candidates, point.x);
  if (currentTarget) {
    if (slotIndex > currentTarget.slotIndex) {
      const boundary = tabAt(candidates, currentTarget.slotIndex);
      if (boundary && point.x < centerX(boundary) + DROP_SLOT_HYSTERESIS_PX) {
        return currentTarget;
      }
    } else if (slotIndex < currentTarget.slotIndex) {
      const boundary = tabAt(candidates, currentTarget.slotIndex - 1);
      if (boundary && point.x > centerX(boundary) - DROP_SLOT_HYSTERESIS_PX) {
        return currentTarget;
      }
    }
  }

  const slotCandidate = tabAt(candidates, slotIndex);
  const lastCandidate = tabAt(candidates, candidates.length - 1);
  if (!lastCandidate) return null;
  const target = slotCandidate
    ? {
        tabId: slotCandidate.tabId,
        side: "before" as const,
        edgeX: slotCandidate.left,
      }
    : {
        tabId: lastCandidate.tabId,
        side: "after" as const,
        edgeX: lastCandidate.right,
      };

  return {
    tabId: target.tabId,
    side: target.side,
    indicatorLeft: target.edgeX - containerRect.left + scrollLeft,
    slotIndex,
  };
}

export function resolveForeignDropTarget({
  draggedTabId,
  point,
  containerRect,
  scrollLeft,
  measuredTabs,
  blockedTabIds,
}: ResolveForeignDropTargetOptions): ForeignDropTarget | null {
  if (!isPointInsideRect(point, containerRect)) return null;

  const candidates = availableTabs(measuredTabs, draggedTabId, blockedTabIds);
  if (candidates.length === 0) {
    return {
      targetIndex: 0,
      indicatorLeft: point.x - containerRect.left + scrollLeft,
      placement: {
        tabId: draggedTabId,
        previousTabId: null,
        nextTabId: null,
        visibleSlotIndex: 0,
      },
    };
  }

  const visibleSlotIndex = findSlotIndex(candidates, point.x);
  const slotCandidate = tabAt(candidates, visibleSlotIndex);
  const lastCandidate = tabAt(candidates, candidates.length - 1);
  if (!lastCandidate) return null;
  const edgeX = slotCandidate?.left ?? lastCandidate.right;
  const targetIndex = candidates
    .slice(0, visibleSlotIndex)
    .filter((candidate) => candidate.isTerminal).length;

  return {
    targetIndex,
    indicatorLeft: edgeX - containerRect.left + scrollLeft,
    placement: {
      tabId: draggedTabId,
      previousTabId: tabAt(candidates, visibleSlotIndex - 1)?.tabId ?? null,
      nextTabId: slotCandidate?.tabId ?? null,
      visibleSlotIndex,
    },
  };
}

export function isValidDropTarget(
  sourceTabId: string,
  target: TabDropTarget | null,
  availableTabIds: readonly string[],
  blockedTabIds: readonly string[]
) {
  return Boolean(
    target &&
    target.tabId !== sourceTabId &&
    availableTabIds.includes(target.tabId) &&
    !blockedTabIds.includes(target.tabId)
  );
}

export function isPointOutsideViewport(point: Point, viewport: ViewportSize) {
  return point.x < 0 || point.x > viewport.width || point.y < 0 || point.y > viewport.height;
}

export function resolveDragRelease({
  active,
  isTerminalTab,
  crossWindowStarted,
  localTarget,
  point,
  viewport,
}: ResolveDragReleaseOptions): DragReleaseDecision {
  if (!active) return { kind: "none" };
  if (localTarget) {
    return {
      kind: "reorder",
      target: localTarget,
      cancelCrossWindow: crossWindowStarted,
    };
  }
  if (isTerminalTab && isPointOutsideViewport(point, viewport)) {
    return { kind: "cross-window-drop" };
  }
  if (crossWindowStarted) return { kind: "cancel-cross-window" };
  return { kind: "none" };
}

export function screenToClientPoint(screenPoint: Point, windowOrigin: Point): Point {
  return {
    x: screenPoint.x - windowOrigin.x,
    y: screenPoint.y - windowOrigin.y,
  };
}

export function clampContextMenuPosition(point: Point, viewport: ViewportSize): Point {
  return {
    x: Math.max(
      CONTEXT_MENU_VIEWPORT_MARGIN,
      Math.min(point.x, viewport.width - CONTEXT_MENU_WIDTH - CONTEXT_MENU_VIEWPORT_MARGIN)
    ),
    y: Math.max(
      CONTEXT_MENU_VIEWPORT_MARGIN,
      Math.min(point.y, viewport.height - CONTEXT_MENU_MAX_HEIGHT - CONTEXT_MENU_VIEWPORT_MARGIN)
    ),
  };
}
