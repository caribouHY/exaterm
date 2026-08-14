import { describe, expect, it } from "vitest";
import { findTerminalPinnedCommand, hasTerminalPromptInRange } from "./terminalCommandModel";
import {
  ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE,
  ARISTA_EOS_DECORATION_PROFILE,
  CISCO_IOS_DECORATION_PROFILE,
  FUJITSU_SIR_DECORATION_PROFILE,
  FURUKAWA_FITELNET_DECORATION_PROFILE,
  VYOS_DECORATION_PROFILE,
} from "./terminalDecorationProfiles";
import type {
  TerminalBufferLike,
  TerminalBufferLineLike,
  TerminalDecorationProfile,
} from "./terminalDecorationTypes";

function createBuffer(
  lines: Array<string | { text: string; isWrapped: boolean }>,
  viewportY: number,
  type: TerminalBufferLike["type"] = "normal"
): TerminalBufferLike {
  const bufferLines: TerminalBufferLineLike[] = lines.map((line) => {
    const value = typeof line === "string" ? { text: line, isWrapped: false } : line;
    return {
      isWrapped: value.isWrapped,
      translateToString: () => value.text,
    };
  });

  return {
    type,
    viewportY,
    length: bufferLines.length,
    getLine: (lineIndex) => bufferLines.find((_line, index) => index === lineIndex),
  };
}

const SYNTHETIC_PROFILE: TerminalDecorationProfile = {
  mode: "general",
  decorationLookback: 20,
  decorationStyle: "synthetic-v1",
  pinnedCommand: true,
  parsePrompt: (line) => {
    const match = /^(\[[^\]]+\]\$)( ?)(.*)$/.exec(line.trimEnd());
    if (!match) return null;
    const [, promptText, commandSeparator, commandText] = match;
    return {
      promptText,
      promptStart: 0,
      commandText,
      commandStart: promptText.length + commandSeparator.length,
      commandSeparator,
      variant: "default",
    };
  },
  isErrorLine: (line) => line.startsWith("FAIL"),
};

describe("findTerminalPinnedCommand", () => {
  it("does not pin a command while its execution line is visible", () => {
    expect(
      findTerminalPinnedCommand(
        createBuffer(["Router#show interfaces", "output"], 0),
        CISCO_IOS_DECORATION_PROFILE
      )
    ).toBeNull();
  });

  it("pins the prompt and command while viewing its long output", () => {
    const buffer = createBuffer(["Router#show interfaces", "line 1", "line 2", "line 3"], 2);

    expect(findTerminalPinnedCommand(buffer, CISCO_IOS_DECORATION_PROFILE)).toEqual({
      displayText: "Router#show interfaces",
      promptText: "Router#",
      commandText: "show interfaces",
      promptVariant: "default",
      promptLineIndex: 0,
      commandLineCount: 1,
    });
  });

  it("joins a wrapped command into one display line", () => {
    const buffer = createBuffer(
      [
        "Router(config)#interface GigabitEthernet",
        { text: "0/0/0.100", isWrapped: true },
        "output",
      ],
      2
    );

    expect(findTerminalPinnedCommand(buffer, CISCO_IOS_DECORATION_PROFILE)).toEqual({
      displayText: "Router(config)#interface GigabitEthernet0/0/0.100",
      promptText: "Router(config)#",
      commandText: "interface GigabitEthernet0/0/0.100",
      promptVariant: "configuration",
      promptLineIndex: 0,
      commandLineCount: 2,
    });
  });

  it("switches after the next command execution line leaves the viewport", () => {
    const buffer = createBuffer(
      ["Router#show clock", "12:00:00", "Router#show version", "version output", "more output"],
      4
    );

    expect(findTerminalPinnedCommand(buffer, CISCO_IOS_DECORATION_PROFILE)?.displayText).toBe(
      "Router#show version"
    );
  });

  it("does not pin the previous command when the next prompt reaches the viewport top", () => {
    const buffer = createBuffer(
      ["Router#show clock", "12:00:00", "Router#show version", "version output"],
      2
    );

    expect(findTerminalPinnedCommand(buffer, CISCO_IOS_DECORATION_PROFILE)).toBeNull();
  });

  it("does not pin output after an empty prompt", () => {
    const buffer = createBuffer(["Router#show clock", "12:00:00", "Router#", ""], 3);

    expect(findTerminalPinnedCommand(buffer, CISCO_IOS_DECORATION_PROFILE)).toBeNull();
  });

  it("honors profiles that disable pinned commands", () => {
    const profile = { ...SYNTHETIC_PROFILE, pinnedCommand: false };
    const buffer = createBuffer(["[lab]$ inspect", "output"], 1);

    expect(findTerminalPinnedCommand(buffer, profile)).toBeNull();
  });

  it("does not pin commands from the alternate buffer", () => {
    const buffer = createBuffer(["Router#show interfaces", "output"], 1, "alternate");

    expect(findTerminalPinnedCommand(buffer, CISCO_IOS_DECORATION_PROFILE)).toBeNull();
  });

  it("works with a non-Cisco prompt parser", () => {
    const buffer = createBuffer(["[lab]$ inspect system", "result", "more result"], 2);

    expect(findTerminalPinnedCommand(buffer, SYNTHETIC_PROFILE)).toMatchObject({
      displayText: "[lab]$ inspect system",
      promptText: "[lab]$",
      commandText: " inspect system",
    });
    expect(hasTerminalPromptInRange(buffer, SYNTHETIC_PROFILE, 0, 1)).toBe(true);
  });

  it("pins Arista EOS commands with configuration prompt coloring", () => {
    const buffer = createBuffer(
      ["switch(config-if-Et24)# description uplink", "output", "more output"],
      2
    );

    expect(findTerminalPinnedCommand(buffer, ARISTA_EOS_DECORATION_PROFILE)).toMatchObject({
      displayText: "switch(config-if-Et24)# description uplink",
      promptText: "switch(config-if-Et24)#",
      commandText: " description uplink",
      promptVariant: "configuration",
    });
  });

  it("pins a VyOS configuration command with its immediate edit context", () => {
    const buffer = createBuffer(
      [
        "[edit interfaces ethernet eth0]",
        "vyos@router:~# set description WAN",
        "output",
        "more output",
      ],
      3
    );

    expect(findTerminalPinnedCommand(buffer, VYOS_DECORATION_PROFILE)).toMatchObject({
      displayText: "vyos@router:~# set description WAN",
      contextText: "[edit interfaces ethernet eth0]",
      promptText: "vyos@router:~#",
      commandText: " set description WAN",
      promptVariant: "configuration",
    });
  });

  it("pins a Fujitsu Si-R config2 command without dropping its configuration name", () => {
    const buffer = createBuffer(
      ["Si-R G121 config2(config)# lan 0 vlan 1", "output", "more output"],
      2
    );

    expect(findTerminalPinnedCommand(buffer, FUJITSU_SIR_DECORATION_PROFILE)).toMatchObject({
      displayText: "Si-R G121 config2(config)# lan 0 vlan 1",
      promptText: "Si-R G121 config2(config)#",
      commandText: " lan 0 vlan 1",
      promptVariant: "configuration",
    });
  });

  it("pins an AlliedWare Plus configuration command", () => {
    const buffer = createBuffer(
      ["ar4050(config-if)# tunnel mode ipsec", "output", "more output"],
      2
    );

    expect(
      findTerminalPinnedCommand(buffer, ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE)
    ).toMatchObject({
      displayText: "ar4050(config-if)# tunnel mode ipsec",
      promptText: "ar4050(config-if)#",
      commandText: " tunnel mode ipsec",
      promptVariant: "configuration",
    });
  });

  it("pins a Furukawa FITELnet configuration command", () => {
    const buffer = createBuffer(
      ["F221-Router(config-if-ge 1/1)# vlan-id 1", "output", "more output"],
      2
    );

    expect(findTerminalPinnedCommand(buffer, FURUKAWA_FITELNET_DECORATION_PROFILE)).toMatchObject({
      displayText: "F221-Router(config-if-ge 1/1)# vlan-id 1",
      promptText: "F221-Router(config-if-ge 1/1)#",
      commandText: " vlan-id 1",
      promptVariant: "configuration",
    });
  });

  it("does not associate a stale VyOS edit context", () => {
    const buffer = createBuffer(
      ["[edit interfaces]", "unrelated output", "vyos@router# commit", "output"],
      3
    );

    expect(findTerminalPinnedCommand(buffer, VYOS_DECORATION_PROFILE)).not.toHaveProperty(
      "contextText"
    );
  });
});
