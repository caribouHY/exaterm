import { FileText, Monitor, Network, Settings, Usb, Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { PointerEvent } from "react";
import { useTranslation } from "react-i18next";
import type {
  AppTabInfo,
  ConnectionType,
  WorkspaceDragPreview,
  WorkspacePointerPosition,
} from "../../types";
import "./TerminalTabs.css";

interface TerminalTabsProps {
  tabs: AppTabInfo[];
  activeTabId: string | null;
  closingTabIds: string[];
  onSelectTab: (id: string) => void;
  onCloseTab: (id: string) => Promise<void>;
  onAddTab: () => void;
  onReorderTabs: (draggedId: string, targetId: string, dropSide: TabDropSide) => void;
  windowId: string;
  dragPreview: WorkspaceDragPreview | null;
  onCrossWindowDragStart: (tabId: string, pointerScreenPosition: WorkspacePointerPosition) => void;
  onCrossWindowDragUpdate: (pointerScreenPosition: WorkspacePointerPosition) => void;
  onCrossWindowDragDrop: (tabId: string, pointerScreenPosition: WorkspacePointerPosition) => void;
  onCrossWindowDragCancel: () => void;
  onCrossWindowDragHover: (targetIndex: number | null) => void;
}

type TabDropSide = "before" | "after";

interface TabDropTarget {
  tabId: string;
  side: TabDropSide;
  indicatorLeft: number;
  slotIndex: number;
}

interface PointerDragState {
  tabId: string;
  pointerId: number;
  startX: number;
  startY: number;
  active: boolean;
  isTerminalTab: boolean;
  crossWindowStarted: boolean;
}

interface ForeignDropTarget {
  targetIndex: number;
  indicatorLeft: number;
}

const DRAG_START_DISTANCE = 4;
const DROP_SLOT_HYSTERESIS_PX = 14;

export default function TerminalTabs({
  tabs,
  activeTabId,
  closingTabIds,
  onSelectTab,
  onCloseTab,
  onAddTab,
  onReorderTabs,
  windowId,
  dragPreview,
  onCrossWindowDragStart,
  onCrossWindowDragUpdate,
  onCrossWindowDragDrop,
  onCrossWindowDragCancel,
  onCrossWindowDragHover,
}: TerminalTabsProps) {
  const { t } = useTranslation();
  const tabsRef = useRef<HTMLDivElement | null>(null);
  const pointerDragState = useRef<PointerDragState | null>(null);
  const dropTargetRef = useRef<TabDropTarget | null>(null);
  const foreignHoverIndexRef = useRef<number | null>(null);
  const suppressNextClick = useRef(false);
  const [draggedTabId, setDraggedTabId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<TabDropTarget | null>(null);
  const [foreignDropTarget, setForeignDropTarget] = useState<ForeignDropTarget | null>(null);

  const updateDropTarget = (target: TabDropTarget | null) => {
    dropTargetRef.current = target;
    setDropTarget(target);
  };

  const getDropTargetAtPoint = (
    sourceTabId: string,
    clientX: number,
    clientY: number,
    currentTarget: TabDropTarget | null
  ): TabDropTarget | null => {
    const container = tabsRef.current;
    if (!container) return null;

    const containerRect = container.getBoundingClientRect();
    if (
      clientY < containerRect.top ||
      clientY > containerRect.bottom ||
      clientX < containerRect.left ||
      clientX > containerRect.right
    ) {
      return null;
    }

    const candidates = Array.from(container.querySelectorAll<HTMLElement>("[data-terminal-tab-id]"))
      .map((element) => {
        const tabId = element.dataset.terminalTabId;
        if (!tabId || tabId === sourceTabId || closingTabIds.includes(tabId)) return null;

        const rect = element.getBoundingClientRect();
        return {
          tabId,
          centerX: rect.left + rect.width / 2,
          left: rect.left,
          right: rect.right,
        };
      })
      .filter((item): item is NonNullable<typeof item> => Boolean(item));

    if (candidates.length === 0) return null;

    let slotIndex = candidates.findIndex((candidate) => clientX < candidate.centerX);
    if (slotIndex === -1) {
      slotIndex = candidates.length;
    }

    if (currentTarget) {
      if (slotIndex > currentTarget.slotIndex) {
        const boundary = candidates[currentTarget.slotIndex]?.centerX;
        if (boundary !== undefined && clientX < boundary + DROP_SLOT_HYSTERESIS_PX) {
          return currentTarget;
        }
      } else if (slotIndex < currentTarget.slotIndex) {
        const boundary = candidates[currentTarget.slotIndex - 1]?.centerX;
        if (boundary !== undefined && clientX > boundary - DROP_SLOT_HYSTERESIS_PX) {
          return currentTarget;
        }
      }
    }

    const target =
      slotIndex < candidates.length
        ? {
            tabId: candidates[slotIndex].tabId,
            side: "before" as const,
            edgeX: candidates[slotIndex].left,
          }
        : {
            tabId: candidates[candidates.length - 1].tabId,
            side: "after" as const,
            edgeX: candidates[candidates.length - 1].right,
          };

    return {
      tabId: target.tabId,
      side: target.side,
      indicatorLeft: target.edgeX - containerRect.left + container.scrollLeft,
      slotIndex,
    };
  };

  const getPointerScreenPosition = (e: PointerEvent): WorkspacePointerPosition => ({
    x: e.screenX,
    y: e.screenY,
  });

  const getForeignDropTargetAtScreenPoint = (
    screenX: number,
    screenY: number
  ): ForeignDropTarget | null => {
    const container = tabsRef.current;
    if (!container) return null;

    const clientX = screenX - window.screenX;
    const clientY = screenY - window.screenY;
    const containerRect = container.getBoundingClientRect();
    if (
      clientY < containerRect.top ||
      clientY > containerRect.bottom ||
      clientX < containerRect.left ||
      clientX > containerRect.right
    ) {
      return null;
    }

    const candidates = Array.from(
      container.querySelectorAll<HTMLElement>("[data-workspace-terminal-tab-id]")
    )
      .map((element) => {
        const tabId = element.dataset.workspaceTerminalTabId;
        if (!tabId || closingTabIds.includes(tabId)) return null;

        const rect = element.getBoundingClientRect();
        return {
          centerX: rect.left + rect.width / 2,
          left: rect.left,
          right: rect.right,
        };
      })
      .filter((item): item is NonNullable<typeof item> => Boolean(item));

    if (candidates.length === 0) {
      return {
        targetIndex: 0,
        indicatorLeft: clientX - containerRect.left + container.scrollLeft,
      };
    }

    let targetIndex = candidates.findIndex((candidate) => clientX < candidate.centerX);
    if (targetIndex === -1) {
      targetIndex = candidates.length;
    }

    const edgeX =
      targetIndex < candidates.length
        ? candidates[targetIndex].left
        : candidates[candidates.length - 1].right;

    return {
      targetIndex,
      indicatorLeft: edgeX - containerRect.left + container.scrollLeft,
    };
  };

  const clearForeignHover = () => {
    setForeignDropTarget(null);
    if (foreignHoverIndexRef.current !== null) {
      foreignHoverIndexRef.current = null;
      onCrossWindowDragHover(null);
    }
  };

  const resetDragState = () => {
    pointerDragState.current = null;
    setDraggedTabId(null);
    updateDropTarget(null);
  };

  const canDropOnTab = (sourceTabId: string, target: TabDropTarget | null) =>
    Boolean(
      target &&
      target.tabId !== sourceTabId &&
      tabs.some((tab) => tab.id === target.tabId) &&
      !closingTabIds.includes(target.tabId)
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

    const target = getForeignDropTargetAtScreenPoint(
      dragPreview.pointer_screen_position.x,
      dragPreview.pointer_screen_position.y
    );
    setForeignDropTarget(target);

    const nextIndex = target?.targetIndex ?? null;
    if (foreignHoverIndexRef.current !== nextIndex) {
      foreignHoverIndexRef.current = nextIndex;
      onCrossWindowDragHover(nextIndex);
    }
  }, [dragPreview, windowId, closingTabIds, tabs]);

  const handlePointerMove = (e: PointerEvent<HTMLButtonElement>) => {
    const dragState = pointerDragState.current;
    if (!dragState || dragState.pointerId !== e.pointerId) return;

    const distance = Math.hypot(e.clientX - dragState.startX, e.clientY - dragState.startY);
    if (!dragState.active) {
      if (distance < DRAG_START_DISTANCE) return;
      dragState.active = true;
      setDraggedTabId(dragState.tabId);
      if (dragState.isTerminalTab) {
        dragState.crossWindowStarted = true;
        onCrossWindowDragStart(dragState.tabId, getPointerScreenPosition(e));
      }
    }

    e.preventDefault();
    if (dragState.isTerminalTab) {
      onCrossWindowDragUpdate(getPointerScreenPosition(e));
    }
    const nextDropTarget = getDropTargetAtPoint(
      dragState.tabId,
      e.clientX,
      e.clientY,
      dropTargetRef.current
    );
    updateDropTarget(canDropOnTab(dragState.tabId, nextDropTarget) ? nextDropTarget : null);
  };

  const handlePointerUp = (e: PointerEvent<HTMLButtonElement>) => {
    const dragState = pointerDragState.current;
    if (!dragState || dragState.pointerId !== e.pointerId) return;

    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }

    if (dragState.active) {
      e.preventDefault();
      e.stopPropagation();
      const target =
        getDropTargetAtPoint(dragState.tabId, e.clientX, e.clientY, dropTargetRef.current) ??
        dropTargetRef.current;
      if (target && canDropOnTab(dragState.tabId, target)) {
        onReorderTabs(dragState.tabId, target.tabId, target.side);
        if (dragState.crossWindowStarted) {
          onCrossWindowDragCancel();
        }
      } else if (
        dragState.isTerminalTab &&
        (e.clientX < 0 ||
          e.clientX > window.innerWidth ||
          e.clientY < 0 ||
          e.clientY > window.innerHeight)
      ) {
        onCrossWindowDragDrop(dragState.tabId, getPointerScreenPosition(e));
      } else if (dragState.crossWindowStarted) {
        onCrossWindowDragCancel();
      }
      suppressNextClick.current = true;
      window.setTimeout(() => {
        suppressNextClick.current = false;
      }, 0);
    }

    resetDragState();
  };

  const handlePointerCancel = (e: PointerEvent<HTMLButtonElement>) => {
    const dragState = pointerDragState.current;
    if (dragState?.pointerId === e.pointerId && dragState.crossWindowStarted) {
      onCrossWindowDragCancel();
    }
    resetDragState();
  };

  const iconFor = (connectionType: ConnectionType) => {
    if (connectionType === "ssh") return <Monitor size={13} />;
    if (connectionType === "telnet") return <Network size={13} />;
    return <Usb size={13} />;
  };

  const labelFor = (tab: AppTabInfo) => {
    switch (tab.kind) {
      case "settings":
        return t("settings.title");
      case "logs":
        return t("logs.title");
      case "terminal":
        return tab.title;
    }
  };

  return (
    <div className="terminal-tabs" ref={tabsRef}>
      {tabs.map((tab) => {
        const isClosing = closingTabIds.includes(tab.id);
        const isTerminalTab = tab.kind === "terminal";
        const isDragging = draggedTabId === tab.id;

        return (
          <button
            key={tab.id}
            type="button"
            className={`terminal-tab ${tab.id === activeTabId ? "terminal-tab--active" : ""} ${
              isDragging ? "terminal-tab--dragging" : ""
            }`}
            data-terminal-tab-id={tab.id}
            data-workspace-terminal-tab-id={isTerminalTab ? tab.id : undefined}
            onPointerDown={(e) => {
              if (isClosing || e.button !== 0) return;
              if ((e.target as HTMLElement).closest(".terminal-tab__close")) return;

              pointerDragState.current = {
                tabId: tab.id,
                pointerId: e.pointerId,
                startX: e.clientX,
                startY: e.clientY,
                active: false,
                isTerminalTab,
                crossWindowStarted: false,
              };
              e.currentTarget.setPointerCapture(e.pointerId);
            }}
            onPointerMove={handlePointerMove}
            onPointerUp={handlePointerUp}
            onPointerCancel={handlePointerCancel}
            onClick={(e) => {
              if (suppressNextClick.current) {
                e.preventDefault();
                e.stopPropagation();
                return;
              }
              if (!isClosing) {
                onSelectTab(tab.id);
              }
            }}
            disabled={isClosing}
            aria-busy={isClosing}
          >
            <span className="terminal-tab__icon">
              {isTerminalTab && iconFor(tab.connectionType)}
              {tab.kind === "settings" && <Settings size={13} />}
              {tab.kind === "logs" && <FileText size={13} />}
            </span>
            {isTerminalTab && (
              <span
                className={`terminal-tab__status ${
                  tab.isConnected
                    ? "terminal-tab__status--connected"
                    : "terminal-tab__status--disconnected"
                }`}
              />
            )}
            <span className="terminal-tab__label">{labelFor(tab)}</span>
            <span
              className="terminal-tab__close"
              onPointerDown={(e) => e.stopPropagation()}
              onClick={(e) => {
                e.stopPropagation();
                if (!isClosing) {
                  void onCloseTab(tab.id);
                }
              }}
            >
              <X size={12} />
            </span>
          </button>
        );
      })}
      <button
        type="button"
        className="terminal-tabs__add"
        onClick={onAddTab}
        aria-label={t("terminal.new_tab")}
      >
        <Plus size={14} />
      </button>
      {dropTarget && (
        <span
          className="terminal-tabs__drop-indicator"
          style={{ left: `${dropTarget.indicatorLeft}px` }}
        />
      )}
      {foreignDropTarget && (
        <span
          className="terminal-tabs__drop-indicator terminal-tabs__drop-indicator--foreign"
          style={{ left: `${foreignDropTarget.indicatorLeft}px` }}
        />
      )}
    </div>
  );
}
