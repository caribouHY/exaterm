import { useCallback, useEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent, RefObject } from "react";
import type {
  AppTabInfo,
  ForeignTabPlacement,
  WorkspaceDragPreview,
  WorkspacePointerPosition,
} from "../../types";
import {
  hasReachedDragThreshold,
  isValidDropTarget,
  resolveDragRelease,
  resolveForeignDropTarget,
  resolveLocalDropTarget,
  screenToClientPoint,
  type ForeignDropTarget,
  type MeasuredTab,
  type Point,
  type RectSnapshot,
  type TabDropSide,
  type TabDropTarget,
} from "./tabDragGeometry";

interface PointerDragState {
  tabId: string;
  pointerId: number;
  start: Point;
  active: boolean;
  isTerminalTab: boolean;
  crossWindowStarted: boolean;
}

interface UseTabPointerDragOptions {
  containerRef: RefObject<HTMLDivElement | null>;
  tabs: AppTabInfo[];
  closingTabIds: string[];
  windowId: string;
  dragPreview: WorkspaceDragPreview | null;
  onReorderTabs: (draggedId: string, targetId: string, dropSide: TabDropSide) => void;
  onCrossWindowDragStart: (tabId: string, pointerScreenPosition: WorkspacePointerPosition) => void;
  onCrossWindowDragUpdate: (pointerScreenPosition: WorkspacePointerPosition) => void;
  onCrossWindowDragDrop: (tabId: string, pointerScreenPosition: WorkspacePointerPosition) => void;
  onCrossWindowDragCancel: () => void;
  onCrossWindowDragHover: (
    targetIndex: number | null,
    placement: ForeignTabPlacement | null
  ) => void;
}

function rectSnapshot(rect: DOMRect): RectSnapshot {
  return {
    left: rect.left,
    right: rect.right,
    top: rect.top,
    bottom: rect.bottom,
  };
}

function measureTabs(container: HTMLDivElement): MeasuredTab[] {
  return Array.from(container.querySelectorAll<HTMLElement>("[data-terminal-tab-id]"))
    .map((element) => {
      const tabId = element.dataset.terminalTabId;
      if (!tabId) return null;

      const rect = element.getBoundingClientRect();
      return {
        tabId,
        isTerminal: Boolean(element.dataset.workspaceTerminalTabId),
        left: rect.left,
        right: rect.right,
      };
    })
    .filter((tab): tab is MeasuredTab => Boolean(tab));
}

function pointerScreenPosition(
  event: ReactPointerEvent<HTMLButtonElement>
): WorkspacePointerPosition {
  return {
    x: event.screenX,
    y: event.screenY,
  };
}

export function useTabPointerDrag({
  containerRef,
  tabs,
  closingTabIds,
  windowId,
  dragPreview,
  onReorderTabs,
  onCrossWindowDragStart,
  onCrossWindowDragUpdate,
  onCrossWindowDragDrop,
  onCrossWindowDragCancel,
  onCrossWindowDragHover,
}: UseTabPointerDragOptions) {
  const pointerDragStateRef = useRef<PointerDragState | null>(null);
  const dropTargetRef = useRef<TabDropTarget | null>(null);
  const foreignHoverSignatureRef = useRef<string | null>(null);
  const suppressNextClickRef = useRef(false);
  const suppressClickTimerRef = useRef<number | null>(null);
  const [draggedTabId, setDraggedTabId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<TabDropTarget | null>(null);
  const [foreignDropTarget, setForeignDropTarget] = useState<ForeignDropTarget | null>(null);

  const updateDropTarget = useCallback((target: TabDropTarget | null) => {
    dropTargetRef.current = target;
    setDropTarget(target);
  }, []);

  const clearForeignHover = useCallback(() => {
    setForeignDropTarget(null);
    foreignHoverSignatureRef.current = null;
  }, []);

  const resetPointerDrag = useCallback(() => {
    pointerDragStateRef.current = null;
    setDraggedTabId(null);
    updateDropTarget(null);
  }, [updateDropTarget]);

  const getLocalDropTarget = useCallback(
    (
      sourceTabId: string,
      point: Point,
      currentTarget: TabDropTarget | null
    ): TabDropTarget | null => {
      const container = containerRef.current;
      if (!container) return null;

      return resolveLocalDropTarget({
        sourceTabId,
        point,
        containerRect: rectSnapshot(container.getBoundingClientRect()),
        scrollLeft: container.scrollLeft,
        measuredTabs: measureTabs(container),
        blockedTabIds: closingTabIds,
        currentTarget,
      });
    },
    [closingTabIds, containerRef]
  );

  const getForeignDropTarget = useCallback(
    (draggedTabId: string, screenPoint: Point): ForeignDropTarget | null => {
      const container = containerRef.current;
      if (!container) return null;

      return resolveForeignDropTarget({
        draggedTabId,
        point: screenToClientPoint(screenPoint, { x: window.screenX, y: window.screenY }),
        containerRect: rectSnapshot(container.getBoundingClientRect()),
        scrollLeft: container.scrollLeft,
        measuredTabs: measureTabs(container),
        blockedTabIds: closingTabIds,
      });
    },
    [closingTabIds, containerRef]
  );

  useEffect(() => {
    if (
      !dragPreview?.active ||
      !dragPreview.pointer_screen_position ||
      !dragPreview.tab_id ||
      dragPreview.source_window_id === windowId
    ) {
      clearForeignHover();
      return;
    }

    const target = getForeignDropTarget(dragPreview.tab_id, dragPreview.pointer_screen_position);
    setForeignDropTarget(target);

    const nextSignature = target
      ? [
          target.placement.tabId,
          target.targetIndex,
          target.placement.visibleSlotIndex,
          target.placement.previousTabId,
          target.placement.nextTabId,
        ].join("\u0000")
      : null;
    if (foreignHoverSignatureRef.current !== nextSignature) {
      foreignHoverSignatureRef.current = nextSignature;
      onCrossWindowDragHover(target?.targetIndex ?? null, target?.placement ?? null);
    }
  }, [
    clearForeignHover,
    dragPreview,
    getForeignDropTarget,
    onCrossWindowDragHover,
    tabs,
    windowId,
  ]);

  useEffect(
    () => () => {
      if (suppressClickTimerRef.current !== null) {
        window.clearTimeout(suppressClickTimerRef.current);
      }
    },
    []
  );

  const beginPointerDrag = useCallback(
    (event: ReactPointerEvent<HTMLButtonElement>, tabId: string, isTerminalTab: boolean) => {
      pointerDragStateRef.current = {
        tabId,
        pointerId: event.pointerId,
        start: { x: event.clientX, y: event.clientY },
        active: false,
        isTerminalTab,
        crossWindowStarted: false,
      };
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    []
  );

  const handlePointerMove = useCallback(
    (event: ReactPointerEvent<HTMLButtonElement>) => {
      const dragState = pointerDragStateRef.current;
      if (!dragState || dragState.pointerId !== event.pointerId) return;

      const point = { x: event.clientX, y: event.clientY };
      if (!dragState.active) {
        if (!hasReachedDragThreshold(dragState.start, point)) return;
        dragState.active = true;
        setDraggedTabId(dragState.tabId);
        if (dragState.isTerminalTab) {
          dragState.crossWindowStarted = true;
          onCrossWindowDragStart(dragState.tabId, pointerScreenPosition(event));
        }
      }

      event.preventDefault();
      if (dragState.isTerminalTab) {
        onCrossWindowDragUpdate(pointerScreenPosition(event));
      }
      const nextTarget = getLocalDropTarget(dragState.tabId, point, dropTargetRef.current);
      const isValid = isValidDropTarget(
        dragState.tabId,
        nextTarget,
        tabs.map((tab) => tab.id),
        closingTabIds
      );
      updateDropTarget(isValid ? nextTarget : null);
    },
    [
      closingTabIds,
      getLocalDropTarget,
      onCrossWindowDragStart,
      onCrossWindowDragUpdate,
      tabs,
      updateDropTarget,
    ]
  );

  const handlePointerUp = useCallback(
    (event: ReactPointerEvent<HTMLButtonElement>) => {
      const dragState = pointerDragStateRef.current;
      if (!dragState || dragState.pointerId !== event.pointerId) return;

      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }

      if (dragState.active) {
        event.preventDefault();
        event.stopPropagation();
        const point = { x: event.clientX, y: event.clientY };
        const measuredTarget =
          getLocalDropTarget(dragState.tabId, point, dropTargetRef.current) ??
          dropTargetRef.current;
        const localTarget = isValidDropTarget(
          dragState.tabId,
          measuredTarget,
          tabs.map((tab) => tab.id),
          closingTabIds
        )
          ? measuredTarget
          : null;
        const decision = resolveDragRelease({
          active: dragState.active,
          isTerminalTab: dragState.isTerminalTab,
          crossWindowStarted: dragState.crossWindowStarted,
          localTarget,
          point,
          viewport: { width: window.innerWidth, height: window.innerHeight },
        });

        switch (decision.kind) {
          case "reorder":
            onReorderTabs(dragState.tabId, decision.target.tabId, decision.target.side);
            if (decision.cancelCrossWindow) {
              onCrossWindowDragCancel();
            }
            break;
          case "cross-window-drop":
            onCrossWindowDragDrop(dragState.tabId, pointerScreenPosition(event));
            break;
          case "cancel-cross-window":
            onCrossWindowDragCancel();
            break;
          case "none":
            break;
        }

        suppressNextClickRef.current = true;
        if (suppressClickTimerRef.current !== null) {
          window.clearTimeout(suppressClickTimerRef.current);
        }
        suppressClickTimerRef.current = window.setTimeout(() => {
          suppressNextClickRef.current = false;
          suppressClickTimerRef.current = null;
        }, 0);
      }

      resetPointerDrag();
    },
    [
      closingTabIds,
      getLocalDropTarget,
      onCrossWindowDragCancel,
      onCrossWindowDragDrop,
      onReorderTabs,
      resetPointerDrag,
      tabs,
    ]
  );

  const handlePointerCancel = useCallback(
    (event: ReactPointerEvent<HTMLButtonElement>) => {
      const dragState = pointerDragStateRef.current;
      if (dragState?.pointerId === event.pointerId && dragState.crossWindowStarted) {
        onCrossWindowDragCancel();
      }
      resetPointerDrag();
    },
    [onCrossWindowDragCancel, resetPointerDrag]
  );

  const consumeSuppressedClick = useCallback(() => suppressNextClickRef.current, []);

  return {
    draggedTabId,
    dropTarget,
    foreignDropTarget,
    beginPointerDrag,
    handlePointerMove,
    handlePointerUp,
    handlePointerCancel,
    resetPointerDrag,
    consumeSuppressedClick,
  };
}
