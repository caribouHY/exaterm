interface ConnectionFinalizationOptions {
  registerSession: () => Promise<void>;
  recordHistory?: () => void;
}

export async function finalizeConnectionSession({
  registerSession,
  recordHistory,
}: ConnectionFinalizationOptions): Promise<void> {
  await registerSession();
  recordHistory?.();
}
