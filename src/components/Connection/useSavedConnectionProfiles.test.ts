import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, SavedConnection } from "../../types";
import { createProfileCopy } from "./connectionProfileCopy";
import { useSavedConnectionProfiles } from "./useSavedConnectionProfiles";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("react", () => ({ useRef: (value: unknown) => ({ current: value }) }));
beforeEach(() => {
  vi.clearAllMocks();
});

describe("saving a copied profile", () => {
  const source: SavedConnection = {
    id: "router",
    connection_type: "ssh",
    host: "router.example",
    port: 22,
    jump_profile_id: "bastion",
    external_control_enabled: false,
  };

  it("uses insert semantics and retains the original when saving a copied draft", async () => {
    const draft = createProfileCopy(source, [source], "router copy");
    const saved = { saved_connections: [source, draft] } as AppConfig;
    vi.mocked(invoke).mockResolvedValue(saved);
    const setConfig = vi.fn();
    const onSaved = vi.fn();
    const controller = useSavedConnectionProfiles({
      setConfig,
      setError: vi.fn(),
      t: (key) => key,
    });
    await controller.saveProfile("", () => draft, onSaved);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("config_saved_connection_upsert", {
      previousId: null,
      profile: draft,
    });
    expect(setConfig).toHaveBeenCalledWith(saved);
    expect(onSaved).toHaveBeenCalledWith(draft);
    expect(source.id).toBe("router");
  });

  it("leaves the new draft intact when another window has saved the same name", async () => {
    const draft = createProfileCopy(source, [source], "router copy");
    const before = JSON.stringify(draft);
    vi.mocked(invoke).mockRejectedValue({
      code: "config.saved_connection_duplicate",
      message: "A saved connection profile with this name already exists",
    });
    const setConfig = vi.fn();
    const setError = vi.fn();
    const onSaved = vi.fn();
    const controller = useSavedConnectionProfiles({ setConfig, setError, t: (key) => key });
    await controller.saveProfile("", () => draft, onSaved);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("config_saved_connection_upsert", {
      previousId: null,
      profile: draft,
    });
    expect(setConfig).not.toHaveBeenCalled();
    expect(onSaved).not.toHaveBeenCalled();
    expect(setError).toHaveBeenLastCalledWith("backend_errors.config_saved_connection_duplicate");
    expect(JSON.stringify(draft)).toBe(before);
    expect(source.id).toBe("router");
  });

  it("refreshes saved config without reselecting an older save over the copy draft", async () => {
    let complete: (value: AppConfig) => void = () => {
      throw new Error("Save did not start");
    };
    vi.mocked(invoke).mockImplementationOnce(
      () =>
        new Promise<AppConfig>((resolve) => {
          complete = resolve;
        })
    );
    const setConfig = vi.fn();
    const onSaved = vi.fn();
    const controller = useSavedConnectionProfiles({
      setConfig,
      setError: vi.fn(),
      t: (key) => key,
    });
    const pending = controller.saveProfile(source.id, () => source, onSaved);
    controller.preserveDraft("ssh");
    const saved = { saved_connections: [source] } as AppConfig;
    complete(saved);
    await pending;
    expect(setConfig).toHaveBeenCalledWith(saved);
    expect(onSaved).not.toHaveBeenCalled();

    const draft = createProfileCopy(source, [source], "router copy");
    vi.mocked(invoke).mockResolvedValue(saved);
    await controller.saveProfile("", () => draft, onSaved);
    expect(onSaved).toHaveBeenCalledExactlyOnceWith(draft);
  });

  it("does not suppress another protocol's save completion", async () => {
    vi.mocked(invoke).mockResolvedValue({ saved_connections: [source] });
    const onSaved = vi.fn();
    const controller = useSavedConnectionProfiles({
      setConfig: vi.fn(),
      setError: vi.fn(),
      t: (key) => key,
    });
    const pending = controller.saveProfile(source.id, () => source, onSaved);
    controller.preserveDraft("telnet");
    await pending;
    expect(onSaved).toHaveBeenCalledExactlyOnceWith(source);
  });
});
