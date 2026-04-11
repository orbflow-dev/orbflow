export interface OrbflowTheme {
  colors: {
    background: string;
    surface: string;
    surfaceHover: string;
    primary: string;
    accent: string;
    text: string;
    textMuted: string;
    border: string;
    success: string;
    error: string;
    warning: string;
  };
  borderRadius: string;
  glassEffect: boolean;
}

// Matches globals.css theme tokens (--orbflow-* custom properties)
export const defaultDarkTheme: OrbflowTheme = {
  colors: {
    background: "#0A0A0C",
    surface: "#141417",
    surfaceHover: "#1E1E22",
    primary: "#7C5CFC",
    accent: "#22D3EE",
    text: "#FAFAFA",
    textMuted: "rgba(255,255,255,0.4)",
    border: "rgba(255,255,255,0.06)",
    success: "#10B981",
    error: "#D9454F",
    warning: "#F59E0B",
  },
  borderRadius: "0.75rem",
  glassEffect: true,
};

export const defaultLightTheme: OrbflowTheme = {
  colors: {
    background: "#F5F5F7",
    surface: "#FFFFFF",
    surfaceHover: "#F0F0F2",
    primary: "#7C5CFC",
    accent: "#0891B2",
    text: "#1A1A2E",
    textMuted: "rgba(0,0,0,0.45)",
    border: "rgba(0,0,0,0.08)",
    success: "#10B981",
    error: "#D9454F",
    warning: "#F59E0B",
  },
  borderRadius: "0.75rem",
  glassEffect: false,
};
