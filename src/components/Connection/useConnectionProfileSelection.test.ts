import { describe, expect, it, vi } from "vitest";
import type { SavedConnection } from "../../types";
import type { ProfileSelectionState } from "./connectionDialogTypes";
import { useConnectionProfileSelection } from "./useConnectionProfileSelection";

vi.mock("react", () => ({ useCallback: (callback: unknown) => callback }));

function mount(selectedProfileIds: ProfileSelectionState = { ssh: "", telnet: "" }) {
  const shared = () => ({
    setProfileName: vi.fn(),
    setGroup: vi.fn(),
    setHost: vi.fn(),
    setPort: vi.fn(),
    setEncoding: vi.fn(),
    setTerminalMode: vi.fn(),
    setMemo: vi.fn(),
    setExternalControlEnabled: vi.fn(),
  });
  const sshSetters = {
    ...shared(),
    setUsername: vi.fn(),
    setAuthMethod: vi.fn(),
    setPrivateKeyPath: vi.fn(),
    setJumpProfileId: vi.fn(),
  };
  const telnetSetters = shared();
  const params = {
    sshProfiles: [
      {
        id: "router",
        connection_type: "ssh",
        group: "Site A",
        jump_profile_id: "bastion",
        host: "stored.example",
        port: 2222,
        username: "operator",
        auth_method: "public_key",
        private_key_path: "stored-key",
        memo: "stored memo",
        external_control_enabled: false,
      },
      { id: "legacy", connection_type: "ssh" },
    ] as SavedConnection[],
    telnetProfiles: [{ id: "switch", connection_type: "telnet", group: "Lab" }],
    selectedProfileIds,
    setSelectedProfileIds: vi.fn(),
    sshSetters,
    telnetSetters,
  };
  return {
    ...useConnectionProfileSelection(params),
    sshSetters,
    telnetSetters,
    setSelectedProfileIds: params.setSelectedProfileIds,
  };
}

describe("saved profile group selection", () => {
  it("loads the group with the selected profile and clears it for legacy and manual sources", () => {
    const selection = mount();
    selection.handleSelectSshProfile("router");
    expect(selection.sshSetters.setGroup).toHaveBeenLastCalledWith("Site A");
    expect(selection.sshSetters.setProfileName).toHaveBeenLastCalledWith("router");
    expect(selection.sshSetters.setJumpProfileId).toHaveBeenLastCalledWith("bastion");
    selection.handleSelectSshProfile("legacy");
    expect(selection.sshSetters.setGroup).toHaveBeenLastCalledWith("");
    selection.handleSelectSshProfile("router");
    selection.handleSelectSshProfile("");
    expect(selection.sshSetters.setGroup).toHaveBeenLastCalledWith("");
    expect(selection.telnetSetters.setGroup).not.toHaveBeenCalled();
  });

  it("keeps Telnet group selection and reset separate from SSH", () => {
    const selection = mount();
    selection.handleSelectTelnetProfile("switch");
    expect(selection.telnetSetters.setGroup).toHaveBeenLastCalledWith("Lab");
    selection.resetTelnetProfileFields();
    expect(selection.telnetSetters.setGroup).toHaveBeenLastCalledWith("");
    expect(selection.sshSetters.setGroup).not.toHaveBeenCalled();
  });
});

describe("saved profile copy selection", () => {
  it("loads the saved SSH snapshot and clears only the copied profile identity", () => {
    const selection = mount({ ssh: "router", telnet: "switch" });
    selection.sshSetters.setHost("unsaved.example");
    expect(selection.copySshProfile("router copy")).toBe(true);
    expect(selection.sshSetters.setProfileName).toHaveBeenLastCalledWith("router copy");
    expect(selection.sshSetters.setHost).toHaveBeenLastCalledWith("stored.example");
    expect(selection.sshSetters.setPort).toHaveBeenLastCalledWith("2222");
    expect(selection.sshSetters.setUsername).toHaveBeenLastCalledWith("operator");
    expect(selection.sshSetters.setAuthMethod).toHaveBeenLastCalledWith("public_key");
    expect(selection.sshSetters.setPrivateKeyPath).toHaveBeenLastCalledWith("stored-key");
    expect(selection.sshSetters.setJumpProfileId).toHaveBeenLastCalledWith("bastion");
    expect(selection.sshSetters.setGroup).toHaveBeenLastCalledWith("Site A");
    expect(selection.sshSetters.setMemo).toHaveBeenLastCalledWith("stored memo");
    expect(selection.sshSetters.setExternalControlEnabled).toHaveBeenLastCalledWith(false);
    const update = selection.setSelectedProfileIds.mock.calls[0][0] as (
      current: ProfileSelectionState
    ) => ProfileSelectionState;
    expect(update({ ssh: "router", telnet: "switch" })).toEqual({ ssh: "", telnet: "switch" });
    expect(selection.telnetSetters.setProfileName).not.toHaveBeenCalled();
  });

  it("creates a Telnet draft without changing the SSH selection", () => {
    const selection = mount({ ssh: "router", telnet: "switch" });
    expect(selection.copyTelnetProfile("switch copy")).toBe(true);
    expect(selection.telnetSetters.setProfileName).toHaveBeenLastCalledWith("switch copy");
    expect(selection.telnetSetters.setGroup).toHaveBeenLastCalledWith("Lab");
    const update = selection.setSelectedProfileIds.mock.calls[0][0] as (
      current: ProfileSelectionState
    ) => ProfileSelectionState;
    expect(update({ ssh: "router", telnet: "switch" })).toEqual({ ssh: "router", telnet: "" });
    expect(selection.sshSetters.setProfileName).not.toHaveBeenCalled();
  });

  it("does not alter form values when the selected saved source is missing", () => {
    const selection = mount({ ssh: "missing", telnet: "missing" });
    expect(selection.copySshProfile("missing copy")).toBe(false);
    expect(selection.copyTelnetProfile("missing copy")).toBe(false);
    expect(selection.setSelectedProfileIds).not.toHaveBeenCalled();
    expect(selection.sshSetters.setProfileName).not.toHaveBeenCalled();
    expect(selection.telnetSetters.setProfileName).not.toHaveBeenCalled();
  });
});
