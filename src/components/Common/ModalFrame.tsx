import type { MouseEvent, ReactNode } from "react";

interface ModalFrameProps {
  children: ReactNode;
  className?: string;
  role?: "dialog" | "alertdialog";
  ariaModal?: boolean;
  ariaLabelledBy?: string;
  ariaDescribedBy?: string;
  onClick?: (event: MouseEvent<HTMLDivElement>) => void;
}

export function ModalFrame({
  children,
  className = "",
  role,
  ariaModal,
  ariaLabelledBy,
  ariaDescribedBy,
  onClick,
}: ModalFrameProps) {
  const classes = ["ui-modal", className].filter(Boolean).join(" ");

  return (
    <div
      className={classes}
      role={role}
      aria-modal={ariaModal}
      aria-labelledby={ariaLabelledBy}
      aria-describedby={ariaDescribedBy}
      onClick={onClick}
    >
      {children}
    </div>
  );
}

interface ModalSectionProps {
  children: ReactNode;
  className?: string;
}

export function ModalHeader({ children, className = "" }: ModalSectionProps) {
  const classes = ["ui-modal__header", className].filter(Boolean).join(" ");

  return <div className={classes}>{children}</div>;
}

export function ModalBody({ children, className = "" }: ModalSectionProps) {
  const classes = ["ui-modal__body", className].filter(Boolean).join(" ");

  return <div className={classes}>{children}</div>;
}

export function ModalFooter({ children, className = "" }: ModalSectionProps) {
  const classes = ["ui-modal__footer", className].filter(Boolean).join(" ");

  return <div className={classes}>{children}</div>;
}

export function ModalTitle({ children, className = "", id }: ModalSectionProps & { id?: string }) {
  const classes = ["ui-modal__title", className].filter(Boolean).join(" ");

  return (
    <div className={classes} id={id}>
      {children}
    </div>
  );
}

export function ModalDescription({
  children,
  className = "",
  id,
}: ModalSectionProps & { id?: string }) {
  const classes = ["ui-modal__description", className].filter(Boolean).join(" ");

  return (
    <p className={classes} id={id}>
      {children}
    </p>
  );
}

export function ModalTarget({ children, className = "" }: ModalSectionProps) {
  const classes = ["ui-modal__target", className].filter(Boolean).join(" ");

  return <div className={classes}>{children}</div>;
}

export function ModalBusy({ children, className = "" }: ModalSectionProps) {
  const classes = ["ui-modal__busy", className].filter(Boolean).join(" ");

  return (
    <div className={classes}>
      <div className="ui-modal__spinner" />
      {children}
    </div>
  );
}
