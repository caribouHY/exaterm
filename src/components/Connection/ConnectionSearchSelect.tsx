import { ChevronDown, X } from "lucide-react";
import { useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import {
  filterConnectionSearchSections,
  moveConnectionSearchSelection,
  resolveConnectionSearchSelection,
  type ConnectionSearchSection,
} from "./connectionSearchModel";

interface ConnectionSearchSelectProps {
  label: string;
  value: string;
  sections: ConnectionSearchSection[];
  onChange: (value: string) => void;
}

const searchOptionId = (id: string, value: string) => `${id}-option-${encodeURIComponent(value)}`;

export function ConnectionSearchSelect({
  label,
  value,
  sections,
  onChange,
}: ConnectionSearchSelectProps) {
  const { t } = useTranslation();
  const id = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [highlighted, setHighlighted] = useState<string | null>(null);
  const [position, setPosition] = useState<CSSProperties | null>(null);
  const positioned = position !== null;
  const allOptions = sections.flatMap((section) => section.options);
  const selected = allOptions.find((option) => option.value === value);
  const filtered = filterConnectionSearchSections(sections, query);
  const options = filtered.flatMap((section) => section.options);
  const activeValue = resolveConnectionSearchSelection(options, highlighted, query);
  const hasMatches = options.some((option) => !option.alwaysVisible);

  const close = (restoreFocus: boolean) => {
    setOpen(false);
    if (restoreFocus) triggerRef.current?.focus();
  };

  const show = () => {
    setQuery("");
    setHighlighted(value);
    setPosition(null);
    setOpen(true);
  };

  const choose = (optionValue: string) => {
    onChange(optionValue);
    close(true);
  };

  useLayoutEffect(() => {
    if (!open) return;
    const reposition = () => {
      const rect = triggerRef.current?.getBoundingClientRect();
      if (!rect) return;
      const below = window.innerHeight - rect.bottom - 8;
      const above = rect.top - 8;
      const upwards = below < 240 && above > below;
      const width = Math.min(rect.width, window.innerWidth - 16);
      setPosition({
        width,
        left: Math.max(8, Math.min(rect.left, window.innerWidth - width - 8)),
        top: upwards ? undefined : rect.bottom + 4,
        bottom: upwards ? window.innerHeight - rect.top + 4 : undefined,
        maxHeight: Math.max(80, Math.min(320, upwards ? above : below)),
      });
    };
    const dismissOutside = (event: Event) => {
      const target = event.target;
      if (
        !(target instanceof Node) ||
        rootRef.current?.contains(target) ||
        menuRef.current?.contains(target)
      )
        return;
      setOpen(false);
    };
    reposition();
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    document.addEventListener("pointerdown", dismissOutside, true);
    document.addEventListener("focusin", dismissOutside);
    const observer = new ResizeObserver(reposition);
    const dialog = rootRef.current?.closest('[role="dialog"]');
    dialog?.addEventListener("animationend", reposition);
    if (triggerRef.current) observer.observe(triggerRef.current);
    return () => {
      observer.disconnect();
      dialog?.removeEventListener("animationend", reposition);
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
      document.removeEventListener("pointerdown", dismissOutside, true);
      document.removeEventListener("focusin", dismissOutside);
    };
  }, [open]);

  useEffect(() => {
    if (open && positioned) inputRef.current?.focus();
  }, [open, positioned]);

  useEffect(() => {
    if (open && activeValue !== null)
      document
        .getElementById(searchOptionId(id, activeValue))
        ?.scrollIntoView({ block: "nearest" });
  }, [open, activeValue, query, id]);

  return (
    <div ref={rootRef} className="connection-dialog__search-select">
      <label className="label" id={`${id}-label`} htmlFor={`${id}-trigger`}>
        {label}
      </label>
      <button
        ref={triggerRef}
        id={`${id}-trigger`}
        type="button"
        className="select connection-dialog__search-trigger"
        aria-labelledby={`${id}-label ${id}-value`}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? `${id}-list` : undefined}
        onClick={() => {
          if (open) close(false);
          else show();
        }}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            show();
          }
        }}
      >
        <span id={`${id}-value`} className="connection-dialog__search-label">
          {selected?.label ??
            t(value ? "connection.profile_unavailable" : "connection.profile_manual")}
        </span>
        <ChevronDown size={14} aria-hidden="true" />
      </button>
      {open &&
        createPortal(
          <div
            ref={menuRef}
            className="connection-dialog__search-menu"
            style={position ?? { visibility: "hidden" }}
            onKeyDown={(event) => {
              if (event.key === "Tab") {
                if (event.shiftKey) event.preventDefault();
                close(true);
              }
            }}
          >
            <div className="connection-dialog__search-input-row">
              <input
                ref={inputRef}
                className="input connection-dialog__search-input"
                role="combobox"
                value={query}
                autoComplete="off"
                aria-label={t("connection.profile_search")}
                placeholder={t("connection.profile_search_placeholder")}
                aria-autocomplete="list"
                aria-expanded="true"
                aria-controls={`${id}-list`}
                aria-activedescendant={
                  activeValue !== null ? searchOptionId(id, activeValue) : undefined
                }
                onChange={(event) => {
                  setQuery(event.target.value);
                  setHighlighted(null);
                }}
                onKeyDown={(event) => {
                  if (event.nativeEvent.isComposing || event.keyCode === 229) {
                    event.stopPropagation();
                    return;
                  }
                  if (event.key === "Escape") {
                    event.preventDefault();
                    event.stopPropagation();
                    close(true);
                  } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                    event.preventDefault();
                    setHighlighted(
                      moveConnectionSearchSelection(
                        options,
                        activeValue,
                        event.key === "ArrowDown" ? "next" : "previous"
                      )
                    );
                  } else if (
                    event.key === "Enter" &&
                    !event.ctrlKey &&
                    !event.metaKey &&
                    !event.altKey
                  ) {
                    event.preventDefault();
                    if (activeValue !== null) choose(activeValue);
                  }
                }}
              />
              {query && (
                <button
                  type="button"
                  className="btn-icon connection-dialog__search-clear"
                  aria-label={t("connection.profile_search_clear")}
                  onClick={() => {
                    setQuery("");
                    setHighlighted(value);
                    inputRef.current?.focus();
                  }}
                >
                  <X size={14} aria-hidden="true" />
                </button>
              )}
            </div>
            <div
              id={`${id}-list`}
              className="connection-dialog__search-list"
              role="listbox"
              aria-label={label}
            >
              {filtered.map((section) => (
                <div
                  key={section.key}
                  role="group"
                  aria-label={section.label}
                  className={`connection-dialog__search-section ${section.label ? "connection-dialog__search-section--grouped" : ""}`}
                >
                  {section.label && (
                    <div
                      className="connection-dialog__search-heading"
                      title={section.label}
                      aria-hidden="true"
                    >
                      {section.label}
                    </div>
                  )}
                  {section.options.map((option) => (
                    <button
                      key={option.value}
                      id={searchOptionId(id, option.value)}
                      type="button"
                      role="option"
                      tabIndex={-1}
                      aria-selected={option.value === value}
                      className={`connection-dialog__search-option ${option.value === activeValue ? "connection-dialog__search-option--highlighted" : ""}`}
                      onMouseDown={(event) => {
                        event.preventDefault();
                      }}
                      onPointerMove={() => {
                        setHighlighted(option.value);
                      }}
                      onClick={() => {
                        choose(option.value);
                      }}
                    >
                      <span className="connection-dialog__search-label">{option.label}</span>
                      {option.detail && (
                        <span className="connection-dialog__search-detail">{option.detail}</span>
                      )}
                    </button>
                  ))}
                </div>
              ))}
            </div>
            {!hasMatches && (
              <div className="connection-dialog__search-empty" role="status">
                {t("connection.profile_search_empty")}
              </div>
            )}
          </div>,
          rootRef.current?.closest('[role="dialog"]') ?? document.body
        )}
    </div>
  );
}
