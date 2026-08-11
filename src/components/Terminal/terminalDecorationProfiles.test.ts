import { describe, expect, it } from "vitest";
import {
  ARISTA_EOS_DECORATION_PROFILE,
  CISCO_IOS_DECORATION_PROFILE,
  FUJITSU_SIR_DECORATION_PROFILE,
  VYOS_DECORATION_PROFILE,
  getTerminalDecorationProfile,
  parseAristaEosPrompt,
  parseCiscoIosPrompt,
  parseFujitsuSirPrompt,
  parseVyosContextLine,
  parseVyosPrompt,
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

describe("parseFujitsuSirPrompt", () => {
  it.each([
    ["Si-R G121# show system information", "config1", "default"],
    ["Si-R G121(config)# lan 0 ip address 192.0.2.1/24 3", "config1", "configuration"],
    ["Si-R G121 config2# show system information", "config2", "default"],
    ["Si-R G121 config2(config)# lan 0 vlan 1", "config2", "configuration"],
    ["branch-router>", "config1", "default"],
    ["branch-router config2(config-ip)# lan 0 ip route 0 default", "config2", "configuration"],
  ] as const)("parses the standard Si-R G prompt %s", (line, configurationFile, variant) => {
    expect(parseFujitsuSirPrompt(line)).toMatchObject({ configurationFile, variant });
  });

  it("keeps config2 in the parsed prompt and separates the command", () => {
    expect(parseFujitsuSirPrompt("Si-R G121 config2(config)# lan 0 vlan 1")).toEqual({
      hostname: "Si-R G121",
      configurationFile: "config2",
      promptText: "Si-R G121 config2(config)#",
      promptStart: 0,
      commandText: "lan 0 vlan 1",
      commandStart: 27,
      commandSeparator: " ",
      variant: "configuration",
    });
  });

  it("accepts the standard separating space used by some G models", () => {
    expect(parseFujitsuSirPrompt("Si-R G120 # show system information")).toMatchObject({
      hostname: "Si-R G120",
      promptText: "Si-R G120 #",
      variant: "default",
    });
  });

  it("rejects custom, config2-like output, and malformed prompts", () => {
    expect(parseFujitsuSirPrompt("[production]# show system information")).toBeNull();
    expect(parseFujitsuSirPrompt("status config2 pending# output")).toBeNull();
    expect(parseFujitsuSirPrompt("Si-R GX500# show system information")).toBeNull();
    expect(parseFujitsuSirPrompt("Si-R G121(config# show running-config")).toBeNull();
    expect(parseFujitsuSirPrompt("Si-R G121(a)(b)(c)(d)# show running-config")).toBeNull();
  });
});

describe("parseVyosPrompt", () => {
  it.each([
    ["vyos@router$ show version", "default"],
    ["vyos@router:~$show interfaces", "default"],
    ["admin@edge-router# set system host-name edge", "configuration"],
    ["admin@edge-router:~#commit", "configuration"],
  ] as const)("parses the standard VyOS prompt %s", (line, variant) => {
    expect(parseVyosPrompt(line)?.variant).toBe(variant);
  });

  it("separates the identity, prompt, separator, and command", () => {
    expect(parseVyosPrompt("vyos@r4-1.5:~# set interfaces ethernet eth0 disable")).toMatchObject({
      username: "vyos",
      hostname: "r4-1.5",
      promptText: "vyos@r4-1.5:~#",
      commandSeparator: " ",
      commandText: "set interfaces ethernet eth0 disable",
      variant: "configuration",
    });
  });

  it("rejects custom and malformed prompts", () => {
    expect(parseVyosPrompt("vyos-router# show configuration")).toBeNull();
    expect(parseVyosPrompt("@router$ show version")).toBeNull();
    expect(parseVyosPrompt("vyos@$ show version")).toBeNull();
    expect(parseVyosPrompt("vyos@router:/tmp$ pwd")).toBeNull();
  });
});

describe("parseVyosContextLine", () => {
  it("parses top-level and nested configuration contexts", () => {
    expect(parseVyosContextLine("[edit]")?.contextText).toBe("[edit]");
    expect(parseVyosContextLine("  [edit interfaces ethernet eth0]")).toEqual({
      contextText: "[edit interfaces ethernet eth0]",
      contextStart: 2,
      variant: "configuration",
    });
  });

  it("rejects unrelated and malformed context lines", () => {
    expect(parseVyosContextLine("edit interfaces ethernet eth0")).toBeNull();
    expect(parseVyosContextLine("[edit ]")).toBeNull();
    expect(parseVyosContextLine("[editor]")).toBeNull();
  });
});

describe("terminal decoration profile registry", () => {
  it("keeps general mode undecorated and resolves device profiles", () => {
    expect(getTerminalDecorationProfile("general")).toBeNull();
    expect(getTerminalDecorationProfile("cisco_ios")).toBe(CISCO_IOS_DECORATION_PROFILE);
    expect(getTerminalDecorationProfile("arista_eos")).toBe(ARISTA_EOS_DECORATION_PROFILE);
    expect(getTerminalDecorationProfile("vyos")).toBe(VYOS_DECORATION_PROFILE);
    expect(getTerminalDecorationProfile("fujitsu_sir")).toBe(FUJITSU_SIR_DECORATION_PROFILE);
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

  it("uses conservative VyOS error matching and the shared scan limit", () => {
    expect(VYOS_DECORATION_PROFILE.decorationLookback).toBe(80);
    expect(VYOS_DECORATION_PROFILE.isErrorLine("Set failed")).toBe(true);
    expect(VYOS_DECORATION_PROFILE.isErrorLine(" Commit failed")).toBe(true);
    expect(VYOS_DECORATION_PROFILE.isErrorLine("Cannot exit: configuration modified.")).toBe(true);
    expect(VYOS_DECORATION_PROFILE.isErrorLine("Invalid command: set interface")).toBe(true);
    expect(
      VYOS_DECORATION_PROFILE.isErrorLine(
        "Configuration path: interfaces ethernet eth9 does not exist"
      )
    ).toBe(true);
    expect(VYOS_DECORATION_PROFILE.isErrorLine("commit completed without error")).toBe(false);
  });

  it("uses Si-R-specific error matching and the shared scan limit", () => {
    const lowercaseWarningLine = ["<", "warning", "> lowercase output"].join("");

    expect(FUJITSU_SIR_DECORATION_PROFILE.decorationLookback).toBe(80);
    expect(FUJITSU_SIR_DECORATION_PROFILE.isErrorLine("<ERROR> Authentication failed.")).toBe(true);
    expect(FUJITSU_SIR_DECORATION_PROFILE.isErrorLine("<WARNING> weak password")).toBe(false);
    expect(FUJITSU_SIR_DECORATION_PROFILE.isWarningLine?.("<WARNING> weak password")).toBe(true);
    expect(FUJITSU_SIR_DECORATION_PROFILE.isWarningLine?.("<ERROR> Authentication failed.")).toBe(
      false
    );
    expect(FUJITSU_SIR_DECORATION_PROFILE.isWarningLine?.(" <WARNING> indented output")).toBe(
      false
    );
    expect(FUJITSU_SIR_DECORATION_PROFILE.isWarningLine?.(lowercaseWarningLine)).toBe(false);
    expect(FUJITSU_SIR_DECORATION_PROFILE.isWarningLine?.("syslog warning count: 0")).toBe(false);
    expect(FUJITSU_SIR_DECORATION_PROFILE.isErrorLine("syslog error count: 0")).toBe(false);
    expect(FUJITSU_SIR_DECORATION_PROFILE.isErrorLine(" <ERROR> indented output")).toBe(false);
  });

  it("uses one shared palette for terminal decoration", () => {
    expect(TERMINAL_DECORATION_COLORS).toEqual({
      prompt: "#7dd3fc",
      configurationPrompt: "#facc15",
      command: "#6ee7b7",
      warning: "#fb923c",
      error: "#f87171",
    });
  });
});
