/** Extract a human-readable error message from an unknown catch value. */
export function extractErrorMessage(err: any): string {
  if (err?.response?.data?.error) {
    return err.response.data.error;
  }
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  if (err && typeof err === "object" && "message" in err) {
    return String((err as { message: unknown }).message);
  }
  return "Unknown error";
}
