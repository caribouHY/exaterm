import { useEffect, useRef, useState, type RefObject } from "react";
import { createPortal } from "react-dom";
import { getVersion } from "@tauri-apps/api/app";
import { useTranslation } from "react-i18next";
import {
  FeedbackMessage,
  ModalBody,
  ModalFooter,
  ModalFrame,
  ModalHeader,
  ModalTitle,
} from "../Common";
import appIcon from "../../../src-tauri/icons/icon.png";

export type HelpDialogKind = "about" | "documentation_error";

interface HelpDialogProps {
  kind: HelpDialogKind;
  onClose: () => void;
  returnFocusRef: RefObject<HTMLButtonElement | null>;
}

export function HelpDialog({ kind, onClose, returnFocusRef }: HelpDialogProps) {
  const { t } = useTranslation();
  const [version, setVersion] = useState<string | null>(null);
  const [versionFailed, setVersionFailed] = useState(false);
  const closeRef = useRef<HTMLButtonElement>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    if (kind !== "about") return;
    let cancelled = false;
    void getVersion().then(
      (value) => {
        if (!cancelled) setVersion(value);
      },
      () => {
        if (!cancelled) setVersionFailed(true);
      }
    );
    return () => {
      cancelled = true;
    };
  }, [kind]);

  useEffect(() => {
    closeRef.current?.focus();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        onCloseRef.current();
      } else if (event.key === "Tab") {
        event.preventDefault();
        event.stopPropagation();
        closeRef.current?.focus();
      }
    };
    const containFocus = (event: FocusEvent) => {
      if (event.target !== closeRef.current) closeRef.current?.focus();
    };
    document.addEventListener("keydown", handleKeyDown, true);
    document.addEventListener("focusin", containFocus);
    return () => {
      document.removeEventListener("keydown", handleKeyDown, true);
      document.removeEventListener("focusin", containFocus);
      returnFocusRef.current?.focus();
    };
  }, [returnFocusRef]);

  return createPortal(
    <div
      className="ui-overlay titlebar-help-overlay"
      onKeyDown={(event) => {
        event.stopPropagation();
      }}
    >
      <ModalFrame
        className="titlebar-help-dialog"
        role="dialog"
        ariaModal
        ariaLabelledBy="titlebar-help-title"
      >
        <ModalHeader>
          <ModalTitle id="titlebar-help-title">
            {t(kind === "about" ? "help.about_title" : "titlebar.menu.documentation")}
          </ModalTitle>
        </ModalHeader>
        <ModalBody className={kind === "about" ? "titlebar-help-dialog__about" : undefined}>
          {kind === "about" ? (
            <>
              <img className="titlebar-help-dialog__icon" src={appIcon} alt="" aria-hidden="true" />
              <strong>ExaTerm</strong>
              <div aria-live="polite">
                {versionFailed ? (
                  <FeedbackMessage tone="error">{t("help.version_failed")}</FeedbackMessage>
                ) : version !== null ? (
                  t("help.version", { version })
                ) : (
                  t("help.version_loading")
                )}
              </div>
              <span>{t("help.license")}</span>
            </>
          ) : (
            <FeedbackMessage tone="error">{t("help.documentation_failed")}</FeedbackMessage>
          )}
        </ModalBody>
        <ModalFooter>
          <button ref={closeRef} className="btn btn-primary" onClick={onClose}>
            {t("help.close")}
          </button>
        </ModalFooter>
      </ModalFrame>
    </div>,
    document.body
  );
}
