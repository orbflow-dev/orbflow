"use client";

import { useState, useCallback } from "react";

export type ThemeMode = "dark" | "light";

export interface ThemeState {
  mode: ThemeMode;
  toggleTheme: () => void;
  setMode: (mode: ThemeMode) => void;
}

/** Headless theme state management — no DOM side effects. */
export function useThemeState(initialMode: ThemeMode = "dark"): ThemeState {
  const [mode, setModeState] = useState<ThemeMode>(initialMode);

  const toggleTheme = useCallback(() => {
    setModeState((prev) => (prev === "dark" ? "light" : "dark"));
  }, []);

  const setMode = useCallback((newMode: ThemeMode) => {
    setModeState(newMode);
  }, []);

  return { mode, toggleTheme, setMode };
}
