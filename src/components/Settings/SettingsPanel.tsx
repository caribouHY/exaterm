import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check } from "lucide-react";
import type { AppConfig } from "../../types";
import "./SettingsPanel.css";

export default function SettingsPanel() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<AppConfig>("config_load").then(setConfig).catch(() => {});
  }, []);

  const handleSave = async () => {
    if (!config) return;
    try {
      await invoke("config_save", { config });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      console.error(e);
    }
  };

  if (!config) return <div className="settings-panel"><p>読み込み中...</p></div>;

  const update = (path: string, value: any) => {
    const newConfig = JSON.parse(JSON.stringify(config));
    const keys = path.split(".");
    let obj = newConfig;
    for (let i = 0; i < keys.length - 1; i++) obj = obj[keys[i]];
    obj[keys[keys.length - 1]] = value;
    setConfig(newConfig);
  };

  return (
    <div className="settings-panel">
      <h2>設定</h2>

      <div className="settings-section">
        <div className="settings-section__title">AI プロバイダ設定</div>
        <div className="settings-row">
          <div>
            <label className="label">デフォルトプロバイダ</label>
            <select className="select" value={config.ai.default_provider} onChange={(e) => update("ai.default_provider", e.target.value)}>
              <option value="OpenAi">OpenAI</option>
              <option value="Anthropic">Anthropic</option>
              <option value="Gemini">Google Gemini</option>
              <option value="Ollama">Ollama</option>
            </select>
          </div>
        </div>
        <div style={{ marginBottom: 14 }}>
          <label className="label">OpenAI API キー</label>
          <input className="input" type="password" value={config.ai.openai_api_key} onChange={(e) => update("ai.openai_api_key", e.target.value)} placeholder="sk-..." />
        </div>
        <div style={{ marginBottom: 14 }}>
          <label className="label">Anthropic API キー</label>
          <input className="input" type="password" value={config.ai.anthropic_api_key} onChange={(e) => update("ai.anthropic_api_key", e.target.value)} placeholder="sk-ant-..." />
        </div>
        <div style={{ marginBottom: 14 }}>
          <label className="label">Google Gemini API キー</label>
          <input className="input" type="password" value={config.ai.gemini_api_key} onChange={(e) => update("ai.gemini_api_key", e.target.value)} placeholder="AIza..." />
        </div>
        <div style={{ marginBottom: 14 }}>
          <label className="label">Ollama ベース URL</label>
          <input className="input" type="text" value={config.ai.ollama_base_url || "http://localhost:11434"} onChange={(e) => update("ai.ollama_base_url", e.target.value)} placeholder="http://localhost:11434" />
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-section__title">ターミナル設定</div>
        <div className="settings-row">
          <div>
            <label className="label">フォントサイズ</label>
            <input className="input" type="number" value={config.terminal.font_size} onChange={(e) => update("terminal.font_size", parseInt(e.target.value))} min={8} max={32} />
          </div>
          <div>
            <label className="label">スクロールバック行数</label>
            <input className="input" type="number" value={config.terminal.scrollback} onChange={(e) => update("terminal.scrollback", parseInt(e.target.value))} />
          </div>
        </div>
        <div>
          <label className="label">フォントファミリー</label>
          <input className="input" value={config.terminal.font_family} onChange={(e) => update("terminal.font_family", e.target.value)} />
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-section__title">ログ設定</div>
        <div className="settings-toggle-row">
          <div className="settings-toggle-label">
            <span>自動セッションログ</span>
            <small>接続時にターミナルの入出力をファイルへ自動記録します</small>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={config.terminal.auto_session_log}
              onChange={(e) => update("terminal.auto_session_log", e.target.checked)}
            />
            <span className="toggle-track" />
          </label>
        </div>
      </div>

      <div className="settings-actions">
        <button className="btn btn-primary" onClick={handleSave}>保存</button>
        {saved && <span className="settings-saved"><Check size={14} /> 保存しました</span>}
      </div>
    </div>
  );
}
