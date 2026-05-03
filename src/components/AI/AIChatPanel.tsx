import { useState, useRef, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ClipboardPaste, Send, Terminal, X } from "lucide-react";
import type { AppConfig, ChatMessage, AiModelInfo } from "../../types";
import { useTranslation } from "react-i18next";
import AIAssistantLogo from "./AIAssistantLogo";
import "./AIChatPanel.css";

type CloudProviderId = "OpenAi" | "AzureOpenAi" | "Anthropic" | "Gemini";
type ProviderId = CloudProviderId | "Ollama";

interface AiSecretStatus {
  openai: boolean;
  azure_openai: boolean;
  anthropic: boolean;
  gemini: boolean;
}

interface AIChatPanelProps {
  onClose: () => void;
  terminalBuffer: React.MutableRefObject<string>;
  messages: ChatMessage[];
  setMessages: React.Dispatch<React.SetStateAction<ChatMessage[]>>;
  selectedProvider: string;
  setSelectedProvider: React.Dispatch<React.SetStateAction<string>>;
  selectedModel: string;
  setSelectedModel: React.Dispatch<React.SetStateAction<string>>;
  onInsertCommand: (command: string) => void;
  canInsertCommand: boolean;
}

const PROVIDERS: Array<{ id: ProviderId; label: string; secretKey?: keyof AiSecretStatus }> = [
  { id: "OpenAi", label: "OpenAI", secretKey: "openai" },
  { id: "AzureOpenAi", label: "Azure OpenAI", secretKey: "azure_openai" },
  { id: "Anthropic", label: "Anthropic", secretKey: "anthropic" },
  { id: "Gemini", label: "Gemini", secretKey: "gemini" },
  { id: "Ollama", label: "Ollama" },
];

const COMMAND_BLOCK_LANGUAGES = new Set([
  "bash",
  "sh",
  "powershell",
  "ps1",
  "cmd",
  "bat",
  "terminal",
  "console",
]);

interface CommandSuggestion {
  language: string;
  command: string;
}

type AssistantContentSegment =
  | { type: "text"; content: string }
  | { type: "command"; suggestion: CommandSuggestion };

function parseAssistantContent(content: string): AssistantContentSegment[] {
  const segments: AssistantContentSegment[] = [];
  const fencePattern = /```([^\n`]*)\n([\s\S]*?)```/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = fencePattern.exec(content)) !== null) {
    const [block, rawLanguage, code] = match;
    const language = rawLanguage.trim().split(/\s+/)[0].toLowerCase();
    const leadingText = content.slice(lastIndex, match.index).trim();

    if (leadingText) {
      segments.push({ type: "text", content: leadingText });
    }

    if (COMMAND_BLOCK_LANGUAGES.has(language)) {
      const command = code.replace(/\r?\n+$/g, "");
      if (command.trim()) {
        segments.push({ type: "command", suggestion: { language, command } });
      }
    } else {
      segments.push({ type: "text", content: block });
    }

    lastIndex = match.index + block.length;
  }

  const trailingText = content.slice(lastIndex).trim();
  if (trailingText) {
    segments.push({ type: "text", content: trailingText });
  }

  return segments;
}

export default function AIChatPanel({
  onClose,
  terminalBuffer,
  messages,
  setMessages,
  selectedProvider,
  setSelectedProvider,
  selectedModel,
  setSelectedModel,
  onInsertCommand,
  canInsertCommand,
}: AIChatPanelProps) {
  const { t, i18n } = useTranslation();
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [models, setModels] = useState<AiModelInfo[]>([]);
  const [useContext, setUseContext] = useState(false);
  const [ollamaBaseUrl, setOllamaBaseUrl] = useState("http://localhost:11434");
  const [azureOpenAiEndpoint, setAzureOpenAiEndpoint] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;

    const loadAiSettings = async () => {
      let cloudModels: AiModelInfo[] = [];
      let cfg: AppConfig | null = null;
      let secretStatus: AiSecretStatus = {
        openai: false,
        azure_openai: false,
        anthropic: false,
        gemini: false,
      };

      try {
        secretStatus = await invoke<AiSecretStatus>("ai_secret_status");
      } catch (e) {
        console.error("AI secret status fetch failed:", e);
      }

      try {
        cloudModels = await invoke<AiModelInfo[]>("ai_get_models");
      } catch (e) {
        console.error("AI models fetch failed:", e);
      }

      try {
        cfg = await invoke<AppConfig>("config_load");
      } catch (e) {
        console.error("Config load failed:", e);
      }

      const ollamaUrl = cfg?.ai?.ollama_base_url || "http://localhost:11434";
      const azureEndpoint = cfg?.ai?.azure_openai_endpoint?.trim() || "";
      const azureDeployment = cfg?.ai?.azure_openai_deployment?.trim() || "";
      const enabledProviders = PROVIDERS.filter((provider) =>
        provider.id === "Ollama"
          ? Boolean(cfg?.ai?.ollama_enabled)
          : provider.id === "AzureOpenAi"
            ? Boolean(
                cfg?.ai?.azure_openai_enabled &&
                secretStatus.azure_openai &&
                azureEndpoint &&
                azureDeployment
              )
            : secretStatus[provider.secretKey!]
      );
      let nextModels = cloudModels.filter((model) =>
        enabledProviders.some((provider) => provider.id === model.provider)
      );

      if (
        cfg?.ai?.azure_openai_enabled &&
        secretStatus.azure_openai &&
        azureEndpoint &&
        azureDeployment
      ) {
        nextModels = [
          ...nextModels.filter((m) => m.provider !== "AzureOpenAi"),
          {
            provider: "AzureOpenAi",
            model_id: azureDeployment,
            display_name: azureDeployment,
          },
        ];
      }

      if (cfg?.ai?.ollama_enabled) {
        try {
          const ollamaModels = await invoke<AiModelInfo[]>("ai_get_ollama_models", {
            baseUrl: ollamaUrl,
          });
          nextModels = [...nextModels.filter((m) => m.provider !== "Ollama"), ...ollamaModels];
        } catch (e) {
          console.error("Ollama models fetch failed:", e);
        }
      }

      if (cancelled) return;

      const savedProvider = cfg?.ai?.default_provider || "OpenAi";
      const rememberedProviderModels = nextModels.filter((m) => m.provider === selectedProvider);
      const hasRememberedModel = rememberedProviderModels.some((m) => m.model_id === selectedModel);
      const hasSavedProviderModels = nextModels.some((model) => model.provider === savedProvider);
      const nextProvider =
        selectedProvider && selectedModel && hasRememberedModel
          ? selectedProvider
          : enabledProviders.some((provider) => provider.id === savedProvider) &&
              hasSavedProviderModels
            ? savedProvider
            : nextModels[0]?.provider || "";
      const providerModels = nextModels.filter((m) => m.provider === nextProvider);
      const savedModel = cfg?.ai?.default_model || "";
      const nextModel =
        selectedProvider && selectedModel && hasRememberedModel
          ? selectedModel
          : providerModels.some((m) => m.model_id === savedModel)
            ? savedModel
            : providerModels[0]?.model_id || nextModels[0]?.model_id || "";

      setModels(nextModels);
      setOllamaBaseUrl(ollamaUrl);
      setAzureOpenAiEndpoint(azureEndpoint);
      setSelectedProvider(nextProvider);
      setSelectedModel(nextModel);
    };

    loadAiSettings();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const providerModels = models.filter((m) => m.provider === selectedProvider);
  const visibleProviders = useMemo(
    () => PROVIDERS.filter((provider) => models.some((model) => model.provider === provider.id)),
    [models]
  );

  useEffect(() => {
    if (
      visibleProviders.length > 0 &&
      !visibleProviders.some((provider) => provider.id === selectedProvider)
    ) {
      setSelectedProvider(visibleProviders[0].id);
      return;
    }

    const availableModels = models.filter((m) => m.provider === selectedProvider);
    if (availableModels.length === 0) {
      if (selectedModel) setSelectedModel("");
      return;
    }
    if (!availableModels.some((m) => m.model_id === selectedModel)) {
      setSelectedModel(availableModels[0].model_id);
    }
  }, [models, selectedProvider, selectedModel, visibleProviders]);

  const handleSend = async () => {
    if (!input.trim() || loading || !selectedModel) return;
    const userMsg: ChatMessage = { role: "user", content: input.trim() };
    const newMessages = [...messages, userMsg];
    setMessages((prev) => [...prev, userMsg]);
    setInput("");
    setLoading(true);

    try {
      const response = await invoke<string>("ai_chat", {
        provider: selectedProvider,
        model: selectedModel,
        messages: newMessages,
        terminalContext: useContext ? terminalBuffer.current : null,
        language: i18n.language,
        ollamaBaseUrl: selectedProvider === "Ollama" ? ollamaBaseUrl : null,
        azureOpenAiEndpoint: selectedProvider === "AzureOpenAi" ? azureOpenAiEndpoint : null,
      });
      setMessages((prev) => [...prev, { role: "assistant", content: response }]);
    } catch (e: any) {
      const detail = typeof e === "string" ? e : e.message || "Unknown error";
      setMessages((prev) => [
        ...prev,
        {
          role: "assistant",
          content: `Error: ${detail}`,
        },
      ]);
    }
    setLoading(false);
  };

  return (
    <div className="ai-panel">
      <div className="ai-panel__header">
        <div className="ai-panel__heading">
          <span className="ai-panel__title">{t("ai.title")}</span>
        </div>
        <button className="btn-icon" onClick={onClose}>
          <X size={14} />
        </button>
      </div>

      <div className="ai-panel__messages">
        {messages.length === 0 ? (
          <div className="ai-panel__welcome">
            <div className="ai-panel__welcome-icon">
              <AIAssistantLogo size="lg" />
            </div>
            <div className="ai-panel__welcome-text">{t("ai.title")}</div>
          </div>
        ) : (
          messages.map((msg, i) => {
            const segments =
              msg.role === "assistant"
                ? parseAssistantContent(msg.content)
                : [{ type: "text" as const, content: msg.content }];

            return (
              <div key={i} className={`ai-message ai-message--${msg.role}`}>
                <span className="ai-message__role">{msg.role === "user" ? "You" : "AI"}</span>
                {segments.map((segment, segmentIndex) =>
                  segment.type === "text" ? (
                    <div className="ai-message__content" key={`${i}-${segmentIndex}`}>
                      {segment.content}
                    </div>
                  ) : (
                    <div className="ai-command" key={`${i}-${segmentIndex}`}>
                      <div className="ai-command-list__title">{t("ai.command_suggestions")}</div>
                      <div className="ai-command__body">
                        <div className="ai-command__header">
                          <span className="ai-command__language">
                            {segment.suggestion.language}
                          </span>
                          <button
                            className="ai-command__insert"
                            type="button"
                            onClick={() => onInsertCommand(segment.suggestion.command)}
                            disabled={!canInsertCommand}
                            title={
                              canInsertCommand
                                ? t("ai.insert_command")
                                : t("ai.insert_command_disabled")
                            }
                          >
                            <ClipboardPaste size={13} />
                            {t("ai.insert_command")}
                          </button>
                        </div>
                        <pre className="ai-command__code">
                          <code>{segment.suggestion.command}</code>
                        </pre>
                      </div>
                    </div>
                  )
                )}
              </div>
            );
          })
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
        <div className="ai-panel__input-row">
          <textarea
            className="ai-panel__input"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSend();
              }
            }}
            placeholder={t("ai.placeholder")}
            rows={1}
          />
          <button
            className="ai-panel__send"
            onClick={handleSend}
            disabled={loading || !input.trim() || !selectedModel}
          >
            <Send size={16} />
          </button>
        </div>

        <div className="ai-panel__bottom-row">
          <button
            className={`ai-panel__context-btn ${useContext ? "ai-panel__context-btn--active" : ""}`}
            onClick={() => setUseContext(!useContext)}
          >
            <Terminal size={12} />
            {useContext ? `${t("ai.context")} ✓` : t("ai.context")}
          </button>

          <div className="ai-panel__provider">
            <select
              value={selectedProvider}
              onChange={(e) => {
                setSelectedProvider(e.target.value);
                const pm = models.filter((m) => m.provider === e.target.value);
                setSelectedModel(pm[0]?.model_id || "");
              }}
            >
              {visibleProviders.map((provider) => (
                <option key={provider.id} value={provider.id}>
                  {provider.label}
                </option>
              ))}
            </select>
            <select
              value={selectedModel}
              onChange={(e) => setSelectedModel(e.target.value)}
              disabled={providerModels.length === 0}
            >
              {providerModels.map((m) => (
                <option key={m.model_id} value={m.model_id}>
                  {m.display_name}
                </option>
              ))}
            </select>
          </div>
        </div>
      </div>
    </div>
  );
}
