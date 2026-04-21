import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X } from "lucide-react";
import type { ConnectionType, PortInfo } from "../../types";
import "./ConnectionDialog.css";

interface ConnectionDialogProps {
  onClose: () => void;
  onConnect: (type: ConnectionType, sessionId: string, title: string) => void;
}

export default function ConnectionDialog({ onClose, onConnect }: ConnectionDialogProps) {
  const [tab, setTab] = useState<ConnectionType>("ssh");
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState("");

  // SSH fields
  const [host, setHost] = useState("192.168.1.1");
  const [port, setPort] = useState("22");
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");

  // Serial fields
  const [ports, setPorts] = useState<PortInfo[]>([]);
  const [selectedPort, setSelectedPort] = useState("");
  const [baudRate, setBaudRate] = useState("9600");
  const [dataBits, setDataBits] = useState("8");
  const [parity, setParity] = useState("none");
  const [stopBits, setStopBits] = useState("1");

  useEffect(() => {
    if (tab === "serial") {
      invoke<PortInfo[]>("serial_list_ports").then((p) => {
        setPorts(p);
        if (p.length > 0 && !selectedPort) setSelectedPort(p[0].name);
      }).catch(() => {});
    }
  }, [tab]);

  const handleConnect = async () => {
    setError("");
    setConnecting(true);
    try {
      if (tab === "ssh") {
        const result = await invoke<{ session_id: string }>("ssh_connect", {
          host, port: parseInt(port), username, password, cols: 120, rows: 30,
        });
        await invoke("logger_start", {
          sessionId: result.session_id,
          connectionType: "ssh",
          target: `${username}@${host}:${port}`,
        });
        onConnect("ssh", result.session_id, `${username}@${host}`);
      } else {
        const sessionId = await invoke<string>("serial_connect", {
          port: selectedPort,
          config: {
            baud_rate: parseInt(baudRate),
            data_bits: parseInt(dataBits),
            parity,
            stop_bits: parseInt(stopBits),
            flow_control: "none",
          },
        });
        await invoke("logger_start", {
          sessionId,
          connectionType: "serial",
          target: selectedPort,
        });
        onConnect("serial", sessionId, selectedPort);
      }
    } catch (e: any) {
      setError(typeof e === "string" ? e : e.message || "接続エラー");
      setConnecting(false);
    }
  };

  return (
    <div className="connection-overlay" onClick={onClose}>
      <div className="connection-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="connection-dialog__header">
          <span className="connection-dialog__title">新規接続</span>
          <button className="btn-icon" onClick={onClose}><X size={16} /></button>
        </div>

        <div className="connection-dialog__tabs">
          <button className={`connection-dialog__tab ${tab === "ssh" ? "connection-dialog__tab--active" : ""}`} onClick={() => setTab("ssh")}>SSH</button>
          <button className={`connection-dialog__tab ${tab === "serial" ? "connection-dialog__tab--active" : ""}`} onClick={() => setTab("serial")}>シリアル</button>
        </div>

        <div className="connection-dialog__body">
          {tab === "ssh" ? (
            <>
              <div className="connection-dialog__row">
                <div>
                  <label className="label">ホスト</label>
                  <input className="input" value={host} onChange={(e) => setHost(e.target.value)} placeholder="192.168.1.1" />
                </div>
                <div style={{ maxWidth: 100 }}>
                  <label className="label">ポート</label>
                  <input className="input" type="number" value={port} onChange={(e) => setPort(e.target.value)} />
                </div>
              </div>
              <div>
                <label className="label">ユーザー名</label>
                <input className="input" value={username} onChange={(e) => setUsername(e.target.value)} />
              </div>
              <div>
                <label className="label">パスワード</label>
                <input className="input" type="password" value={password} onChange={(e) => setPassword(e.target.value)} onKeyDown={(e) => e.key === "Enter" && handleConnect()} />
              </div>
            </>
          ) : (
            <>
              <div>
                <label className="label">ポート</label>
                <select className="select" style={{ width: "100%" }} value={selectedPort} onChange={(e) => setSelectedPort(e.target.value)}>
                  {ports.length === 0 && <option value="">ポートが見つかりません</option>}
                  {ports.map((p) => <option key={p.name} value={p.name}>{p.name} ({p.port_type})</option>)}
                </select>
              </div>
              <div className="connection-dialog__row">
                <div>
                  <label className="label">ボーレート</label>
                  <select className="select" style={{ width: "100%" }} value={baudRate} onChange={(e) => setBaudRate(e.target.value)}>
                    {["300","1200","2400","4800","9600","19200","38400","57600","115200"].map((r) => <option key={r} value={r}>{r}</option>)}
                  </select>
                </div>
                <div>
                  <label className="label">データビット</label>
                  <select className="select" style={{ width: "100%" }} value={dataBits} onChange={(e) => setDataBits(e.target.value)}>
                    {["5","6","7","8"].map((d) => <option key={d} value={d}>{d}</option>)}
                  </select>
                </div>
              </div>
              <div className="connection-dialog__row">
                <div>
                  <label className="label">パリティ</label>
                  <select className="select" style={{ width: "100%" }} value={parity} onChange={(e) => setParity(e.target.value)}>
                    <option value="none">なし</option>
                    <option value="odd">奇数</option>
                    <option value="even">偶数</option>
                  </select>
                </div>
                <div>
                  <label className="label">ストップビット</label>
                  <select className="select" style={{ width: "100%" }} value={stopBits} onChange={(e) => setStopBits(e.target.value)}>
                    <option value="1">1</option>
                    <option value="2">2</option>
                  </select>
                </div>
              </div>
            </>
          )}
          {error && <div className="connection-dialog__error">{error}</div>}
        </div>

        <div className="connection-dialog__footer">
          {connecting ? (
            <div className="connection-dialog__connecting">
              <div className="connection-dialog__spinner" />
              接続中...
            </div>
          ) : (
            <>
              <button className="btn btn-ghost" onClick={onClose}>キャンセル</button>
              <button className="btn btn-primary" onClick={handleConnect}>接続</button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
