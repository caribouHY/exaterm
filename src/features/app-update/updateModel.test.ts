import { describe, expect, it } from "vitest";
import {
  INITIAL_APP_UPDATE_STATE,
  appUpdateReducer,
  shouldCheckForUpdatesOnStartup,
  updateDownloadPercent,
  type AppUpdateMetadata,
} from "./updateModel";

const update: AppUpdateMetadata = {
  version: "0.9.0",
  currentVersion: "0.8.0",
};

describe("app update state", () => {
  it("keeps automatic checking, no-update, and errors silent", () => {
    const checking = appUpdateReducer(INITIAL_APP_UPDATE_STATE, {
      type: "check_started",
      source: "automatic",
    });
    expect(checking).toEqual({ phase: "closed" });
    expect(
      appUpdateReducer(checking, {
        type: "check_succeeded",
        source: "automatic",
        update: null,
      })
    ).toEqual({ phase: "closed" });
    expect(
      appUpdateReducer(checking, {
        type: "check_failed",
        source: "automatic",
      })
    ).toEqual({ phase: "closed" });
  });

  it("shows manual results and available updates", () => {
    expect(
      appUpdateReducer(
        { phase: "checking", source: "manual" },
        { type: "check_succeeded", source: "manual", update: null }
      )
    ).toEqual({ phase: "up_to_date" });
    expect(
      appUpdateReducer(
        { phase: "checking", source: "manual" },
        { type: "check_succeeded", source: "manual", update }
      )
    ).toEqual({ phase: "available", update });
  });

  it("tracks active-session confirmation and download progress", () => {
    const confirmation = appUpdateReducer(
      { phase: "available", update },
      { type: "active_sessions_required", update, count: 2 }
    );
    expect(confirmation).toEqual({
      phase: "confirm_active_sessions",
      update,
      activeSessionCount: 2,
    });

    const downloading = appUpdateReducer(confirmation, {
      type: "install_started",
      update,
    });
    const started = appUpdateReducer(downloading, {
      type: "download_started",
      contentLength: 100,
    });
    expect(appUpdateReducer(started, { type: "download_progress", chunkLength: 25 })).toMatchObject(
      { downloadedBytes: 25, contentLength: 100 }
    );
    expect(appUpdateReducer(started, { type: "download_finished" })).toEqual({
      phase: "installing",
      update,
    });
  });

  it("supports retrying after check and install failures", () => {
    const checkError = appUpdateReducer(
      { phase: "checking", source: "manual" },
      { type: "check_failed", source: "manual" }
    );
    expect(checkError).toEqual({ phase: "error", source: "manual" });
    expect(appUpdateReducer(checkError, { type: "check_started", source: "manual" })).toEqual({
      phase: "checking",
      source: "manual",
    });

    expect(
      appUpdateReducer(
        {
          phase: "downloading",
          update,
          downloadedBytes: 0,
          contentLength: null,
        },
        { type: "install_failed" }
      )
    ).toEqual({ phase: "error", source: "install" });
  });
});

describe("app update helpers", () => {
  it("checks only once in the main window when enabled", () => {
    expect(shouldCheckForUpdatesOnStartup("main", true, false)).toBe(true);
    expect(shouldCheckForUpdatesOnStartup("workspace-1", true, false)).toBe(false);
    expect(shouldCheckForUpdatesOnStartup("main", false, false)).toBe(false);
    expect(shouldCheckForUpdatesOnStartup("main", true, true)).toBe(false);
  });

  it("calculates bounded download percentages", () => {
    expect(updateDownloadPercent(25, 100)).toBe(25);
    expect(updateDownloadPercent(125, 100)).toBe(100);
    expect(updateDownloadPercent(10, null)).toBeNull();
  });
});
