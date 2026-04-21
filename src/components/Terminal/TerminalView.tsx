import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { SearchAddon } from "@xterm/addon-search";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Monitor } from "lucide-react";
import "@xterm/xterm/css/xterm.css";
import "./TerminalView.css";

interface TerminalViewProps {
  sessionId: string | null;
  connectionType: "ssh" | "serial";
  isConnected: boolean;
  isActive: boolean;
  onOpenConnection: () => void;
  onTerminalData?: (data: string) => void;
}

export default function TerminalView({
  sessionId, connectionType, isConnected, isActive, onOpenConnection, onTerminalData,
}: TerminalViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);

  // Create the terminal once when session is connected — never tear it down on tab switch
  useEffect(() => {
    if (!containerRef.current || !sessionId || !isConnected) return;

    const term = new Terminal({
      fontFamily: "'JetBrains Mono', Consolas, 'Courier New', monospace",
      fontSize: 14,
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
      cursorStyle: "block",
      scrollback: 10000,
      allowProposedApi: true,
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
    const writeCmd = connectionType === "ssh" ? "ssh_write" : "serial_write";
    term.onData((data) => {
      invoke(writeCmd, { sessionId, data }).catch(console.error);
    });

    // 設定を読み込み、自動ログが有効な場合のみロギングを開始
    let loggingEnabled = true; // デフォルトは有効
    invoke<{ terminal: { auto_session_log: boolean } }>("config_load")
      .then((cfg) => { loggingEnabled = cfg.terminal.auto_session_log; })
      .catch(() => {}); // 取得失敗時はデフォルト(true)のまま

    // Backend data -> terminal
    const eventPrefix = connectionType === "ssh" ? "ssh://data" : "serial://data";
    const unlistenData = listen<string>(`${eventPrefix}/${sessionId}`, (event) => {
      term.write(event.payload);
      if (onTerminalData) onTerminalData(event.payload);
      // 自動ログが有効なときのみ書き込み
      if (loggingEnabled) {
        invoke("logger_append", { sessionId, data: event.payload }).catch(() => {});
      }
    });

    // Resize handling
    const resizeCmd = connectionType === "ssh" ? "ssh_resize" : null;
    const handleResize = () => {
      fitAddon.fit();
      if (resizeCmd && sessionId) {
        invoke(resizeCmd, { sessionId, cols: term.cols, rows: term.rows }).catch(() => {});
      }
    };

    const resizeObserver = new ResizeObserver(handleResize);
    resizeObserver.observe(containerRef.current);

    return () => {
      unlistenData.then((fn) => fn());
      resizeObserver.disconnect();
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, [sessionId, isConnected, connectionType]);

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

  if (!sessionId || !isConnected) {
    return (
      <div className={`terminal-view ${!isActive ? "terminal-view--hidden" : ""}`}>
        <div className="terminal-view__empty">
          <div className="terminal-view__empty-icon">
            <Monitor size={32} color="#fff" />
          </div>
          <div className="terminal-view__empty-title">ExaTerm</div>
          <div className="terminal-view__empty-desc">
            接続を開始してターミナルセッションを開きます
          </div>
          <button className="btn btn-primary" onClick={onOpenConnection}>
            新規接続
          </button>
          <div className="terminal-view__empty-shortcuts">
            <div className="terminal-view__shortcut">
              <span className="terminal-view__key">Ctrl+N</span>
              <span>新規接続</span>
            </div>
            <div className="terminal-view__shortcut">
              <span className="terminal-view__key">Ctrl+T</span>
              <span>新規タブ</span>
            </div>
            <div className="terminal-view__shortcut">
              <span className="terminal-view__key">Ctrl+,</span>
              <span>設定</span>
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
}
