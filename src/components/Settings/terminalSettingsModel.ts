export function parseBoundedInteger(value: string, min: number, max: number): number | null {
  if (!/^\d+$/.test(value)) return null;

  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < min || parsed > max) return null;

  return parsed;
}

export function finalizeBoundedInteger(
  value: string,
  currentValue: number,
  min: number,
  max: number
): number {
  if (!/^\d+$/.test(value)) return currentValue;

  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) return currentValue;

  return Math.min(Math.max(parsed, min), max);
}
