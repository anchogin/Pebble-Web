import type { ImportedBackgroundImage } from "./ipc-types";

export async function importBackgroundImage(file: File): Promise<ImportedBackgroundImage> {
  const url = URL.createObjectURL(file);
  return { path: url, filename: file.name, size: file.size };
}

export async function deleteBackgroundImage(path: string): Promise<void> {
  if (path.startsWith("blob:")) {
    URL.revokeObjectURL(path);
  }
}

export function backgroundImageUrl(path: string): string {
  return path;
}
