import { useEffect, useRef } from "react";

export type PopoverMenuItem =
  | {
      key: string;
      label: string;
      shortcut?: string;
      active?: boolean;
      disabled?: boolean;
      action: () => void;
    }
  | {
      key: string;
      separator: true;
    };

interface PopoverMenuProps {
  items: PopoverMenuItem[];
  onAction: (action: () => void) => void;
  className?: string;
  autoFocus?: boolean;
  ariaLabelledBy?: string;
  onClose?: () => void;
  onNavigateHorizontal?: (direction: "previous" | "next") => void;
}

export function PopoverMenu({
  items,
  onAction,
  className = "",
  autoFocus = false,
  ariaLabelledBy,
  onClose,
  onNavigateHorizontal,
}: PopoverMenuProps) {
  const classes = ["ui-popover-menu", className].filter(Boolean).join(" ");
  const menuRef = useRef<HTMLDivElement>(null);

  const getEnabledItems = () =>
    Array.from(
      menuRef.current?.querySelectorAll<HTMLButtonElement>(
        ".ui-popover-menu__item:not(:disabled)"
      ) ?? []
    );

  useEffect(() => {
    if (autoFocus) {
      getEnabledItems()[0]?.focus();
    }
  }, [autoFocus]);

  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const enabledItems = getEnabledItems();
    const currentIndex = enabledItems.findIndex((item) => item === document.activeElement);
    let nextItem: HTMLButtonElement | undefined;

    switch (event.key) {
      case "ArrowDown":
        nextItem = enabledItems[(currentIndex + 1) % enabledItems.length];
        break;
      case "ArrowUp":
        nextItem = enabledItems[(currentIndex - 1 + enabledItems.length) % enabledItems.length];
        break;
      case "Home":
        nextItem = enabledItems[0];
        break;
      case "End":
        nextItem = enabledItems[enabledItems.length - 1];
        break;
      case "ArrowLeft":
        if (!onNavigateHorizontal) return;
        onNavigateHorizontal("previous");
        break;
      case "ArrowRight":
        if (!onNavigateHorizontal) return;
        onNavigateHorizontal("next");
        break;
      case "Escape":
        if (!onClose) return;
        onClose();
        break;
      case "Tab":
        onClose?.();
        return;
      default:
        return;
    }

    event.preventDefault();
    event.stopPropagation();
    nextItem?.focus();
  };

  return (
    <div
      ref={menuRef}
      className={classes}
      role="menu"
      aria-labelledby={ariaLabelledBy}
      onKeyDown={handleKeyDown}
    >
      {items.map((item) => {
        if ("separator" in item) {
          return <div key={item.key} className="ui-popover-menu__separator" role="separator" />;
        }

        return (
          <button
            key={item.key}
            className={`ui-popover-menu__item ${
              item.active ? "ui-popover-menu__item--active" : ""
            }`}
            role="menuitem"
            tabIndex={autoFocus ? -1 : undefined}
            disabled={item.disabled}
            onClick={() => {
              if (item.disabled) return;
              onAction(item.action);
            }}
          >
            <span className="ui-popover-menu__item-label">{item.label}</span>
            {item.shortcut && <span className="ui-popover-menu__shortcut">{item.shortcut}</span>}
          </button>
        );
      })}
    </div>
  );
}
