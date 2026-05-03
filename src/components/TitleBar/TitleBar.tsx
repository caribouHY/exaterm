import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ChevronDown, Menu, Minus, Square, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ViewMode } from "../../types";
import AIAssistantLogo from "../AI/AIAssistantLogo";
import "./TitleBar.css";

interface TitleBarProps {
  activeView: ViewMode;
  showAiPanel: boolean;
  onViewChange: (view: ViewMode) => void;
  onOpenConnection: () => void;
  onToggleAiPanel: () => void;
}

export default function TitleBar({
  activeView,
  showAiPanel,
  onViewChange,
  onOpenConnection,
  onToggleAiPanel,
}: TitleBarProps) {
  const { t } = useTranslation();
  const appWindow = getCurrentWindow();
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isMenuOpen) return;

    const handlePointerDown = (event: MouseEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) {
        setIsMenuOpen(false);
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsMenuOpen(false);
      }
    };

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isMenuOpen]);

  const runMenuAction = (action: () => void) => {
    action();
    setIsMenuOpen(false);
  };

  const menuItems = [
    {
      key: "new_connection",
      label: t("titlebar.menu.new_connection"),
      shortcut: "Ctrl+N",
      active: false,
      action: onOpenConnection,
    },
    { key: "separator-new-ai", separator: true },
    {
      key: "ai_assistant",
      label: t("titlebar.menu.ai_assistant"),
      active: showAiPanel,
      action: onToggleAiPanel,
    },
    {
      key: "logs",
      label: t("titlebar.menu.logs"),
      active: activeView === "logs",
      action: () => onViewChange("logs"),
    },
    { key: "separator-logs-settings", separator: true },
    {
      key: "settings",
      label: t("titlebar.menu.settings"),
      shortcut: "Ctrl+,",
      active: activeView === "settings",
      action: () => onViewChange("settings"),
    },
  ];

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar__left">
        <div className="titlebar__logo">E</div>

        <div className="titlebar__menu" ref={menuRef}>
          <button
            className={`titlebar__menu-trigger ${isMenuOpen ? "titlebar__menu-trigger--open" : ""}`}
            onClick={() => setIsMenuOpen((current) => !current)}
            aria-expanded={isMenuOpen}
            aria-haspopup="menu"
          >
            <Menu size={14} />
            <span>{t("titlebar.menu.label")}</span>
            <ChevronDown size={13} />
          </button>

          {isMenuOpen && (
            <div className="titlebar__menu-popover" role="menu">
              {menuItems.map((item) => {
                if ("separator" in item) {
                  return (
                    <div key={item.key} className="titlebar__menu-separator" role="separator" />
                  );
                }

                return (
                  <button
                    key={item.key}
                    className={`titlebar__menu-item ${
                      item.active ? "titlebar__menu-item--active" : ""
                    }`}
                    role="menuitem"
                    onClick={() => runMenuAction(item.action)}
                  >
                    <span className="titlebar__menu-item-label">{item.label}</span>
                    {item.shortcut && (
                      <span className="titlebar__menu-shortcut">{item.shortcut}</span>
                    )}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </div>

      <div className="titlebar__name">ExaTerm</div>

      <div className="titlebar__controls">
        <button
          className={`titlebar__ai-btn ${showAiPanel ? "titlebar__ai-btn--active" : ""}`}
          onClick={onToggleAiPanel}
          aria-label={t("titlebar.menu.ai_assistant")}
          title={t("titlebar.menu.ai_assistant")}
        >
          <AIAssistantLogo size="sm" />
        </button>
        <button
          className="titlebar__btn"
          onClick={() => appWindow.minimize()}
          aria-label={t("titlebar.minimize")}
        >
          <Minus size={14} />
        </button>
        <button
          className="titlebar__btn"
          onClick={async () => {
            if (await appWindow.isMaximized()) {
              appWindow.unmaximize();
            } else {
              appWindow.maximize();
            }
          }}
          aria-label={t("titlebar.maximize")}
        >
          <Square size={12} />
        </button>
        <button
          className="titlebar__btn titlebar__btn--close"
          onClick={() => appWindow.close()}
          aria-label={t("titlebar.close")}
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
