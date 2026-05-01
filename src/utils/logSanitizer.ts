import type { LogFormat } from "../types";

type ParserState = "normal" | "esc" | "csi" | "osc";

interface TerminalLogSanitizer {
  push: (data: string) => string;
  flush: () => string;
}

class StreamingControlStripper implements TerminalLogSanitizer {
  private state: ParserState = "normal";

  push(data: string): string {
    let output = "";

    for (const char of data) {
      const code = char.charCodeAt(0);

      if (this.state === "esc") {
        if (char === "[") {
          this.state = "csi";
        } else if (char === "]") {
          this.state = "osc";
        } else {
          this.state = "normal";
        }
        continue;
      }

      if (this.state === "csi") {
        if (code >= 0x40 && code <= 0x7e) {
          this.state = "normal";
        }
        continue;
      }

      if (this.state === "osc") {
        if (char === "\u0007") {
          this.state = "normal";
        } else if (char === "\u001b") {
          this.state = "esc";
        }
        continue;
      }

      if (char === "\u001b") {
        this.state = "esc";
      } else if (char === "\n" || char === "\r" || char === "\t" || code >= 0x20) {
        output += char;
      }
    }

    return output;
  }

  flush(): string {
    this.state = "normal";
    return "";
  }
}

class DisplayLogSanitizer implements TerminalLogSanitizer {
  private state: ParserState = "normal";
  private csi = "";
  private line: string[] = [];
  private cursor = 0;

  push(data: string): string {
    let output = "";

    for (const char of data) {
      const code = char.charCodeAt(0);

      if (this.state === "esc") {
        if (char === "[") {
          this.state = "csi";
          this.csi = "";
        } else if (char === "]") {
          this.state = "osc";
        } else {
          this.state = "normal";
        }
        continue;
      }

      if (this.state === "csi") {
        this.csi += char;
        if (code >= 0x40 && code <= 0x7e) {
          this.applyCsi(this.csi);
          this.state = "normal";
          this.csi = "";
        }
        continue;
      }

      if (this.state === "osc") {
        if (char === "\u0007") {
          this.state = "normal";
        } else if (char === "\u001b") {
          this.state = "esc";
        }
        continue;
      }

      if (char === "\u001b") {
        this.state = "esc";
      } else if (char === "\b" || char === "\u007f") {
        this.backspace();
      } else if (char === "\r") {
        this.cursor = 0;
      } else if (char === "\n") {
        output += `${this.line.join("")}\n`;
        this.line = [];
        this.cursor = 0;
      } else if (char === "\t") {
        this.insertText("\t");
      } else if (code >= 0x20) {
        this.insertText(char);
      }
    }

    return output;
  }

  flush(): string {
    const output = this.line.length > 0 ? this.line.join("") : "";
    this.state = "normal";
    this.csi = "";
    this.line = [];
    this.cursor = 0;
    return output;
  }

  private insertText(text: string) {
    for (const char of text) {
      this.line.splice(this.cursor, 0, char);
      this.cursor += 1;
    }
  }

  private backspace() {
    if (this.cursor <= 0) return;
    this.line.splice(this.cursor - 1, 1);
    this.cursor -= 1;
  }

  private applyCsi(sequence: string) {
    const command = sequence[sequence.length - 1];
    const params = sequence.slice(0, -1);
    const amount = Number.parseInt(params.split(";")[0] || "1", 10) || 1;

    if (command === "D") {
      this.cursor = Math.max(0, this.cursor - amount);
    } else if (command === "C") {
      this.cursor = Math.min(this.line.length, this.cursor + amount);
    } else if (command === "G") {
      this.cursor = Math.min(this.line.length, Math.max(0, amount - 1));
    } else if (command === "K") {
      const mode = params || "0";
      if (mode === "0") {
        this.line.splice(this.cursor);
      } else if (mode === "1") {
        this.line.splice(0, this.cursor);
        this.cursor = 0;
      } else if (mode === "2") {
        this.line = [];
        this.cursor = 0;
      }
    }
  }
}

export function createTerminalLogSanitizer(format: LogFormat): TerminalLogSanitizer {
  if (format === "strip_controls") {
    return new StreamingControlStripper();
  }
  return new DisplayLogSanitizer();
}
