import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TabInfo, Encoding, TerminalMode, ManualLogWriteMode } from "../../types";
import { getTerminalModeOptions } from "../../utils/terminalModes";
import {
  createStatusBarPaletteItems,
  filterStatusBarPaletteItems,
  getStatusBarPaletteLabelSegments,
  moveStatusBarPaletteSelection,
  resolveStatusBarPaletteSelection,
  type StatusBarMenuKind,
  type StatusBarPaletteItem,
} from "./statusBarMenuModel";
import "./StatusBarPalette.css";

export type StatusBarPaletteCloseReason = "action" | "confirm" | "escape" | "outside" | "tab";

interface StatusBarPaletteProps {
  kind: StatusBarMenuKind;
  activeTab: TabInfo;
  onEncodingChange: (encoding: Encoding) => void;
  onTerminalModeChange: (terminalMode: TerminalMode) => void;
  onStartManualLog: (writeMode: ManualLogWriteMode) => void;
  onStopManualLog: () => void;
  onSetManualLoggingPaused: (paused: boolean) => void;
  onClose: (reason: StatusBarPaletteCloseReason) => void;
}

export default function StatusBarPalette({
  kind,
  activeTab,
  onEncodingChange,
  onTerminalModeChange,
  onStartManualLog,
  onStopManualLog,
  onSetManualLoggingPaused,
  onClose,
}: StatusBarPaletteProps) {
  const { t } = useTranslation();
  const paletteRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const terminalModes = useMemo(() => getTerminalModeOptions(t), [t]);
  const items = useMemo(
    () =>
      createStatusBarPaletteItems({
        kind,
        activeTab,
        terminalModes,
        labels: {
          logStartOverwrite: {
            display: t("statusbar.log_start_manual_overwrite"),
            english: t("statusbar.log_start_manual_overwrite", { lng: "en" }),
          },
          logStartAppend: {
            display: t("statusbar.log_start_manual_append"),
            english: t("statusbar.log_start_manual_append", { lng: "en" }),
          },
          logStop: {
            display: t("statusbar.log_stop_manual"),
            english: t("statusbar.log_stop_manual", { lng: "en" }),
          },
          logPause: {
            display: t("statusbar.log_pause"),
            english: t("statusbar.log_pause", { lng: "en" }),
          },
          logResume: {
            display: t("statusbar.log_resume"),
            english: t("statusbar.log_resume", { lng: "en" }),
          },
        },
        actions: {
          onEncodingChange,
          onTerminalModeChange,
          onStartManualLog,
          onStopManualLog,
          onSetManualLoggingPaused,
        },
      }),
    [
      activeTab,
      kind,
      onEncodingChange,
      onSetManualLoggingPaused,
      onStartManualLog,
      onStopManualLog,
      onTerminalModeChange,
      t,
      terminalModes,
    ]
  );
  const filteredItems = useMemo(() => filterStatusBarPaletteItems(items, query), [items, query]);
  const [highlightedKey, setHighlightedKey] = useState<string | null>(() =>
    resolveStatusBarPaletteSelection(items, null)
  );

  const placeholder =
    kind === "log"
      ? t("statusbar.palette.search_log")
      : kind === "terminalMode"
        ? t("statusbar.palette.search_terminal_mode")
        : t("statusbar.palette.search_encoding");
  const listId = "statusbar-palette-list";

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    setHighlightedKey((current) => resolveStatusBarPaletteSelection(filteredItems, current));
  }, [filteredItems]);

  useEffect(() => {
    if (!highlightedKey) return;
    document
      .getElementById(`statusbar-palette-option-${highlightedKey}`)
      ?.scrollIntoView({ block: "nearest" });
  }, [highlightedKey]);

  useEffect(() => {
    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (paletteRef.current?.contains(target)) return;
      if (target instanceof Element && target.closest("[data-statusbar-menu-trigger]")) return;
      onClose("outside");
    };

    document.addEventListener("mousedown", handlePointerDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
    };
  }, [onClose]);

  const runItem = (item: StatusBarPaletteItem, closeReason: "action" | "confirm") => {
    if (item.disabled) return;
    item.action();
    onClose(closeReason);
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing) return;

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      setHighlightedKey((current) =>
        moveStatusBarPaletteSelection(
          filteredItems,
          current,
          event.key === "ArrowDown" ? "next" : "previous"
        )
      );
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      const highlightedItem = filteredItems.find((item) => item.key === highlightedKey);
      if (highlightedItem) runItem(highlightedItem, "confirm");
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose("escape");
      return;
    }

    if (event.key === "Tab") {
      onClose("tab");
    }
  };

  return (
    <div
      id="statusbar-command-palette"
      ref={paletteRef}
      className="statusbar-palette"
      role="dialog"
      aria-label={placeholder}
    >
      <input
        ref={inputRef}
        className="statusbar-palette__input"
        type="text"
        role="combobox"
        value={query}
        placeholder={placeholder}
        aria-label={placeholder}
        aria-autocomplete="list"
        aria-controls={listId}
        aria-expanded="true"
        aria-activedescendant={
          highlightedKey ? `statusbar-palette-option-${highlightedKey}` : undefined
        }
        onChange={(event) => {
          setQuery(event.target.value);
        }}
        onKeyDown={handleKeyDown}
      />
      <div id={listId} className="statusbar-palette__list" role="listbox">
        {filteredItems.length === 0 ? (
          <div className="statusbar-palette__empty" role="status">
            {t("statusbar.palette.no_results")}
          </div>
        ) : (
          filteredItems.map((item) => {
            const isHighlighted = item.key === highlightedKey;
            return (
              <button
                id={`statusbar-palette-option-${item.key}`}
                key={item.key}
                className={`statusbar-palette__item ${
                  isHighlighted ? "statusbar-palette__item--highlighted" : ""
                }`}
                type="button"
                role="option"
                aria-selected={isHighlighted}
                disabled={item.disabled}
                onMouseEnter={() => {
                  if (!item.disabled) setHighlightedKey(item.key);
                }}
                onClick={() => {
                  runItem(item, "action");
                }}
              >
                <span className="statusbar-palette__item-label">
                  {getStatusBarPaletteLabelSegments(item.label, item.searchLabel ? "" : query).map(
                    (segment, index) =>
                      segment.matched ? (
                        <mark
                          key={`${item.key}_segment_${index}`}
                          className="statusbar-palette__match"
                        >
                          {segment.text}
                        </mark>
                      ) : (
                        <span key={`${item.key}_segment_${index}`}>{segment.text}</span>
                      )
                  )}
                  {item.searchLabel ? (
                    <span className="statusbar-palette__search-label">
                      {" — "}
                      {getStatusBarPaletteLabelSegments(item.searchLabel, query).map(
                        (segment, index) =>
                          segment.matched ? (
                            <mark
                              key={`${item.key}_search_segment_${index}`}
                              className="statusbar-palette__match"
                            >
                              {segment.text}
                            </mark>
                          ) : (
                            <span key={`${item.key}_search_segment_${index}`}>{segment.text}</span>
                          )
                      )}
                    </span>
                  ) : null}
                  {item.active ? <span>{` (${t("statusbar.palette.current")})`}</span> : null}
                </span>
              </button>
            );
          })
        )}
      </div>
    </div>
  );
}
