import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ShortcutConfig, ViewMode } from "../../types";
import AIAssistantLogo from "../AI/AIAssistantLogo";
import { PopoverMenu } from "../Common";
import { createTitleBarMenus, type TitleBarMenuKey } from "./titleBarMenuModel";
import appIcon from "../../../src-tauri/icons/icon.png";
import "./TitleBar.css";

interface TitleBarProps {
  activeView: ViewMode;
  showAiPanel: boolean;
  shortcuts: ShortcutConfig;
  onViewChange: (view: ViewMode) => void;
  onOpenConnection: () => void;
  onOpenWindow: () => void;
  canAccessTerminal: boolean;
  canCopyTerminal: boolean;
  canPasteTerminal: boolean;
  onSelectAllTerminal: () => void;
  onCopyTerminal: () => void;
  onPasteTerminal: () => void;
  onClearTerminalViewport: () => void;
  onClearTerminalBuffer: () => void;
  onToggleAiPanel: () => void;
  onCheckForUpdates: () => void;
  onExit: () => void;
}

export default function TitleBar({
  activeView,
  showAiPanel,
  shortcuts,
  onViewChange,
  onOpenConnection,
  onOpenWindow,
  canAccessTerminal,
  canCopyTerminal,
  canPasteTerminal,
  onSelectAllTerminal,
  onCopyTerminal,
  onPasteTerminal,
  onClearTerminalViewport,
  onClearTerminalBuffer,
  onToggleAiPanel,
  onCheckForUpdates,
  onExit,
}: TitleBarProps) {
  const { t } = useTranslation();
  const appWindow = getCurrentWindow();
  const [openMenu, setOpenMenu] = useState<TitleBarMenuKey | null>(null);
  const menuBarRef = useRef<HTMLDivElement>(null);
  const fileTriggerRef = useRef<HTMLButtonElement>(null);
  const editTriggerRef = useRef<HTMLButtonElement>(null);
  const getTriggerRef = (menu: TitleBarMenuKey) =>
    menu === "file" ? fileTriggerRef : editTriggerRef;

  useEffect(() => {
    if (!openMenu) return;

    const handlePointerDown = (event: MouseEvent) => {
      if (!menuBarRef.current?.contains(event.target as Node)) {
        setOpenMenu(null);
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        getTriggerRef(openMenu).current?.focus();
        setOpenMenu(null);
      }
    };

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [openMenu]);

  const runMenuAction = (action: () => void) => {
    setOpenMenu(null);
    action();
  };

  const menus = createTitleBarMenus({
    activeView,
    shortcuts,
    canAccessTerminal,
    canCopyTerminal,
    canPasteTerminal,
    labels: {
      newConnection: t("titlebar.menu.new_connection"),
      newWindow: t("titlebar.menu.new_window"),
      sessionLogs: t("titlebar.menu.session_logs"),
      settings: t("titlebar.menu.settings"),
      checkUpdates: t("titlebar.menu.check_updates"),
      exit: t("titlebar.menu.exit"),
      selectAll: t("titlebar.menu.terminal_select_all"),
      copy: t("titlebar.menu.terminal_copy"),
      paste: t("titlebar.menu.terminal_paste"),
      clearViewport: t("titlebar.menu.terminal_clear_viewport"),
      clearBuffer: t("titlebar.menu.terminal_clear_buffer"),
    },
    actions: {
      openConnection: onOpenConnection,
      openWindow: onOpenWindow,
      openSessionLogs: () => {
        onViewChange("logs");
      },
      openSettings: () => {
        onViewChange("settings");
      },
      checkUpdates: onCheckForUpdates,
      exit: onExit,
      selectAll: onSelectAllTerminal,
      copy: onCopyTerminal,
      paste: onPasteTerminal,
      clearViewport: onClearTerminalViewport,
      clearBuffer: onClearTerminalBuffer,
    },
  });

  const switchMenu = (menu: TitleBarMenuKey) => {
    setOpenMenu(menu);
  };

  const closeMenuAndRestoreFocus = () => {
    if (openMenu) {
      getTriggerRef(openMenu).current?.focus();
    }
    setOpenMenu(null);
  };

  const navigateMenu = () => {
    const target = openMenu === "file" ? "edit" : "file";
    switchMenu(target);
  };

  const handleTriggerKeyDown = (
    event: React.KeyboardEvent<HTMLButtonElement>,
    menu: TitleBarMenuKey
  ) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      switchMenu(menu);
      return;
    }
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      const target = menu === "file" ? "edit" : "file";
      getTriggerRef(target).current?.focus();
      if (openMenu) {
        switchMenu(target);
      }
    }
  };

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar__left">
        <img className="titlebar__logo" src={appIcon} alt="" aria-hidden="true" draggable={false} />

        <div className="titlebar__menubar" ref={menuBarRef} role="menubar">
          {(["file", "edit"] as const).map((menu) => {
            const isOpen = openMenu === menu;
            const triggerId = `titlebar-${menu}-menu-trigger`;
            return (
              <div
                key={menu}
                className="titlebar__menu"
                onPointerEnter={() => {
                  if (openMenu && !isOpen) {
                    switchMenu(menu);
                  }
                }}
              >
                <button
                  ref={(element) => {
                    getTriggerRef(menu).current = element;
                  }}
                  id={triggerId}
                  className={`titlebar__menu-trigger ${isOpen ? "titlebar__menu-trigger--open" : ""}`}
                  onClick={() => {
                    setOpenMenu(isOpen ? null : menu);
                  }}
                  onKeyDown={(event) => {
                    handleTriggerKeyDown(event, menu);
                  }}
                  aria-expanded={isOpen}
                  aria-haspopup="menu"
                  role="menuitem"
                >
                  {t(`titlebar.menu.${menu}`)}
                </button>

                {isOpen && (
                  <PopoverMenu
                    items={menu === "file" ? menus.file : menus.edit}
                    onAction={runMenuAction}
                    className={`titlebar__menu-popover titlebar__menu-popover--${menu}`}
                    autoFocus
                    ariaLabelledBy={triggerId}
                    onClose={closeMenuAndRestoreFocus}
                    onNavigateHorizontal={navigateMenu}
                  />
                )}
              </div>
            );
          })}
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
