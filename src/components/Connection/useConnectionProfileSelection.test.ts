import { describe, expect, it, vi } from "vitest";
import { useConnectionProfileSelection } from "./useConnectionProfileSelection";

vi.mock("react", () => ({ useCallback: (callback: unknown) => callback }));

function mount() {
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
      { id: "router", connection_type: "ssh", group: "Site A", jump_profile_id: "bastion" },
      { id: "legacy", connection_type: "ssh" },
    ],
    telnetProfiles: [{ id: "switch", connection_type: "telnet", group: "Lab" }],
    selectedProfileIds: { ssh: "", telnet: "" },
    setSelectedProfileIds: vi.fn(),
    sshSetters,
    telnetSetters,
  };
  return { ...useConnectionProfileSelection(params), sshSetters, telnetSetters };
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
