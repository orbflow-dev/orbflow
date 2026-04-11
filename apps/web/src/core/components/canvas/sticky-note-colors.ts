/** Color definitions for sticky notes, keyed by color name. */
export type ColorDef = { bg: string; border: string; text: string };

export const LIGHT_COLORS: Record<string, ColorDef> = {
  yellow: { bg: "#FEFCE8", border: "#FDE047", text: "#713F12" },
  blue:   { bg: "#EFF6FF", border: "#93C5FD", text: "#1E3A5F" },
  green:  { bg: "#F0FDF4", border: "#86EFAC", text: "#14532D" },
  pink:   { bg: "#FDF2F8", border: "#F9A8D4", text: "#831843" },
  purple: { bg: "#FAF5FF", border: "#C4B5FD", text: "#3B0764" },
};

export const DARK_COLORS: Record<string, ColorDef> = {
  yellow: { bg: "#1C1A08", border: "#A37E10", text: "#FDE68A" },
  blue:   { bg: "#0B1120", border: "#2563EB", text: "#93C5FD" },
  green:  { bg: "#061210", border: "#16A34A", text: "#86EFAC" },
  pink:   { bg: "#1A0810", border: "#BE185D", text: "#F9A8D4" },
  purple: { bg: "#110A1C", border: "#7C3AED", text: "#C4B5FD" },
};

export const COLOR_NAMES = Object.keys(LIGHT_COLORS);
