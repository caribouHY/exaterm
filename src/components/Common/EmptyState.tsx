import type { ReactNode } from "react";

interface EmptyStateProps {
  icon?: ReactNode;
  title?: ReactNode;
  children?: ReactNode;
  className?: string;
}

export function EmptyState({ icon, title, children, className = "" }: EmptyStateProps) {
  const classes = ["ui-empty-state", className].filter(Boolean).join(" ");

  return (
    <div className={classes}>
      {icon && <div className="ui-empty-state__icon">{icon}</div>}
      {title && <div className="ui-empty-state__title">{title}</div>}
      {children && <div className="ui-empty-state__body">{children}</div>}
    </div>
  );
}
