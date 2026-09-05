import type { Terminal } from "@xterm/xterm";
import { describe, expect, it } from "vitest";
import { createTerminalDecorationController } from "./terminalDecorationController";
import {
  FUJITSU_SIR_DECORATION_PROFILE,
  FURUKAWA_FITELNET_DECORATION_PROFILE,
  JUNIPER_JUNOS_DECORATION_PROFILE,
  VYOS_DECORATION_PROFILE,
} from "./terminalDecorationProfiles";
import { TERMINAL_DECORATION_COLORS } from "./terminalDecorationTheme";
import type { TerminalDecorationProfile, TerminalPinnedCommand } from "./terminalDecorationTypes";

class FakeMarker {
  isDisposed = false;

  constructor(public line: number) {}

  dispose() {
    this.isDisposed = true;
  }
}

class FakeDecoration {
  isDisposed = false;
  private readonly disposeListeners: Array<() => void> = [];

  constructor(readonly foregroundColor?: string) {}

  onDispose(listener: () => void) {
    this.disposeListeners.push(listener);
    return { dispose: () => undefined };
  }

  dispose() {
    if (this.isDisposed) return;
    this.isDisposed = true;
    this.disposeListeners.forEach((listener) => {
      listener();
    });
  }
}

const SYNTHETIC_PROFILE: TerminalDecorationProfile = {
  mode: "general",
  decorationLookback: 10,
  decorationStyle: "synthetic-v1",
  pinnedCommand: true,
  parsePrompt: (line) => {
    const match = /^(Device\$)( ?)(.*)$/.exec(line.trimEnd());
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

function createTerminal(lines: string[], viewportY: number) {
  const markers: FakeMarker[] = [];
  const decorations: FakeDecoration[] = [];
  const bufferLines = lines.map((text) => ({
    isWrapped: false,
    translateToString: () => text,
  }));
  const activeBuffer = {
    type: "normal",
    viewportY,
    length: bufferLines.length,
    baseY: 0,
    cursorY: bufferLines.length - 1,
    getLine: (lineIndex: number) => bufferLines.find((_line, index) => index === lineIndex),
  };
  const terminal = {
    rows: 24,
    buffer: { active: activeBuffer },
    registerMarker: (cursorYOffset: number) => {
      const marker = new FakeMarker(activeBuffer.baseY + activeBuffer.cursorY + cursorYOffset);
      markers.push(marker);
      return marker;
    },
    registerDecoration: ({ foregroundColor }: { foregroundColor?: string }) => {
      const decoration = new FakeDecoration(foregroundColor);
      decorations.push(decoration);
      return decoration;
    },
  } as unknown as Terminal;

  return { terminal, markers, decorations };
}

describe("createTerminalDecorationController", () => {
  it("clears old decorations when switching profiles and can decorate the same terminal again", () => {
    const pinnedUpdates: Array<TerminalPinnedCommand | null> = [];
    const { terminal, markers, decorations } = createTerminal(
      ["Device$ inspect", "FAIL request", "result"],
      2
    );
    const controller = createTerminalDecorationController({
      onPinnedCommandChange: (command) => pinnedUpdates.push(command),
    });

    controller.setProfile(SYNTHETIC_PROFILE, terminal);

    expect(decorations).toHaveLength(3);
    expect(pinnedUpdates[pinnedUpdates.length - 1]?.displayText).toBe("Device$ inspect");
    const firstMarkers = [...markers];
    const firstDecorations = [...decorations];

    controller.setProfile(null, terminal);

    expect(firstMarkers.every((marker) => marker.isDisposed)).toBe(true);
    expect(firstDecorations.every((decoration) => decoration.isDisposed)).toBe(true);
    expect(pinnedUpdates[pinnedUpdates.length - 1]).toBeNull();

    controller.setProfile(SYNTHETIC_PROFILE, terminal);

    expect(decorations.length).toBeGreaterThan(firstDecorations.length);
    expect(pinnedUpdates[pinnedUpdates.length - 1]?.displayText).toBe("Device$ inspect");
  });

  it("keeps alternate buffers undecorated", () => {
    const { terminal, decorations } = createTerminal(["Device$ inspect", "result"], 1);
    Object.assign(terminal.buffer.active, { type: "alternate" });
    const controller = createTerminalDecorationController({
      onPinnedCommandChange: () => undefined,
    });

    controller.setProfile(SYNTHETIC_PROFILE, terminal);

    expect(decorations).toHaveLength(0);
  });

  it("decorates a VyOS edit context only when followed by a configuration prompt", () => {
    const valid = createTerminal(
      ["[edit interfaces ethernet eth0]", "vyos@router:~# set description WAN", "output"],
      2
    );
    const validController = createTerminalDecorationController({
      onPinnedCommandChange: () => undefined,
    });

    validController.setProfile(VYOS_DECORATION_PROFILE, valid.terminal);
    expect(valid.decorations).toHaveLength(3);

    const stale = createTerminal(
      ["[edit interfaces]", "unrelated output", "vyos@router# commit", "output"],
      3
    );
    const staleController = createTerminalDecorationController({
      onPinnedCommandChange: () => undefined,
    });

    staleController.setProfile(VYOS_DECORATION_PROFILE, stale.terminal);
    expect(stale.decorations).toHaveLength(2);
  });

  it("decorates a Juniper Junos edit context only when followed by a configuration prompt", () => {
    const valid = createTerminal(
      ["[edit system]", "admin@router# set host-name edge", "output"],
      2
    );
    const validController = createTerminalDecorationController({
      onPinnedCommandChange: () => undefined,
    });

    validController.setProfile(JUNIPER_JUNOS_DECORATION_PROFILE, valid.terminal);
    expect(valid.decorations).toHaveLength(3);

    const stale = createTerminal(
      ["[edit system]", "unrelated output", "admin@router# commit", "output"],
      3
    );
    const staleController = createTerminalDecorationController({
      onPinnedCommandChange: () => undefined,
    });

    staleController.setProfile(JUNIPER_JUNOS_DECORATION_PROFILE, stale.terminal);
    expect(stale.decorations).toHaveLength(2);
  });

  it("uses distinct error and warning colors for Fujitsu Si-R messages", () => {
    const { terminal, decorations } = createTerminal(
      [
        "Si-R G121# show system information",
        "<ERROR> Authentication failed.",
        "<WARNING> weak password",
        "output",
      ],
      3
    );
    const controller = createTerminalDecorationController({
      onPinnedCommandChange: () => undefined,
    });

    controller.setProfile(FUJITSU_SIR_DECORATION_PROFILE, terminal);

    const foregroundColors = decorations.map((decoration) => decoration.foregroundColor);
    expect(foregroundColors).toContain(TERMINAL_DECORATION_COLORS.error);
    expect(foregroundColors).toContain(TERMINAL_DECORATION_COLORS.warning);
  });

  it("uses distinct error and warning colors for Furukawa FITELnet messages", () => {
    const { terminal, decorations } = createTerminal(
      [
        "F221-Router# commit",
        "<ERROR> Invalid input detected at '^' marker.",
        "<WARNING> Configuration is unsaved",
        "% saving working-config",
      ],
      3
    );
    const controller = createTerminalDecorationController({
      onPinnedCommandChange: () => undefined,
    });

    controller.setProfile(FURUKAWA_FITELNET_DECORATION_PROFILE, terminal);

    const foregroundColors = decorations.map((decoration) => decoration.foregroundColor);
    expect(foregroundColors).toContain(TERMINAL_DECORATION_COLORS.error);
    expect(foregroundColors).toContain(TERMINAL_DECORATION_COLORS.warning);
    expect(decorations).toHaveLength(4);
  });
});
