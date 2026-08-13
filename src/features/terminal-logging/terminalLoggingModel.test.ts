import { describe, expect, it } from "vitest";
import {
  canPauseManualLog,
  canResumeManualLog,
  shouldAppendManualLog,
} from "./terminalLoggingModel";

describe("terminalLoggingModel", () => {
  it("does not offer pause or resume when logging is inactive", () => {
    expect(canPauseManualLog(false, false)).toBe(false);
    expect(canResumeManualLog(false, false)).toBe(false);
  });

  it("stops appends while logging is paused", () => {
    expect(shouldAppendManualLog(true, true)).toBe(false);
    expect(canResumeManualLog(true, true)).toBe(true);
  });

  it("resumes appends for subsequent output", () => {
    expect(shouldAppendManualLog(true, true)).toBe(false);
    expect(shouldAppendManualLog(true, false)).toBe(true);
    expect(canPauseManualLog(true, false)).toBe(true);
    expect(canResumeManualLog(true, false)).toBe(false);
  });

  it("stops appends after logging stops", () => {
    expect(shouldAppendManualLog(false, false)).toBe(false);
  });
});
