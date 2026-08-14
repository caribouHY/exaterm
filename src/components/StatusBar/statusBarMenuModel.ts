import type {
  Encoding,
  ManualLogWriteMode,
  ShortcutConfig,
  TabInfo,
  TerminalMode,
} from "../../types";
import { formatShortcut } from "../../features/shortcuts/shortcutModel";
import {
  canPauseManualLog,
  canResumeManualLog,
} from "../../features/terminal-logging/terminalLoggingModel";
import type { TerminalModeOption } from "../../utils/terminalModes";

export type StatusBarMenuKind = "log" | "terminalMode" | "encoding";

export interface StatusBarPaletteItem {
  key: string;
  label: string;
  searchLabel?: string;
  shortcut?: string;
  active?: boolean;
  disabled?: boolean;
  action: () => void;
}

export interface StatusBarPaletteLabelSegment {
  text: string;
  matched: boolean;
}

export const STATUS_BAR_ENCODINGS: readonly { label: string; value: Encoding }[] = [
  { label: "UTF-8", value: "utf-8" },
  { label: "Shift-JIS", value: "shift-jis" },
  { label: "EUC-JP", value: "euc-jp" },
];

interface StatusBarPaletteLogLabel {
  display: string;
  english: string;
}

interface StatusBarPaletteLabels {
  logStartOverwrite: StatusBarPaletteLogLabel;
  logStartAppend: StatusBarPaletteLogLabel;
  logStop: StatusBarPaletteLogLabel;
  logPause: StatusBarPaletteLogLabel;
  logResume: StatusBarPaletteLogLabel;
}

interface StatusBarPaletteActions {
  onEncodingChange: (encoding: Encoding) => void;
  onTerminalModeChange: (terminalMode: TerminalMode) => void;
  onStartManualLog: (writeMode: ManualLogWriteMode) => void;
  onStopManualLog: () => void;
  onSetManualLoggingPaused: (paused: boolean) => void;
}

interface CreateStatusBarPaletteItemsOptions {
  kind: StatusBarMenuKind;
  activeTab: TabInfo;
  shortcuts: ShortcutConfig;
  terminalModes: TerminalModeOption[];
  labels: StatusBarPaletteLabels;
  actions: StatusBarPaletteActions;
}

function createLogPaletteLabels({
  display,
  english,
}: StatusBarPaletteLogLabel): Pick<StatusBarPaletteItem, "label" | "searchLabel"> {
  return display === english ? { label: display } : { label: display, searchLabel: english };
}

export function createStatusBarPaletteItems({
  kind,
  activeTab,
  shortcuts,
  terminalModes,
  labels,
  actions,
}: CreateStatusBarPaletteItemsOptions): StatusBarPaletteItem[] {
  if (kind === "encoding") {
    return STATUS_BAR_ENCODINGS.map((encoding) => ({
      key: `encoding_${encoding.value}`,
      label: encoding.label,
      active: activeTab.encoding === encoding.value,
      action: () => {
        actions.onEncodingChange(encoding.value);
      },
    }));
  }

  if (kind === "terminalMode") {
    return terminalModes.map((mode) => ({
      key: `terminal_mode_${mode.value}`,
      label: `${mode.label} (${mode.cliValue})`,
      active: activeTab.terminalMode === mode.value,
      action: () => {
        actions.onTerminalModeChange(mode.value);
      },
    }));
  }

  const isManualLogging = Boolean(activeTab.isManualLogging);
  const isManualLoggingPaused = Boolean(activeTab.isManualLoggingPaused);
  const shortcut = (binding: ShortcutConfig[keyof ShortcutConfig]) =>
    formatShortcut(binding) || undefined;

  return [
    {
      key: "log_start_overwrite",
      ...createLogPaletteLabels(labels.logStartOverwrite),
      shortcut: shortcut(shortcuts.terminal_log_start_overwrite),
      disabled: !activeTab.isConnected || isManualLogging,
      action: () => {
        actions.onStartManualLog("overwrite");
      },
    },
    {
      key: "log_start_append",
      ...createLogPaletteLabels(labels.logStartAppend),
      shortcut: shortcut(shortcuts.terminal_log_start_append),
      disabled: !activeTab.isConnected || isManualLogging,
      action: () => {
        actions.onStartManualLog("append");
      },
    },
    {
      key: "log_stop",
      ...createLogPaletteLabels(labels.logStop),
      shortcut: shortcut(shortcuts.terminal_log_stop),
      disabled: !isManualLogging,
      action: actions.onStopManualLog,
    },
    {
      key: "log_pause",
      ...createLogPaletteLabels(labels.logPause),
      shortcut: shortcut(shortcuts.terminal_log_pause),
      disabled:
        !activeTab.isConnected || !canPauseManualLog(isManualLogging, isManualLoggingPaused),
      action: () => {
        actions.onSetManualLoggingPaused(true);
      },
    },
    {
      key: "log_resume",
      ...createLogPaletteLabels(labels.logResume),
      shortcut: shortcut(shortcuts.terminal_log_resume),
      disabled:
        !activeTab.isConnected || !canResumeManualLog(isManualLogging, isManualLoggingPaused),
      action: () => {
        actions.onSetManualLoggingPaused(false);
      },
    },
  ];
}

export function filterStatusBarPaletteItems(
  items: StatusBarPaletteItem[],
  query: string
): StatusBarPaletteItem[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!normalizedQuery) return items;

  return items.filter((item) =>
    (item.searchLabel ?? item.label).toLocaleLowerCase().includes(normalizedQuery)
  );
}

export function getStatusBarPaletteLabelSegments(
  label: string,
  query: string
): StatusBarPaletteLabelSegment[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!normalizedQuery) return [{ text: label, matched: false }];

  const normalizedLabel = label.toLocaleLowerCase();
  const segments: StatusBarPaletteLabelSegment[] = [];
  let cursor = 0;

  while (cursor < label.length) {
    const matchIndex = normalizedLabel.indexOf(normalizedQuery, cursor);
    if (matchIndex < 0) break;

    if (matchIndex > cursor) {
      segments.push({ text: label.slice(cursor, matchIndex), matched: false });
    }

    const matchEnd = matchIndex + normalizedQuery.length;
    segments.push({ text: label.slice(matchIndex, matchEnd), matched: true });
    cursor = matchEnd;
  }

  if (cursor < label.length) {
    segments.push({ text: label.slice(cursor), matched: false });
  }

  return segments;
}

export function resolveStatusBarPaletteSelection(
  items: StatusBarPaletteItem[],
  preferredKey: string | null
): string | null {
  const firstEnabledItem = items.find((item) => !item.disabled);
  if (!firstEnabledItem) return null;
  if (preferredKey && items.some((item) => !item.disabled && item.key === preferredKey)) {
    return preferredKey;
  }

  return items.find((item) => !item.disabled && item.active)?.key ?? firstEnabledItem.key;
}

export function moveStatusBarPaletteSelection(
  items: StatusBarPaletteItem[],
  currentKey: string | null,
  direction: "previous" | "next"
): string | null {
  const enabledItems = items.filter((item) => !item.disabled);
  if (enabledItems.length === 0) return null;

  const currentIndex = enabledItems.findIndex((item) => item.key === currentKey);
  if (currentIndex < 0) {
    const boundaryIndex = direction === "next" ? 0 : enabledItems.length - 1;
    return enabledItems.find((_item, index) => index === boundaryIndex)?.key ?? null;
  }

  const offset = direction === "next" ? 1 : -1;
  const nextIndex = (currentIndex + offset + enabledItems.length) % enabledItems.length;
  return enabledItems.find((_item, index) => index === nextIndex)?.key ?? null;
}
