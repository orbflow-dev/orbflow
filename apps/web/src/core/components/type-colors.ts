/** Type-to-color mapping for port handles and field type badges. */
const TYPE_COLORS: Record<string, string> = {
  string: "#10B981",
  number: "#F59E0B",
  boolean: "#A855F7",
  object: "#3B82F6",
  array: "#EC4899",
};

export function getTypeColor(type: string): string {
  return TYPE_COLORS[type] || "#71717A";
}

/** Friendly type labels for non-technical users. */
const TYPE_LABELS: Record<string, string> = {
  string: "Text",
  number: "Number",
  boolean: "Yes/No",
  object: "Data",
  array: "List",
};

export function getTypeLabel(type: string): string {
  return TYPE_LABELS[type] || type;
}
