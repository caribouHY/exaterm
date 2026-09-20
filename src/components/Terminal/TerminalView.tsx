import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { SearchAddon } from "@xterm/addon-search";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useTranslation } from "react-i18next";
import type {
  ConnectionType,
  Encoding,
  ShortcutConfig,
  TerminalConfig,
  TerminalMode,
} from "../../types";
import {
  findShortcutAction,
  type TerminalLogShortcutAction,
} from "../../features/shortcuts/shortcutModel";
import { shouldAppendManualLog } from "../../features/terminal-logging/terminalLoggingModel";
import { createManualLogBufferWriter } from "../../features/terminal-logging/manualLogBufferWriter";
import { createTerminalLogSanitizer } from "../../utils/logSanitizer";
import {
  createTerminalDecorationController,
  type TerminalDecorationController,
} from "./terminalDecorationController";
import {
  createTerminalOutputSyncController,
  type TerminalOutputEventPayload,
  type TerminalOutputSnapshot,
  type TerminalOutputSyncController,
} from "./terminalOutputSyncController";
import { getTerminalDecorationProfile } from "./terminalDecorationProfiles";
import { getTerminalPromptColor, TERMINAL_DECORATION_COLORS } from "./terminalDecorationTheme";
import type { TerminalPinnedCommand } from "./terminalDecorationTypes";
import { clearTerminalBuffer, clearTerminalViewport } from "./terminalClearActions";
import { getTerminalControlInput } from "./terminalControlInput";
import "@xterm/xterm/css/xterm.css";
import "./TerminalView.css";

interface TerminalViewProps {
  sessionId: string;
  connectionType: ConnectionType;
  isConnected: boolean;
  isActive: boolean;
  encoding: Encoding;
  isManualLogging: boolean;
  isManualLoggingPaused: boolean;
  terminalConfig?: TerminalConfig;
  shortcuts: ShortcutConfig;
  terminalMode: TerminalMode;
  onTerminalData?: (data: string) => void;
  onTerminalModeShortcut?: () => void;
  onTerminalLogShortcut?: (action: TerminalLogShortcutAction) => void;
  onTerminalSelectionChange?: (hasSelection: boolean) => void;
}

export interface TerminalViewHandle {
  focus: () => void;
  isFocused: () => boolean;
  insertText: (text: string) => void;
  selectAll: () => void;
  copySelection: () => void;
  paste: () => void;
  clearViewport: () => void;
  clearBuffer: () => void;
  flushManualLogBuffer: () => Promise<void>;
  flushLogBuffersForMove: () => Promise<void>;
}

interface TerminalEditActions {
  selectAll: () => void;
  copySelection: () => void;
  paste: () => void;
}

const EMPTY_TERMINAL_EDIT_ACTIONS: TerminalEditActions = {
  selectAll: () => {},
  copySelection: () => {},
  paste: () => {},
};

const CONNECTION_COMMANDS: Record<
  ConnectionType,
  {
    write: string;
    dataEvent: string;
    errorEvent: string;
    errorReplayedBySnapshot: boolean;
    resize: string | null;
  }
> = {
  ssh: {
    write: "ssh_write",
    dataEvent: "ssh://data",
    errorEvent: "ssh://error",
    errorReplayedBySnapshot: true,
    resize: "ssh_resize",
  },
  serial: {
    write: "serial_write",
    dataEvent: "serial://data",
    errorEvent: "serial://error",
    errorReplayedBySnapshot: false,
    resize: null,
  },
  telnet: {
    write: "telnet_write",
    dataEvent: "telnet://data",
    errorEvent: "telnet://error",
    errorReplayedBySnapshot: false,
    resize: "telnet_resize",
  },
};

const getConnectionCommands = (connectionType: ConnectionType) => {
  switch (connectionType) {
    case "ssh":
      return CONNECTION_COMMANDS.ssh;
    case "serial":
      return CONNECTION_COMMANDS.serial;
    case "telnet":
      return CONNECTION_COMMANDS.telnet;
  }
};

function normalizeCursorStyle(cursorStyle: string | undefined): "block" | "bar" | "underline" {
  if (cursorStyle === "bar" || cursorStyle === "underline") {
    return cursorStyle;
  }

  return "block";
}

const TerminalView = forwardRef<TerminalViewHandle, TerminalViewProps>(function TerminalView(
  {
    sessionId,
    connectionType,
    isConnected,
    isActive,
    encoding,
    isManualLogging,
    isManualLoggingPaused,
    terminalConfig,
    shortcuts,
    terminalMode,
    onTerminalData,
    onTerminalModeShortcut,
    onTerminalLogShortcut,
    onTerminalSelectionChange,
  },
  ref
) {
  const { t } = useTranslation();
  const [pinnedCommand, setPinnedCommand] = useState<TerminalPinnedCommand | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const outputSyncControllerRef = useRef<TerminalOutputSyncController | null>(null);
  const isConnectedRef = useRef(isConnected);
  const isActiveRef = useRef(isActive);
  const isManualLoggingRef = useRef(isManualLogging);
  const isManualLoggingPausedRef = useRef(isManualLoggingPaused);
  const shortcutsRef = useRef(shortcuts);
  const onTerminalModeShortcutRef = useRef(onTerminalModeShortcut);
  const onTerminalLogShortcutRef = useRef(onTerminalLogShortcut);
  const onTerminalSelectionChangeRef = useRef(onTerminalSelectionChange);
  const clipboardActionInProgressRef = useRef(false);
  const terminalEditActionsRef = useRef<TerminalEditActions>(EMPTY_TERMINAL_EDIT_ACTIONS);
  const decorationControllerRef = useRef<TerminalDecorationController | null>(null);
  const decorationController =
    decorationControllerRef.current ??
    (decorationControllerRef.current = createTerminalDecorationController({
      onPinnedCommandChange: setPinnedCommand,
    }));
  const decorationProfile = getTerminalDecorationProfile(terminalMode);
  const refreshDecorationsAfterClear = useCallback(
    (terminal: Terminal) => {
      decorationController.clear();
      decorationController.schedule(terminal, true);
    },
    [decorationController]
  );

  const clearViewport = useCallback(() => {
    const terminal = termRef.current;
    if (!terminal) return;
    clearTerminalViewport(terminal, () => {
      refreshDecorationsAfterClear(terminal);
    });
  }, [refreshDecorationsAfterClear]);

  const clearBuffer = useCallback(() => {
    const terminal = termRef.current;
    if (!terminal) return;
    clearTerminalBuffer(terminal, () => {
      refreshDecorationsAfterClear(terminal);
    });
  }, [refreshDecorationsAfterClear]);

  useEffect(() => {
    shortcutsRef.current = shortcuts;
  }, [shortcuts]);

  useEffect(() => {
    isActiveRef.current = isActive;
  }, [isActive]);

  useEffect(() => {
    onTerminalModeShortcutRef.current = onTerminalModeShortcut;
  }, [onTerminalModeShortcut]);

  useEffect(() => {
    onTerminalLogShortcutRef.current = onTerminalLogShortcut;
  }, [onTerminalLogShortcut]);

  useEffect(() => {
    onTerminalSelectionChangeRef.current = onTerminalSelectionChange;
  }, [onTerminalSelectionChange]);
  const manualLogSanitizerRef = useRef(
    createTerminalLogSanitizer(terminalConfig?.log_format ?? "display")
  );
  const manualLogBufferWriter = useMemo(
    () =>
      createManualLogBufferWriter((data) =>
        invoke("logger_append", {
          sessionId,
          data,
        })
      ),
    [sessionId]
  );

  useEffect(() => {
    isConnectedRef.current = isConnected;
  }, [isConnected]);

  useEffect(() => {
    isManualLoggingRef.current = isManualLogging;
  }, [isManualLogging]);

  useEffect(() => {
    if (
      isManualLoggingPaused &&
      !isManualLoggingPausedRef.current &&
      sessionId &&
      isManualLoggingRef.current
    ) {
      const logText = manualLogSanitizerRef.current.flush();
      void manualLogBufferWriter.flush(logText).catch(() => {});
    }
    isManualLoggingPausedRef.current = isManualLoggingPaused;
  }, [isManualLoggingPaused, manualLogBufferWriter, sessionId]);

  useImperativeHandle(
    ref,
    () => ({
      focus: () => {
        termRef.current?.focus();
      },
      isFocused: () => {
        const container = containerRef.current;
        return Boolean(container && container.contains(document.activeElement));
      },
      insertText: (text: string) => {
        if (!sessionId || !isConnectedRef.current) return;
        const data = text.replace(/\r?\n+$/g, "");
        if (!data) return;
        const term = termRef.current;
        if (!term) return;
        term.paste(data);
        term.focus();
      },
      selectAll: () => {
        terminalEditActionsRef.current.selectAll();
      },
      copySelection: () => {
        terminalEditActionsRef.current.copySelection();
      },
      paste: () => {
        terminalEditActionsRef.current.paste();
      },
      clearViewport,
      clearBuffer,
      flushManualLogBuffer: async () => {
        if (!sessionId) return;
        const logText = manualLogSanitizerRef.current.flush();
        await manualLogBufferWriter.flush(logText);
      },
      flushLogBuffersForMove: async () => {
        if (!sessionId || !isManualLoggingRef.current) return;
        const logText = manualLogSanitizerRef.current.flush();
        await manualLogBufferWriter.flush(logText);
      },
    }),
    [clearBuffer, clearViewport, manualLogBufferWriter, sessionId]
  );

  useEffect(() => {
    outputSyncControllerRef.current?.setEncoding(encoding);
  }, [encoding]);

  useEffect(() => {
    manualLogSanitizerRef.current = createTerminalLogSanitizer(
      terminalConfig?.log_format ?? "display"
    );
  }, [terminalConfig?.log_format]);

  useEffect(() => {
    decorationController.setProfile(decorationProfile, termRef.current ?? undefined);
  }, [decorationController, decorationProfile]);

  // Create the terminal once per session and keep it mounted after disconnect.
  useEffect(() => {
    const terminalElement = containerRef.current;
    if (!terminalElement || !sessionId || termRef.current) return;

    const term = new Terminal({
      fontFamily:
        terminalConfig?.font_family || "'JetBrains Mono', Consolas, 'Courier New', monospace",
      fontSize: terminalConfig?.font_size || 14,
      theme: {
        background: "#1e1e1e",
        foreground: "#cccccc",
        cursor: "#aeafad",
        selectionBackground: "#264f78",
        black: "#1e1e1e",
        red: "#f44747",
        green: "#4ec9b0",
        yellow: "#dcdcaa",
        blue: "#569cd6",
        magenta: "#c586c0",
        cyan: "#9cdcfe",
        white: "#d4d4d4",
        brightBlack: "#808080",
        brightRed: "#f44747",
        brightGreen: "#4ec9b0",
        brightYellow: "#dcdcaa",
        brightBlue: "#569cd6",
        brightMagenta: "#c586c0",
        brightCyan: "#9cdcfe",
        brightWhite: "#ffffff",
      },
      cursorBlink: true,
      cursorStyle: normalizeCursorStyle(terminalConfig?.cursor_style),
      scrollback: terminalConfig?.scrollback || 10000,
      allowProposedApi: true,
    });

    const fitAddon = new FitAddon();
    const webLinksAddon = new WebLinksAddon();
    const searchAddon = new SearchAddon();

    term.loadAddon(fitAddon);
    term.loadAddon(webLinksAddon);
    term.loadAddon(searchAddon);

    term.open(terminalElement);
    fitAddon.fit();

    termRef.current = term;
    fitRef.current = fitAddon;

    // Terminal input -> backend
    const protocol = getConnectionCommands(connectionType);
    term.onData((data) => {
      if (!isConnectedRef.current) return;
      invoke(protocol.write, { sessionId, data }).catch(console.error);
    });
    const scrollDecorationDisposable = term.onScroll(() => {
      decorationController.schedule(term);
    });
    const selectionDisposable = term.onSelectionChange(() => {
      onTerminalSelectionChangeRef.current?.(term.hasSelection());
    });
    let disposed = false;

    const copyTerminalSelection = async (clearSelectionAfterCopy: boolean) => {
      const selection = term.getSelection();
      if (selection.length === 0) {
        return;
      }

      await writeText(selection);
      if (clearSelectionAfterCopy && !disposed) {
        term.clearSelection();
      }
    };

    const pasteClipboardIntoTerminal = async () => {
      if (!isConnectedRef.current) {
        return;
      }

      const clipboardText = await readText();
      if (disposed || clipboardText.length === 0) {
        return;
      }

      const hasMultipleLines = clipboardText.includes("\n") || clipboardText.includes("\r");
      if (hasMultipleLines) {
        const shouldPaste = await confirm(
          t("terminal.multiline_paste_message", { content: clipboardText }),
          {
            title: t("terminal.multiline_paste_title"),
            kind: "warning",
            okLabel: t("terminal.multiline_paste_confirm"),
            cancelLabel: t("terminal.multiline_paste_cancel"),
          }
        );

        if (!shouldPaste || disposed) {
          return;
        }
      }

      term.paste(clipboardText);
    };

    const runClipboardAction = (action: () => Promise<void>) => {
      if (clipboardActionInProgressRef.current) {
        return;
      }
      clipboardActionInProgressRef.current = true;

      void action()
        .catch(() => {
          // Clipboard failures should not send anything to the terminal.
        })
        .finally(() => {
          clipboardActionInProgressRef.current = false;
          if (!disposed) {
            term.focus();
          }
        });
    };

    const terminalEditActions: TerminalEditActions = {
      selectAll: () => {
        term.selectAll();
        term.focus();
      },
      copySelection: () => {
        runClipboardAction(() => copyTerminalSelection(false));
      },
      paste: () => {
        runClipboardAction(pasteClipboardIntoTerminal);
      },
    };
    terminalEditActionsRef.current = terminalEditActions;

    term.attachCustomKeyEventHandler((event) => {
      if (findShortcutAction(shortcutsRef.current, event, "application")) {
        return false;
      }

      const action = findShortcutAction(shortcutsRef.current, event, "terminal");
      if (!action) {
        const controlInput = getTerminalControlInput(event);
        if (!controlInput) {
          return true;
        }

        event.preventDefault();
        event.stopPropagation();
        if (event.type === "keydown") {
          term.input(controlInput);
        }
        return false;
      }

      event.preventDefault();
      event.stopPropagation();
      if (event.type !== "keydown" || event.repeat) {
        return false;
      }

      switch (action) {
        case "terminal_select_all":
          terminalEditActions.selectAll();
          break;
        case "terminal_copy":
          terminalEditActions.copySelection();
          break;
        case "terminal_paste":
          terminalEditActions.paste();
          break;
        case "terminal_clear_viewport":
          clearTerminalViewport(term, () => {
            refreshDecorationsAfterClear(term);
          });
          break;
        case "terminal_clear_buffer":
          clearTerminalBuffer(term, () => {
            refreshDecorationsAfterClear(term);
          });
          break;
        case "terminal_mode_menu":
          if (isActiveRef.current) {
            onTerminalModeShortcutRef.current?.();
          }
          break;
        case "terminal_log_start_overwrite":
        case "terminal_log_start_append":
        case "terminal_log_stop":
        case "terminal_log_pause":
        case "terminal_log_resume":
          if (isActiveRef.current && isConnectedRef.current) {
            onTerminalLogShortcutRef.current?.(action);
          }
          break;
      }
      return false;
    });

    const handleContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      runClipboardAction(
        term.hasSelection() ? () => copyTerminalSelection(true) : pasteClipboardIntoTerminal
      );
    };

    terminalElement.addEventListener("contextmenu", handleContextMenu);

    const writeTerminalText = (text: string) => {
      if (!text) return;
      term.write(text, () => {
        decorationController.schedule(term);
      });
      if (onTerminalData) onTerminalData(text);
      if (shouldAppendManualLog(isManualLoggingRef.current, isManualLoggingPausedRef.current)) {
        const logText = manualLogSanitizerRef.current.push(text);
        if (logText) {
          void manualLogBufferWriter.append(logText).catch(() => {});
        }
      }
    };
    const outputSyncController = createTerminalOutputSyncController({
      sessionId,
      encoding,
      maxChars: terminalConfig?.scrollback ?? 20000,
      channels: [
        { event: `${protocol.dataEvent}/${sessionId}`, replayedBySnapshot: true },
        {
          event: `${protocol.errorEvent}/${sessionId}`,
          replayedBySnapshot: protocol.errorReplayedBySnapshot,
        },
      ],
      write: writeTerminalText,
      dependencies: {
        listen: (event, handler) => listen<TerminalOutputEventPayload>(event, handler),
        getSnapshot: (currentSessionId, maxChars) =>
          invoke<TerminalOutputSnapshot>("terminal_output_snapshot_get", {
            sessionId: currentSessionId,
            maxChars,
          }),
        getDelta: (currentSessionId, cursor, maxChars) =>
          invoke<TerminalOutputSnapshot>("terminal_output_delta_get", {
            sessionId: currentSessionId,
            cursor,
            maxChars,
          }),
      },
    });
    outputSyncControllerRef.current = outputSyncController;
    void outputSyncController.start().catch(() => {});

    // Resize handling
    const resizeCmd = protocol.resize;
    const handleResize = () => {
      fitAddon.fit();
      decorationController.schedule(term, true);
      if (resizeCmd && sessionId && isConnectedRef.current) {
        invoke(resizeCmd, { sessionId, cols: term.cols, rows: term.rows }).catch(() => {});
      }
    };

    const resizeObserver = new ResizeObserver(handleResize);
    resizeObserver.observe(terminalElement);

    return () => {
      disposed = true;
      terminalElement.removeEventListener("contextmenu", handleContextMenu);
      outputSyncController.dispose();
      if (outputSyncControllerRef.current === outputSyncController) {
        outputSyncControllerRef.current = null;
      }
      if (isManualLoggingRef.current) {
        const logText = manualLogSanitizerRef.current.flush();
        void manualLogBufferWriter.flush(logText).catch(() => {});
      }
      resizeObserver.disconnect();
      scrollDecorationDisposable.dispose();
      selectionDisposable.dispose();
      onTerminalSelectionChangeRef.current?.(false);
      if (terminalEditActionsRef.current === terminalEditActions) {
        terminalEditActionsRef.current = EMPTY_TERMINAL_EDIT_ACTIONS;
      }
      decorationController.clear();
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, [sessionId, connectionType, manualLogBufferWriter, refreshDecorationsAfterClear]);

  // Re-fit the terminal whenever this tab becomes active (container goes from display:none to visible)
  useEffect(() => {
    if (isActive && fitRef.current) {
      // Small delay to allow the browser to lay out the now-visible container
      const timer = setTimeout(() => {
        fitRef.current?.fit();
        if (termRef.current) {
          decorationController.schedule(termRef.current, true);
        }
        termRef.current?.focus();
      }, 50);
      return () => {
        clearTimeout(timer);
      };
    }
  }, [isActive]);

  // Update terminal options when config changes
  useEffect(() => {
    if (termRef.current && terminalConfig) {
      termRef.current.options.fontSize = terminalConfig.font_size;
      termRef.current.options.fontFamily = terminalConfig.font_family;
      termRef.current.options.cursorStyle = normalizeCursorStyle(terminalConfig.cursor_style);
      termRef.current.options.scrollback = terminalConfig.scrollback;

      // Re-fit to adjust for potential size changes
      setTimeout(() => {
        fitRef.current?.fit();
        if (termRef.current) {
          decorationController.schedule(termRef.current, true);
        }
      }, 50);
    }
  }, [terminalConfig]);

  return (
    <div className={`terminal-view ${!isActive ? "terminal-view--hidden" : ""}`}>
      <div ref={containerRef} className="terminal-view__terminal" />
      {pinnedCommand && decorationProfile ? (
        <div
          className="terminal-view__pinned-command-overlay"
          style={{
            fontFamily:
              terminalConfig?.font_family || "'JetBrains Mono', Consolas, 'Courier New', monospace",
            fontSize: terminalConfig?.font_size || 14,
          }}
          aria-hidden="true"
        >
          {pinnedCommand.contextText ? (
            <div
              className="terminal-view__pinned-command-line"
              style={{ color: getTerminalPromptColor("configuration") }}
            >
              {pinnedCommand.contextText}
            </div>
          ) : null}
          <div className="terminal-view__pinned-command-line">
            <span
              style={{
                color: getTerminalPromptColor(pinnedCommand.promptVariant),
              }}
            >
              {pinnedCommand.promptText}
            </span>
            <span style={{ color: TERMINAL_DECORATION_COLORS.command }}>
              {pinnedCommand.commandText}
            </span>
          </div>
        </div>
      ) : null}
    </div>
  );
});

export default TerminalView;
