export function getStartupElapsedMs(_now = Date.now()) {
  return 0;
}

export function formatStartupTiming(label: string, elapsedMs: number) {
  return `[startup] ${label}: ${elapsedMs}ms`;
}

export function logStartupTiming(label: string, _now = Date.now()) {
  console.info(`[startup] ${label}`);
  return 0;
}
