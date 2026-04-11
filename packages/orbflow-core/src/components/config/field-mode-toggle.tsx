"use client";

import { useMemo, useCallback, type ReactNode } from "react";

export interface FieldModeToggleRenderData {
  mode: "static" | "expression";
  isStatic: boolean;
  isExpression: boolean;
  toggle: () => void;
  setStatic: () => void;
  setExpression: () => void;
}

export interface FieldModeToggleProps {
  mode: "static" | "expression";
  onModeChange: (mode: "static" | "expression") => void;
  children: (data: FieldModeToggleRenderData) => ReactNode;
}

/**
 * Headless render-prop component for a Fixed/Expression mode toggle.
 * Provides mode state and toggle callbacks without any default rendering.
 */
export function FieldModeToggle({
  mode,
  onModeChange,
  children,
}: FieldModeToggleProps): ReactNode {
  const toggle = useCallback(() => {
    onModeChange(mode === "static" ? "expression" : "static");
  }, [mode, onModeChange]);

  const setStatic = useCallback(() => {
    onModeChange("static");
  }, [onModeChange]);

  const setExpression = useCallback(() => {
    onModeChange("expression");
  }, [onModeChange]);

  const renderData: FieldModeToggleRenderData = useMemo(
    () => ({
      mode,
      isStatic: mode === "static",
      isExpression: mode === "expression",
      toggle,
      setStatic,
      setExpression,
    }),
    [mode, toggle, setStatic, setExpression]
  );

  return children(renderData);
}
