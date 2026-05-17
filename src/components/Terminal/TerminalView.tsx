import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import type { IDecoration, IMarker } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { SearchAddon } from "@xterm/addon-search";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import type { ConnectionType, Encoding, TerminalConfig, TerminalMode } from "../../types";
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
  terminalMode: TerminalMode;
  onOpenConnection: () => void;
  onTerminalData?: (data: string) => void;
}

export interface TerminalViewHandle {
  insertText: (text: string) => void;
  flushManualLogBuffer: () => Promise<void>;
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

interface TerminalOutputSnapshot {
  output: string;
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
  const decorationFrameRef = useRef<number | null>(null);
  const promptDecorationsRef = useRef<Map<number, PromptDecorationSet>>(new Map());
  const errorDecorationsRef = useRef<Map<number, LineDecorationSet>>(new Map());
  const autoLogSanitizerRef = useRef(
    createTerminalLogSanitizer(terminalConfig?.log_format ?? "display")
  );
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
  const terminalModeDecorators: Partial<Record<TerminalMode, (term: Terminal) => void>> = {
    cisco_ios: (term) => decorateCiscoIosTerminal(term),
  };

  const cancelScheduledDecoration = () => {
    if (decorationFrameRef.current === null) return;
    window.cancelAnimationFrame(decorationFrameRef.current);
    decorationFrameRef.current = null;
  };

  const scheduleTerminalModeDecoration = (term: Terminal) => {
    if (terminalModeRef.current === DEFAULT_TERMINAL_MODE) return;
    if (decorationFrameRef.current !== null) return;

    decorationFrameRef.current = window.requestAnimationFrame(() => {
      decorationFrameRef.current = null;
      terminalModeDecorators[terminalModeRef.current]?.(term);
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

  const decorateCiscoIosErrors = (term: Terminal) => {
    const buffer = term.buffer.active;
    if (buffer.type === "alternate") return;

    const cursorLineIndex = buffer.baseY + buffer.cursorY;
    const firstLineIndex = Math.max(0, cursorLineIndex - ciscoIosDecorationLookback);

    for (let lineIndex = firstLineIndex; lineIndex <= cursorLineIndex; lineIndex += 1) {
      const line = buffer.getLine(lineIndex)?.translateToString(true) ?? "";
      const trimmedLine = line.trimEnd();
      if (!trimmedLine || ciscoIosPromptPattern.test(trimmedLine)) continue;
      if (!ciscoIosErrorPatterns.some((pattern) => pattern.test(trimmedLine))) continue;

      const decorationStart = Math.max(0, trimmedLine.search(/\S/));
      const decorationWidth = trimmedLine.length - decorationStart;
      if (decorationWidth <= 0) continue;

      const signature = `${decorationStart}:${decorationWidth}:${trimmedLine}`;
      const existingDecorationSet = errorDecorationsRef.current.get(lineIndex);
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
  };

  const decorateCiscoIosPrompt = (term: Terminal) => {
    const buffer = term.buffer.active;
    if (buffer.type === "alternate") return;

    const cursorLineIndex = buffer.baseY + buffer.cursorY;
    const firstLineIndex = Math.max(0, cursorLineIndex - ciscoIosDecorationLookback);
    const visitedPromptLineIndexes = new Set<number>();

    for (let lineIndex = firstLineIndex; lineIndex <= cursorLineIndex; lineIndex += 1) {
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
        cursorLineIndex
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

    promptDecorationsRef.current.forEach((decorationSet, decoratedLineIndex) => {
      if (
        decoratedLineIndex < firstLineIndex ||
        decoratedLineIndex > cursorLineIndex ||
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
    cursorLineIndex: number
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

    for (let lineIndex = promptLineIndex + 1; lineIndex <= cursorLineIndex; lineIndex += 1) {
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
    const decorator = terminalModeDecorators[terminalMode];
    if (!decorator) {
      clearModeDecorations();
      return;
    }
    if (termRef.current) {
      decorator(termRef.current);
    }
  }, [terminalMode]);

  // Create the terminal once per session and keep it mounted after disconnect.
  useEffect(() => {
    if (!containerRef.current || !sessionId || termRef.current) return;

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
      cursorStyle: (terminalConfig?.cursor_style as any) || "block",
      scrollback: terminalConfig?.scrollback || 10000,
      allowProposedApi: true,
    });

    term.attachCustomKeyEventHandler((e) => {
      if (e.ctrlKey || e.metaKey) {
        const key = e.key.toLowerCase();
        if (key === "n" || key === "t" || key === ",") {
          return false;
        }
      }
      return true;
    });

    const fitAddon = new FitAddon();
    const webLinksAddon = new WebLinksAddon();
    const searchAddon = new SearchAddon();

    term.loadAddon(fitAddon);
    term.loadAddon(webLinksAddon);
    term.loadAddon(searchAddon);

    term.open(containerRef.current);
    fitAddon.fit();

    termRef.current = term;
    fitRef.current = fitAddon;

    // Terminal input -> backend
    const protocol = connectionCommands[connectionType];
    term.onData((data) => {
      if (!isConnectedRef.current) return;
      invoke(protocol.write, { sessionId, data }).catch(console.error);
    });

    // Backend data -> terminal
    const eventPrefix = protocol.dataEvent;
    const errorPrefix = protocol.errorEvent;

    const writeTerminalText = (text: string) => {
      if (!text) return;
      term.write(text, () => scheduleTerminalModeDecoration(term));
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

    const handleData = (event: { payload: number[] }) => {
      const data = new Uint8Array(event.payload);
      const text = decoderRef.current.decode(data, { stream: true });
      writeTerminalText(text);
    };

    let disposed = false;
    let unlistenData: Promise<() => void> | null = null;
    let unlistenError: Promise<() => void> | null = null;

    invoke<TerminalOutputSnapshot>("terminal_output_snapshot_get", {
      sessionId,
      maxChars: terminalConfig?.scrollback ?? 20000,
    })
      .then((snapshot) => {
        if (!disposed) {
          writeTerminalText(snapshot.output);
        }
      })
      .catch(() => {})
      .finally(() => {
        if (disposed) return;
        unlistenData = listen<number[]>(`${eventPrefix}/${sessionId}`, handleData);
        unlistenError = listen<number[]>(`${errorPrefix}/${sessionId}`, handleData);
      });

    // Resize handling
    const resizeCmd = protocol.resize;
    const handleResize = () => {
      fitAddon.fit();
      if (resizeCmd && sessionId && isConnectedRef.current) {
        invoke(resizeCmd, { sessionId, cols: term.cols, rows: term.rows }).catch(() => {});
      }
    };

    const resizeObserver = new ResizeObserver(handleResize);
    resizeObserver.observe(containerRef.current);

    return () => {
      disposed = true;
      unlistenData?.then((fn) => fn());
      unlistenError?.then((fn) => fn());
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
        termRef.current?.focus();
      }, 50);
      return () => clearTimeout(timer);
    }
  }, [isActive]);

  // Update terminal options when config changes
  useEffect(() => {
    if (termRef.current && terminalConfig) {
      termRef.current.options.fontSize = terminalConfig.font_size;
      termRef.current.options.fontFamily = terminalConfig.font_family;
      termRef.current.options.cursorStyle = terminalConfig.cursor_style as any;
      termRef.current.options.scrollback = terminalConfig.scrollback;

      // Re-fit to adjust for potential size changes
      setTimeout(() => {
        fitRef.current?.fit();
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
            <div className="terminal-view__shortcut">
              <span className="terminal-view__key">Ctrl+N</span>
              <span>{t("connection.new")}</span>
            </div>
            <div className="terminal-view__shortcut">
              <span className="terminal-view__key">Ctrl+T</span>
              <span>{t("terminal.new_tab")}</span>
            </div>
            <div className="terminal-view__shortcut">
              <span className="terminal-view__key">Ctrl+,</span>
              <span>{t("titlebar.menu.settings")}</span>
            </div>
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
