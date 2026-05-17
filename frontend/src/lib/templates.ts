const STORAGE_KEY = "pebble-templates";

export interface EmailTemplate {
  id: string;
  name: string;
  subject: string;
  body: string;
  createdAt: number;
}

function loadTemplates(): EmailTemplate[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function persistTemplates(templates: EmailTemplate[]): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(templates));
}

export async function listTemplates(): Promise<EmailTemplate[]> {
  return loadTemplates();
}

export async function saveTemplate(template: Omit<EmailTemplate, "id" | "createdAt">): Promise<EmailTemplate> {
  const templates = loadTemplates();
  const newTemplate: EmailTemplate = {
    ...template,
    id: crypto.randomUUID(),
    createdAt: Date.now(),
  };
  templates.push(newTemplate);
  persistTemplates(templates);
  return newTemplate;
}

export async function deleteTemplate(id: string): Promise<void> {
  const templates = loadTemplates().filter((t) => t.id !== id);
  persistTemplates(templates);
}
