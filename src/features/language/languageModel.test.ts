import { describe, expect, it } from "vitest";
import { resolveAppLanguage } from "./languageModel";

describe("resolveAppLanguage", () => {
  it("uses an explicitly configured supported language", () => {
    expect(resolveAppLanguage("en", "ja-JP")).toBe("en");
    expect(resolveAppLanguage("ja", "en-US")).toBe("ja");
  });

  it("resolves Japanese system locales to Japanese", () => {
    expect(resolveAppLanguage("system", "ja-JP")).toBe("ja");
    expect(resolveAppLanguage(undefined, "JA-jp")).toBe("ja");
  });

  it("falls back to English for other system locales and unknown values", () => {
    expect(resolveAppLanguage("system", "en-US")).toBe("en");
    expect(resolveAppLanguage("system", "fr-FR")).toBe("en");
    expect(resolveAppLanguage("unsupported", undefined)).toBe("en");
  });
});
