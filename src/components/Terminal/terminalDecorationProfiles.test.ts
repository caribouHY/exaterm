import { describe, expect, it } from "vitest";
import {
  ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE,
  ARISTA_EOS_DECORATION_PROFILE,
  CISCO_IOS_DECORATION_PROFILE,
  FUJITSU_SIR_DECORATION_PROFILE,
  FURUKAWA_FITELNET_DECORATION_PROFILE,
  VYOS_DECORATION_PROFILE,
  getTerminalDecorationProfile,
  parseAristaEosPrompt,
  parseAlliedTelesisAwplusPrompt,
  parseCiscoIosPrompt,
  parseFujitsuSirPrompt,
  parseFurukawaFitelnetPrompt,
  parseVyosContextLine,
  parseVyosPrompt,
} from "./terminalDecorationProfiles";
import { TERMINAL_DECORATION_COLORS } from "./terminalDecorationTheme";

const ALLIED_TELESIS_AWPLUS_DOCUMENTED_ERROR_FIXTURES = {
  x330: [
    "% Incomplete command.",
    "% Invalid input detected at '^' marker.",
    "% Can't find interface ppp0",
    "% Unrecognized command",
    "Login incorrect",
    "% Working set must contain only single node for this command",
  ],
  ar3050sAr4050s: [
    "% Incomplete command.",
    "% Invalid input detected at '^' marker.",
    "% Can't find interface ppp0",
    "% Unrecognized command",
    "Login incorrect",
    "% Working set must contain only single node for this command",
  ],
} as const;

const ALLIED_TELESIS_AWPLUS_DOCUMENTED_WARNING_FIXTURES = [
  "% Warning: Telnet is insecure and deprecated. Please use SSH.",
] as const;

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

describe("parseAlliedTelesisAwplusPrompt", () => {
  it.each([
    ["awplus> show version", "default"],
    ["awplus#show system", "default"],
    ["edge-switch(config)# hostname edge-switch", "configuration"],
    ["edge-switch(config-if)# description uplink", "configuration"],
    ["edge-switch(dhcp-config)# network 192.0.2.0/24", "configuration"],
    ["edge-switch(g8032-profile-config)# description ring", "configuration"],
    ["ar4050(config-apn)# apn example", "configuration"],
    ["ar4050(config-router)# network 192.0.2.0/24", "configuration"],
    ["ar4050(config-vrf)# description branch", "configuration"],
    ["ar4050(config-pbr)# policy-based-routing enable", "configuration"],
    ["ar4050(ca-trustpoint)# enrollment terminal", "configuration"],
  ] as const)("parses the AlliedWare Plus prompt %s", (line, variant) => {
    expect(parseAlliedTelesisAwplusPrompt(line)?.variant).toBe(variant);
  });

  it("separates the hostname, mode, prompt, and command", () => {
    expect(parseAlliedTelesisAwplusPrompt("branch-fw(config-if)# tunnel mode ipsec")).toEqual({
      hostname: "branch-fw",
      mode: "config-if",
      promptText: "branch-fw(config-if)#",
      promptStart: 0,
      commandText: "tunnel mode ipsec",
      commandStart: 22,
      commandSeparator: " ",
      variant: "configuration",
    });
  });

  it("rejects login prompts, output, and malformed prompts", () => {
    expect(parseAlliedTelesisAwplusPrompt("awplus login: manager")).toBeNull();
    expect(parseAlliedTelesisAwplusPrompt("Password:")).toBeNull();
    expect(parseAlliedTelesisAwplusPrompt("status output without a prompt")).toBeNull();
    expect(parseAlliedTelesisAwplusPrompt("edge switch(config)# show version")).toBeNull();
    expect(parseAlliedTelesisAwplusPrompt("edge-switch(config)> show version")).toBeNull();
    expect(parseAlliedTelesisAwplusPrompt("edge-switch(config# show version")).toBeNull();
    expect(parseAlliedTelesisAwplusPrompt("edge-switch(config)(sub)# show version")).toBeNull();
  });
});

describe("parseFurukawaFitelnetPrompt", () => {
  it.each([
    ["> show version", "default"],
    ["#show running.cfg", "default"],
    ["F70-Router> show status", "default"],
    ["F71_Router# show version", "default"],
    ["F220_Router(config)# hostname branch", "configuration"],
    ["F221-Router#show version", "default"],
    ["(config)# hostname branch-router", "configuration"],
    ["Router(config)# interface GigaEthernet 1/1", "configuration"],
    ["(config-if-ge 1/1)# vlan-id 1", "configuration"],
    ["Router(config-if-ch 1)# ip address 192.0.2.1 255.255.255.0", "configuration"],
  ] as const)("parses the FITELnet prompt %s", (line, variant) => {
    expect(parseFurukawaFitelnetPrompt(line)?.variant).toBe(variant);
  });

  it("separates hostname, mode, prompt, and command", () => {
    expect(parseFurukawaFitelnetPrompt("branch-router(config-if-ge 1/1)# vlan-id 1")).toEqual({
      hostname: "branch-router",
      mode: "config-if-ge 1/1",
      promptText: "branch-router(config-if-ge 1/1)#",
      promptStart: 0,
      commandText: "vlan-id 1",
      commandStart: 33,
      commandSeparator: " ",
      variant: "configuration",
    });
  });

  it("rejects shell output, malformed modes, and excessive lengths", () => {
    expect(parseFurukawaFitelnetPrompt("operator@router:~$ pwd")).toBeNull();
    expect(parseFurukawaFitelnetPrompt("Router (config)# show running.cfg")).toBeNull();
    expect(parseFurukawaFitelnetPrompt("Router(config)> show running.cfg")).toBeNull();
    expect(parseFurukawaFitelnetPrompt("Router(config# show running.cfg")).toBeNull();
    expect(parseFurukawaFitelnetPrompt("Router(config)(sub)# show running.cfg")).toBeNull();
    expect(parseFurukawaFitelnetPrompt("Router(config-)# show running.cfg")).toBeNull();
    expect(parseFurukawaFitelnetPrompt("Router(config-if )# show running.cfg")).toBeNull();
    expect(parseFurukawaFitelnetPrompt(`${"r".repeat(255)}# show version`)).toBeNull();
    expect(
      parseFurukawaFitelnetPrompt(`Router(config-${"x".repeat(129)})# show running.cfg`)
    ).toBeNull();
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
    expect(getTerminalDecorationProfile("allied_telesis_awplus")).toBe(
      ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE
    );
    expect(getTerminalDecorationProfile("furukawa_fitelnet")).toBe(
      FURUKAWA_FITELNET_DECORATION_PROFILE
    );
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

  it.each(Object.entries(ALLIED_TELESIS_AWPLUS_DOCUMENTED_ERROR_FIXTURES))(
    "matches documented AlliedWare Plus errors from %s",
    (_manual, errors) => {
      errors.forEach((line) =>
        expect(ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE.isErrorLine(line)).toBe(true)
      );
    }
  );

  it("preserves the AlliedWare Plus scan limit", () => {
    expect(ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE.decorationLookback).toBe(80);
  });

  it("matches documented FITELnet errors and warnings", () => {
    const errors = [
      "ERROR:'Tunnel mode ipsec' exceeds max configurations. (20001/20000)",
      "<ERROR> Incomplete command",
      "<ERROR> Invalid input detected at '^' marker.",
      "% Can not refresh",
      "% Command failed.",
      "% Entry not found.",
      '% Cannot resolve "invalid.example" (Name or service not known)',
      "% Invalid source address",
      "% Invalid default ICMP source address",
      "% Please answer 'yes or 'no'.",
      "% A decimal number between 1 and 255.",
      "% A Hex number between 0x0000 and 0xffff.",
      "% Bad minimum size",
      "% Bad maximum size",
      "% Bad Interval size",
      "% Only one source route option allowed",
      "% No room for that option",
      "% Up to 9 routes can be specified",
      "% Invalid Number of Hops",
      "% Invalid Number of Timestamps",
      "% No such VRF",
      "This command cannot be executed.",
      "Time out, Operation failed.",
      'Unknown protocol -"ipx", type ping ? for help',
      "Unknown output interface GigaEthernet9/9",
      "Unknown source interface GigaEthernet9/9",
      "****Warning! sendto failed***",
      "packet too short (8 bytes) from 192.0.2.1",
      "wrong total length 84 instead of 100",
      "wrong data byte #4 should have been ff but was 00",
      "unknown option 0xff",
    ];

    expect(FURUKAWA_FITELNET_DECORATION_PROFILE.decorationLookback).toBe(80);
    errors.forEach((line) =>
      expect(FURUKAWA_FITELNET_DECORATION_PROFILE.isErrorLine(line)).toBe(true)
    );
    expect(
      FURUKAWA_FITELNET_DECORATION_PROFILE.isWarningLine?.(
        "WARNING: You have NOT saved after editing working.cfg."
      )
    ).toBe(true);
    expect(
      FURUKAWA_FITELNET_DECORATION_PROFILE.isWarningLine?.("<WARNING> Configuration is unsaved")
    ).toBe(true);
  });

  it("does not treat FITELnet progress, success, or statistics as errors", () => {
    const normalOutput = [
      "% Command succeeded.",
      "% reading configuration file",
      "% reading configuration file.",
      "% saving working-config",
      "% Key pair was generated at: Mon Jul 10 13:52:56 2023",
      "% Key type: SSH2-RSA Key",
      "100% |***************************************| 10199 / 10199 (Bytes)",
      "STOP: rollback-config timer",
      "Rollback-config is running.",
      "0 invalid packet received, 0 not synchronized received",
      "request send error: 0",
      "Invalid argument : 0",
      " ERROR: indented output",
      "ERROR:",
      " <ERROR> indented output",
      "<ERROR>",
      " WARNING: indented output",
      "WARNING:",
      " <WARNING> indented output",
      "<WARNING>",
      '% Cannot resolve "" (reason)',
      "% A decimal number between one and 255.",
      "% A Hex number between 0x0000 and 0xgggg.",
      "Unknown output interface edge port",
      "unknown option 0xzz",
    ];

    normalOutput.forEach((line) => {
      expect(FURUKAWA_FITELNET_DECORATION_PROFILE.isErrorLine(line)).toBe(false);
      expect(FURUKAWA_FITELNET_DECORATION_PROFILE.isWarningLine?.(line)).toBe(false);
    });
  });

  it("rejects long and incomplete FITELnet error-like output", () => {
    const longPadding = " ".repeat(4096);
    const errorLikeOutput = [
      `${longPadding}ERROR: indented output`,
      `% Cannot resolve "${"x".repeat(4096)}`,
      `% Cannot resolve "host" ()`,
      `% A decimal number between ${"1".repeat(4096)} and invalid.`,
      `Unknown output interface ${"edge ".repeat(1024)}`,
      `wrong total length ${"1".repeat(4096)} instead of invalid`,
    ];

    errorLikeOutput.forEach((line) =>
      expect(FURUKAWA_FITELNET_DECORATION_PROFILE.isErrorLine(line)).toBe(false)
    );
  });

  it("matches bounded AlliedWare Plus compatibility errors", () => {
    const errors = [
      "% Error",
      "% Bad secret",
      "Bad passwords",
      "% Ambiguous command.",
      "Connection timed out while opening the session",
      "% Internal error: Access-list is not found.",
      "'copy startup-config' returned error code: 7",
      "Bad mask /33",
      "% 192.0.2.0/24 overlaps with 192.0.2.128/25",
      "% ACL Error: entry rejected",
      "Command authorization failed for user",
    ];

    errors.forEach((line) =>
      expect(ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE.isErrorLine(line)).toBe(true)
    );
  });

  it("matches case-insensitive documented errors with display padding", () => {
    expect(
      ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE.isErrorLine("   % iNCOMPLETE COMMAND.   ")
    ).toBe(true);
    expect(ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE.isErrorLine("   LOGIN INCORRECT   ")).toBe(
      true
    );
  });

  it("handles long compatibility prefixes without broad regular expressions", () => {
    const displayPadding = " ".repeat(4096);
    expect(
      ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE.isErrorLine(`${displayPadding}% Ambiguous command.`)
    ).toBe(true);
    expect(
      ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE.isErrorLine(`${displayPadding}Ambiguous commandX`)
    ).toBe(false);
  });

  it("does not color warnings, informational output, or partial error-like text", () => {
    const normalOutput = [
      ...ALLIED_TELESIS_AWPLUS_DOCUMENTED_WARNING_FIXTURES,
      "% Service Informational: operation completed",
      "user.warning awplus NSM[123]: Port up notification received",
      "                   ^",
      "unambiguous command result",
      "% route not found but recovered",
      "%foooverlaps withbar",
      "% Error-free status",
      "% Default password needs to be changed.",
      "% route not found!",
      "'x' returned error code: 7",
      "% ACL Error:",
    ];

    normalOutput.forEach((line) =>
      expect(ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE.isErrorLine(line)).toBe(false)
    );
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
