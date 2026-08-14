export type ConfiguredLanguage = "system" | "en" | "ja";
export type EffectiveLanguage = "en" | "ja";

export function resolveAppLanguage(
  language: string | undefined,
  systemLanguage: string | undefined
): EffectiveLanguage {
  if (language === "ja" || language === "en") {
    return language;
  }

  return systemLanguage?.toLowerCase().startsWith("ja") ? "ja" : "en";
}
