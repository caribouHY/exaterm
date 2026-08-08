import { describe, expect, it } from "vitest";
import {
  CISCO_IOS_DECORATION_PROFILE,
  getTerminalDecorationProfile,
  parseCiscoIosPrompt,
} from "./terminalDecorationProfiles";
import { TERMINAL_DECORATION_COLORS } from "./terminalDecorationTheme";

describe("parseCiscoIosPrompt", () => {
  it("parses configuration prompts without a regular expression", () => {
    expect(parseCiscoIosPrompt("Router(config-if)# description uplink")).toEqual({
      hostname: "Router",
      promptText: "Router(config-if)#",
      promptStart: 0,
      commandText: "description uplink",
      commandStart: 19,
      commandSeparator: " ",
      variant: "configuration",
    });
  });

  it("uses the default variant when no configuration mode is present", () => {
    expect(parseCiscoIosPrompt("Router#show clock")?.variant).toBe("default");
  });

  it("rejects malformed and excessively nested mode prompts", () => {
    expect(parseCiscoIosPrompt("Router(config#show run")).toBeNull();
    expect(parseCiscoIosPrompt("Router()#show run")).toBeNull();
    expect(parseCiscoIosPrompt("Router(a)(b)(c)(d)#show run")).toBeNull();
  });
});

describe("terminal decoration profile registry", () => {
  it("keeps general mode undecorated and resolves Cisco IOS", () => {
    expect(getTerminalDecorationProfile("general")).toBeNull();
    expect(getTerminalDecorationProfile("cisco_ios")).toBe(CISCO_IOS_DECORATION_PROFILE);
  });

  it("preserves the Cisco IOS scan limit and error matching", () => {
    expect(CISCO_IOS_DECORATION_PROFILE.decorationLookback).toBe(80);
    expect(CISCO_IOS_DECORATION_PROFILE.isErrorLine("% Invalid input detected")).toBe(true);
    expect(CISCO_IOS_DECORATION_PROFILE.isErrorLine("Command Rejected: denied")).toBe(true);
    expect(
      CISCO_IOS_DECORATION_PROFILE.isErrorLine("Command Rejected (authorization): denied")
    ).toBe(true);
    expect(CISCO_IOS_DECORATION_PROFILE.isErrorLine("interface is up")).toBe(false);
  });

  it("uses one shared palette for terminal decoration", () => {
    expect(TERMINAL_DECORATION_COLORS).toEqual({
      prompt: "#7dd3fc",
      configurationPrompt: "#facc15",
      command: "#6ee7b7",
      error: "#f87171",
    });
  });
});
