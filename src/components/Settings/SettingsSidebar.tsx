import { useTranslation } from "react-i18next";
import { SETTINGS_CATEGORIES, type SettingsCategoryId } from "./settingsModel";

interface SettingsSidebarProps {
  activeCategory: SettingsCategoryId;
  onCategoryChange: (category: SettingsCategoryId) => void;
}

export function SettingsSidebar({ activeCategory, onCategoryChange }: SettingsSidebarProps) {
  const { t } = useTranslation();

  return (
    <>
      <div className="settings-category-select">
        <label className="label" htmlFor="settings-category-select">
          {t("settings.category_select_label")}
        </label>
        <select
          id="settings-category-select"
          className="select"
          value={activeCategory}
          onChange={(event) => {
            onCategoryChange(event.target.value as SettingsCategoryId);
          }}
        >
          {SETTINGS_CATEGORIES.map((category) => (
            <option key={category.id} value={category.id}>
              {t(category.labelKey)}
            </option>
          ))}
        </select>
      </div>

      <nav className="settings-category-nav" aria-label={t("settings.category_select_label")}>
        {SETTINGS_CATEGORIES.map((category) => (
          <button
            key={category.id}
            type="button"
            className={`settings-category-button ${
              activeCategory === category.id ? "settings-category-button--active" : ""
            }`}
            aria-current={activeCategory === category.id ? "page" : undefined}
            onClick={() => {
              onCategoryChange(category.id);
            }}
          >
            {t(category.labelKey)}
          </button>
        ))}
      </nav>
    </>
  );
}
