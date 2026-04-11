/** Safely extract an error message from an unknown thrown value. */
export function toMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}
