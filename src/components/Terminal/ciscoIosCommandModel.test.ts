import { describe, expect, it } from "vitest";
import {
  findCiscoIosPinnedCommand,
  type CiscoIosBuffer,
  type CiscoIosBufferLine,
} from "./ciscoIosCommandModel";

function createBuffer(
  lines: Array<string | { text: string; isWrapped: boolean }>,
  viewportY: number,
  type: CiscoIosBuffer["type"] = "normal"
): CiscoIosBuffer {
  const bufferLines: CiscoIosBufferLine[] = lines.map((line) => {
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
    getLine: (lineIndex) => bufferLines[lineIndex],
  };
}

describe("findCiscoIosPinnedCommand", () => {
  it("does not pin a command while its execution line is visible", () => {
    expect(
      findCiscoIosPinnedCommand(createBuffer(["Router#show interfaces", "output"], 0), "cisco_ios")
    ).toBeNull();
  });

  it("pins the prompt and command while viewing its long output", () => {
    const buffer = createBuffer(["Router#show interfaces", "line 1", "line 2", "line 3"], 2);

    expect(findCiscoIosPinnedCommand(buffer, "cisco_ios")).toEqual({
      displayText: "Router#show interfaces",
      promptText: "Router#",
      commandText: "show interfaces",
      isConfigPrompt: false,
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

    expect(findCiscoIosPinnedCommand(buffer, "cisco_ios")).toEqual({
      displayText: "Router(config)#interface GigabitEthernet0/0/0.100",
      promptText: "Router(config)#",
      commandText: "interface GigabitEthernet0/0/0.100",
      isConfigPrompt: true,
      promptLineIndex: 0,
      commandLineCount: 2,
    });
  });

  it("switches after the next command execution line leaves the viewport", () => {
    const buffer = createBuffer(
      ["Router#show clock", "12:00:00", "Router#show version", "version output", "more output"],
      4
    );

    expect(findCiscoIosPinnedCommand(buffer, "cisco_ios")?.displayText).toBe("Router#show version");
  });

  it("does not pin the previous command when the next prompt reaches the viewport top", () => {
    const buffer = createBuffer(
      ["Router#show clock", "12:00:00", "Router#show version", "version output"],
      2
    );

    expect(findCiscoIosPinnedCommand(buffer, "cisco_ios")).toBeNull();
  });

  it("does not pin output after an empty prompt", () => {
    const buffer = createBuffer(["Router#show clock", "12:00:00", "Router#", ""], 3);

    expect(findCiscoIosPinnedCommand(buffer, "cisco_ios")).toBeNull();
  });

  it("does not pin commands in general mode", () => {
    const buffer = createBuffer(["Router#show interfaces", "output"], 1);

    expect(findCiscoIosPinnedCommand(buffer, "general")).toBeNull();
  });

  it("does not pin commands from the alternate buffer", () => {
    const buffer = createBuffer(["Router#show interfaces", "output"], 1, "alternate");

    expect(findCiscoIosPinnedCommand(buffer, "cisco_ios")).toBeNull();
  });
});
