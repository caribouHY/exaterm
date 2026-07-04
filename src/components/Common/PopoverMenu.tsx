export type PopoverMenuItem =
  | {
      key: string;
      label: string;
      shortcut?: string;
      active?: boolean;
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
}

export function PopoverMenu({ items, onAction, className = "" }: PopoverMenuProps) {
  const classes = ["ui-popover-menu", className].filter(Boolean).join(" ");

  return (
    <div className={classes} role="menu">
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
            onClick={() => {
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
