import { describe, expect, it, vi } from "vitest";
import { createLanguageSyncController, type LanguageSyncStage } from "./languageSync";

function createDeferred() {
  let resolvePromise = () => {};
  const promise = new Promise<void>((resolve) => {
    resolvePromise = () => {
      resolve();
    };
  });
  return { promise, resolve: resolvePromise };
}

function createController(overrides?: {
  loadConfiguredLanguage?: () => Promise<string | undefined>;
  getFrontendLanguage?: () => string | undefined;
  changeFrontendLanguage?: (language: "en" | "ja") => Promise<void>;
}) {
  const errors: Array<{ stage: LanguageSyncStage; error: unknown }> = [];
  const changeFrontendLanguage =
    overrides?.changeFrontendLanguage ?? vi.fn(async (_language: "en" | "ja") => {});
  const controller = createLanguageSyncController({
    loadConfiguredLanguage: overrides?.loadConfiguredLanguage ?? vi.fn(async () => "ja"),
    getFrontendLanguage: overrides?.getFrontendLanguage ?? (() => "en"),
    changeFrontendLanguage,
    systemLanguage: "en-US",
    reportError: (stage, error) => errors.push({ stage, error }),
  });

  return { controller, errors, changeFrontendLanguage };
}

describe("createLanguageSyncController", () => {
  it("applies the configured language to the frontend", async () => {
    const { controller, changeFrontendLanguage } = createController();

    await controller.requestSync();

    expect(changeFrontendLanguage).toHaveBeenCalledWith("ja");
  });

  it("uses the system language when the initial config load fails", async () => {
    const loadError = new Error("load failed");
    const { controller, errors, changeFrontendLanguage } = createController({
      loadConfiguredLanguage: vi.fn(async () => {
        throw loadError;
      }),
      getFrontendLanguage: () => "ja",
    });

    await controller.requestSync();

    expect(changeFrontendLanguage).toHaveBeenCalledWith("en");
    expect(errors).toEqual([{ stage: "config_load", error: loadError }]);
  });

  it("retains the applied language after a later config load failure", async () => {
    const loadError = new Error("load failed");
    const loadConfiguredLanguage = vi
      .fn<() => Promise<string | undefined>>()
      .mockResolvedValueOnce("ja")
      .mockRejectedValueOnce(loadError);
    const { controller, errors, changeFrontendLanguage } = createController({
      loadConfiguredLanguage,
    });

    await controller.requestSync();
    await controller.requestSync();

    expect(changeFrontendLanguage).toHaveBeenCalledTimes(1);
    expect(errors).toEqual([{ stage: "config_load", error: loadError }]);
  });

  it("reports a frontend update failure", async () => {
    const frontendError = new Error("frontend failed");
    const { controller, errors } = createController({
      changeFrontendLanguage: vi.fn(async () => {
        throw frontendError;
      }),
    });

    await controller.requestSync();

    expect(errors).toEqual([{ stage: "frontend", error: frontendError }]);
  });

  it("skips a redundant frontend language update", async () => {
    const { controller, changeFrontendLanguage } = createController({
      getFrontendLanguage: () => "ja",
    });

    await controller.requestSync();

    expect(changeFrontendLanguage).not.toHaveBeenCalled();
  });

  it("serializes overlapping refresh requests", async () => {
    let currentLanguage = "en";
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
    const changeFrontendLanguage = vi.fn(async (language: "en" | "ja") => {
      currentLanguage = language;
    });
    const { controller } = createController({
      loadConfiguredLanguage,
      getFrontendLanguage: () => currentLanguage,
      changeFrontendLanguage,
    });

    const firstSync = controller.requestSync();
    const overlappingSync = controller.requestSync();
    finishFirstLoad?.("ja");
    await Promise.all([firstSync, overlappingSync]);

    expect(loadConfiguredLanguage).toHaveBeenCalledTimes(2);
    expect(changeFrontendLanguage).toHaveBeenNthCalledWith(1, "ja");
    expect(changeFrontendLanguage).toHaveBeenNthCalledWith(2, "en");
  });

  it("finishes an in-flight frontend update when disposed", async () => {
    const frontendStarted = createDeferred();
    const frontendUpdate = createDeferred();
    const changeFrontendLanguage = vi.fn(() => {
      frontendStarted.resolve();
      return frontendUpdate.promise;
    });
    const { controller } = createController({
      changeFrontendLanguage,
    });

    const sync = controller.requestSync();
    await frontendStarted.promise;
    controller.dispose();
    frontendUpdate.resolve();
    await sync;

    expect(changeFrontendLanguage).toHaveBeenCalledTimes(1);
  });
});
