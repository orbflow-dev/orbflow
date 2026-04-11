"use client";

import type { ConditionRule, CelOperator } from "../../types/schema";
import { NodeIcon } from "../icons";
import { cn } from "../../utils/cn";

interface ConditionRowProps {
  rule: ConditionRule;
  fieldOptions: { label: string; celPath: string; type: string }[];
  onChange: (updated: ConditionRule) => void;
  onDelete: () => void;
}

const OPERATORS: { value: CelOperator; label: string; shortLabel: string; types: string[] }[] = [
  { value: "==", label: "equals", shortLabel: "=", types: ["string", "number", "boolean"] },
  { value: "!=", label: "not equals", shortLabel: "≠", types: ["string", "number", "boolean"] },
  { value: ">", label: "greater than", shortLabel: ">", types: ["number"] },
  { value: "<", label: "less than", shortLabel: "<", types: ["number"] },
  { value: ">=", label: "greater or equal", shortLabel: "≥", types: ["number"] },
  { value: "<=", label: "less or equal", shortLabel: "≤", types: ["number"] },
  { value: "contains", label: "contains", shortLabel: "∋", types: ["string"] },
  { value: "startsWith", label: "starts with", shortLabel: "a…", types: ["string"] },
  { value: "endsWith", label: "ends with", shortLabel: "…z", types: ["string"] },
];

const selectStyles = cn(
  "rounded-md border border-orbflow-border bg-orbflow-add-btn-bg px-2 py-1.5",
  "text-body-sm font-mono text-orbflow-text-secondary",
  "focus:border-electric-indigo/40",
  "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
  "hover:bg-orbflow-surface-hover cursor-pointer",
  "transition-colors truncate appearance-none",
);

export function ConditionRow({
  rule,
  fieldOptions,
  onChange,
  onDelete,
}: ConditionRowProps) {
  const selectedField = fieldOptions.find((f) => f.celPath === rule.field);
  const fieldType = selectedField?.type || "string";
  const availableOps = OPERATORS.filter((op) => op.types.includes(fieldType));
  const hasField = !!rule.field;

  return (
    <div className="flex items-center gap-1.5 group">
      {/* Field selector */}
      <div className="relative flex-1 min-w-0">
        <select
          value={rule.field}
          onChange={(e) => onChange({ ...rule, field: e.target.value })}
          aria-label="Condition field"
          className={cn(
            selectStyles,
            "w-full pr-6",
            !hasField && "text-orbflow-text-ghost italic",
          )}
        >
          <option value="" className="bg-orbflow-surface">
            Select field...
          </option>
          {fieldOptions.map((f) => (
            <option key={f.celPath} value={f.celPath} className="bg-orbflow-surface">
              {f.label}
            </option>
          ))}
        </select>
        <NodeIcon
          name="chevron-down"
          className="absolute right-1.5 top-1/2 -translate-y-1/2 w-2.5 h-2.5 text-orbflow-text-ghost pointer-events-none"
        />
      </div>

      {/* Operator */}
      <div className="relative w-24 shrink-0">
        <select
          value={rule.operator}
          onChange={(e) =>
            onChange({ ...rule, operator: e.target.value as CelOperator })
          }
          aria-label="Condition operator"
          className={cn(selectStyles, "w-full pr-6")}
        >
          {availableOps.map((op) => (
            <option key={op.value} value={op.value} className="bg-orbflow-surface">
              {op.label}
            </option>
          ))}
        </select>
        <NodeIcon
          name="chevron-down"
          className="absolute right-1.5 top-1/2 -translate-y-1/2 w-2.5 h-2.5 text-orbflow-text-ghost pointer-events-none"
        />
      </div>

      {/* Value */}
      <input
        type={fieldType === "number" ? "number" : "text"}
        value={String(rule.value)}
        aria-label="Condition value"
        onChange={(e) => {
          const val =
            fieldType === "number" ? Number(e.target.value) : e.target.value;
          onChange({ ...rule, value: val });
        }}
        placeholder="value"
        className={cn(
          "w-28 shrink-0 rounded-md border border-orbflow-border bg-orbflow-add-btn-bg px-2 py-1.5",
          "text-body-sm font-mono text-orbflow-text-secondary placeholder:text-orbflow-text-ghost",
          "focus:border-electric-indigo/40",
          "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
          "hover:bg-orbflow-surface-hover transition-colors",
        )}
      />

      {/* Delete */}
      <button
        onClick={onDelete}
        className={cn(
          "w-7 h-7 shrink-0 flex items-center justify-center rounded-md transition-all cursor-pointer",
          "text-orbflow-text-ghost opacity-0 group-hover:opacity-100",
          "hover:text-rose-400 hover:bg-rose-400/10",
          "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none focus-visible:opacity-100",
        )}
        aria-label="Remove condition"
        title="Remove condition"
      >
        <NodeIcon name="trash" className="w-3 h-3" />
      </button>
    </div>
  );
}
