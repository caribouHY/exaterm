import { describe, expect, it } from "vitest";
import {
  canPauseManualLog,
  canResumeManualLog,
  shouldAppendAutoLog,
  shouldAppendManualLog,
} from "./terminalLoggingModel";

describe("terminalLoggingModel", () => {
  it("does not offer pause or resume when only automatic logging is active", () => {
    expect(canPauseManualLog(false, false)).toBe(false);
    expect(canResumeManualLog(false, false)).toBe(false);
    expect(shouldAppendAutoLog(true)).toBe(true);
  });

  it("stops only manual appends while manual logging is paused", () => {
    expect(shouldAppendAutoLog(true)).toBe(true);
    expect(shouldAppendManualLog(true, true)).toBe(false);
    expect(canResumeManualLog(true, true)).toBe(true);
  });

  it("resumes manual appends for subsequent output", () => {
    expect(shouldAppendManualLog(true, true)).toBe(false);
    expect(shouldAppendManualLog(true, false)).toBe(true);
    expect(shouldAppendAutoLog(true)).toBe(true);
    expect(canPauseManualLog(true, false)).toBe(true);
    expect(canResumeManualLog(true, false)).toBe(false);
  });

  it("keeps automatic logging writable after manual logging stops", () => {
    expect(shouldAppendAutoLog(true)).toBe(true);
    expect(shouldAppendManualLog(false, false)).toBe(false);
  });
});
