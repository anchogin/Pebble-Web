const STORAGE_KEY = "pebble-signatures";

function loadSignatures(): Record<string, string> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

export async function getSignature(accountId: string): Promise<string> {
  return loadSignatures()[accountId] || "";
}

export async function setSignature(accountId: string, signature: string): Promise<void> {
  const sigs = loadSignatures();
  sigs[accountId] = signature;
  localStorage.setItem(STORAGE_KEY, JSON.stringify(sigs));
}
