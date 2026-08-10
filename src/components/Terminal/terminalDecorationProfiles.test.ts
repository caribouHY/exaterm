import { describe, expect, it } from "vitest";
import {
  ARISTA_EOS_DECORATION_PROFILE,
  CISCO_IOS_DECORATION_PROFILE,
  getTerminalDecorationProfile,
  parseAristaEosPrompt,
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

describe("parseAristaEosPrompt", () => {
  it.each([
    ["switch>", "default"],
    ["switch#show version", "default"],
    ["switch(config)# interface Ethernet1", "configuration"],
    ["switch(config-if-Et24)# description uplink", "configuration"],
    ["switch(config-router-bgp)# neighbor 192.0.2.1 remote-as 65001", "configuration"],
    ["switch(config-s-change1)# show session-config diffs", "configuration"],
  ] as const)("parses the standard EOS prompt %s", (line, variant) => {
    expect(parseAristaEosPrompt(line)?.variant).toBe(variant);
  });

  it("separates the prompt, separator, and command", () => {
    expect(parseAristaEosPrompt("switch(config-if-Et24)# description uplink")).toMatchObject({
      hostname: "switch",
      promptText: "switch(config-if-Et24)#",
      commandSeparator: " ",
      commandText: "description uplink",
      variant: "configuration",
    });
  });

  it("rejects Bash, custom, and malformed prompts", () => {
    expect(parseAristaEosPrompt("[admin@switch ~]$ pwd")).toBeNull();
    expect(parseAristaEosPrompt("CUSTOM-PROMPT$ show version")).toBeNull();
    expect(parseAristaEosPrompt("switch(config# show running-config")).toBeNull();
  });
});

describe("terminal decoration profile registry", () => {
  it("keeps general mode undecorated and resolves device profiles", () => {
    expect(getTerminalDecorationProfile("general")).toBeNull();
    expect(getTerminalDecorationProfile("cisco_ios")).toBe(CISCO_IOS_DECORATION_PROFILE);
    expect(getTerminalDecorationProfile("arista_eos")).toBe(ARISTA_EOS_DECORATION_PROFILE);
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

  it("uses EOS-specific error matching and the shared scan limit", () => {
    expect(ARISTA_EOS_DECORATION_PROFILE.decorationLookback).toBe(80);
    expect(ARISTA_EOS_DECORATION_PROFILE.isErrorLine("% Ambiguous command")).toBe(true);
    expect(ARISTA_EOS_DECORATION_PROFILE.isErrorLine(" % Incomplete command")).toBe(true);
    expect(ARISTA_EOS_DECORATION_PROFILE.isErrorLine("% Invalid input (at token 1: 'bogus')")).toBe(
      true
    );
    expect(ARISTA_EOS_DECORATION_PROFILE.isErrorLine("Command Rejected: denied")).toBe(false);
    expect(ARISTA_EOS_DECORATION_PROFILE.isErrorLine("interface is up")).toBe(false);
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
