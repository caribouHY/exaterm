import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";
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
  ShortcutBinding,
  ShortcutConfig,
  TerminalConfig,
  TerminalMode,
} from "../../types";
import {
  findShortcutAction,
  formatShortcut,
  type TerminalLogShortcutAction,
} from "../../features/shortcuts/shortcutModel";
import {
  shouldAppendAutoLog,
  shouldAppendManualLog,
} from "../../features/terminal-logging/terminalLoggingModel";
import { createTerminalLogSanitizer } from "../../utils/logSanitizer";
import {
  createTerminalDecorationController,
  type TerminalDecorationController,
} from "./terminalDecorationController";
import { getTerminalDecorationProfile } from "./terminalDecorationProfiles";
import { getTerminalPromptColor, TERMINAL_DECORATION_COLORS } from "./terminalDecorationTheme";
import type { TerminalPinnedCommand } from "./terminalDecorationTypes";
import { clearTerminalBuffer, clearTerminalViewport } from "./terminalClearActions";
import appIcon from "../../../src-tauri/icons/icon.png";
import "@xterm/xterm/css/xterm.css";
import "./TerminalView.css";

interface TerminalViewProps {
  sessionId: string | null;
  connectionType: ConnectionType;
  isConnected: boolean;
  isActive: boolean;
  encoding: Encoding;
  isAutoLogging: boolean;
  isManualLogging: boolean;
  isManualLoggingPaused: boolean;
  terminalConfig?: TerminalConfig;
  shortcuts: ShortcutConfig;
  terminalMode: TerminalMode;
  onOpenConnection: () => void;
  onTerminalData?: (data: string) => void;
  onTerminalLogShortcut?: (action: TerminalLogShortcutAction) => void;
  onTerminalSelectionChange?: (hasSelection: boolean) => void;
}

export interface TerminalViewHandle {
  insertText: (text: string) => void;
  selectAll: () => void;
  copySelection: () => void;
  paste: () => void;
  clearViewport: () => void;
  clearBuffer: () => void;
  flushManualLogBuffer: () => Promise<void>;
  flushLogBuffersForMove: () => Promise<void>;
}

interface TerminalOutputSnapshot {
  session_id: string;
  output: string;
  truncated: boolean;
  available_chars: number;
  start_cursor: number;
  cursor: number;
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
    isAutoLogging,
    isManualLogging,
    isManualLoggingPaused,
    terminalConfig,
    shortcuts,
    terminalMode,
    onOpenConnection,
    onTerminalData,
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
  const decoderRef = useRef(new TextDecoder(encoding));
  const isConnectedRef = useRef(isConnected);
  const isActiveRef = useRef(isActive);
  const isAutoLoggingRef = useRef(isAutoLogging);
  const isManualLoggingRef = useRef(isManualLogging);
  const isManualLoggingPausedRef = useRef(isManualLoggingPaused);
  const shortcutsRef = useRef(shortcuts);
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
  const autoLogSanitizerRef = useRef(
    createTerminalLogSanitizer(terminalConfig?.log_format ?? "display")
  );

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
    onTerminalLogShortcutRef.current = onTerminalLogShortcut;
  }, [onTerminalLogShortcut]);

  useEffect(() => {
    onTerminalSelectionChangeRef.current = onTerminalSelectionChange;
  }, [onTerminalSelectionChange]);
  const manualLogSanitizerRef = useRef(
    createTerminalLogSanitizer(terminalConfig?.log_format ?? "display")
  );

  const connectionCommands: Record<
    ConnectionType,
    { write: string; dataEvent: string; errorEvent: string; resize: string | null }
  > = {
    ssh: {
      write: "ssh_write",
      dataEvent: "ssh://data",
      errorEvent: "ssh://error",
      resize: "ssh_resize",
    },
    serial: {
      write: "serial_write",
      dataEvent: "serial://data",
      errorEvent: "serial://error",
      resize: null,
    },
    telnet: {
      write: "telnet_write",
      dataEvent: "telnet://data",
      errorEvent: "telnet://error",
      resize: "telnet_resize",
    },
  };

  useEffect(() => {
    isConnectedRef.current = isConnected;
  }, [isConnected]);

  useEffect(() => {
    isAutoLoggingRef.current = isAutoLogging;
  }, [isAutoLogging]);

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
      if (logText) {
        invoke("logger_append_to_mode", { sessionId, logMode: "manual", data: logText }).catch(
          () => {}
        );
      }
    }
    isManualLoggingPausedRef.current = isManualLoggingPaused;
  }, [isManualLoggingPaused, sessionId]);

  useImperativeHandle(
    ref,
    () => ({
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
        if (!logText) return;
        await invoke("logger_append_to_mode", {
          sessionId,
          logMode: "manual",
          data: logText,
        });
      },
      flushLogBuffersForMove: async () => {
        if (!sessionId) return;
        const appendTasks: Promise<unknown>[] = [];
        if (shouldAppendAutoLog(isAutoLoggingRef.current)) {
          const logText = autoLogSanitizerRef.current.flush();
          if (logText) {
            appendTasks.push(
              invoke("logger_append_to_mode", {
                sessionId,
                logMode: "auto",
                data: logText,
              })
            );
          }
        }
        if (isManualLoggingRef.current) {
          const logText = manualLogSanitizerRef.current.flush();
          if (logText) {
            appendTasks.push(
              invoke("logger_append_to_mode", {
                sessionId,
                logMode: "manual",
                data: logText,
              })
            );
          }
        }
        await Promise.all(appendTasks);
      },
    }),
    [clearBuffer, clearViewport, sessionId]
  );

  // Update decoder when encoding changes
  useEffect(() => {
    decoderRef.current = new TextDecoder(encoding);
  }, [encoding]);

  useEffect(() => {
    autoLogSanitizerRef.current = createTerminalLogSanitizer(
      terminalConfig?.log_format ?? "display"
    );
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
    const protocol = connectionCommands[connectionType];
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
        return true;
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

    // Backend data -> terminal
    const eventPrefix = protocol.dataEvent;
    const errorPrefix = protocol.errorEvent;

    const writeTerminalText = (text: string) => {
      if (!text) return;
      term.write(text, () => {
        decorationController.schedule(term);
      });
      if (onTerminalData) onTerminalData(text);
      if (shouldAppendAutoLog(isAutoLoggingRef.current)) {
        const logText = autoLogSanitizerRef.current.push(text);
        if (logText) {
          invoke("logger_append_to_mode", { sessionId, logMode: "auto", data: logText }).catch(
            () => {}
          );
        }
      }
      if (shouldAppendManualLog(isManualLoggingRef.current, isManualLoggingPausedRef.current)) {
        const logText = manualLogSanitizerRef.current.push(text);
        if (logText) {
          invoke("logger_append_to_mode", { sessionId, logMode: "manual", data: logText }).catch(
            () => {}
          );
        }
      }
    };

    const maxInitialDeltaDrains = 5;
    let bufferedInitialOutput: string[] = [];
    let bufferedInitialOutputSeen = false;
    let initialOutputSyncComplete = false;

    const handleData = (event: { payload: number[] }) => {
      const data = new Uint8Array(event.payload);
      const text = decoderRef.current.decode(data, { stream: true });
      if (!initialOutputSyncComplete) {
        bufferedInitialOutput.push(text);
        bufferedInitialOutputSeen = true;
        return;
      }
      writeTerminalText(text);
    };

    let unlistenData: Promise<() => void> | null = null;
    let unlistenError: Promise<() => void> | null = null;

    const dataListener = listen<number[]>(`${eventPrefix}/${sessionId}`, handleData);
    const errorListener = listen<number[]>(`${errorPrefix}/${sessionId}`, handleData);
    unlistenData = dataListener;
    unlistenError = errorListener;

    Promise.all([dataListener, errorListener])
      .then(async () => {
        if (disposed) return;
        try {
          const snapshot = await invoke<TerminalOutputSnapshot>("terminal_output_snapshot_get", {
            sessionId,
            maxChars: terminalConfig?.scrollback ?? 20000,
          });
          if (disposed) return;

          writeTerminalText(snapshot.output);
          let cursor = snapshot.cursor;
          for (let attempt = 0; attempt < maxInitialDeltaDrains; attempt += 1) {
            bufferedInitialOutputSeen = false;
            const delta = await invoke<TerminalOutputSnapshot>("terminal_output_delta_get", {
              sessionId,
              cursor,
              maxChars: terminalConfig?.scrollback ?? 20000,
            });
            if (disposed) return;

            writeTerminalText(delta.output);
            cursor = delta.cursor;

            if (!bufferedInitialOutputSeen) break;
          }
        } catch {
          if (!disposed) {
            bufferedInitialOutput.forEach((text) => {
              writeTerminalText(text);
            });
          }
        } finally {
          bufferedInitialOutput = [];
          initialOutputSyncComplete = true;
        }
      })
      .catch(() => {
        if (!disposed) {
          bufferedInitialOutput.forEach((text) => writeTerminalText(text));
          bufferedInitialOutput = [];
          initialOutputSyncComplete = true;
        }
      });

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
      void unlistenData?.then((fn) => {
        fn();
      });
      void unlistenError?.then((fn) => {
        fn();
      });
      if (shouldAppendAutoLog(isAutoLoggingRef.current)) {
        const logText = autoLogSanitizerRef.current.flush();
        if (logText) {
          invoke("logger_append_to_mode", { sessionId, logMode: "auto", data: logText }).catch(
            () => {}
          );
        }
      }
      if (isManualLoggingRef.current) {
        const logText = manualLogSanitizerRef.current.flush();
        if (logText) {
          invoke("logger_append_to_mode", { sessionId, logMode: "manual", data: logText }).catch(
            () => {}
          );
        }
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
  }, [sessionId, connectionType, refreshDecorationsAfterClear]);

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

  if (!sessionId) {
    return (
      <div className={`terminal-view ${!isActive ? "terminal-view--hidden" : ""}`}>
        <div className="terminal-view__empty">
          <img className="terminal-view__empty-icon" src={appIcon} alt="" aria-hidden="true" />
          <div className="terminal-view__empty-title">ExaTerm</div>
          <div className="terminal-view__empty-desc">{t("terminal.empty_desc")}</div>
          <button className="btn btn-primary" onClick={onOpenConnection}>
            {t("connection.new")}
          </button>
          <div className="terminal-view__empty-shortcuts">
            {(
              [
                { binding: shortcuts.new_connection, label: t("connection.new") },
                { binding: shortcuts.new_window, label: t("titlebar.menu.new_window") },
                { binding: shortcuts.open_settings, label: t("titlebar.menu.settings") },
              ] satisfies Array<{ binding: ShortcutBinding | null; label: string }>
            ).map(({ binding, label }) => {
              const shortcut = formatShortcut(binding);
              return shortcut ? (
                <div className="terminal-view__shortcut" key={String(label)}>
                  <span className="terminal-view__key">{shortcut}</span>
                  <span>{label}</span>
                </div>
              ) : null;
            })}
          </div>
        </div>
      </div>
    );
  }

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
