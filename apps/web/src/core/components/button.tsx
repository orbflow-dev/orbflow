"use client";

import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "../utils/cn";
import { NodeIcon } from "./icons";

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger" | "icon";
type ButtonSize = "sm" | "md";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  icon?: string;
  iconPosition?: "left" | "right";
  loading?: boolean;
}

const variantStyles: Record<ButtonVariant, string> = {
  primary:
    "bg-electric-indigo text-white hover:bg-electric-indigo/90 shadow-lg shadow-electric-indigo/20 hover:shadow-electric-indigo/30",
  secondary:
    "bg-orbflow-surface text-orbflow-text-secondary border border-orbflow-border hover:bg-orbflow-surface-hover",
  ghost:
    "text-orbflow-text-muted hover:text-orbflow-text hover:bg-orbflow-surface-hover",
  danger:
    "bg-rose-500/10 text-rose-400 hover:bg-rose-500/20 border border-rose-500/20",
  icon:
    "text-orbflow-text-muted hover:text-orbflow-text hover:bg-orbflow-controls-btn-hover",
};

const sizeStyles: Record<ButtonSize, Record<ButtonVariant, string>> = {
  sm: {
    primary: "px-3 py-1.5 text-body rounded-lg",
    secondary: "px-3 py-1.5 text-body rounded-lg",
    ghost: "px-2 py-1 text-body rounded-lg",
    danger: "px-3 py-1.5 text-body rounded-lg",
    icon: "p-1.5 rounded-md",
  },
  md: {
    primary: "px-4 py-2 text-body-lg rounded-lg",
    secondary: "px-4 py-2 text-body-lg rounded-lg",
    ghost: "px-2 py-1.5 text-body-lg rounded-lg",
    danger: "px-4 py-2 text-body-lg rounded-lg",
    icon: "p-2 rounded-lg",
  },
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      variant = "secondary",
      size = "md",
      icon,
      iconPosition = "left",
      loading,
      className,
      children,
      disabled,
      ...props
    },
    ref,
  ) => {
    const iconSize = variant === "icon" ? "w-4 h-4" : "w-3.5 h-3.5";

    return (
      <button
        ref={ref}
        disabled={disabled || loading}
        className={cn(
          "inline-flex items-center justify-center gap-1.5 font-medium",
          "transition-colors duration-150",
          "disabled:opacity-40 disabled:cursor-not-allowed",
          "active:scale-[0.97]",
          "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
          variantStyles[variant],
          sizeStyles[size][variant],
          className,
        )}
        {...props}
      >
        {loading ? (
          <NodeIcon name="loader" className="w-3.5 h-3.5 animate-spin" />
        ) : icon && iconPosition === "left" ? (
          <NodeIcon name={icon} className={iconSize} />
        ) : null}
        {children}
        {!loading && icon && iconPosition === "right" && (
          <NodeIcon name={icon} className={iconSize} />
        )}
      </button>
    );
  },
);
Button.displayName = "Button";
