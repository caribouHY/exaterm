import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import "./TitleBar.css";

export default function TitleBar() {
  const { t } = useTranslation();
  const appWindow = getCurrentWindow();

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar__brand">
        <div className="titlebar__logo">E</div>
        <span className="titlebar__name">ExaTerm</span>
      </div>
      <div className="titlebar__center">{t("titlebar.subtitle")}</div>
      <div className="titlebar__controls">
        <button
          className="titlebar__btn"
          onClick={() => appWindow.minimize()}
          aria-label={t("titlebar.minimize")}
        >
          <Minus size={14} />
        </button>
        <button
          className="titlebar__btn"
          onClick={async () => {
            if (await appWindow.isMaximized()) {
              appWindow.unmaximize();
            } else {
              appWindow.maximize();
            }
          }}
          aria-label={t("titlebar.maximize")}
        >
          <Square size={12} />
        </button>
        <button
          className="titlebar__btn titlebar__btn--close"
          onClick={() => appWindow.close()}
          aria-label={t("titlebar.close")}
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
