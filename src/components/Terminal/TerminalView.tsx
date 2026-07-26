import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import type { IDecoration, IMarker } from "@xterm/xterm";
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
import { findShortcutAction, formatShortcut } from "../../features/shortcuts/shortcutModel";
import { createTerminalLogSanitizer } from "../../utils/logSanitizer";
import { DEFAULT_TERMINAL_MODE } from "../../utils/terminalModes";
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
  isLoggingPaused: boolean;
  terminalConfig?: TerminalConfig;
  shortcuts: ShortcutConfig;
  terminalMode: TerminalMode;
  onOpenConnection: () => void;
  onTerminalData?: (data: string) => void;
}

export interface TerminalViewHandle {
  insertText: (text: string) => void;
  flushManualLogBuffer: () => Promise<void>;
  flushLogBuffersForMove: () => Promise<void>;
}

interface PromptDecorationSet {
  promptSignature: string;
  commandSignature: string;
  marker: IMarker;
  promptDecoration: IDecoration;
  commandDecorations: CommandDecoration[];
}

interface LineDecorationSet {
  signature: string;
  marker: IMarker;
  decoration: IDecoration;
}

interface CommandDecoration {
  decoration: IDecoration;
  marker?: IMarker;
}

interface CommandDecorationSegment {
  lineIndex: number;
  x: number;
  width: number;
  text: string;
}

interface LineRange {
  firstLineIndex: number;
  lastLineIndex: number;
}

interface TerminalOutputSnapshot {
  session_id: string;
  output: string;
  truncated: boolean;
  available_chars: number;
  start_cursor: number;
  cursor: number;
}

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
    isLoggingPaused,
    terminalConfig,
    shortcuts,
    terminalMode,
    onOpenConnection,
    onTerminalData,
  },
  ref
) {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const decoderRef = useRef(new TextDecoder(encoding));
  const isConnectedRef = useRef(isConnected);
  const isAutoLoggingRef = useRef(isAutoLogging);
  const isManualLoggingRef = useRef(isManualLogging);
  const isLoggingPausedRef = useRef(isLoggingPaused);
  const terminalModeRef = useRef(terminalMode);
  const shortcutsRef = useRef(shortcuts);
  const decorationFrameRef = useRef<number | null>(null);
  const decorationRebuildRef = useRef(false);
  const contextMenuActionInProgressRef = useRef(false);
  const promptDecorationsRef = useRef<Map<number, PromptDecorationSet>>(new Map());
  const errorDecorationsRef = useRef<Map<number, LineDecorationSet>>(new Map());
  const autoLogSanitizerRef = useRef(
    createTerminalLogSanitizer(terminalConfig?.log_format ?? "display")
  );

  useEffect(() => {
    shortcutsRef.current = shortcuts;
  }, [shortcuts]);
  const manualLogSanitizerRef = useRef(
    createTerminalLogSanitizer(terminalConfig?.log_format ?? "display")
  );

  const ciscoIosPromptPattern = /^([\w+\-.:/\[\]]+)((?:\([^)]+\)){0,3})([>#]) ?(.*)$/;
  const ciscoIosConfigPromptPattern = /^.+\(config(-.*)?\)#$/;
  const ciscoIosDecorationLookback = 80;
  const ciscoIosErrorPatterns = [
    /ERROR:/i,
    /% ?Bad secret/,
    /(?:^|%) Bad passwords/,
    /invalid input/i,
    /(?:incomplete|ambiguous) command/i,
    /connection timed out/i,
    /[^\r\n]+ not found/,
    /'[^']+' +returned error code: ?\d+/,
    /Bad mask/i,
    /% ?\S+ ?overlaps with ?\S+/i,
    /% ?\S+ ?Error: ?\s+/i,
    /% ?\S+ ?Informational: ?\s+/i,
    /Command authorization failed/,
    /Command Rejected(\s*\([^)]*\))?\s*: ?\s+/i,
    /% General session commands not allowed under the address family/i,
    /% BGP: Error initializing topology/i,
    /%SNMP agent not enabled/i,
    /% Invalid/i,
    /%You must disable VTPv1 and VTPv2 or switch to VTPv3 before configuring a VLAN name longer than 32 characters/i,
  ];
  const canDecorateTerminalMode = (mode: TerminalMode) => {
    return mode === "cisco_ios";
  };

  const decorateTerminalMode = (term: Terminal, mode: TerminalMode) => {
    switch (mode) {
      case "cisco_ios":
        decorateCiscoIosTerminal(term);
        return true;
      case DEFAULT_TERMINAL_MODE:
        return false;
      default:
        return false;
    }
  };

  const cancelScheduledDecoration = () => {
    decorationRebuildRef.current = false;
    if (decorationFrameRef.current === null) return;
    window.cancelAnimationFrame(decorationFrameRef.current);
    decorationFrameRef.current = null;
  };

  const scheduleTerminalModeDecoration = (term: Terminal, rebuild = false) => {
    if (!canDecorateTerminalMode(terminalModeRef.current)) return;
    decorationRebuildRef.current = decorationRebuildRef.current || rebuild;
    if (decorationFrameRef.current !== null) return;

    decorationFrameRef.current = window.requestAnimationFrame(() => {
      decorationFrameRef.current = null;
      if (decorationRebuildRef.current) {
        clearModeDecorations();
        decorationRebuildRef.current = false;
      }
      decorateTerminalMode(term, terminalModeRef.current);
    });
  };

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
    if (isLoggingPaused && !isLoggingPausedRef.current && sessionId) {
      if (isAutoLoggingRef.current) {
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
    }
    isLoggingPausedRef.current = isLoggingPaused;
  }, [isLoggingPaused, sessionId]);

  const disposeCommandDecoration = ({ decoration, marker }: CommandDecoration) => {
    decoration.dispose();
    marker?.dispose();
  };

  const disposePromptDecorationSet = ({
    commandDecorations,
    promptDecoration,
    marker,
  }: PromptDecorationSet) => {
    commandDecorations.forEach(disposeCommandDecoration);
    promptDecoration.dispose();
    marker.dispose();
  };

  const disposeLineDecorationSet = ({ decoration, marker }: LineDecorationSet) => {
    decoration.dispose();
    marker.dispose();
  };

  const clearModeDecorations = () => {
    promptDecorationsRef.current.forEach(disposePromptDecorationSet);
    promptDecorationsRef.current.clear();
    errorDecorationsRef.current.forEach(disposeLineDecorationSet);
    errorDecorationsRef.current.clear();
  };

  const decorateCiscoIosTerminal = (term: Terminal) => {
    decorateCiscoIosPrompt(term);
    decorateCiscoIosErrors(term);
  };

  const getCiscoIosDecorationRanges = (term: Terminal): LineRange[] => {
    const buffer = term.buffer.active;
    if (buffer.type === "alternate") return [];

    const lastBufferLineIndex = Math.max(0, buffer.length - 1);
    const cursorLineIndex = Math.min(lastBufferLineIndex, buffer.baseY + buffer.cursorY);
    const viewportLastLineIndex = Math.min(lastBufferLineIndex, buffer.viewportY + term.rows - 1);
    const ranges: LineRange[] = [];

    const addRange = (firstLineIndex: number, lastLineIndex: number) => {
      const nextRange = {
        firstLineIndex: Math.max(0, firstLineIndex),
        lastLineIndex: Math.min(lastBufferLineIndex, lastLineIndex),
      };
      if (nextRange.lastLineIndex < nextRange.firstLineIndex) return;
      ranges.push(nextRange);
    };

    addRange(cursorLineIndex - ciscoIosDecorationLookback, cursorLineIndex);
    addRange(buffer.viewportY - ciscoIosDecorationLookback, viewportLastLineIndex);

    return ranges
      .sort((a, b) => a.firstLineIndex - b.firstLineIndex)
      .reduce<LineRange[]>((mergedRanges, range) => {
        const previousRange = mergedRanges[mergedRanges.length - 1];
        if (!previousRange || range.firstLineIndex > previousRange.lastLineIndex + 1) {
          mergedRanges.push({ ...range });
          return mergedRanges;
        }

        previousRange.lastLineIndex = Math.max(previousRange.lastLineIndex, range.lastLineIndex);
        return mergedRanges;
      }, []);
  };

  const isLineInRanges = (lineIndex: number, ranges: LineRange[]) =>
    ranges.some((range) => lineIndex >= range.firstLineIndex && lineIndex <= range.lastLineIndex);

  const decorateCiscoIosErrors = (term: Terminal) => {
    const buffer = term.buffer.active;
    if (buffer.type === "alternate") return;

    const cursorLineIndex = buffer.baseY + buffer.cursorY;
    const decorationRanges = getCiscoIosDecorationRanges(term);
    const visitedErrorLineIndexes = new Set<number>();

    decorationRanges.forEach(({ firstLineIndex, lastLineIndex }) => {
      for (let lineIndex = firstLineIndex; lineIndex <= lastLineIndex; lineIndex += 1) {
        const line = buffer.getLine(lineIndex)?.translateToString(true) ?? "";
        const trimmedLine = line.trimEnd();
        if (!trimmedLine || ciscoIosPromptPattern.test(trimmedLine)) continue;
        if (!ciscoIosErrorPatterns.some((pattern) => pattern.test(trimmedLine))) continue;

        const decorationStart = Math.max(0, trimmedLine.search(/\S/));
        const decorationWidth = trimmedLine.length - decorationStart;
        if (decorationWidth <= 0) continue;

        const signature = `${decorationStart}:${decorationWidth}:${trimmedLine}`;
        const existingDecorationSet = errorDecorationsRef.current.get(lineIndex);
        visitedErrorLineIndexes.add(lineIndex);
        if (existingDecorationSet?.signature === signature) continue;
        if (existingDecorationSet) {
          disposeLineDecorationSet(existingDecorationSet);
          errorDecorationsRef.current.delete(lineIndex);
        }

        const marker = term.registerMarker(lineIndex - cursorLineIndex);
        if (!marker) continue;

        const decoration = term.registerDecoration({
          marker,
          x: decorationStart,
          width: decorationWidth,
          foregroundColor: "#f87171",
          layer: "top",
        });

        if (!decoration) {
          marker.dispose();
          continue;
        }

        decoration.onDispose(() => errorDecorationsRef.current.delete(lineIndex));
        errorDecorationsRef.current.set(lineIndex, { signature, marker, decoration });
      }
    });

    errorDecorationsRef.current.forEach((decorationSet, decoratedLineIndex) => {
      if (
        !isLineInRanges(decoratedLineIndex, decorationRanges) ||
        visitedErrorLineIndexes.has(decoratedLineIndex)
      ) {
        return;
      }

      disposeLineDecorationSet(decorationSet);
      errorDecorationsRef.current.delete(decoratedLineIndex);
    });
  };

  const decorateCiscoIosPrompt = (term: Terminal) => {
    const buffer = term.buffer.active;
    if (buffer.type === "alternate") return;

    const cursorLineIndex = buffer.baseY + buffer.cursorY;
    const decorationRanges = getCiscoIosDecorationRanges(term);
    const visitedPromptLineIndexes = new Set<number>();

    decorationRanges.forEach(({ firstLineIndex, lastLineIndex }) => {
      for (let lineIndex = firstLineIndex; lineIndex <= lastLineIndex; lineIndex += 1) {
        const bufferLine = buffer.getLine(lineIndex);
        if (!bufferLine || bufferLine.isWrapped) continue;

        const trimmedLine = bufferLine.translateToString(true).trimEnd();
        const promptMatch = ciscoIosPromptPattern.exec(trimmedLine);
        if (!promptMatch) continue;

        const hostname = promptMatch[1];
        const configMode = promptMatch[2];
        const terminator = promptMatch[3];
        const commandText = promptMatch[4].trimEnd();
        const promptText = `${hostname}${configMode}${terminator}`;
        const isConfigPrompt = ciscoIosConfigPromptPattern.test(promptText);
        const hostnameStart = trimmedLine.indexOf(hostname);
        if (hostnameStart < 0 || hostname.length === 0) continue;
        const promptWidth = promptText.length;
        if (promptWidth === 0) continue;
        const commandStart = hostnameStart + promptMatch[0].length - promptMatch[4].length;
        const commandSegments = collectCiscoIosCommandSegments(
          buffer,
          lineIndex,
          commandStart,
          commandText,
          lastLineIndex
        );
        const promptSignature = `${hostnameStart}:${promptWidth}:${promptText}:${isConfigPrompt}`;
        const commandSignature = commandSegments
          .map(
            (segment) =>
              `${segment.lineIndex - lineIndex}:${segment.x}:${segment.width}:${segment.text}`
          )
          .join("\n");
        const existingDecorationSet = promptDecorationsRef.current.get(lineIndex);
        visitedPromptLineIndexes.add(lineIndex);

        if (existingDecorationSet?.promptSignature === promptSignature) {
          if (existingDecorationSet.commandSignature === commandSignature) continue;

          existingDecorationSet.commandDecorations.forEach(disposeCommandDecoration);
          existingDecorationSet.commandDecorations = registerCiscoIosCommandDecorations(
            term,
            existingDecorationSet.marker,
            lineIndex,
            commandSegments,
            cursorLineIndex
          );
          existingDecorationSet.commandSignature = commandSignature;
          continue;
        }

        if (existingDecorationSet) {
          disposePromptDecorationSet(existingDecorationSet);
          promptDecorationsRef.current.delete(lineIndex);
        }

        const marker = term.registerMarker(lineIndex - cursorLineIndex);
        if (!marker) continue;

        const promptDecoration = term.registerDecoration({
          marker,
          x: hostnameStart,
          width: promptWidth,
          foregroundColor: isConfigPrompt ? "#facc15" : "#7dd3fc",
          backgroundColor: isConfigPrompt ? "#3a2f0a" : "#0f2f3f",
          layer: "top",
        });

        if (!promptDecoration) {
          marker.dispose();
          continue;
        }

        promptDecoration.onRender((element) => {
          element.classList.add("terminal-view__cisco-hostname");
          if (isConfigPrompt) {
            element.classList.add("terminal-view__cisco-hostname--config");
          }
        });

        const commandDecorations = registerCiscoIosCommandDecorations(
          term,
          marker,
          lineIndex,
          commandSegments,
          cursorLineIndex
        );

        promptDecoration.onDispose(() => promptDecorationsRef.current.delete(lineIndex));
        promptDecorationsRef.current.set(lineIndex, {
          promptSignature,
          commandSignature,
          marker,
          promptDecoration,
          commandDecorations,
        });
      }
    });

    promptDecorationsRef.current.forEach((decorationSet, decoratedLineIndex) => {
      if (
        !isLineInRanges(decoratedLineIndex, decorationRanges) ||
        visitedPromptLineIndexes.has(decoratedLineIndex)
      ) {
        return;
      }

      disposePromptDecorationSet(decorationSet);
      promptDecorationsRef.current.delete(decoratedLineIndex);
    });
  };

  const collectCiscoIosCommandSegments = (
    buffer: Terminal["buffer"]["active"],
    promptLineIndex: number,
    commandStart: number,
    commandText: string,
    lastLineIndex: number
  ): CommandDecorationSegment[] => {
    if (buffer.type === "alternate") return [];

    const segments: CommandDecorationSegment[] = [];
    if (commandText.length > 0) {
      segments.push({
        lineIndex: promptLineIndex,
        x: commandStart,
        width: commandText.length,
        text: commandText,
      });
    }

    for (let lineIndex = promptLineIndex + 1; lineIndex <= lastLineIndex; lineIndex += 1) {
      const wrappedLine = buffer.getLine(lineIndex);
      if (!wrappedLine?.isWrapped) break;

      const wrappedText = wrappedLine.translateToString(true).trimEnd();
      if (wrappedText.length > 0) {
        segments.push({
          lineIndex,
          x: 0,
          width: wrappedText.length,
          text: wrappedText,
        });
      }
    }

    return segments;
  };
  const registerCiscoIosCommandDecorations = (
    term: Terminal,
    promptMarker: IMarker,
    promptLineIndex: number,
    segments: CommandDecorationSegment[],
    cursorLineIndex: number
  ): CommandDecoration[] => {
    const commandDecorations: CommandDecoration[] = [];

    segments.forEach((segment) => {
      const isPromptLine = segment.lineIndex === promptLineIndex;
      const marker = isPromptLine
        ? promptMarker
        : term.registerMarker(segment.lineIndex - cursorLineIndex);
      if (!marker) return;

      const decoration = term.registerDecoration({
        marker,
        x: segment.x,
        width: segment.width,
        foregroundColor: "#6ee7b7",
        layer: "top",
      });

      if (!decoration) {
        if (!isPromptLine) marker.dispose();
        return;
      }

      commandDecorations.push({
        decoration,
        marker: isPromptLine ? undefined : marker,
      });
    });

    return commandDecorations;
  };

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
        if (isAutoLoggingRef.current) {
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
    [sessionId]
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
    terminalModeRef.current = terminalMode;
    cancelScheduledDecoration();
    if (terminalMode === DEFAULT_TERMINAL_MODE) {
      clearModeDecorations();
      return;
    }
    if (!canDecorateTerminalMode(terminalMode)) {
      clearModeDecorations();
      return;
    }
    if (termRef.current) {
      decorateTerminalMode(termRef.current, terminalMode);
    }
  }, [terminalMode]);

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

    term.attachCustomKeyEventHandler((e) => {
      return findShortcutAction(shortcutsRef.current, e) === null;
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
      scheduleTerminalModeDecoration(term);
    });
    let disposed = false;

    const handleContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      if (contextMenuActionInProgressRef.current) return;
      contextMenuActionInProgressRef.current = true;

      void (async () => {
        try {
          const selection = term.getSelection();
          if (selection.length > 0) {
            await writeText(selection);
            if (!disposed) {
              term.clearSelection();
            }
            return;
          }

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

          if (!disposed) {
            term.paste(clipboardText);
          }
        } catch {
          // Clipboard failures should not send anything to the terminal.
        } finally {
          contextMenuActionInProgressRef.current = false;
          if (!disposed) {
            term.focus();
          }
        }
      })();
    };

    terminalElement.addEventListener("contextmenu", handleContextMenu);

    // Backend data -> terminal
    const eventPrefix = protocol.dataEvent;
    const errorPrefix = protocol.errorEvent;

    const writeTerminalText = (text: string) => {
      if (!text) return;
      term.write(text, () => {
        scheduleTerminalModeDecoration(term);
      });
      if (onTerminalData) onTerminalData(text);
      if (isAutoLoggingRef.current && !isLoggingPausedRef.current) {
        const logText = autoLogSanitizerRef.current.push(text);
        if (logText) {
          invoke("logger_append_to_mode", { sessionId, logMode: "auto", data: logText }).catch(
            () => {}
          );
        }
      }
      if (isManualLoggingRef.current && !isLoggingPausedRef.current) {
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
      scheduleTerminalModeDecoration(term, true);
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
      if (isAutoLoggingRef.current && !isLoggingPausedRef.current) {
        const logText = autoLogSanitizerRef.current.flush();
        if (logText) {
          invoke("logger_append_to_mode", { sessionId, logMode: "auto", data: logText }).catch(
            () => {}
          );
        }
      }
      if (isManualLoggingRef.current && !isLoggingPausedRef.current) {
        const logText = manualLogSanitizerRef.current.flush();
        if (logText) {
          invoke("logger_append_to_mode", { sessionId, logMode: "manual", data: logText }).catch(
            () => {}
          );
        }
      }
      resizeObserver.disconnect();
      scrollDecorationDisposable.dispose();
      cancelScheduledDecoration();
      clearModeDecorations();
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, [sessionId, connectionType]);

  // Re-fit the terminal whenever this tab becomes active (container goes from display:none to visible)
  useEffect(() => {
    if (isActive && fitRef.current) {
      // Small delay to allow the browser to lay out the now-visible container
      const timer = setTimeout(() => {
        fitRef.current?.fit();
        if (termRef.current) {
          scheduleTerminalModeDecoration(termRef.current, true);
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
          scheduleTerminalModeDecoration(termRef.current, true);
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
                { binding: shortcuts.new_tab, label: t("terminal.new_tab") },
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
    </div>
  );
});

export default TerminalView;
