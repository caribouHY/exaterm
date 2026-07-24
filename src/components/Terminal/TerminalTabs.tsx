import { FileText, Monitor, Network, Settings, Usb, Plus, X } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  AppTabInfo,
  ConnectionType,
  ForeignTabPlacement,
  TabInfo,
  WorkspaceDragPreview,
  WorkspacePointerPosition,
} from "../../types";
import { PopoverMenu, type PopoverMenuItem } from "../Common";
import { clampContextMenuPosition, type TabDropSide } from "./tabDragGeometry";
import { useTabPointerDrag } from "./useTabPointerDrag";
import "./TerminalTabs.css";

interface TerminalTabsProps {
  tabs: AppTabInfo[];
  activeTabId: string | null;
  closingTabIds: string[];
  onSelectTab: (id: string) => void;
  onCloseTab: (id: string) => Promise<void>;
  onMoveTabToNewWindow: (id: string) => Promise<void>;
  onOpenSameDestination: (tab: TabInfo) => void;
  onAddTab: () => void;
  onReorderTabs: (draggedId: string, targetId: string, dropSide: TabDropSide) => void;
  windowId: string;
  dragPreview: WorkspaceDragPreview | null;
  onCrossWindowDragStart: (tabId: string, pointerScreenPosition: WorkspacePointerPosition) => void;
  onCrossWindowDragUpdate: (pointerScreenPosition: WorkspacePointerPosition) => void;
  onCrossWindowDragDrop: (tabId: string, pointerScreenPosition: WorkspacePointerPosition) => void;
  onCrossWindowDragCancel: () => void;
  onCrossWindowDragHover: (
    targetIndex: number | null,
    placement: ForeignTabPlacement | null
  ) => void;
}

interface TabContextMenuState {
  tabId: string;
  x: number;
  y: number;
}

export default function TerminalTabs({
  tabs,
  activeTabId,
  closingTabIds,
  onSelectTab,
  onCloseTab,
  onMoveTabToNewWindow,
  onOpenSameDestination,
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
  const tabRefs = useRef(new Map<string, HTMLButtonElement>());
  const addTabRef = useRef<HTMLButtonElement | null>(null);
  const contextMenuRef = useRef<HTMLDivElement | null>(null);
  const [contextMenu, setContextMenu] = useState<TabContextMenuState | null>(null);
  const visibleTabOrder = tabs.map((tab) => tab.id).join("\u0000");
  const pointerDrag = useTabPointerDrag({
    containerRef: tabsRef,
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
  });

  const ensureActiveTabVisible = useCallback(() => {
    if (!activeTabId) return;

    const container = tabsRef.current;
    const activeTab = tabRefs.current.get(activeTabId);
    if (!container || !activeTab) return;

    const containerRect = container.getBoundingClientRect();
    const activeTabRect = activeTab.getBoundingClientRect();
    const addTabRect = addTabRef.current?.getBoundingClientRect();
    const visibleLeft = containerRect.left;
    const visibleRight = addTabRect
      ? Math.min(containerRect.right, addTabRect.left)
      : containerRect.right;

    if (activeTabRect.left < visibleLeft) {
      container.scrollLeft += activeTabRect.left - visibleLeft;
    } else if (activeTabRect.right > visibleRight) {
      container.scrollLeft += activeTabRect.right - visibleRight;
    }
  }, [activeTabId]);

  useLayoutEffect(() => {
    ensureActiveTabVisible();

    if (!activeTabId || typeof ResizeObserver === "undefined") return;

    const container = tabsRef.current;
    const activeTab = tabRefs.current.get(activeTabId);
    const addTab = addTabRef.current;
    if (!container || !activeTab || !addTab) return;

    const resizeObserver = new ResizeObserver(ensureActiveTabVisible);
    resizeObserver.observe(container);
    resizeObserver.observe(activeTab);
    resizeObserver.observe(addTab);

    return () => {
      resizeObserver.disconnect();
    };
  }, [activeTabId, ensureActiveTabVisible, visibleTabOrder]);

  const closeContextMenu = () => {
    setContextMenu(null);
  };

  useEffect(() => {
    if (!contextMenu) return;

    const handleMouseDown = (event: MouseEvent) => {
      if (!contextMenuRef.current?.contains(event.target as Node)) {
        closeContextMenu();
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeContextMenu();
      }
    };

    const handleWindowChange = () => {
      closeContextMenu();
    };

    document.addEventListener("mousedown", handleMouseDown);
    document.addEventListener("keydown", handleKeyDown);
    window.addEventListener("resize", handleWindowChange);
    window.addEventListener("blur", handleWindowChange);
    return () => {
      document.removeEventListener("mousedown", handleMouseDown);
      document.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("resize", handleWindowChange);
      window.removeEventListener("blur", handleWindowChange);
    };
  }, [contextMenu]);

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

  const contextMenuTab = contextMenu
    ? tabs.find((tab) => tab.id === contextMenu.tabId) || null
    : null;
  const contextMenuItems: PopoverMenuItem[] = contextMenuTab
    ? [
        ...(contextMenuTab.kind === "terminal"
          ? [
              ...(contextMenuTab.connectionType !== "serial" && contextMenuTab.connectionInfo
                ? [
                    {
                      key: "open_same_destination",
                      label: t("terminal.tab_menu.open_same_destination"),
                      action: () => {
                        onOpenSameDestination(contextMenuTab);
                      },
                    },
                  ]
                : []),
              {
                key: "move_to_new_window",
                label: t("terminal.tab_menu.move_to_new_window"),
                disabled: closingTabIds.includes(contextMenuTab.id),
                action: () => {
                  void onMoveTabToNewWindow(contextMenuTab.id);
                },
              },
              { key: "separator-move-close", separator: true as const },
            ]
          : []),
        {
          key: "close",
          label: t("terminal.tab_menu.close"),
          disabled: closingTabIds.includes(contextMenuTab.id),
          action: () => {
            void onCloseTab(contextMenuTab.id);
          },
        },
      ]
    : [];

  return (
    <div className="terminal-tabs" ref={tabsRef}>
      {tabs.map((tab) => {
        const isClosing = closingTabIds.includes(tab.id);
        const isTerminalTab = tab.kind === "terminal";
        const isDragging = pointerDrag.draggedTabId === tab.id;

        return (
          <button
            key={tab.id}
            ref={(element) => {
              if (element) {
                tabRefs.current.set(tab.id, element);
              } else {
                tabRefs.current.delete(tab.id);
              }
            }}
            type="button"
            className={`terminal-tab ${tab.id === activeTabId ? "terminal-tab--active" : ""} ${
              isDragging ? "terminal-tab--dragging" : ""
            }`}
            data-terminal-tab-id={tab.id}
            data-workspace-terminal-tab-id={isTerminalTab ? tab.id : undefined}
            onPointerDown={(e) => {
              if (isClosing || e.button !== 0) return;
              if ((e.target as HTMLElement).closest(".terminal-tab__close")) return;

              pointerDrag.beginPointerDrag(e, tab.id, isTerminalTab);
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              pointerDrag.resetPointerDrag();
              const position = clampContextMenuPosition(
                { x: e.clientX, y: e.clientY },
                { width: window.innerWidth, height: window.innerHeight }
              );
              setContextMenu({
                tabId: tab.id,
                x: position.x,
                y: position.y,
              });
            }}
            onPointerMove={pointerDrag.handlePointerMove}
            onPointerUp={pointerDrag.handlePointerUp}
            onPointerCancel={pointerDrag.handlePointerCancel}
            onClick={(e) => {
              if (pointerDrag.consumeSuppressedClick()) {
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
              onPointerDown={(e) => {
                e.stopPropagation();
              }}
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
        ref={addTabRef}
        type="button"
        className="terminal-tabs__add"
        onClick={onAddTab}
        aria-label={t("terminal.new_tab")}
      >
        <Plus size={14} />
      </button>
      {pointerDrag.dropTarget && (
        <span
          className="terminal-tabs__drop-indicator"
          style={{ left: `${pointerDrag.dropTarget.indicatorLeft}px` }}
        />
      )}
      {pointerDrag.foreignDropTarget && (
        <span
          className="terminal-tabs__drop-indicator terminal-tabs__drop-indicator--foreign"
          style={{ left: `${pointerDrag.foreignDropTarget.indicatorLeft}px` }}
        />
      )}
      {contextMenu && contextMenuItems.length > 0 && (
        <div
          ref={contextMenuRef}
          className="terminal-tabs__context-menu"
          style={{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }}
        >
          <PopoverMenu
            items={contextMenuItems}
            onAction={(action) => {
              action();
              closeContextMenu();
            }}
          />
        </div>
      )}
    </div>
  );
}
