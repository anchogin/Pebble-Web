const LEGACY_STORAGE_KEY = "pebble-templates";

export interface EmailTemplate {
  id: string;
  name: string;
  subject: string;
  body: string;
  createdAt: number;
}

function clearLegacyTemplates() {
  try {
    localStorage.removeItem(LEGACY_STORAGE_KEY);
  } catch { /* ignored */ }
}

export async function listTemplates(): Promise<EmailTemplate[]> {
  clearLegacyTemplates();
  // Not implemented in web backend
  return [];
}

export async function saveTemplate(_template: Omit<EmailTemplate, "id" | "createdAt">): Promise<EmailTemplate> {
  throw new Error("Not implemented: saveTemplate");
}

export async function deleteTemplate(_id: string): Promise<void> {
  throw new Error("Not implemented: deleteTemplate");
}
