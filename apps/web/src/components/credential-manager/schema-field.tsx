"use client";

import { useState } from "react";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";

interface SchemaFieldDefinition {
  key: string;
  label: string;
  type: string;
  required?: boolean;
  default?: unknown;
  description?: string;
  enum?: string[];
}

interface SchemaFieldProps {
  field: SchemaFieldDefinition;
  value: unknown;
  onChange: (value: unknown) => void;
}

/** Renders a single schema-driven field with proper input type */
export function SchemaField({ field, value, onChange }: SchemaFieldProps) {
  const [showSecret, setShowSecret] = useState(false);

  const isPassword =
    field.key.toLowerCase().includes("password") ||
    field.key.toLowerCase().includes("secret") ||
    field.key.toLowerCase().includes("api_key") ||
    field.key.toLowerCase().includes("token");

  if (field.type === "boolean") {
    return (
      <div className="flex items-center justify-between py-1">
        <div>
          <label className="text-body font-medium text-orbflow-text-muted">
            {field.label}
          </label>
          {field.description && (
            <p className="text-caption text-orbflow-text-ghost mt-0.5">
              {field.description}
            </p>
          )}
        </div>
        <button
          onClick={() => onChange(!value)}
          role="switch"
          aria-checked={Boolean(value)}
          aria-label={field.label}
          className={cn(
            "relative w-9 h-5 rounded-full transition-colors focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
            value ? "bg-electric-indigo" : "bg-orbflow-border"
          )}
        >
          <div
            className={cn(
              "absolute top-0.5 w-4 h-4 rounded-full bg-white shadow-sm transition-transform",
              value ? "translate-x-4" : "translate-x-0.5"
            )}
          />
        </button>
      </div>
    );
  }

  if (field.enum && field.enum.length > 0) {
    return (
      <div>
        <label className="text-body font-medium text-orbflow-text-muted block mb-1.5">
          {field.label}
          {field.required && (
            <span className="text-rose-400/70 ml-0.5">*</span>
          )}
        </label>
        {field.description && (
          <p className="text-caption text-orbflow-text-ghost mb-1.5">
            {field.description}
          </p>
        )}
        <select
          value={String(value ?? field.default ?? "")}
          onChange={(e) => onChange(e.target.value)}
          className="w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3.5 py-2.5
            text-body-lg text-orbflow-text-secondary cursor-pointer
            focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50 transition-colors"
        >
          <option value="">Select...</option>
          {field.enum.map((opt) => (
            <option key={opt} value={opt}>
              {opt}
            </option>
          ))}
        </select>
      </div>
    );
  }

  return (
    <div>
      <label className="text-body font-medium text-orbflow-text-muted block mb-1.5">
        {field.label}
        {field.required && (
          <span className="text-rose-400/70 ml-0.5">*</span>
        )}
      </label>
      {field.description && (
        <p className="text-caption text-orbflow-text-ghost mb-1.5">
          {field.description}
        </p>
      )}
      <div className="relative">
        <input
          type={
            isPassword && !showSecret
              ? "password"
              : field.type === "number"
                ? "number"
                : "text"
          }
          value={String(value ?? "")}
          onChange={(e) =>
            onChange(
              field.type === "number" ? Number(e.target.value) : e.target.value
            )
          }
          placeholder={
            field.default !== undefined
              ? String(field.default)
              : `Enter ${field.label.toLowerCase()}...`
          }
          className={cn(
            "w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3.5 py-2.5",
            "text-body-lg text-orbflow-text-secondary placeholder:text-orbflow-text-ghost",
            "focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50 transition-colors",
            isPassword && "pr-10 font-mono"
          )}
        />
        {isPassword && (
          <button
            type="button"
            onClick={() => setShowSecret((prev) => !prev)}
            className="absolute right-2.5 top-1/2 -translate-y-1/2 p-1 rounded-md text-orbflow-text-ghost
              hover:text-orbflow-text-muted hover:bg-orbflow-surface-hover transition-colors
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            aria-label={showSecret ? "Hide value" : "Show value"}
            title={showSecret ? "Hide" : "Show"}
          >
            <NodeIcon
              name={showSecret ? "eye-off" : "eye"}
              className="w-4 h-4"
            />
          </button>
        )}
      </div>
    </div>
  );
}
