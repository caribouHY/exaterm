import { Sparkles } from "lucide-react";
import "./AIAssistantLogo.css";

interface AIAssistantLogoProps {
  size?: "sm" | "md" | "lg";
}

export default function AIAssistantLogo({ size = "md" }: AIAssistantLogoProps) {
  return (
    <span className={`ai-assistant-logo ai-assistant-logo--${size}`} aria-hidden="true">
      <Sparkles className="ai-assistant-logo__sparkles" />
    </span>
  );
}
