/* -- Shape detection utilities ----------------------- */

export type DataShape =
  | "empty"
  | "primitive-string"
  | "primitive-number"
  | "primitive-boolean"
  | "array-objects"
  | "array-primitives"
  | "flat-object"
  | "nested-object";

export function detectShape(data: unknown): DataShape {
  if (data === null || data === undefined) return "empty";
  if (typeof data === "string") return "primitive-string";
  if (typeof data === "number") return "primitive-number";
  if (typeof data === "boolean") return "primitive-boolean";
  if (Array.isArray(data)) {
    if (data.length === 0) return "empty";
    if (data.every((item) => typeof item === "object" && item !== null && !Array.isArray(item)))
      return "array-objects";
    return "array-primitives";
  }
  // typeof data === "object" is guaranteed here (all other types handled above)
  if (Object.keys(data as object).length === 0) return "empty";
  const values = Object.values(data as object);
  const allPrimitive = values.every((v) => v === null || typeof v !== "object");
  return allPrimitive ? "flat-object" : "nested-object";
}

export function isUrl(value: unknown): boolean {
  return typeof value === "string" && /^https?:\/\//.test(value);
}

export function isHttpResponse(data: unknown): boolean {
  if (typeof data !== "object" || data === null) return false;
  const d = data as Record<string, unknown>;
  return ("statusCode" in d || "status_code" in d || "status" in d) && ("body" in d || "headers" in d);
}
