import { ChevronDown } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

interface ProfileGroupFieldProps {
  value: string;
  groupNames: string[];
  onChange: (value: string) => void;
}

export function ProfileGroupField({ value, groupNames, onChange }: ProfileGroupFieldProps) {
  const { t } = useTranslation();
  const id = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [highlighted, setHighlighted] = useState(-1);
  const options = ["", ...groupNames];

  useEffect(() => {
    if (open && highlighted >= 0) {
      listRef.current?.children[highlighted]?.scrollIntoView({ block: "nearest" });
    }
  }, [open, highlighted]);

  const select = (name: string) => {
    onChange(name);
    setOpen(false);
    setHighlighted(-1);
    inputRef.current?.focus();
  };

  return (
    <div
      className="connection-dialog__group"
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false);
      }}
    >
      <label className="label" htmlFor={id}>
        {t("connection.profile_group")}
      </label>
      <div className="connection-dialog__group-control">
        <input
          ref={inputRef}
          id={id}
          className="input connection-dialog__group-input"
          role="combobox"
          autoComplete="off"
          aria-autocomplete="list"
          aria-expanded={open}
          aria-controls={open ? `${id}-groups` : undefined}
          aria-activedescendant={
            open && highlighted >= 0 ? `${id}-group-${highlighted}` : undefined
          }
          value={value}
          placeholder={t("connection.profile_ungrouped")}
          onChange={(event) => {
            onChange(event.target.value);
            setOpen(true);
            setHighlighted(-1);
          }}
          onClick={() => setOpen(true)}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing || event.keyCode === 229) {
              event.stopPropagation();
              return;
            }
            if (event.key === "ArrowDown" || event.key === "ArrowUp") {
              event.preventDefault();
              setOpen(true);
              const direction = event.key === "ArrowDown" ? 1 : -1;
              setHighlighted((current) => {
                if (!open || current < 0) return direction === 1 ? 0 : options.length - 1;
                return (current + direction + options.length) % options.length;
              });
            } else if (event.key === "Escape" && open) {
              event.preventDefault();
              event.stopPropagation();
              setOpen(false);
              setHighlighted(-1);
            } else if (
              event.key === "Enter" &&
              open &&
              !event.ctrlKey &&
              !event.metaKey &&
              !event.altKey
            ) {
              event.preventDefault();
              if (highlighted >= 0 && highlighted < options.length) select(options[highlighted]);
              else setOpen(false);
            } else if (event.key === "Tab") {
              setOpen(false);
            }
          }}
        />
        <button
          className="connection-dialog__group-toggle"
          type="button"
          aria-label={t("connection.profile_group")}
          aria-expanded={open}
          aria-controls={open ? `${id}-groups` : undefined}
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => {
            inputRef.current?.focus();
            setOpen(!open);
            setHighlighted(-1);
          }}
        >
          <ChevronDown size={14} aria-hidden="true" />
        </button>
      </div>
      {open && (
        <div
          ref={listRef}
          id={`${id}-groups`}
          className="connection-dialog__group-list"
          role="listbox"
          aria-label={t("connection.profile_group")}
        >
          {options.map((name, index) => (
            <button
              key={name}
              id={`${id}-group-${index}`}
              className={`connection-dialog__group-option ${highlighted === index ? "connection-dialog__group-option--highlighted" : ""}`}
              type="button"
              role="option"
              aria-selected={name === value.trim()}
              tabIndex={-1}
              onMouseDown={(event) => event.preventDefault()}
              onPointerMove={() => setHighlighted(index)}
              onClick={() => select(name)}
            >
              {name || t("connection.profile_ungrouped")}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
