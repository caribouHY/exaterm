import { FileText, Monitor, Network, Settings, Usb, Plus, X } from "lucide-react";
import { useRef, useState } from "react";
import type { PointerEvent } from "react";
import { useTranslation } from "react-i18next";
import type { AppTabInfo, ConnectionType } from "../../types";
import "./TerminalTabs.css";

interface TerminalTabsProps {
  tabs: AppTabInfo[];
  activeTabId: string | null;
  closingTabIds: string[];
  onSelectTab: (id: string) => void;
  onCloseTab: (id: string) => Promise<void>;
  onAddTab: () => void;
  onReorderTabs: (draggedId: string, targetId: string) => void;
}

interface PointerDragState {
  tabId: string;
  pointerId: number;
  startX: number;
  startY: number;
  active: boolean;
}

const DRAG_START_DISTANCE = 4;

export default function TerminalTabs({
  tabs,
  activeTabId,
  closingTabIds,
  onSelectTab,
  onCloseTab,
  onAddTab,
  onReorderTabs,
}: TerminalTabsProps) {
  const { t } = useTranslation();
  const pointerDragState = useRef<PointerDragState | null>(null);
  const suppressNextClick = useRef(false);
  const [draggedTabId, setDraggedTabId] = useState<string | null>(null);
  const [dragOverTabId, setDragOverTabId] = useState<string | null>(null);

  const getTabIdAtPoint = (clientX: number, clientY: number) => {
    const element = document.elementFromPoint(clientX, clientY);
    const tabElement = element?.closest<HTMLElement>("[data-terminal-tab-id]");
    return tabElement?.dataset.terminalTabId ?? null;
  };

  const resetDragState = () => {
    pointerDragState.current = null;
    setDraggedTabId(null);
    setDragOverTabId(null);
  };

  const canDropOnTab = (sourceTabId: string, targetTabId: string | null) =>
    Boolean(
      targetTabId &&
      targetTabId !== sourceTabId &&
      tabs.some((tab) => tab.id === targetTabId) &&
      !closingTabIds.includes(targetTabId)
    );

  const handlePointerMove = (e: PointerEvent<HTMLButtonElement>) => {
    const dragState = pointerDragState.current;
    if (!dragState || dragState.pointerId !== e.pointerId) return;

    const distance = Math.hypot(e.clientX - dragState.startX, e.clientY - dragState.startY);
    if (!dragState.active) {
      if (distance < DRAG_START_DISTANCE) return;
      dragState.active = true;
      setDraggedTabId(dragState.tabId);
    }

    e.preventDefault();
    const targetTabId = getTabIdAtPoint(e.clientX, e.clientY);
    setDragOverTabId(canDropOnTab(dragState.tabId, targetTabId) ? targetTabId : null);
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
      const targetTabId = getTabIdAtPoint(e.clientX, e.clientY) ?? dragOverTabId;
      if (targetTabId && canDropOnTab(dragState.tabId, targetTabId)) {
        onReorderTabs(dragState.tabId, targetTabId);
      }
      suppressNextClick.current = true;
      window.setTimeout(() => {
        suppressNextClick.current = false;
      }, 0);
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
    <div className="terminal-tabs">
      {tabs.map((tab) => {
        const isClosing = closingTabIds.includes(tab.id);
        const isTerminalTab = tab.kind === "terminal";
        const isDragging = draggedTabId === tab.id;
        const isDragOver = dragOverTabId === tab.id && draggedTabId !== tab.id;

        return (
          <button
            key={tab.id}
            type="button"
            className={`terminal-tab ${tab.id === activeTabId ? "terminal-tab--active" : ""} ${
              isDragging ? "terminal-tab--dragging" : ""
            } ${isDragOver ? "terminal-tab--drag-over" : ""}`}
            data-terminal-tab-id={tab.id}
            onPointerDown={(e) => {
              if (isClosing || e.button !== 0) return;
              if ((e.target as HTMLElement).closest(".terminal-tab__close")) return;

              pointerDragState.current = {
                tabId: tab.id,
                pointerId: e.pointerId,
                startX: e.clientX,
                startY: e.clientY,
                active: false,
              };
              e.currentTarget.setPointerCapture(e.pointerId);
            }}
            onPointerMove={handlePointerMove}
            onPointerUp={handlePointerUp}
            onPointerCancel={resetDragState}
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
    </div>
  );
}
