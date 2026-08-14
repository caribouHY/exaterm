export function shouldAppendManualLog(
  isManualLogging: boolean,
  isManualLoggingPaused: boolean
): boolean {
  return isManualLogging && !isManualLoggingPaused;
}

export function canPauseManualLog(
  isManualLogging: boolean,
  isManualLoggingPaused: boolean
): boolean {
  return isManualLogging && !isManualLoggingPaused;
}

export function canResumeManualLog(
  isManualLogging: boolean,
  isManualLoggingPaused: boolean
): boolean {
  return isManualLogging && isManualLoggingPaused;
}
