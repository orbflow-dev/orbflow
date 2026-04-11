/**
 * Generate a human-friendly node ID from a schema name.
 * e.g. "HTTP Request" → "http_request_1", "http_request_2"
 */
export function generateNodeSlug(schemaName: string, existingIds: string[]): string {
  const base = schemaName
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_|_$/g, "");

  let counter = 1;
  while (existingIds.includes(`${base}_${counter}`)) {
    counter++;
  }
  return `${base}_${counter}`;
}
