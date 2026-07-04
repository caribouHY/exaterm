import type { ReactNode } from "react";

export type FeedbackTone = "error" | "warning" | "success" | "info";

interface FeedbackMessageProps {
  tone: FeedbackTone;
  children: ReactNode;
  className?: string;
}

export function FeedbackMessage({ tone, children, className = "" }: FeedbackMessageProps) {
  const classes = ["ui-message", `ui-message--${tone}`, className].filter(Boolean).join(" ");

  return <div className={classes}>{children}</div>;
}
