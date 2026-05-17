const LEGACY_STORAGE_KEY = "pebble-signatures";

function clearLegacySignatures() {
  try {
    localStorage.removeItem(LEGACY_STORAGE_KEY);
  } catch { /* ignored */ }
}

export async function getSignature(_accountId: string): Promise<string> {
  clearLegacySignatures();
  return "";
}

export async function setSignature(_accountId: string, _signature: string): Promise<void> {
  throw new Error("Not implemented: setSignature");
}
