import { useRef, useState } from "react";
import type { PointerEvent } from "react";
import { FileText, Monitor, Network, Settings, Usb, Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AppTabInfo, ConnectionType } from "../../types";
import "./TerminalTabs.css";

type DropPosition = "before" | "after";

const DRAG_THRESHOLD_PX = 4;

interface DragSession {
  sourceId: string;
  startX: number;
  startY: number;
  isDragging: boolean;
}

interface DragGhost {
  x: number;
  y: number;
  tab: AppTabInfo;
}

interface TabBounds {
  id: string;
  midpoint: number;
}

interface TerminalTabsProps {
  tabs: AppTabInfo[];
  activeTabId: string | null;
  closingTabIds: string[];
  onSelectTab: (id: string) => void;
  onCloseTab: (id: string) => Promise<void>;
  onAddTab: () => void;
  onReorderTab: (sourceId: string, targetId: string, position: DropPosition) => void;
}

export default function TerminalTabs({
  tabs,
  activeTabId,
  closingTabIds,
  onSelectTab,
  onCloseTab,
  onAddTab,
  onReorderTab,
}: TerminalTabsProps) {
  const { t } = useTranslation();
  const [draggedTabId, setDraggedTabId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<{
    id: string;
    position: DropPosition;
  } | null>(null);
  const [dragGhost, setDragGhost] = useState<DragGhost | null>(null);
  const tabsRef = useRef<HTMLDivElement | null>(null);
  const dragSessionRef = useRef<DragSession | null>(null);
  const suppressClickRef = useRef(false);

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

  const renderTabContent = (tab: AppTabInfo) => {
    const isTerminalTab = tab.kind === "terminal";

    return (
      <>
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
      </>
    );
  };

  const clearDragState = () => {
    dragSessionRef.current = null;
    setDraggedTabId(null);
    setDropTarget(null);
    setDragGhost(null);
  };

  const updateDropTarget = (nextDropTarget: { id: string; position: DropPosition } | null) => {
    setDropTarget((current) => {
      if (current?.id === nextDropTarget?.id && current?.position === nextDropTarget?.position) {
        return current;
      }
      return nextDropTarget;
    });
  };

  const findDropTarget = (
    event: PointerEvent<HTMLButtonElement>,
    sourceId: string
  ): { id: string; position: DropPosition } | null => {
    const tabBounds: TabBounds[] = Array.from(
      tabsRef.current?.querySelectorAll<HTMLButtonElement>("[data-tab-id]") ?? []
    ).flatMap((element) => {
      const id = element.dataset.tabId;
      if (!id || id === sourceId || closingTabIds.includes(id)) return [];

      const rect = element.getBoundingClientRect();
      return [
        {
          id,
          midpoint: rect.left + rect.width / 2,
        },
      ];
    });

    if (tabBounds.length === 0) {
      updateDropTarget(null);
      return null;
    }

    const nextIndex = tabBounds.findIndex((tab) => event.clientX < tab.midpoint);
    const nextDropTarget =
      nextIndex === -1
        ? {
            id: tabBounds[tabBounds.length - 1].id,
            position: "after" as const,
          }
        : {
            id: tabBounds[nextIndex].id,
            position: "before" as const,
          };

    updateDropTarget(nextDropTarget);
    return nextDropTarget;
  };

  return (
    <div className="terminal-tabs" ref={tabsRef}>
      {tabs.map((tab) => {
        const isClosing = closingTabIds.includes(tab.id);
        const isDragged = draggedTabId === tab.id;
        const dropPosition = dropTarget?.id === tab.id ? dropTarget.position : null;
        const className = [
          "terminal-tab",
          tab.id === activeTabId ? "terminal-tab--active" : "",
          isDragged ? "terminal-tab--dragging" : "",
          dropPosition === "before" ? "terminal-tab--drop-before" : "",
          dropPosition === "after" ? "terminal-tab--drop-after" : "",
        ]
          .filter(Boolean)
          .join(" ");

        return (
          <button
            key={tab.id}
            type="button"
            className={className}
            data-tab-id={tab.id}
            onClick={() => {
              if (suppressClickRef.current) {
                suppressClickRef.current = false;
                return;
              }

              if (!isClosing) {
                onSelectTab(tab.id);
              }
            }}
            onPointerDown={(event) => {
              if (isClosing || event.button !== 0) return;

              dragSessionRef.current = {
                sourceId: tab.id,
                startX: event.clientX,
                startY: event.clientY,
                isDragging: false,
              };
              event.currentTarget.setPointerCapture(event.pointerId);
            }}
            onPointerMove={(event) => {
              const session = dragSessionRef.current;
              if (!session || session.sourceId !== tab.id) return;

              const distance = Math.hypot(
                event.clientX - session.startX,
                event.clientY - session.startY
              );
              if (!session.isDragging && distance < DRAG_THRESHOLD_PX) return;

              if (!session.isDragging) {
                session.isDragging = true;
                suppressClickRef.current = true;
                setDraggedTabId(session.sourceId);
              }

              event.preventDefault();
              setDragGhost({ x: event.clientX, y: event.clientY, tab });
              findDropTarget(event, session.sourceId);
            }}
            onPointerUp={(event) => {
              const session = dragSessionRef.current;
              if (!session || session.sourceId !== tab.id) return;

              const target = session.isDragging ? findDropTarget(event, session.sourceId) : null;
              if (target) {
                onReorderTab(session.sourceId, target.id, target.position);
              }

              if (event.currentTarget.hasPointerCapture(event.pointerId)) {
                event.currentTarget.releasePointerCapture(event.pointerId);
              }
              if (session.isDragging) {
                event.preventDefault();
                window.setTimeout(() => {
                  suppressClickRef.current = false;
                }, 0);
              }
              clearDragState();
            }}
            onPointerCancel={(event) => {
              if (event.currentTarget.hasPointerCapture(event.pointerId)) {
                event.currentTarget.releasePointerCapture(event.pointerId);
              }
              if (dragSessionRef.current?.isDragging) {
                window.setTimeout(() => {
                  suppressClickRef.current = false;
                }, 0);
              }
              clearDragState();
            }}
            onLostPointerCapture={() => {
              if (!dragSessionRef.current) {
                return;
              }
              if (dragSessionRef.current.isDragging) {
                window.setTimeout(() => {
                  suppressClickRef.current = false;
                }, 0);
              }
              clearDragState();
            }}
            disabled={isClosing}
            aria-busy={isClosing}
          >
            {renderTabContent(tab)}
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
      {dragGhost && (
        <div
          className="terminal-tab-ghost"
          style={{
            transform: `translate(${dragGhost.x + 10}px, ${dragGhost.y + 8}px)`,
          }}
        >
          {renderTabContent(dragGhost.tab)}
        </div>
      )}
    </div>
  );
}
