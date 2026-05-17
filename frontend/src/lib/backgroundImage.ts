import type { ImportedBackgroundImage } from "./ipc-types";

export async function importBackgroundImage(_file: File): Promise<ImportedBackgroundImage> {
  throw new Error("Not implemented: importBackgroundImage (desktop-only)");
}

export async function deleteBackgroundImage(_path: string): Promise<void> {
  throw new Error("Not implemented: deleteBackgroundImage (desktop-only)");
}

export function backgroundImageUrl(path: string): string {
  return path;
}
