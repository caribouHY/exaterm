import { FileText, Monitor, Network, Settings, Usb, Plus, X } from "lucide-react";
import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { message } from "@tauri-apps/plugin-dialog";
import { isClosableDisconnectedTab } from "../../features/workspace-tabs/tabStripModel";
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
  isClosingDisconnectedTabs: boolean;
  onSelectTab: (id: string) => void;
  onCloseTab: (id: string) => Promise<void>;
  onCloseDisconnectedTabs: () => Promise<void>;
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
  isClosingDisconnectedTabs,
  onSelectTab,
  onCloseTab,
  onCloseDisconnectedTabs,
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
  const tabIdPrefix = useId();
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

  const ensureControlVisible = useCallback((control: Element) => {
    const container = tabsRef.current;
    if (!container) return;

    const containerRect = container.getBoundingClientRect();
    const controlRect = control.getBoundingClientRect();
    const addTabRect = addTabRef.current?.getBoundingClientRect();
    const visibleLeft = containerRect.left;
    const visibleRight = addTabRect
      ? Math.min(containerRect.right, addTabRect.left)
      : containerRect.right;

    if (controlRect.left < visibleLeft) {
      container.scrollLeft += controlRect.left - visibleLeft;
    } else if (controlRect.right > visibleRight) {
      container.scrollLeft += controlRect.right - visibleRight;
    }
  }, []);

  useLayoutEffect(() => {
    const ensureTabVisible = () => {
      const focused = document.activeElement;
      if (focused && focused.closest(".terminal-tab")?.parentElement === tabsRef.current) {
        ensureControlVisible(focused);
        return;
      }

      const activeTab = activeTabId ? tabRefs.current.get(activeTabId)?.parentElement : null;
      if (activeTab) ensureControlVisible(activeTab);
    };
    ensureTabVisible();

    if (typeof ResizeObserver === "undefined") return;
    const container = tabsRef.current;
    const addTab = addTabRef.current;
    if (!container || !addTab) return;

    const resizeObserver = new ResizeObserver(ensureTabVisible);
    resizeObserver.observe(container);
    resizeObserver.observe(addTab);
    for (const tab of tabRefs.current.values()) {
      if (tab.parentElement) resizeObserver.observe(tab.parentElement);
    }

    return () => {
      resizeObserver.disconnect();
    };
  }, [activeTabId, ensureControlVisible, visibleTabOrder]);

  const focusAfterTabRemoval = (tabId: string, skipTab: (tab: AppTabInfo) => boolean) => {
    const index = tabs.findIndex((tab) => tab.id === tabId);
    const canFocus = (tab: AppTabInfo) => !closingTabIds.includes(tab.id) && !skipTab(tab);
    const target =
      index < 0
        ? undefined
        : (tabs.slice(index + 1).find(canFocus) ?? tabs.slice(0, index).reverse().find(canFocus));
    (target ? tabRefs.current.get(target.id) : addTabRef.current)?.focus();
  };

  const closeTab = (tabId: string, restoreFocus: boolean) => {
    if (restoreFocus) focusAfterTabRemoval(tabId, () => false);
    void onCloseTab(tabId);
  };

  const dismissContextMenu = () => {
    if (contextMenu) {
      const trigger = tabRefs.current.get(contextMenu.tabId);
      if (trigger && !trigger.disabled) {
        trigger.focus();
      } else {
        focusAfterTabRemoval(contextMenu.tabId, () => false);
      }
    }
    setContextMenu(null);
  };

  const openContextMenu = (tabId: string, x: number, y: number) => {
    pointerDrag.resetPointerDrag();
    const position = clampContextMenuPosition(
      { x, y },
      { width: window.innerWidth, height: window.innerHeight }
    );
    setContextMenu({ tabId, ...position });
  };

  useEffect(() => {
    if (!contextMenu) return;

    const handleMouseDown = (event: MouseEvent) => {
      if (!contextMenuRef.current?.contains(event.target as Node)) {
        setContextMenu(null);
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setContextMenu(null);
      }
    };

    const handleWindowChange = () => {
      setContextMenu(null);
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
  const copySessionId = async (sessionId: string) => {
    try {
      await writeText(sessionId);
    } catch {
      await message(t("terminal.tab_menu.copy_session_id_failed"), {
        title: t("terminal.tab_menu.copy_session_id"),
        kind: "error",
      }).catch(() => {
        console.error("Failed to display session ID copy error dialog.");
      });
    }
  };
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
                key: "copy_session_id",
                label: t("terminal.tab_menu.copy_session_id"),
                disabled: closingTabIds.includes(contextMenuTab.id) || !contextMenuTab.sessionId,
                action: () => {
                  void copySessionId(contextMenuTab.sessionId);
                },
              },
              {
                key: "move_to_new_window",
                label: t("terminal.tab_menu.move_to_new_window"),
                disabled: closingTabIds.includes(contextMenuTab.id),
                action: () => {
                  focusAfterTabRemoval(contextMenuTab.id, () => false);
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
            closeTab(contextMenuTab.id, true);
          },
        },
        ...(contextMenuTab.kind === "terminal"
          ? [
              {
                key: "close_disconnected",
                label: t("terminal.tab_menu.close_disconnected"),
                disabled:
                  isClosingDisconnectedTabs ||
                  !tabs.some((tab) => isClosableDisconnectedTab(tab, closingTabIds)),
                action: () => {
                  if (isClosableDisconnectedTab(contextMenuTab, closingTabIds)) {
                    focusAfterTabRemoval(contextMenuTab.id, (tab) =>
                      isClosableDisconnectedTab(tab, closingTabIds)
                    );
                  }
                  void onCloseDisconnectedTabs();
                },
              },
            ]
          : []),
      ]
    : [];

  return (
    <div className="terminal-tabs" ref={tabsRef}>
      {tabs.map((tab) => {
        const isClosing = closingTabIds.includes(tab.id);
        const isTerminalTab = tab.kind === "terminal";
        const isDragging = pointerDrag.draggedTabId === tab.id;
        const triggerId = `${tabIdPrefix}-${tab.id}`;
        const statusId = `${triggerId}-status`;
        const label = labelFor(tab);

        return (
          <div
            key={tab.id}
            className={`terminal-tab ${tab.id === activeTabId ? "terminal-tab--active" : ""} ${
              isDragging ? "terminal-tab--dragging" : ""
            }`}
            data-terminal-tab-id={tab.id}
            data-workspace-terminal-tab-id={isTerminalTab ? tab.id : undefined}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              openContextMenu(tab.id, e.clientX, e.clientY);
            }}
          >
            <button
              id={triggerId}
              ref={(element) => {
                if (element) {
                  tabRefs.current.set(tab.id, element);
                } else {
                  tabRefs.current.delete(tab.id);
                }
              }}
              type="button"
              className="terminal-tab__select"
              aria-current={tab.id === activeTabId ? "true" : undefined}
              aria-busy={isClosing}
              aria-describedby={isTerminalTab ? statusId : undefined}
              aria-haspopup="menu"
              aria-expanded={contextMenu?.tabId === tab.id}
              aria-keyshortcuts="Shift+F10 ContextMenu"
              disabled={isClosing}
              onFocus={(e) => {
                ensureControlVisible(e.currentTarget);
              }}
              onPointerDown={(e) => {
                if (!isClosing && e.button === 0) {
                  pointerDrag.beginPointerDrag(e, tab.id, isTerminalTab);
                }
              }}
              onPointerMove={pointerDrag.handlePointerMove}
              onPointerUp={pointerDrag.handlePointerUp}
              onPointerCancel={pointerDrag.handlePointerCancel}
              onKeyDown={(e) => {
                if (e.altKey || e.ctrlKey || e.metaKey) return;
                if (e.key !== "ContextMenu" && !(e.shiftKey && e.key === "F10")) return;
                e.preventDefault();
                e.stopPropagation();
                if (e.repeat) return;

                const { left, bottom } = e.currentTarget.getBoundingClientRect();
                openContextMenu(tab.id, left, bottom);
              }}
              onClick={(e) => {
                if (e.detail !== 0 && pointerDrag.consumeSuppressedClick()) {
                  e.preventDefault();
                  e.stopPropagation();
                  return;
                }
                if (!isClosing) onSelectTab(tab.id);
              }}
            >
              <span className="terminal-tab__icon" aria-hidden="true">
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
                  aria-hidden="true"
                />
              )}
              <span className="terminal-tab__label">{label}</span>
            </button>
            {isTerminalTab && (
              <span id={statusId} hidden>
                {t(tab.isConnected ? "statusbar.connected" : "statusbar.disconnected")}
              </span>
            )}
            <button
              type="button"
              className="terminal-tab__close"
              aria-label={t("terminal.close_named_tab", { name: label })}
              disabled={isClosing}
              onFocus={(e) => {
                ensureControlVisible(e.currentTarget);
              }}
              onClick={(e) => {
                closeTab(tab.id, e.detail === 0);
              }}
            >
              <X size={12} aria-hidden="true" />
            </button>
          </div>
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
      {contextMenu && contextMenuTab && (
        <div
          key={contextMenu.tabId}
          ref={contextMenuRef}
          className="terminal-tabs__context-menu"
          style={{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }}
        >
          <PopoverMenu
            items={contextMenuItems}
            autoFocus
            ariaLabelledBy={`${tabIdPrefix}-${contextMenu.tabId}`}
            onClose={dismissContextMenu}
            onAction={(action) => {
              dismissContextMenu();
              action();
            }}
          />
        </div>
      )}
    </div>
  );
}
