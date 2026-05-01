import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { SearchAddon } from "@xterm/addon-search";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Monitor } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ConnectionType, Encoding, TerminalConfig } from "../../types";
import { createTerminalLogSanitizer } from "../../utils/logSanitizer";
import "@xterm/xterm/css/xterm.css";
import "./TerminalView.css";

interface TerminalViewProps {
  sessionId: string | null;
  connectionType: ConnectionType;
  isConnected: boolean;
  isActive: boolean;
  encoding: Encoding;
  isLoggingActive: boolean;
  terminalConfig?: TerminalConfig;
  onOpenConnection: () => void;
  onTerminalData?: (data: string) => void;
}

export interface TerminalViewHandle {
  insertText: (text: string) => void;
}

const TerminalView = forwardRef<TerminalViewHandle, TerminalViewProps>(function TerminalView(
  {
    sessionId,
    connectionType,
    isConnected,
    isActive,
    encoding,
    isLoggingActive,
    terminalConfig,
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
  const logSanitizerRef = useRef(
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
    isLoggingActiveRef.current = isLoggingActive;
  }, [isLoggingActive]);

  useImperativeHandle(
    ref,
    () => ({
      insertText: (text: string) => {
        if (!sessionId || !isConnectedRef.current) return;
        const data = text.replace(/\r?\n+$/g, "");
        if (!data) return;
        const protocol = connectionCommands[connectionType];
        invoke(protocol.write, { sessionId, data }).catch(console.error);
        termRef.current?.focus();
      },
    }),
    [sessionId, connectionType]
  );

  // Update decoder when encoding changes
  useEffect(() => {
    decoderRef.current = new TextDecoder(encoding);
  }, [encoding]);

  useEffect(() => {
    logSanitizerRef.current = createTerminalLogSanitizer(terminalConfig?.log_format ?? "display");
  }, [terminalConfig?.log_format]);

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
      term.write(text);
      if (onTerminalData) onTerminalData(text);
      if (isLoggingActiveRef.current) {
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
      if (isLoggingActiveRef.current) {
        const logText = logSanitizerRef.current.flush();
        if (logText) {
          invoke("logger_append", { sessionId, data: logText }).catch(() => {});
        }
      }
      resizeObserver.disconnect();
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
          <div className="terminal-view__empty-icon">
            <Monitor size={32} color="#fff" />
          </div>
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
              <span>{t("sidebar.settings")}</span>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={`terminal-view ${!isActive ? "terminal-view--hidden" : ""}`}>
      <div ref={containerRef} style={{ height: "100%" }} />
    </div>
  );
});

export default TerminalView;
