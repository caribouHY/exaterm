import { describe, expect, it, vi } from "vitest";
import { createLanguageSyncController, type LanguageSyncStage } from "./languageSync";

function createController(overrides?: {
  loadConfiguredLanguage?: () => Promise<string | undefined>;
  getFrontendLanguage?: () => string | undefined;
  changeFrontendLanguage?: (language: "en" | "ja") => Promise<void>;
  setBackendLanguage?: (language: "en" | "ja") => Promise<void>;
}) {
  const errors: Array<{ stage: LanguageSyncStage; error: unknown }> = [];
  const changeFrontendLanguage =
    overrides?.changeFrontendLanguage ?? vi.fn(async (_language: "en" | "ja") => {});
  const setBackendLanguage =
    overrides?.setBackendLanguage ?? vi.fn(async (_language: "en" | "ja") => {});
  const controller = createLanguageSyncController({
    loadConfiguredLanguage: overrides?.loadConfiguredLanguage ?? vi.fn(async () => "ja"),
    getFrontendLanguage: overrides?.getFrontendLanguage ?? (() => "en"),
    changeFrontendLanguage,
    setBackendLanguage,
    systemLanguage: "en-US",
    reportError: (stage, error) => errors.push({ stage, error }),
  });

  return { controller, errors, changeFrontendLanguage, setBackendLanguage };
}

describe("createLanguageSyncController", () => {
  it("applies the configured language to the frontend and backend", async () => {
    const { controller, changeFrontendLanguage, setBackendLanguage } = createController();

    await controller.requestSync();

    expect(changeFrontendLanguage).toHaveBeenCalledWith("ja");
    expect(setBackendLanguage).toHaveBeenCalledWith("ja");
  });

  it("uses the system language when the initial config load fails", async () => {
    const loadError = new Error("load failed");
    const { controller, errors, changeFrontendLanguage, setBackendLanguage } = createController({
      loadConfiguredLanguage: vi.fn(async () => {
        throw loadError;
      }),
      getFrontendLanguage: () => "ja",
    });

    await controller.requestSync();

    expect(changeFrontendLanguage).toHaveBeenCalledWith("en");
    expect(setBackendLanguage).toHaveBeenCalledWith("en");
    expect(errors).toEqual([{ stage: "config_load", error: loadError }]);
  });

  it("retains the applied language after a later config load failure", async () => {
    const loadError = new Error("load failed");
    const loadConfiguredLanguage = vi
      .fn<() => Promise<string | undefined>>()
      .mockResolvedValueOnce("ja")
      .mockRejectedValueOnce(loadError);
    const { controller, errors, setBackendLanguage } = createController({
      loadConfiguredLanguage,
    });

    await controller.requestSync();
    await controller.requestSync();

    expect(setBackendLanguage).toHaveBeenCalledTimes(1);
    expect(errors).toEqual([{ stage: "config_load", error: loadError }]);
  });

  it("continues backend synchronization when the frontend update fails", async () => {
    const frontendError = new Error("frontend failed");
    const { controller, errors, setBackendLanguage } = createController({
      changeFrontendLanguage: vi.fn(async () => {
        throw frontendError;
      }),
    });

    await controller.requestSync();

    expect(setBackendLanguage).toHaveBeenCalledWith("ja");
    expect(errors).toEqual([{ stage: "frontend", error: frontendError }]);
  });

  it("reports a backend failure without undoing the frontend update", async () => {
    const backendError = new Error("backend failed");
    const { controller, errors, changeFrontendLanguage } = createController({
      setBackendLanguage: vi.fn(async () => {
        throw backendError;
      }),
    });

    await controller.requestSync();

    expect(changeFrontendLanguage).toHaveBeenCalledWith("ja");
    expect(errors).toEqual([{ stage: "backend", error: backendError }]);
  });

  it("skips a redundant frontend language update", async () => {
    const { controller, changeFrontendLanguage, setBackendLanguage } = createController({
      getFrontendLanguage: () => "ja",
    });

    await controller.requestSync();

    expect(changeFrontendLanguage).not.toHaveBeenCalled();
    expect(setBackendLanguage).toHaveBeenCalledWith("ja");
  });

  it("serializes overlapping refresh requests", async () => {
    let finishFirstLoad: ((language: string) => void) | undefined;
    const loadConfiguredLanguage = vi
      .fn<() => Promise<string | undefined>>()
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finishFirstLoad = resolve;
          })
      )
      .mockResolvedValueOnce("en");
    const { controller, setBackendLanguage } = createController({ loadConfiguredLanguage });

    const firstSync = controller.requestSync();
    const overlappingSync = controller.requestSync();
    finishFirstLoad?.("ja");
    await Promise.all([firstSync, overlappingSync]);

    expect(loadConfiguredLanguage).toHaveBeenCalledTimes(2);
    expect(setBackendLanguage).toHaveBeenNthCalledWith(1, "ja");
    expect(setBackendLanguage).toHaveBeenNthCalledWith(2, "en");
  });
});
