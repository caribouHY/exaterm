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
  isLoggingActive: boolean;
  isLoggingPaused: boolean;
  terminalConfig?: TerminalConfig;
  terminalMode: TerminalMode;
  onOpenConnection: () => void;
  onTerminalData?: (data: string) => void;
}

export interface TerminalViewHandle {
  insertText: (text: string) => void;
}

interface PromptDecorationSet {
  promptSignature: string;
  commandSignature: string;
  marker: IMarker;
  promptDecoration: IDecoration;
  commandDecoration?: IDecoration;
}

interface LineDecorationSet {
  signature: string;
  marker: IMarker;
  decoration: IDecoration;
}

const TerminalView = forwardRef<TerminalViewHandle, TerminalViewProps>(function TerminalView(
  {
    sessionId,
    connectionType,
    isConnected,
    isActive,
    encoding,
    isLoggingActive,
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
  const isLoggingActiveRef = useRef(isLoggingActive);
  const isLoggingPausedRef = useRef(isLoggingPaused);
  const terminalModeRef = useRef(terminalMode);
  const promptDecorationsRef = useRef<Map<number, PromptDecorationSet>>(new Map());
  const errorDecorationsRef = useRef<Map<number, LineDecorationSet>>(new Map());
  const logSanitizerRef = useRef(
    createTerminalLogSanitizer(terminalConfig?.log_format ?? "display")
  );

  const ciscoIosPromptPattern = /^([\w+\-.:/\[\]]+)((?:\([^)]+\)){0,3})([>#]) ?(.*)$/;
  const ciscoIosConfigPromptPattern = /^.+\(config(-.*)?\)#$/;
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
    if (isLoggingActiveRef.current && !isLoggingActive && sessionId) {
      const logText = logSanitizerRef.current.flush();
      if (logText) {
        invoke("logger_append", { sessionId, data: logText }).catch(() => {});
      }
    }
    isLoggingActiveRef.current = isLoggingActive;
  }, [isLoggingActive, sessionId]);

  useEffect(() => {
    if (isLoggingActiveRef.current && !isLoggingPausedRef.current && isLoggingPaused && sessionId) {
      const logText = logSanitizerRef.current.flush();
      if (logText) {
        invoke("logger_append", { sessionId, data: logText }).catch(() => {});
      }
    }
    isLoggingPausedRef.current = isLoggingPaused;
  }, [isLoggingPaused, sessionId]);

  const disposePromptDecorationSet = ({
    commandDecoration,
    promptDecoration,
    marker,
  }: PromptDecorationSet) => {
    commandDecoration?.dispose();
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
    const firstLineIndex = Math.max(0, cursorLineIndex - 80);

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

    const lineIndex = buffer.baseY + buffer.cursorY;
    const line = buffer.getLine(lineIndex)?.translateToString(true) ?? "";
    const trimmedLine = line.trimEnd();
    const promptMatch = ciscoIosPromptPattern.exec(trimmedLine);
    if (!promptMatch) return;

    const hostname = promptMatch[1];
    const configMode = promptMatch[2];
    const terminator = promptMatch[3];
    const commandText = promptMatch[4].trimEnd();
    const promptText = `${hostname}${configMode}${terminator}`;
    const isConfigPrompt = ciscoIosConfigPromptPattern.test(promptText);
    const hostnameStart = trimmedLine.indexOf(hostname);
    if (hostnameStart < 0 || hostname.length === 0) return;
    const promptWidth = promptText.length;
    if (promptWidth === 0) return;
    const commandStart = hostnameStart + promptMatch[0].length - promptMatch[4].length;
    const commandWidth = commandText.length;
    const promptSignature = `${hostnameStart}:${promptWidth}:${isConfigPrompt}`;
    const commandSignature = `${commandStart}:${commandText}`;
    const existingDecorationSet = promptDecorationsRef.current.get(lineIndex);
    if (existingDecorationSet?.promptSignature === promptSignature) {
      if (existingDecorationSet.commandSignature === commandSignature) return;

      existingDecorationSet.commandDecoration?.dispose();
      existingDecorationSet.commandDecoration = undefined;
      existingDecorationSet.commandSignature = commandSignature;

      if (commandWidth > 0) {
        existingDecorationSet.commandDecoration = term.registerDecoration({
          marker: existingDecorationSet.marker,
          x: commandStart,
          width: commandWidth,
          foregroundColor: "#6ee7b7",
          layer: "top",
        });
      }
      return;
    }

    if (existingDecorationSet) {
      disposePromptDecorationSet(existingDecorationSet);
      promptDecorationsRef.current.delete(lineIndex);
    }

    const marker = term.registerMarker(0);
    if (!marker) return;

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
      return;
    }

    promptDecoration.onRender((element) => {
      element.classList.add("terminal-view__cisco-hostname");
      if (isConfigPrompt) {
        element.classList.add("terminal-view__cisco-hostname--config");
      }
    });

    let commandDecoration: IDecoration | undefined;
    if (commandWidth > 0) {
      commandDecoration = term.registerDecoration({
        marker,
        x: commandStart,
        width: commandWidth,
        foregroundColor: "#6ee7b7",
        layer: "top",
      });
    }

    promptDecoration.onDispose(() => promptDecorationsRef.current.delete(lineIndex));
    promptDecorationsRef.current.set(lineIndex, {
      promptSignature,
      commandSignature,
      marker,
      promptDecoration,
      commandDecoration,
    });
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
    }),
    [sessionId]
  );

  // Update decoder when encoding changes
  useEffect(() => {
    decoderRef.current = new TextDecoder(encoding);
  }, [encoding]);

  useEffect(() => {
    logSanitizerRef.current = createTerminalLogSanitizer(terminalConfig?.log_format ?? "display");
  }, [terminalConfig?.log_format]);

  useEffect(() => {
    terminalModeRef.current = terminalMode;
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

    const handleData = (event: { payload: number[] }) => {
      const data = new Uint8Array(event.payload);
      const text = decoderRef.current.decode(data, { stream: true });
      term.write(text, () => terminalModeDecorators[terminalModeRef.current]?.(term));
      if (onTerminalData) onTerminalData(text);
      if (isLoggingActiveRef.current && !isLoggingPausedRef.current) {
        const logText = logSanitizerRef.current.push(text);
        if (logText) {
          invoke("logger_append", { sessionId, data: logText }).catch(() => {});
        }
      }
    };

    const unlistenData = listen<number[]>(`${eventPrefix}/${sessionId}`, handleData);
    const unlistenError = listen<number[]>(`${errorPrefix}/${sessionId}`, handleData);

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
      unlistenData.then((fn) => fn());
      unlistenError.then((fn) => fn());
      if (isLoggingActiveRef.current && !isLoggingPausedRef.current) {
        const logText = logSanitizerRef.current.flush();
        if (logText) {
          invoke("logger_append", { sessionId, data: logText }).catch(() => {});
        }
      }
      resizeObserver.disconnect();
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
