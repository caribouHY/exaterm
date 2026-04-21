import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Bot, Send, Terminal, X } from "lucide-react";
import type { ChatMessage, AiModelInfo } from "../../types";
import "./AIChatPanel.css";

interface AIChatPanelProps {
  onClose: () => void;
  terminalBuffer: string;
}

export default function AIChatPanel({ onClose, terminalBuffer }: AIChatPanelProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [models, setModels] = useState<AiModelInfo[]>([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [selectedProvider, setSelectedProvider] = useState("OpenAi");
  const [useContext, setUseContext] = useState(false);
  const [apiKeys, setApiKeys] = useState({ openai: "", anthropic: "", gemini: "" });
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<AiModelInfo[]>("ai_get_models").then((m) => {
      setModels(m);
      if (m.length > 0) setSelectedModel(m[0].model_id);
    });
    invoke<any>("config_load").then((cfg) => {
      setApiKeys({
        openai: cfg.ai.openai_api_key || "",
        anthropic: cfg.ai.anthropic_api_key || "",
        gemini: cfg.ai.gemini_api_key || "",
      });
      if (cfg.ai.default_provider) setSelectedProvider(cfg.ai.default_provider);
      if (cfg.ai.default_model) setSelectedModel(cfg.ai.default_model);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const providerModels = models.filter((m) => m.provider === selectedProvider);
  const currentApiKey = selectedProvider === "OpenAi" ? apiKeys.openai
    : selectedProvider === "Anthropic" ? apiKeys.anthropic : apiKeys.gemini;

  const handleSend = async () => {
    if (!input.trim() || loading) return;
    const userMsg: ChatMessage = { role: "user", content: input.trim() };
    const newMessages = [...messages, userMsg];
    setMessages(newMessages);
    setInput("");
    setLoading(true);

    try {
      const response = await invoke<string>("ai_chat", {
        provider: selectedProvider,
        model: selectedModel,
        messages: newMessages,
        terminalContext: useContext ? terminalBuffer : null,
        apiKey: currentApiKey,
      });
      setMessages([...newMessages, { role: "assistant", content: response }]);
    } catch (e: any) {
      setMessages([...newMessages, {
        role: "assistant",
        content: `エラー: ${typeof e === "string" ? e : e.message || "不明なエラー"}\n\nAPIキーが設定されているか確認してください（設定 > AI設定）`,
      }]);
    }
    setLoading(false);
  };

  return (
    <div className="ai-panel">
      <div className="ai-panel__header">
        <span className="ai-panel__title">AI アシスタント</span>
        <div className="ai-panel__provider">
          <select value={selectedProvider} onChange={(e) => {
            setSelectedProvider(e.target.value);
            const pm = models.filter((m) => m.provider === e.target.value);
            if (pm.length > 0) setSelectedModel(pm[0].model_id);
          }}>
            <option value="OpenAi">OpenAI</option>
            <option value="Anthropic">Anthropic</option>
            <option value="Gemini">Gemini</option>
          </select>
          <select value={selectedModel} onChange={(e) => setSelectedModel(e.target.value)}>
            {providerModels.map((m) => (
              <option key={m.model_id} value={m.model_id}>{m.display_name}</option>
            ))}
          </select>
        </div>
        <button className="btn-icon" onClick={onClose}><X size={14} /></button>
      </div>

      <div className="ai-panel__messages">
        {messages.length === 0 ? (
          <div className="ai-panel__welcome">
            <div className="ai-panel__welcome-icon"><Bot size={24} color="#fff" /></div>
            <div className="ai-panel__welcome-text">
              AIアシスタントに質問できます。<br />
              ターミナル出力をコンテキストとして送信することも可能です。
            </div>
          </div>
        ) : (
          messages.map((msg, i) => (
            <div key={i} className={`ai-message ai-message--${msg.role}`}>
              <span className="ai-message__role">{msg.role === "user" ? "あなた" : "AI"}</span>
              <div className="ai-message__content">{msg.content}</div>
            </div>
          ))
        )}
        {loading && (
          <div className="ai-panel__loading">
            <span className="ai-panel__loading-dot" />
            <span className="ai-panel__loading-dot" />
            <span className="ai-panel__loading-dot" />
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      <div className="ai-panel__input-area">
        <button
          className={`ai-panel__context-btn ${useContext ? "ai-panel__context-btn--active" : ""}`}
          onClick={() => setUseContext(!useContext)}
        >
          <Terminal size={12} />
          {useContext ? "ターミナル出力を含む ✓" : "ターミナル出力を含む"}
        </button>
        <div className="ai-panel__input-row">
          <textarea
            className="ai-panel__input"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); } }}
            placeholder="メッセージを入力..."
            rows={1}
          />
          <button className="ai-panel__send" onClick={handleSend} disabled={loading || !input.trim()}>
            <Send size={16} />
          </button>
        </div>
      </div>
    </div>
  );
}
