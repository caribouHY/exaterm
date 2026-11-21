import {
  useLayoutEffect,
  useRef,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from "react";

interface ModalFrameProps {
  children: ReactNode;
  className?: string;
  role?: "dialog" | "alertdialog";
  ariaModal?: boolean;
  ariaLabelledBy?: string;
  ariaDescribedBy?: string;
  focusTarget?: string;
  onClick?: (event: MouseEvent<HTMLDivElement>) => void;
  onKeyDown?: (event: KeyboardEvent<HTMLDivElement>) => void;
}

const tabStops = "button, input, select, textarea, a[href], [tabindex]";

function getTabStops(frame: HTMLDivElement) {
  return [...frame.querySelectorAll<HTMLElement>(tabStops)].filter(
    (element) =>
      element.tabIndex >= 0 &&
      !element.matches(":disabled") &&
      !element.closest("[inert]") &&
      element.getClientRects().length > 0 &&
      getComputedStyle(element).visibility === "visible"
  );
}

export function ModalFrame({
  children,
  className = "",
  role,
  ariaModal,
  ariaLabelledBy,
  ariaDescribedBy,
  focusTarget,
  onClick,
  onKeyDown,
}: ModalFrameProps) {
  const frameRef = useRef<HTMLDivElement>(null);
  const classes = ["ui-modal", className].filter(Boolean).join(" ");

  useLayoutEffect(() => {
    const frame = frameRef.current;
    if (!frame || focusTarget === undefined || frame.closest("[inert]")) return;
    const active = document.activeElement;
    if (active !== frame && frame.contains(active) && !active?.matches(":disabled")) {
      return;
    }
    const stops = getTabStops(frame);
    const first = stops.length ? stops[0] : frame;
    const target = stops.find((element) => element.matches(focusTarget)) ?? first;
    target.focus();
  });

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    onKeyDown?.(event);
    if (focusTarget === undefined || event.defaultPrevented || event.key !== "Tab") return;
    const frame = event.currentTarget;
    const stops = getTabStops(frame);
    const active = document.activeElement;
    const first = stops.length ? stops[0] : frame;
    const last = stops.length ? stops[stops.length - 1] : frame;
    const edge = event.shiftKey ? first : last;
    if (active === edge || active === frame || stops.length === 0) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    }
  };

  return (
    <div
      ref={frameRef}
      className={classes}
      role={role}
      aria-modal={ariaModal}
      aria-labelledby={ariaLabelledBy}
      aria-describedby={ariaDescribedBy}
      tabIndex={focusTarget === undefined ? undefined : -1}
      onBlur={
        focusTarget === undefined
          ? undefined
          : (event) => {
              if (event.relatedTarget) return;
              const frame = event.currentTarget;
              // A click on the dialog surface can leave focus on the document body.
              queueMicrotask(() => {
                if (
                  document.activeElement === document.body &&
                  frame.isConnected &&
                  !frame.closest("[inert]")
                ) {
                  frame.focus();
                }
              });
            }
      }
      onClick={onClick}
      onKeyDown={handleKeyDown}
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
